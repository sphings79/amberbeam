//! FTP and FTPS.
//!
//! Unlike SFTP, where several transfers are channels inside one connection,
//! every simultaneous FTP transfer is a **separate login**. That is why the
//! default concurrency for FTP is four rather than eight: shared hosters
//! commonly allow three to eight logins per address, and a client that opens
//! one per file is refused by the server rather than by anything it did wrong.
//!
//! Connections are therefore pooled and reused. Logging in costs two round
//! trips, and a queue of a thousand small files would otherwise spend most of
//! its time doing it.

pub mod list;
pub mod tls;

use std::sync::Arc;

use suppaftp::tokio::AsyncRustlsFtpStream;
use suppaftp::types::FileType;
use suppaftp::{FtpError, Status};
use tokio::sync::{Mutex, Semaphore};

use crate::endpoint::EndpointId;
use crate::error::{Error, PathProblem, Result};
use crate::events::{ConnectionState, Events, LogDirection};
use crate::fs::{DirEntry, EntryKind, Listing};
use crate::ops::Measurement;
use crate::stream::{FtpTransfer, Reader, Writer};

/// How a connection encrypts, and whether it insists on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Encryption {
    /// Plain FTP. Possible because many old servers speak nothing else, and
    /// marked as such wherever the connection is shown.
    None,
    /// `AUTH TLS` on the ordinary port. The default for new entries.
    #[default]
    Explicit,
    /// TLS from the first byte, historically on port 990.
    Implicit,
}

/// Everything one FTP connection needs.
#[derive(Debug, Clone)]
pub struct FtpParams {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub encryption: Encryption,
    /// Passive is what works behind a router. Active exists for the rare
    /// server that insists on it.
    pub passive: bool,
    pub concurrency: u8,
    pub retries: u8,
    pub temporary_name: bool,
    /// Seconds between keep-alive commands, or `None` to let an idle
    /// connection be dropped and opened again when it is next needed.
    pub keep_alive: Option<u32>,
    /// Fallback when the server cannot speak UTF-8.
    pub latin1: bool,
    /// What the user has already agreed to about this server's certificate.
    /// Plain [`CertificateDecision::TrustedOnly`] on every first attempt.
    pub certificate: tls::CertificateDecision,
}

/// What the server said it can do.
#[derive(Debug, Clone, Default)]
pub struct Abilities {
    /// Machine readable listings. Preferred whenever offered.
    pub mlsd: bool,
    /// Continuing a transfer from an offset. Without it, a broken transfer
    /// starts again — and AmberBeam says so beforehand.
    pub rest: bool,
    pub utf8: bool,
    pub size: bool,
    /// Setting a file's modification time. Without it an uploaded file carries
    /// the time it arrived, not the time it was written.
    pub mfmt: bool,
    /// Asking for a data connection in the way that works over IPv6.
    ///
    /// PASV answers with four numbers and two more, which is an IPv4 address
    /// and nothing else. Over IPv6 there is no address it could give, so the
    /// server refuses and every listing and every transfer fails — on a
    /// connection that opened perfectly well. EPSV answers with a port and
    /// means "the same host you are already talking to", which works either
    /// way.
    pub epsv: bool,
}

impl Abilities {
    fn from_features(features: &suppaftp::types::Features) -> Self {
        let has = |name: &str| features.keys().any(|key| key.eq_ignore_ascii_case(name));
        let mfmt = has("MFMT");
        let rest = features
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case("REST"))
            .map(|(_, value)| {
                // "REST STREAM" is the one that means what we need. A server
                // announcing REST without STREAM is not offering byte offsets.
                value
                    .as_deref()
                    .map(|value| value.to_uppercase().contains("STREAM"))
                    .unwrap_or(false)
            })
            .unwrap_or(false);
        Self {
            // RFC 3659 says the MLST feature line covers both commands, and
            // plenty of servers list only that one — ProFTPD among them.
            // Looking for the literal word MLSD threw away machine-readable
            // listings on servers that offer them, and fell back to parsing
            // the human-readable one, where a name with a space in it and a
            // date without a year are guesses rather than facts.
            mlsd: has("MLSD") || has("MLST"),
            rest,
            utf8: has("UTF8"),
            size: has("SIZE"),
            mfmt,
            epsv: has("EPSV"),
        }
    }
}

/// What the server answered a hand-typed command.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawReply {
    pub code: u32,
    pub text: String,
    /// Whether this command wanted a data connection, and the control
    /// connection was therefore thrown away afterwards.
    pub connection_dropped: bool,
}

/// Whether a command expects a second connection to be opened for it.
///
/// These are the ones that leave a control connection mid-sentence when they
/// are sent by hand: the server announces a transfer and waits for somebody to
/// take it, and every later reply is then an answer to the wrong question. The
/// listing commands belong here too — they are transfers that happen to carry
/// file names.
///
/// `PASV` and its relatives are in the list for the same reason from the other
/// end: they leave a socket open that the next real transfer would walk into.
pub fn wants_data_channel(command: &str) -> bool {
    const VERBS: [&str; 11] = [
        "RETR", "STOR", "STOU", "APPE", "LIST", "NLST", "MLSD", "PASV", "EPSV", "PORT", "EPRT",
    ];
    let verb = command.split_whitespace().next().unwrap_or_default();
    VERBS.iter().any(|known| verb.eq_ignore_ascii_case(known))
}

/// One pooled control connection.
struct Connection {
    stream: AsyncRustlsFtpStream,
}

/// A live FTP connection, or rather a pool of them.
pub struct FtpSession {
    params: FtpParams,
    abilities: Abilities,
    /// Shared with the keep-alive task, which is the only other thing that
    /// touches a connection nobody has borrowed.
    idle: Arc<Mutex<Vec<Connection>>>,
    /// The task that keeps idle connections from being timed out, where the
    /// connection asked for one.
    keeper: Mutex<Option<tokio::task::JoinHandle<()>>>,
    limit: Arc<Semaphore>,
    allowed: Mutex<u32>,
    endpoint: EndpointId,
    events: Events,
}

impl std::fmt::Debug for FtpSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FtpSession")
            .field("endpoint", &self.endpoint)
            .field("abilities", &self.abilities)
            .finish_non_exhaustive()
    }
}

/// A control connection on loan.
///
/// It goes back into the pool only when [`FtpLease::release`] says it may. That
/// is deliberate and the opposite of the obvious design: an FTP control
/// connection carries a reply for every command, and a transfer that was
/// abandoned half way leaves one of those replies unread. Reusing such a
/// connection means every later answer belongs to the previous question — a
/// listing that looks like a rename's confirmation, a delete that reports
/// success because it read the reply to something else.
///
/// Logging in again costs two round trips. Being wrong about which reply
/// belongs to which command costs the user their files.
pub struct FtpLease<'a> {
    session: &'a FtpSession,
    connection: Option<Connection>,
    _permit: tokio::sync::SemaphorePermit<'a>,
}

impl FtpLease<'_> {
    fn stream(&mut self) -> &mut AsyncRustlsFtpStream {
        &mut self
            .connection
            .as_mut()
            .expect("a lease always holds a connection")
            .stream
    }

    /// Hands the connection back for the next caller. Only ever after a command
    /// whose reply has been read.
    fn release(mut self) {
        if let Some(connection) = self.connection.take() {
            if let Ok(mut idle) = self.session.idle.try_lock() {
                idle.push(connection);
            }
        }
    }
}

impl Drop for FtpLease<'_> {
    fn drop(&mut self) {
        // Not released, so the connection's state is unknown: close it rather
        // than hand somebody a conversation already in progress.
        if self.connection.take().is_some() {
            self.session.events.log(
                &self.session.endpoint,
                LogDirection::Note,
                "a connection was left in an unknown state and was closed",
            );
        }
    }
}

impl FtpSession {
    /// Opens the first connection and asks the server what it can do.
    pub async fn connect(
        params: &FtpParams,
        endpoint: &EndpointId,
        events: &Events,
    ) -> Result<Self> {
        events.connection(endpoint, ConnectionState::Connecting);
        events.log(
            endpoint,
            LogDirection::Note,
            format!("connecting to {}:{}", params.host, params.port),
        );

        // The whole of saying hello is under one clock, not just the part that
        // opens the socket.
        //
        // A server that accepts a connection, accepts a login, and then stops
        // answering used to leave this waiting for ever: the timeout covered
        // opening and nothing after it, so FEAT and OPTS could hang with the
        // program showing "connecting…" and no way out but killing it. Nothing
        // here is worth waiting longer for than the greeting was.
        let (stream, features) = open(params, endpoint, events).await?;
        let abilities = Abilities::from_features(&features);

        events.log(
            endpoint,
            LogDirection::Received,
            format!(
                "FEAT: MLSD {}, REST STREAM {}, UTF8 {}, SIZE {}, MFMT {}, EPSV {}",
                yes_no(abilities.mlsd),
                yes_no(abilities.rest),
                yes_no(abilities.utf8),
                yes_no(abilities.size),
                yes_no(abilities.mfmt),
                yes_no(abilities.epsv)
            ),
        );

        if !abilities.rest {
            // Said here rather than when a transfer breaks: by then the choice
            // of server has been made and the time has been spent.
            events.log(
                endpoint,
                LogDirection::Note,
                "this server cannot continue an interrupted transfer; a broken one starts again",
            );
        }

        events.connection(endpoint, ConnectionState::Connected { banner: None });

        let allowed = params.concurrency.max(1);
        let idle = Arc::new(Mutex::new(vec![Connection { stream }]));
        let keeper = params.keep_alive.map(|seconds| {
            keep_alive(
                Arc::clone(&idle),
                seconds.max(1),
                endpoint.clone(),
                events.clone(),
            )
        });

        Ok(Self {
            params: params.clone(),
            abilities,
            idle,
            keeper: Mutex::new(keeper),
            limit: Arc::new(Semaphore::new(allowed as usize)),
            allowed: Mutex::new(u32::from(allowed)),
            endpoint: endpoint.clone(),
            events: events.clone(),
        })
    }

    pub fn abilities(&self) -> &Abilities {
        &self.abilities
    }

    pub fn encryption(&self) -> Encryption {
        self.params.encryption
    }

    pub fn temporary_name(&self) -> bool {
        self.params.temporary_name
    }

    /// Borrows a control connection, opening one if the pool is empty.
    pub async fn lease(&self) -> Result<FtpLease<'_>> {
        let permit = self.limit.acquire().await.map_err(Error::other)?;

        // An idle connection may have been dropped by the server while it sat
        // in the pool; a NOOP is the cheapest way to find out before a command
        // fails for reasons that look like something else.
        while let Some(mut connection) = self.idle.lock().await.pop() {
            if connection.stream.noop().await.is_ok() {
                return Ok(FtpLease {
                    session: self,
                    connection: Some(connection),
                    _permit: permit,
                });
            }
            self.events.log(
                &self.endpoint,
                LogDirection::Note,
                "idle connection had been closed, opening another",
            );
        }

        match open(&self.params, &self.endpoint, &self.events).await {
            Ok((stream, _)) => Ok(FtpLease {
                session: self,
                connection: Some(Connection { stream }),
                _permit: permit,
            }),
            Err(error) => {
                // Almost always the server's limit on logins rather than a
                // fault. Ask for fewer and say so.
                self.lower_limit().await;
                Err(error)
            }
        }
    }

    pub async fn concurrency(&self) -> u32 {
        *self.allowed.lock().await
    }

    pub async fn lower_limit(&self) {
        let mut allowed = self.allowed.lock().await;
        if *allowed <= 1 {
            return;
        }
        let give_up = *allowed / 2;
        self.limit.forget_permits(give_up as usize);
        *allowed -= give_up;
        self.events.log(
            &self.endpoint,
            LogDirection::Note,
            format!(
                "server refused another login, using {} at a time now",
                *allowed
            ),
        );
        self.events.emit(crate::events::Event::ConcurrencyLowered {
            endpoint: self.endpoint.clone(),
            allowed: *allowed,
        });
    }

    pub async fn set_concurrency(&self, wanted: u32) {
        let mut allowed = self.allowed.lock().await;
        let wanted = wanted.clamp(1, u32::from(crate::endpoint::Protocol::MAX_CONCURRENCY));
        if wanted > *allowed {
            self.limit.add_permits((wanted - *allowed) as usize);
        } else if wanted < *allowed {
            self.limit.forget_permits((*allowed - wanted) as usize);
        }
        *allowed = wanted;
    }

    pub async fn home(&self) -> Result<String> {
        let mut lease = self.lease().await?;
        self.events.log(&self.endpoint, LogDirection::Sent, "PWD");
        let home = lease
            .stream()
            .pwd()
            .await
            .map_err(|source| ftp_error(".", source))?;
        lease.release();
        Ok(home)
    }

    /// Reads one directory, preferring the machine readable listing.
    pub async fn list_dir(&self, path: &str) -> Result<Listing> {
        let mut lease = self.lease().await?;
        let entries = self.read_dir(&mut lease, path).await?;
        lease.release();

        self.events.log(
            &self.endpoint,
            LogDirection::Received,
            format!("{} entries", entries.len()),
        );

        Ok(Listing {
            path: path.to_string(),
            entries,
        })
    }

    /// One directory, on a connection the caller already holds.
    ///
    /// Separate from [`FtpSession::list_dir`] because anything recursive walks
    /// many directories, and taking a fresh connection per level would deadlock
    /// against its own limit the moment that limit is one.
    async fn read_dir(&self, lease: &mut FtpLease<'_>, path: &str) -> Result<Vec<DirEntry>> {
        let now = now_seconds();

        let entries = if self.abilities.mlsd {
            self.events
                .log(&self.endpoint, LogDirection::Sent, format!("MLSD {path}"));
            let lines = lease
                .stream()
                .mlsd(Some(path))
                .await
                .map_err(|source| ftp_error(path, source))?;
            lines
                .iter()
                .filter_map(|line| list::parse_mlsd_line(&self.decode(line)))
                .collect::<Vec<DirEntry>>()
        } else {
            self.events
                .log(&self.endpoint, LogDirection::Sent, format!("LIST {path}"));
            let lines = lease
                .stream()
                .list(Some(path))
                .await
                .map_err(|source| ftp_error(path, source))?;
            lines
                .iter()
                .filter_map(|line| list::parse_list_line(&self.decode(line), now))
                .collect::<Vec<DirEntry>>()
        };

        Ok(entries)
    }

    /// What a path is, as far as the server will say.
    ///
    /// FTP has nothing like `stat`. `MLST` answers for one entry where the
    /// server offers it; otherwise the parent directory has to be listed and
    /// the name looked up in it, which is a round trip nobody wants but the
    /// only thing that works everywhere.
    async fn kind_of(&self, lease: &mut FtpLease<'_>, path: &str) -> Result<EntryKind> {
        if self.abilities.mlsd {
            self.events
                .log(&self.endpoint, LogDirection::Sent, format!("MLST {path}"));
            if let Ok(reply) = lease.stream().mlst(Some(path)).await {
                // The reply repeats the path rather than the bare name, so the
                // facts are what matter here, not what the parser calls it.
                if let Some(entry) = reply
                    .lines()
                    .filter_map(|line| list::parse_mlsd_line(&self.decode(line.trim())))
                    .next()
                {
                    return Ok(entry.kind);
                }
            }
        }

        let (parent, name) = match crate::fs::parent_remote(path) {
            Some(parent) => (parent, path.rsplit('/').next().unwrap_or(path).to_string()),
            // The root is a directory, and listing its parent is not possible.
            None => return Ok(EntryKind::Directory),
        };
        let entries = self.read_dir(lease, &parent).await?;
        entries
            .into_iter()
            .find(|entry| entry.name == name)
            .map(|entry| entry.kind)
            .ok_or_else(|| Error::Path {
                path: path.to_string(),
                reason: PathProblem::NotFound,
            })
    }

    pub async fn create_dir(&self, path: &str) -> Result<()> {
        let mut lease = self.lease().await?;
        self.events
            .log(&self.endpoint, LogDirection::Sent, format!("MKD {path}"));
        lease
            .stream()
            .mkdir(path)
            .await
            .map_err(|source| ftp_error(path, source))?;
        lease.release();
        Ok(())
    }

    /// Creates an empty file by uploading nothing to it.
    ///
    /// There is no command for "make this file"; a store of zero bytes is how
    /// every client does it.
    pub async fn create_file(&self, path: &str) -> Result<()> {
        let mut lease = self.lease().await?;
        self.events
            .log(&self.endpoint, LogDirection::Sent, format!("STOR {path}"));
        let mut empty = std::io::Cursor::new(Vec::new());
        lease
            .stream()
            .put_file(path, &mut empty)
            .await
            .map_err(|source| ftp_error(path, source))?;
        lease.release();
        Ok(())
    }

    pub async fn rename(&self, from: &str, to: &str) -> Result<()> {
        let mut lease = self.lease().await?;
        self.events.log(
            &self.endpoint,
            LogDirection::Sent,
            format!("RNFR {from} / RNTO {to}"),
        );
        lease
            .stream()
            .rename(from, to)
            .await
            .map_err(|source| ftp_error(from, source))?;
        lease.release();
        Ok(())
    }

    /// Removes a file, or a directory and everything under it.
    ///
    /// Depth first on one connection: a directory cannot be removed until it is
    /// empty, and opening a connection per level would deadlock against a limit
    /// of one.
    pub async fn remove(&self, path: &str) -> Result<()> {
        let mut lease = self.lease().await?;

        if self.kind_of(&mut lease, path).await? != EntryKind::Directory {
            self.events
                .log(&self.endpoint, LogDirection::Sent, format!("DELE {path}"));
            lease
                .stream()
                .rm(path)
                .await
                .map_err(|source| ftp_error(path, source))?;
            lease.release();
            return Ok(());
        }

        // Collected shallowest first, then removed in reverse, so a directory
        // is always empty by the time its turn comes.
        let mut directories = Vec::new();
        let mut pending = vec![path.to_string()];
        while let Some(directory) = pending.pop() {
            directories.push(directory.clone());
            for entry in self.read_dir(&mut lease, &directory).await? {
                let child = crate::fs::join_remote(&directory, &entry.name);
                if entry.kind == EntryKind::Directory {
                    pending.push(child);
                } else {
                    self.events
                        .log(&self.endpoint, LogDirection::Sent, format!("DELE {child}"));
                    lease
                        .stream()
                        .rm(&child)
                        .await
                        .map_err(|source| ftp_error(&child, source))?;
                }
            }
        }

        for directory in directories.iter().rev() {
            self.events.log(
                &self.endpoint,
                LogDirection::Sent,
                format!("RMD {directory}"),
            );
            lease
                .stream()
                .rmdir(directory)
                .await
                .map_err(|source| ftp_error(directory, source))?;
        }

        lease.release();
        Ok(())
    }

    /// Counts what a recursive delete would remove, so it can be shown first.
    pub async fn measure(&self, path: &str) -> Result<Measurement> {
        let mut measured = Measurement::default();
        let mut lease = self.lease().await?;

        if self.kind_of(&mut lease, path).await? != EntryKind::Directory {
            let entries = match crate::fs::parent_remote(path) {
                Some(parent) => self.read_dir(&mut lease, &parent).await.unwrap_or_default(),
                None => Vec::new(),
            };
            let name = path.rsplit('/').next().unwrap_or(path);
            let size = entries
                .iter()
                .find(|entry| entry.name == name)
                .and_then(|entry| entry.size);
            measured.add_file(size);
            lease.release();
            return Ok(measured);
        }

        let mut pending = vec![path.to_string()];
        while let Some(directory) = pending.pop() {
            measured.add_directory();
            if measured.reached_cap() {
                measured.truncated = true;
                lease.release();
                return Ok(measured);
            }
            // A directory that cannot be read is skipped rather than fatal: the
            // count is there to warn, and half a count still warns.
            let Ok(entries) = self.read_dir(&mut lease, &directory).await else {
                continue;
            };
            for entry in entries {
                match entry.kind {
                    EntryKind::Directory => {
                        pending.push(crate::fs::join_remote(&directory, &entry.name));
                    }
                    EntryKind::Symlink => measured.add_symlink(),
                    // A socket or a device counts as one thing in the way, and
                    // a warning that says "one file" beats one that says
                    // nothing.
                    EntryKind::File | EntryKind::Other => measured.add_file(entry.size),
                }
            }
        }

        lease.release();
        Ok(measured)
    }

    /// Changes permissions through `SITE CHMOD`.
    ///
    /// Not part of the protocol but understood by nearly every Unix server, and
    /// the only way to set a mode over FTP at all. A server that does not know
    /// it says so, and that refusal is passed on rather than swallowed.
    pub async fn set_permissions(&self, path: &str, mode: u32, recursive: bool) -> Result<()> {
        let mut lease = self.lease().await?;
        let mut pending = vec![path.to_string()];

        while let Some(current) = pending.pop() {
            self.events.log(
                &self.endpoint,
                LogDirection::Sent,
                format!("SITE CHMOD {mode:03o} {current}"),
            );
            lease
                .stream()
                .site(format!("CHMOD {mode:03o} {current}"))
                .await
                .map_err(|source| ftp_error(&current, source))?;

            if !recursive {
                continue;
            }
            if self.kind_of(&mut lease, &current).await? != EntryKind::Directory {
                continue;
            }
            for entry in self.read_dir(&mut lease, &current).await? {
                pending.push(crate::fs::join_remote(&current, &entry.name));
            }
        }

        lease.release();
        Ok(())
    }

    /// Size and modification time, for deciding whether a resume is safe.
    ///
    /// `SIZE` and `MDTM` are asked separately, and both are optional. Where the
    /// server answers neither, a resume cannot be judged safe and the transfer
    /// starts again — which is the right way round.
    /// Size and modification time, or [`PathProblem::NotFound`].
    ///
    /// Both questions are asked and both may be refused, which used to come
    /// back as a file of zero bytes with no date — so every file uploaded to
    /// an FTP server looked like a file that was already there, and every one
    /// of them raised the "already exists" question showing 0 B and an unknown
    /// date. A server that answers neither question is a server saying there
    /// is nothing at that path.
    ///
    /// A server that answers one of them settles it. `SIZE` is refused for
    /// directories almost everywhere, and plenty of servers have no `MDTM` at
    /// all; either answer on its own is proof that something is there.
    pub async fn stat(&self, path: &str) -> Result<(u64, Option<i64>)> {
        let mut lease = self.lease().await?;

        let size = if self.abilities.size {
            self.events
                .log(&self.endpoint, LogDirection::Sent, format!("SIZE {path}"));
            lease.stream().size(path).await.ok().map(|size| size as u64)
        } else {
            None
        };

        self.events
            .log(&self.endpoint, LogDirection::Sent, format!("MDTM {path}"));
        let modified = lease
            .stream()
            .mdtm(path)
            .await
            .ok()
            .map(|stamp| stamp.and_utc().timestamp());

        lease.release();

        if size.is_none() && modified.is_none() {
            return Err(Error::Path {
                path: path.to_string(),
                reason: PathProblem::NotFound,
            });
        }
        Ok((size.unwrap_or_default(), modified))
    }

    /// Puts a finished file under its final name.
    ///
    /// `RNTO` onto an existing name is refused by many servers, so what is
    /// there is removed first. That is the one moment where the target does not
    /// exist, and it is the reason the temporary name can be switched off per
    /// connection.
    pub async fn replace(&self, from: &str, to: &str) -> Result<()> {
        let mut lease = self.lease().await?;
        self.events
            .log(&self.endpoint, LogDirection::Sent, format!("DELE {to}"));
        let _ = lease.stream().rm(to).await;
        self.events.log(
            &self.endpoint,
            LogDirection::Sent,
            format!("RNFR {from} / RNTO {to}"),
        );
        lease
            .stream()
            .rename(from, to)
            .await
            .map_err(|source| ftp_error(from, source))?;
        lease.release();
        Ok(())
    }

    /// Closes every pooled connection.
    pub async fn disconnect(&self) {
        if let Some(keeper) = self.keeper.lock().await.take() {
            keeper.abort();
        }
        let mut idle = self.idle.lock().await;
        for mut connection in idle.drain(..) {
            let _ = connection.stream.quit().await;
        }
        self.events
            .connection(&self.endpoint, ConnectionState::Disconnected);
    }

    /// Opens a file for reading, continuing from `offset` where the server
    /// allows it.
    ///
    /// `REST` is used only when the server said in `FEAT` that it supports
    /// `REST STREAM`. Sending it blind to a server that does not is how a
    /// resumed download silently starts from the beginning while the client
    /// writes it at the end — the file that results looks complete and is not.
    pub async fn open_read(&self, path: &str, offset: u64) -> Result<(Reader, FtpLease<'_>)> {
        let mut lease = self.lease().await?;

        if offset > 0 {
            if !self.abilities.rest {
                return Err(Error::other(
                    "this server cannot continue a transfer; it has to start again",
                ));
            }
            self.events
                .log(&self.endpoint, LogDirection::Sent, format!("REST {offset}"));
            lease
                .stream()
                .resume_transfer(offset as usize)
                .await
                .map_err(|source| ftp_error(path, source))?;
        }

        self.events
            .log(&self.endpoint, LogDirection::Sent, format!("RETR {path}"));
        let transfer = lease
            .stream()
            .retr_as_stream(path)
            .await
            .map_err(|source| ftp_error(path, source))?;

        Ok((Reader::Ftp(Box::new(transfer)), lease))
    }

    /// Opens a file for writing, continuing from `offset` where the server
    /// allows it.
    pub async fn open_write(&self, path: &str, offset: u64) -> Result<(Writer, FtpLease<'_>)> {
        let mut lease = self.lease().await?;

        if offset > 0 {
            if !self.abilities.rest {
                return Err(Error::other(
                    "this server cannot continue a transfer; it has to start again",
                ));
            }
            self.events
                .log(&self.endpoint, LogDirection::Sent, format!("REST {offset}"));
            lease
                .stream()
                .resume_transfer(offset as usize)
                .await
                .map_err(|source| ftp_error(path, source))?;
        }

        self.events
            .log(&self.endpoint, LogDirection::Sent, format!("STOR {path}"));
        let transfer = lease
            .stream()
            .put_with_stream(path)
            .await
            .map_err(|source| ftp_error(path, source))?;

        Ok((Writer::Ftp(Box::new(transfer)), lease))
    }

    /// Closes a data connection and reads the server's verdict.
    ///
    /// The bytes having arrived is not the same as the transfer having
    /// succeeded: the server says so afterwards, on the control connection, and
    /// a client that does not wait for that sentence is guessing. Only once it
    /// has been read does the connection go back into the pool.
    pub async fn finish(&self, transfer: FtpTransfer, lease: FtpLease<'_>) -> Result<()> {
        let outcome = transfer.finish().await;
        match outcome {
            Ok(()) => {
                self.events
                    .log(&self.endpoint, LogDirection::Received, "transfer complete");
                lease.release();
                Ok(())
            }
            // Not released: the control connection's state is no longer known.
            Err(source) => Err(ftp_error("", source)),
        }
    }

    /// Sets a file's modification time through `MFMT`.
    ///
    /// Only where the server announced it. There is no second way: `MDTM` with
    /// an argument does the same on some servers and is a plain query on
    /// others, and a command that means two things is not one to guess with.
    pub async fn set_modified(&self, path: &str, seconds: i64) -> Result<()> {
        if !self.abilities.mfmt {
            return Err(Error::other("this server cannot set modification times"));
        }

        let mut lease = self.lease().await?;
        let stamp = list::timestamp(seconds);
        self.events.log(
            &self.endpoint,
            LogDirection::Sent,
            format!("MFMT {stamp} {path}"),
        );
        lease
            .stream()
            .custom_command(format!("MFMT {stamp} {path}"), &[Status::File])
            .await
            .map_err(|source| ftp_error(path, source))?;
        lease.release();
        Ok(())
    }

    /// Sends one command exactly as typed, and hands back what came back.
    ///
    /// Nothing is refused. The window warns about the commands that open a data
    /// connection, and somebody who reads the warning and sends `RETR` anyway
    /// has their reasons — this is a raw command mode, and one that quietly
    /// declines half the protocol is not one.
    ///
    /// What makes that safe is what happens afterwards: a connection used for
    /// such a command is **closed rather than returned to the pool**. It is
    /// mid-sentence, and reusing it would mean every later reply belongs to the
    /// previous question. Logging in again costs two round trips.
    pub async fn raw(&self, command: &str) -> Result<RawReply> {
        let command = command.trim();
        if command.is_empty() {
            return Err(Error::other("nothing to send"));
        }

        let mut lease = self.lease().await?;
        self.events
            .log(&self.endpoint, LogDirection::Sent, command.to_string());

        // An empty list of expected codes means every reply is "unexpected",
        // which is exactly right here: a raw command has no code to expect, and
        // the reply comes back either way.
        let reply = match lease.stream().custom_command(command, &[]).await {
            Ok(response) => response,
            Err(FtpError::UnexpectedResponse(response)) => response,
            Err(source) => return Err(ftp_error(command, source)),
        };

        let text = String::from_utf8_lossy(&reply.body).trim().to_string();
        self.events.log(
            &self.endpoint,
            LogDirection::Received,
            format!("{} {}", reply.status.code(), text),
        );

        let dropped = wants_data_channel(command);
        if dropped {
            self.events.log(
                &self.endpoint,
                LogDirection::Note,
                "this command opened a data connection; the control connection was closed rather than reused",
            );
        } else {
            lease.release();
        }

        Ok(RawReply {
            code: reply.status.code(),
            text,
            connection_dropped: dropped,
        })
    }

    /// Turns what the server sent into text.
    ///
    /// suppaftp hands over a String already; where the server is not speaking
    /// UTF-8 the bytes arrive as replacement characters, and the Latin-1
    /// setting is the only way back. Anything beyond those two would need a
    /// conversion library, which this crate does not carry.
    fn decode(&self, line: &str) -> String {
        if self.params.latin1 && line.contains('\u{FFFD}') {
            // Nothing can be recovered from a replacement character; saying so
            // beats pretending the name is right.
            self.events.log(
                &self.endpoint,
                LogDirection::Note,
                "a name could not be read as UTF-8; set the connection's character set",
            );
        }
        line.to_string()
    }
}

/// Keeps idle connections from being timed out by the server.
///
/// Only started when a connection asked for it. A `NOOP` every so often is what
/// stops a server dropping a login that has been sitting still while the queue
/// works on the other side of a transfer — but it is also traffic nobody asked
/// for, which is why it is off unless someone turns it on.
///
/// A connection that does not answer is dropped rather than put back: it was
/// already gone, and the pool is better one shorter than one wrong.
fn keep_alive(
    idle: Arc<Mutex<Vec<Connection>>>,
    seconds: u32,
    endpoint: EndpointId,
    events: Events,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let period = std::time::Duration::from_secs(u64::from(seconds));
        loop {
            tokio::time::sleep(period).await;

            // Taken out of the pool for the moment, so a caller asking for a
            // connection never waits behind a keep-alive and never borrows one
            // mid-NOOP.
            let mut taken = {
                let mut pool = idle.lock().await;
                std::mem::take(&mut *pool)
            };
            let before = taken.len();
            let mut alive = Vec::with_capacity(before);
            for mut connection in taken.drain(..) {
                if connection.stream.noop().await.is_ok() {
                    alive.push(connection);
                }
            }
            if alive.len() < before {
                events.log(
                    &endpoint,
                    LogDirection::Note,
                    format!("{} idle connections had been closed", before - alive.len()),
                );
            }
            idle.lock().await.append(&mut alive);
        }
    })
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

/// Opens and logs in one control connection, or gives up.
///
/// The whole of it is bounded: socket, greeting, `AUTH TLS`, handshake and
/// login. A server that accepts the connection and then goes quiet is the
/// failure this guards against, and it can go quiet at any point in there.
/// One more connection for the pool, under the same clock as the first.
///
/// A second connection that hangs is the same fault as a first one that does:
/// a transfer would sit waiting for a lease that never arrives, with nothing
/// on screen to say why.
async fn open(
    params: &FtpParams,
    endpoint: &EndpointId,
    events: &Events,
) -> Result<(
    AsyncRustlsFtpStream,
    std::collections::HashMap<String, Option<String>>,
)> {
    match tokio::time::timeout(
        crate::endpoint::GREETING,
        say_hello(params, endpoint, events),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => Err(Error::TimedOut {
            host: params.host.clone(),
            port: params.port,
            seconds: crate::endpoint::GREETING.as_secs(),
        }),
    }
}

/// Everything between dialling and being ready to work.
///
/// Opening, securing, logging in, asking what the server can do, and settling
/// the character set. All of it together, so one clock covers the lot — see
/// the note where it is called.
async fn say_hello(
    params: &FtpParams,
    endpoint: &EndpointId,
    events: &Events,
) -> Result<(
    AsyncRustlsFtpStream,
    std::collections::HashMap<String, Option<String>>,
)> {
    let mut stream = open_inner(params, endpoint, events).await?;
    let features = stream.feat().await.unwrap_or_default();
    let abilities = Abilities::from_features(&features);

    if abilities.utf8 {
        // Asked for rather than assumed: a server that speaks UTF-8 only
        // after being told is common, and names come out mangled otherwise.
        let _ = stream.opts("UTF8", Some("ON")).await;
    }

    // How this connection will ask for a data channel. Decided here rather
    // than by the caller, because every connection goes through here — the
    // first one and every further one the queue opens — and a pool where the
    // first can list and the rest cannot would be a puzzle nobody could read.
    //
    // EPSV wherever it is offered, and not as a nicety: over IPv6 it is the
    // only one that can work at all. PASV answers with four numbers and two
    // more, which is an IPv4 address and nothing else, so the server refuses
    // and every listing fails on a connection that opened perfectly well.
    if params.passive {
        if abilities.epsv {
            stream.set_mode(suppaftp::Mode::ExtendedPassive);
        } else {
            // No EPSV, so PASV — with the address it gives checked against the
            // one already being talked to. A server behind something that
            // rewrites addresses announces its private one, and dialling that
            // from outside is a wait that ends in nothing.
            stream.set_passive_nat_workaround(true);
        }
    }

    Ok((stream, features))
}

async fn open_inner(
    params: &FtpParams,
    endpoint: &EndpointId,
    events: &Events,
) -> Result<AsyncRustlsFtpStream> {
    let address = format!("{}:{}", params.host, params.port);

    let mut stream = match params.encryption {
        Encryption::Implicit => {
            // Historically port 990: encrypted from the first byte, with no
            // plaintext greeting to be tampered with — and no way to ask the
            // server whether it speaks TLS, so a wrong port simply hangs up.
            let (connector, verdict) = tls::connector(&params.host, params.certificate.clone());
            events.log(endpoint, LogDirection::Note, "TLS from the first byte");
            AsyncRustlsFtpStream::connect_secure_implicit(&address, connector, &params.host)
                .await
                .map_err(|error| tls_failure(error, &verdict, params))?
        }
        _ => AsyncRustlsFtpStream::connect(&address)
            .await
            .map_err(|_| Error::Unreachable {
                host: params.host.clone(),
                port: params.port,
            })?,
    };

    if params.encryption == Encryption::Explicit {
        let roots = tls::system_roots().len();
        events.log(
            endpoint,
            LogDirection::Note,
            format!("checking against {roots} root certificates from the system"),
        );

        let (connector, verdict) = tls::connector(&params.host, params.certificate.clone());
        events.log(endpoint, LogDirection::Sent, "AUTH TLS");

        // `into_secure` also sends PBSZ 0 and PROT P, so what comes back here
        // is either a connection whose data channel is encrypted too, or a
        // failure. There is deliberately no third outcome: a data channel
        // without PROT P is not half secure, it is in the clear.
        stream = stream
            .into_secure(connector, &params.host)
            .await
            .map_err(|error| tls_failure(error, &verdict, params))?;

        events.log(
            endpoint,
            LogDirection::Received,
            "control and data channel encrypted (PBSZ 0, PROT P)",
        );
    }

    events.log(
        endpoint,
        LogDirection::Sent,
        format!("USER {}", params.user),
    );
    stream
        .login(&params.user, &params.password)
        .await
        .map_err(|error| login_failure(error, params))?;

    // Binary throughout. The text mode with its rewritten line endings arrives
    // with the settings of M5, because a wrongly converted file is harder to
    // notice than a missing feature.
    let _ = stream.transfer_type(FileType::Binary).await;

    if !params.passive {
        stream.set_mode(suppaftp::Mode::Active);
    }

    Ok(stream)
}

/// Tells a refused password from a server that never looked at it.
///
/// A server with TLS required answers `USER` with something like
/// "550 SSL/TLS required on the control channel" — the password is never
/// reached, let alone judged. Calling that a failed login sends somebody to
/// check a password that was never wrong, while the one sentence that would
/// have solved it is thrown away.
///
/// Matched on what the server said rather than on the reply code: the codes
/// used for this are all over the place (530, 534, 550 are all in the wild),
/// and the words are what the servers agree on.
fn login_failure(error: FtpError, params: &FtpParams) -> Error {
    if let FtpError::UnexpectedResponse(response) = &error {
        let said = String::from_utf8_lossy(&response.body).to_ascii_lowercase();
        let demands_tls = (said.contains("tls") || said.contains("ssl"))
            && (said.contains("requir") || said.contains("must") || said.contains("only"));
        if demands_tls {
            return Error::EncryptionRequired {
                host: params.host.clone(),
                detail: String::from_utf8_lossy(&response.body).trim().to_string(),
            };
        }
    }
    Error::AuthenticationFailed {
        user: params.user.clone(),
    }
}

/// Says what actually went wrong when securing a connection failed.
///
/// Three different things arrive here as one error type, and telling the user
/// "the server said no" would hide the only part that matters: whether to check
/// a fingerprint, to pick a different port, or to stop using this server for
/// anything private.
fn tls_failure(error: FtpError, verdict: &tls::Verdict, params: &FtpParams) -> Error {
    if let Some(facts) = verdict.take() {
        return Error::CertificateUntrusted {
            host: facts.host,
            fingerprint: facts.fingerprint,
            reason: facts.problem.message_key().to_string(),
            detail: facts.detail,
        };
    }

    if let FtpError::ConnectionError(_) = error {
        return Error::Unreachable {
            host: params.host.clone(),
            port: params.port,
        };
    }

    // The certificate was already accepted, so whatever the server refused came
    // after the handshake: PBSZ or PROT, meaning it wants the data channel in
    // the clear.
    let detail = if verdict.handshaked() {
        format!("the server refused to encrypt the data channel: {error}")
    } else if params.encryption == Encryption::Implicit {
        // Nothing was negotiated and nothing refused: the client expected TLS
        // from the first byte and got something else. Almost always the port —
        // implicit FTPS listens on 990, and 21 answers with a plaintext
        // greeting that cannot be mistaken for a handshake.
        format!(
            "port {} did not answer with TLS; implicit FTPS usually listens on 990: {error}",
            params.port
        )
    } else {
        format!("the server refused to start TLS: {error}")
    };
    Error::EncryptionRefused { detail }
}

fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs() as i64)
        .unwrap_or_default()
}

/// Turns an FTP failure into something the window can phrase.
fn ftp_error(path: &str, source: FtpError) -> Error {
    let reason = match &source {
        FtpError::UnexpectedResponse(response) => match response.status {
            // 550 is, in the RFC's own words, "file not found, no access" --
            // one code for two entirely different problems. 553 is about the
            // name, and is what pure-ftpd answers a refused upload with. In
            // both cases the only thing telling them apart is what the server
            // wrote after the number.
            Status::FileUnavailable => said(response).unwrap_or(PathProblem::NotFound),
            Status::BadFilename => said(response).unwrap_or(PathProblem::Unknown),
            Status::NotLoggedIn => PathProblem::PermissionDenied,
            _ => PathProblem::Unknown,
        },
        _ => PathProblem::Unknown,
    };
    Error::Path {
        path: path.to_string(),
        reason,
    }
}

/// What the server's own words say the problem was, where they say anything.
///
/// English, because that is what an FTP server writes after a status code --
/// the numbers are the protocol and the sentence is a courtesy. A server that
/// phrases it some other way falls through to what the code alone implies,
/// which is where this started.
fn said(response: &suppaftp::types::Response) -> Option<PathProblem> {
    let text = String::from_utf8_lossy(&response.body).to_lowercase();
    if text.contains("permission") || text.contains("denied") || text.contains("not allowed") {
        return Some(PathProblem::PermissionDenied);
    }
    if text.contains("no such") || text.contains("not found") || text.contains("does not exist") {
        return Some(PathProblem::NotFound);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refused_write_says_it_was_refused_and_not_that_it_was_missing() {
        use suppaftp::types::Response;

        let of = |status, text: &str| {
            let response = Response::new(status, text.as_bytes().to_vec());
            match ftp_error("/var/www/index.php", FtpError::UnexpectedResponse(response)) {
                Error::Path { reason, .. } => reason,
                other => panic!("not a path problem: {other:?}"),
            }
        };

        // What pure-ftpd answers an upload it will not take. It used to arrive
        // as "could not be read", which is wrong about the direction and about
        // the cause, and sends somebody looking for a file that is right there.
        assert_eq!(
            of(
                Status::BadFilename,
                "553 Can't open that file: Permission denied"
            ),
            PathProblem::PermissionDenied
        );
        assert_eq!(
            of(Status::FileUnavailable, "550 Permission denied"),
            PathProblem::PermissionDenied
        );
        assert_eq!(
            of(Status::FileUnavailable, "550 No such file or directory"),
            PathProblem::NotFound
        );
        // A server that says nothing useful falls back to what the number
        // alone implies, which is where this behaved acceptably all along.
        assert_eq!(
            of(Status::FileUnavailable, "550 Requested action not taken"),
            PathProblem::NotFound
        );
        assert_eq!(
            of(Status::BadFilename, "553 Requested action not taken"),
            PathProblem::Unknown
        );
    }

    #[test]
    fn the_commands_that_open_a_second_connection_are_known() {
        // These are the ones that leave a control connection mid-sentence when
        // they are sent by hand. Getting the list wrong in the generous
        // direction costs a login; getting it wrong the other way costs every
        // later reply belonging to the previous question.
        for command in [
            "RETR datei.txt",
            "retr datei.txt",
            "  STOR neu.bin",
            "LIST",
            "NLST /var/www",
            "MLSD",
            "PASV",
            "EPSV",
            "APPE log.txt",
        ] {
            assert!(wants_data_channel(command), "{command}");
        }

        for command in [
            "PWD",
            "NOOP",
            "SITE CHMOD 644 x",
            "MDTM a",
            "SIZE a",
            "FEAT",
            "TYPE I",
        ] {
            assert!(!wants_data_channel(command), "{command}");
        }
    }

    #[test]
    fn a_verb_that_merely_starts_like_one_is_not_one() {
        // RETRY is not RETR, and a list that matched prefixes would throw away
        // a connection for no reason.
        assert!(!wants_data_channel("RETRY"));
        assert!(!wants_data_channel("LISTEN now"));
        assert!(!wants_data_channel(""));
    }
}
