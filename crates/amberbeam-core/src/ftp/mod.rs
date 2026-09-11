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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encryption {
    /// Plain FTP. Possible because many old servers speak nothing else, and
    /// marked as such wherever the connection is shown.
    None,
    /// `AUTH TLS` on the ordinary port. The default for new entries.
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
            mlsd: has("MLSD"),
            rest,
            utf8: has("UTF8"),
            size: has("SIZE"),
            mfmt,
        }
    }
}

/// One pooled control connection.
struct Connection {
    stream: AsyncRustlsFtpStream,
}

/// A live FTP connection, or rather a pool of them.
pub struct FtpSession {
    params: FtpParams,
    abilities: Abilities,
    idle: Mutex<Vec<Connection>>,
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

        let mut stream = open(params, endpoint, events).await?;

        let features = stream.feat().await.unwrap_or_default();
        let abilities = Abilities::from_features(&features);
        events.log(
            endpoint,
            LogDirection::Received,
            format!(
                "FEAT: MLSD {}, REST STREAM {}, UTF8 {}",
                yes_no(abilities.mlsd),
                yes_no(abilities.rest),
                yes_no(abilities.utf8)
            ),
        );

        if abilities.utf8 {
            // Asked for rather than assumed: a server that speaks UTF-8 only
            // after being told is common, and names come out mangled otherwise.
            let _ = stream.opts("UTF8", Some("ON")).await;
        }

        events.connection(endpoint, ConnectionState::Connected { banner: None });

        let allowed = params.concurrency.max(1);
        Ok(Self {
            params: params.clone(),
            abilities,
            idle: Mutex::new(vec![Connection { stream }]),
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
            Ok(stream) => Ok(FtpLease {
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
    pub async fn stat(&self, path: &str) -> Result<(u64, Option<i64>)> {
        let mut lease = self.lease().await?;

        let size = if self.abilities.size {
            self.events
                .log(&self.endpoint, LogDirection::Sent, format!("SIZE {path}"));
            lease.stream().size(path).await.unwrap_or_default() as u64
        } else {
            0
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
        Ok((size, modified))
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

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

/// Opens and logs in one control connection.
async fn open(
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
        .map_err(|_| Error::AuthenticationFailed {
            user: params.user.clone(),
        })?;

    // Binary throughout. The text mode with its rewritten line endings arrives
    // with the settings of M5, because a wrongly converted file is harder to
    // notice than a missing feature.
    let _ = stream.transfer_type(FileType::Binary).await;

    if !params.passive {
        stream.set_mode(suppaftp::Mode::Active);
    }

    Ok(stream)
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
            Status::FileUnavailable => PathProblem::NotFound,
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
