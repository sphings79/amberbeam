//! SFTP: connecting, authenticating, listing.
//!
//! One connection carries one SFTP channel. Several transfers at the same time
//! become several channels inside it, which is why the default concurrency for
//! SFTP is eight rather than the ten OpenSSH allows — see
//! [`crate::endpoint::Protocol::default_concurrency`]. That part arrives with
//! milestone M2; what is here is enough to connect and read directories.

pub mod auth;
pub mod hostkey;

use std::sync::Arc;

use russh::client::{self, Handle};
use russh::keys::known_hosts::{
    known_host_keys, known_host_keys_path, learn_known_hosts, learn_known_hosts_path,
};
use russh::keys::{PublicKey, PublicKeyOrCertificate};
use russh::Disconnect;
use russh_sftp::client::SftpSession as RawSftp;
use russh_sftp::protocol::FileType;
use tokio::sync::{Mutex, Semaphore};

use crate::endpoint::EndpointId;
use crate::error::{Error, PathProblem, Result};
use crate::events::{ConnectionState, Events, LogDirection};
use crate::fs::{join_remote, DirEntry, EntryKind, Listing, Permissions};
use crate::ops::Measurement;
use crate::stream::{Reader, Writer};

pub use auth::AuthMethod;
pub use hostkey::{fingerprint, HostKeyDecision};

/// Everything needed for one attempt.
#[derive(Debug, Clone)]
pub struct ConnectParams {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub method: AuthMethod,
    /// What the user has already agreed to about the server's key.
    pub host_key: HostKeyDecision,
    /// How many transfers this connection may run at once. Per connection,
    /// because a shared hoster and a machine of one's own are not the same
    /// thing — with a global setting as the fallback when nothing was said.
    pub concurrency: u8,
    /// How often a broken transfer is retried before the job is paused.
    pub retries: u8,
    /// Which `known_hosts` file to consult. `None` means the one the terminal
    /// uses, `~/.ssh/known_hosts`, which is the point on a desktop. The
    /// container build of M7 has no such home directory, and tests must not
    /// write into the developer's own file — both need to say where.
    pub known_hosts: Option<String>,
}

/// The russh side of host key checking.
///
/// The decision itself lives in [`hostkey::verify`], which is tested on its
/// own; this only reads `known_hosts` and hands the answer back.
struct Verifier {
    host: String,
    port: u16,
    decision: HostKeyDecision,
    verdict: Arc<hostkey::Verdict>,
    known_hosts: Option<String>,
}

impl client::Handler for Verifier {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        offered: &PublicKeyOrCertificate,
    ) -> std::result::Result<bool, Self::Error> {
        let key = match offered {
            PublicKeyOrCertificate::PublicKey { key, .. } => key.clone(),
            PublicKeyOrCertificate::Certificate(certificate) => {
                PublicKey::new(certificate.public_key().clone(), "")
            }
        };
        let recorded: Vec<PublicKey> = match &self.known_hosts {
            Some(path) => known_host_keys_path(&self.host, self.port, path),
            None => known_host_keys(&self.host, self.port),
        }
        .unwrap_or_default()
        .into_iter()
        .map(|(_line, key)| key)
        .collect();

        Ok(hostkey::verify(
            &self.host,
            &key,
            &self.decision,
            &self.verdict,
            &recorded,
        ))
    }
}

/// A live SFTP connection.
///
/// One SSH connection carries several SFTP channels. The one called `browse`
/// belongs to the pane the user is clicking in and is never lent out, so a
/// listing stays instant while eight transfers are running — which is the whole
/// reason for opening more than one channel in the first place.
///
/// Channels are cheap, but not free: OpenSSH caps them through `MaxSessions`,
/// whose default is ten. That is why the default concurrency for SFTP is eight
/// rather than ten — see [`crate::endpoint::Protocol::default_concurrency`] —
/// and why a server that refuses one is not an error but a signal to ask for
/// fewer.
pub struct SftpSession {
    /// Locked only to open a channel or to say goodbye. Everything else works
    /// on channels, so browsing and eight transfers never queue behind one
    /// another on a lock.
    handle: Mutex<Handle<Verifier>>,
    browse: RawSftp,
    /// Channels opened for transfers, handed out and returned.
    idle: Mutex<Vec<RawSftp>>,
    /// How many transfer channels may exist at once.
    limit: Arc<Semaphore>,
    /// What the limit currently stands at, since a semaphore does not say.
    allowed: Mutex<u32>,
    endpoint: EndpointId,
    events: Events,
}

/// A transfer channel on loan. Returns itself when dropped.
pub struct Lease<'a> {
    session: &'a SftpSession,
    channel: Option<RawSftp>,
    _permit: tokio::sync::SemaphorePermit<'a>,
}

impl Lease<'_> {
    fn sftp(&self) -> &RawSftp {
        self.channel
            .as_ref()
            .expect("a lease always holds a channel")
    }
}

impl Drop for Lease<'_> {
    fn drop(&mut self) {
        if let Some(channel) = self.channel.take() {
            // Back into the pool rather than closed: opening a channel costs a
            // round trip, and a queue of a thousand small files would spend
            // most of its time on that.
            if let Ok(mut idle) = self.session.idle.try_lock() {
                idle.push(channel);
            }
        }
    }
}

impl std::fmt::Debug for SftpSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SftpSession")
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}

impl SftpSession {
    /// Opens a connection, or explains why it did not open.
    ///
    /// An unknown or changed host key ends here with
    /// [`Error::HostKeyUnknown`] or [`Error::HostKeyChanged`]. The window shows
    /// the fingerprint, and a second attempt carries the user's decision in
    /// [`ConnectParams::host_key`]. Nothing is written to `known_hosts` before
    /// the connection has actually worked.
    pub async fn connect(
        params: &ConnectParams,
        endpoint: &EndpointId,
        events: &Events,
    ) -> Result<Self> {
        events.connection(endpoint, ConnectionState::Connecting);
        events.log(
            endpoint,
            LogDirection::Note,
            format!("connecting to {}:{}", params.host, params.port),
        );

        let verdict = hostkey::Verdict::shared();
        let verifier = Verifier {
            host: params.host.clone(),
            port: params.port,
            decision: params.host_key.clone(),
            verdict: Arc::clone(&verdict),
            known_hosts: params.known_hosts.clone(),
        };

        let config = Arc::new(client::Config::default());
        let mut handle =
            match client::connect(config, (params.host.as_str(), params.port), verifier).await {
                Ok(handle) => handle,
                Err(source) => {
                    // A refused host key closes the connection, so the reason
                    // is in the verdict rather than in russh's error.
                    let error = verdict.take_refusal().unwrap_or(Error::Unreachable {
                        host: params.host.clone(),
                        port: params.port,
                    });
                    let _ = source;
                    events.connection(
                        endpoint,
                        ConnectionState::Failed {
                            error: error.clone(),
                        },
                    );
                    return Err(error);
                }
            };

        if let Some(error) = verdict.take_refusal() {
            events.connection(
                endpoint,
                ConnectionState::Failed {
                    error: error.clone(),
                },
            );
            return Err(error);
        }

        events.log(
            endpoint,
            LogDirection::Note,
            format!("authenticating as {}", params.user),
        );
        if let Err(error) =
            auth::authenticate(&mut handle, &params.user, &params.method, events, endpoint).await
        {
            events.connection(
                endpoint,
                ConnectionState::Failed {
                    error: error.clone(),
                },
            );
            return Err(error);
        }

        // From here on a failure has to close the connection too, for the same
        // reason: an authenticated session that nobody uses is still a session
        // the server is counting.
        let opened = async {
            let channel = handle.channel_open_session().await.map_err(Error::other)?;
            channel
                .request_subsystem(true, "sftp")
                .await
                .map_err(Error::other)?;
            RawSftp::new(channel.into_stream())
                .await
                .map_err(Error::other)
        }
        .await;

        let sftp = match opened {
            Ok(sftp) => sftp,
            Err(error) => {
                let _ = handle.disconnect(Disconnect::ByApplication, "", "en").await;
                events.connection(
                    endpoint,
                    ConnectionState::Failed {
                        error: error.clone(),
                    },
                );
                return Err(error);
            }
        };

        // Only now, with the connection proven end to end, is the key worth
        // recording. Writing it earlier would remember a server that then
        // refused every credential.
        if let Some(key) = verdict.take_to_learn() {
            let written = match &params.known_hosts {
                Some(path) => learn_known_hosts_path(&params.host, params.port, &key, path),
                None => learn_known_hosts(&params.host, params.port, &key),
            };
            match written {
                Ok(()) => events.log(
                    endpoint,
                    LogDirection::Note,
                    format!("added {} to known_hosts", hostkey::fingerprint(&key)),
                ),
                Err(source) => events.log(
                    endpoint,
                    LogDirection::Note,
                    format!("could not write known_hosts: {source}"),
                ),
            }
        }

        events.connection(endpoint, ConnectionState::Connected { banner: None });
        events.log(endpoint, LogDirection::Received, "sftp subsystem ready");

        let allowed = params.concurrency.max(1);
        Ok(Self {
            handle: Mutex::new(handle),
            browse: sftp,
            idle: Mutex::new(Vec::new()),
            limit: Arc::new(Semaphore::new(allowed as usize)),
            allowed: Mutex::new(u32::from(allowed)),
            endpoint: endpoint.clone(),
            events: events.clone(),
        })
    }

    /// Opens one more SFTP channel on the same connection.
    async fn open_channel(&self) -> Result<RawSftp> {
        let channel = self
            .handle
            .lock()
            .await
            .channel_open_session()
            .await
            .map_err(Error::other)?;
        channel
            .request_subsystem(true, "sftp")
            .await
            .map_err(Error::other)?;
        RawSftp::new(channel.into_stream())
            .await
            .map_err(Error::other)
    }

    /// Borrows a transfer channel, waiting when all of them are busy.
    pub async fn lease(&self) -> Result<Lease<'_>> {
        let permit = self.limit.acquire().await.map_err(Error::other)?;

        if let Some(channel) = self.idle.lock().await.pop() {
            return Ok(Lease {
                session: self,
                channel: Some(channel),
                _permit: permit,
            });
        }

        match self.open_channel().await {
            Ok(channel) => Ok(Lease {
                session: self,
                channel: Some(channel),
                _permit: permit,
            }),
            Err(error) => {
                // A refused channel is almost always the server's session limit
                // rather than a fault. Asking for fewer and saying so beats
                // failing the rest of the queue one job at a time.
                self.lower_limit().await;
                Err(error)
            }
        }
    }

    /// How many transfers this connection currently allows at once.
    pub async fn concurrency(&self) -> u32 {
        *self.allowed.lock().await
    }

    /// Halves what the connection asks of the server, never below one.
    ///
    /// Deliberately one way. Creeping back up would walk into the same wall
    /// every few minutes; raising it again is the user's decision, and they
    /// have the setting.
    pub async fn lower_limit(&self) {
        let mut allowed = self.allowed.lock().await;
        if *allowed <= 1 {
            return;
        }
        let give_up = *allowed / 2;
        // Permits are taken away as they become free, so running transfers
        // finish rather than being cut off.
        self.limit.forget_permits(give_up as usize);
        *allowed -= give_up;
        self.events.log(
            &self.endpoint,
            LogDirection::Note,
            format!(
                "server refused another channel, using {} at a time now",
                *allowed
            ),
        );
    }

    /// Raises the limit, for when the user changes the setting.
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

    /// Where the server puts us when we do not say otherwise.
    pub async fn home(&self) -> Result<String> {
        self.events
            .log(&self.endpoint, LogDirection::Sent, "realpath .");
        self.browse
            .canonicalize(".")
            .await
            .map_err(|source| path_error(".", source))
    }

    /// Reads one directory.
    ///
    /// Symlinks cost one extra round trip each, because whether a link leads
    /// into a directory decides whether a pane can be entered — and the listing
    /// itself does not say.
    pub async fn list_dir(&self, path: &str) -> Result<Listing> {
        self.events.log(
            &self.endpoint,
            LogDirection::Sent,
            format!("opendir {path}"),
        );

        let listed = self
            .browse
            .read_dir(path)
            .await
            .map_err(|source| path_error(path, source))?;

        let mut entries = Vec::new();
        for entry in listed {
            let name = entry.file_name();
            if name == "." || name == ".." {
                continue;
            }
            let meta = entry.metadata();
            let kind = kind_of(entry.file_type());

            let (link_target, kind_of_target) = if kind == EntryKind::Symlink {
                let full = join_remote(path, &name);
                let target = self.browse.read_link(&full).await.ok();
                let resolved = self
                    .browse
                    .metadata(&full)
                    .await
                    .ok()
                    .map(|meta| kind_of(meta.file_type()));
                (target, resolved)
            } else {
                (None, None)
            };

            entries.push(DirEntry {
                name,
                kind,
                size: (kind == EntryKind::File).then_some(meta.size).flatten(),
                modified: meta.mtime.map(i64::from),
                permissions: meta.permissions.map(|bits| Permissions(bits & 0o777)),
                // SFTP version 3 sends numbers; newer servers may send names.
                owner: meta
                    .user
                    .clone()
                    .or_else(|| meta.uid.map(|id| id.to_string())),
                group: meta
                    .group
                    .clone()
                    .or_else(|| meta.gid.map(|id| id.to_string())),
                link_target,
                kind_of_target,
            });
        }

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

    pub async fn create_dir(&self, path: &str) -> Result<()> {
        self.events
            .log(&self.endpoint, LogDirection::Sent, format!("mkdir {path}"));
        self.browse
            .create_dir(path)
            .await
            .map_err(|source| path_error(path, source))
    }

    /// Creates an empty file, and refuses to overwrite one that is there.
    ///
    /// `EXCLUDE` makes the server itself refuse an existing file. Asking first
    /// and creating afterwards would leave a gap in which the file could
    /// appear — and the whole point of this call is that nothing is lost.
    pub async fn create_file(&self, path: &str) -> Result<()> {
        use russh_sftp::protocol::OpenFlags;

        self.events
            .log(&self.endpoint, LogDirection::Sent, format!("create {path}"));
        match self
            .browse
            .open_with_flags(
                path,
                OpenFlags::CREATE | OpenFlags::EXCLUDE | OpenFlags::WRITE,
            )
            .await
        {
            Ok(file) => {
                let _ = file.close().await;
                Ok(())
            }
            Err(source) => {
                // Servers disagree on which status an existing file earns —
                // "failure" is as common as "file already exists" — so an
                // existing file is recognised rather than guessed from the code.
                if self.browse.try_exists(path).await.unwrap_or(false) {
                    Err(Error::Path {
                        path: path.to_string(),
                        reason: PathProblem::AlreadyExists,
                    })
                } else {
                    Err(path_error(path, source))
                }
            }
        }
    }

    pub async fn rename(&self, from: &str, to: &str) -> Result<()> {
        // Servers differ on whether rename replaces the target. Asking first
        // means the answer is the same everywhere, and a typo in a new name
        // never removes a file nobody mentioned.
        if self.browse.try_exists(to).await.unwrap_or(false) {
            return Err(Error::Path {
                path: to.to_string(),
                reason: PathProblem::AlreadyExists,
            });
        }
        self.events.log(
            &self.endpoint,
            LogDirection::Sent,
            format!("rename {from} -> {to}"),
        );
        self.browse
            .rename(from, to)
            .await
            .map_err(|source| path_error(from, source))
    }

    /// Counts what a recursive delete would remove, up to the cap.
    pub async fn measure(&self, path: &str) -> Result<Measurement> {
        let mut measured = Measurement::default();
        let meta = self
            .browse
            .symlink_metadata(path)
            .await
            .map_err(|source| path_error(path, source))?;

        if meta.is_symlink() {
            measured.add_symlink();
            return Ok(measured);
        }
        if !meta.is_dir() {
            measured.add_file(meta.size);
            return Ok(measured);
        }

        let mut pending = vec![path.to_string()];
        while let Some(directory) = pending.pop() {
            measured.add_directory();
            if measured.reached_cap() {
                measured.truncated = true;
                return Ok(measured);
            }
            let Ok(entries) = self.browse.read_dir(directory.clone()).await else {
                continue;
            };
            for entry in entries {
                let name = entry.file_name();
                if name == "." || name == ".." {
                    continue;
                }
                let child = join_remote(&directory, &name);
                match kind_of(entry.file_type()) {
                    // Counted as a link, never walked into: what it points at
                    // is not part of what would be removed.
                    EntryKind::Symlink => measured.add_symlink(),
                    EntryKind::Directory => {
                        pending.push(child);
                        continue;
                    }
                    _ => measured.add_file(entry.metadata().size),
                }
                if measured.reached_cap() {
                    measured.truncated = true;
                    return Ok(measured);
                }
            }
        }
        Ok(measured)
    }

    /// Removes a file, a link, or a whole directory.
    ///
    /// SFTP has no recursive delete, so the tree is walked and emptied from the
    /// inside out. Links are unlinked, never followed.
    pub async fn remove(&self, path: &str) -> Result<()> {
        let meta = self
            .browse
            .symlink_metadata(path)
            .await
            .map_err(|source| path_error(path, source))?;

        if meta.is_symlink() || !meta.is_dir() {
            self.events
                .log(&self.endpoint, LogDirection::Sent, format!("remove {path}"));
            return self
                .browse
                .remove_file(path)
                .await
                .map_err(|source| path_error(path, source));
        }

        // Depth first, deepest last in the list, so directories are emptied
        // before they are removed.
        let mut directories = Vec::new();
        let mut pending = vec![path.to_string()];
        while let Some(directory) = pending.pop() {
            directories.push(directory.clone());
            let entries = self
                .browse
                .read_dir(directory.clone())
                .await
                .map_err(|source| path_error(&directory, source))?;
            for entry in entries {
                let name = entry.file_name();
                if name == "." || name == ".." {
                    continue;
                }
                let child = join_remote(&directory, &name);
                if kind_of(entry.file_type()) == EntryKind::Directory {
                    pending.push(child);
                } else {
                    self.browse
                        .remove_file(&child)
                        .await
                        .map_err(|source| path_error(&child, source))?;
                }
            }
        }

        self.events.log(
            &self.endpoint,
            LogDirection::Sent,
            format!("rmdir {} director(ies)", directories.len()),
        );
        for directory in directories.into_iter().rev() {
            self.browse
                .remove_dir(&directory)
                .await
                .map_err(|source| path_error(&directory, source))?;
        }
        Ok(())
    }

    /// Sets the nine permission bits, optionally through a whole tree.
    ///
    /// Links are skipped. SFTP version 3 has no way to set a link's own
    /// attributes, so the only thing on offer is changing the target's — which
    /// would reach outside what the user selected.
    pub async fn set_permissions(&self, path: &str, mode: u32, recursive: bool) -> Result<()> {
        let meta = self
            .browse
            .symlink_metadata(path)
            .await
            .map_err(|source| path_error(path, source))?;
        if meta.is_symlink() {
            return Ok(());
        }

        self.events.log(
            &self.endpoint,
            LogDirection::Sent,
            format!("setstat {path} {mode:o}"),
        );
        self.chmod(path, mode).await?;
        if !recursive || !meta.is_dir() {
            return Ok(());
        }

        let mut pending = vec![path.to_string()];
        while let Some(directory) = pending.pop() {
            let Ok(entries) = self.browse.read_dir(directory.clone()).await else {
                continue;
            };
            for entry in entries {
                let name = entry.file_name();
                if name == "." || name == ".." {
                    continue;
                }
                let child = join_remote(&directory, &name);
                match kind_of(entry.file_type()) {
                    EntryKind::Symlink => continue,
                    EntryKind::Directory => {
                        let _ = self.chmod(&child, mode).await;
                        pending.push(child);
                    }
                    _ => {
                        let _ = self.chmod(&child, mode).await;
                    }
                }
            }
        }
        Ok(())
    }

    async fn chmod(&self, path: &str, mode: u32) -> Result<()> {
        let attributes = russh_sftp::protocol::FileAttributes {
            permissions: Some(mode & 0o777),
            ..Default::default()
        };
        self.browse
            .set_metadata(path, attributes)
            .await
            .map_err(|source| path_error(path, source))
    }

    /// Opens a file for reading on a transfer channel, positioned at `offset`.
    ///
    /// The channel is held for as long as the reader lives, which is the whole
    /// transfer. That is what the concurrency limit counts.
    pub async fn open_read(&self, path: &str, offset: u64) -> Result<(Reader, Lease<'_>)> {
        use tokio::io::AsyncSeekExt;

        let lease = self.lease().await?;
        let mut file = lease
            .sftp()
            .open(path)
            .await
            .map_err(|source| path_error(path, source))?;
        if offset > 0 {
            file.seek(std::io::SeekFrom::Start(offset))
                .await
                .map_err(Error::other)?;
        }
        Ok((Reader::Sftp(Box::new(file)), lease))
    }

    /// Opens a file for writing on a transfer channel, positioned at `offset`.
    ///
    /// An upload that continues must not truncate — that is the point of
    /// continuing. Starting from zero truncates, because then the file is being
    /// replaced.
    pub async fn open_write(&self, path: &str, offset: u64) -> Result<(Writer, Lease<'_>)> {
        use russh_sftp::protocol::OpenFlags;
        use tokio::io::AsyncSeekExt;

        let lease = self.lease().await?;
        let flags = if offset > 0 {
            OpenFlags::CREATE | OpenFlags::WRITE
        } else {
            OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE
        };
        let mut file = lease
            .sftp()
            .open_with_flags(path, flags)
            .await
            .map_err(|source| path_error(path, source))?;
        if offset > 0 {
            file.seek(std::io::SeekFrom::Start(offset))
                .await
                .map_err(Error::other)?;
        }
        Ok((Writer::Sftp(Box::new(file)), lease))
    }

    /// Renames over whatever is there.
    ///
    /// SFTP version 3 refuses to rename onto an existing name, so the old file
    /// is removed first. Between the two calls the target is briefly absent —
    /// unavoidable without the newer posix-rename extension, and still better
    /// than writing a half file under the final name from the start.
    pub async fn replace(&self, from: &str, to: &str) -> Result<()> {
        if self.browse.try_exists(to).await.unwrap_or(false) {
            let _ = self.browse.remove_file(to).await;
        }
        self.events.log(
            &self.endpoint,
            LogDirection::Sent,
            format!("rename {from} -> {to}"),
        );
        self.browse
            .rename(from, to)
            .await
            .map_err(|source| path_error(from, source))
    }

    /// Size and modification time, for deciding whether a resume is safe.
    pub async fn stat(&self, path: &str) -> Result<(u64, Option<i64>)> {
        let meta = self
            .browse
            .metadata(path)
            .await
            .map_err(|source| path_error(path, source))?;
        Ok((meta.size.unwrap_or(0), meta.mtime.map(i64::from)))
    }

    /// Carries a modification time across a transfer.
    pub async fn set_modified(&self, path: &str, seconds: i64) -> Result<()> {
        let existing = self
            .browse
            .metadata(path)
            .await
            .map_err(|source| path_error(path, source))?;
        let attributes = russh_sftp::protocol::FileAttributes {
            // Access time comes along because the protocol sets both or
            // neither; keeping the one already there is the closest to leaving
            // it alone.
            atime: existing.atime,
            mtime: u32::try_from(seconds).ok(),
            ..Default::default()
        };
        self.browse
            .set_metadata(path, attributes)
            .await
            .map_err(|source| path_error(path, source))
    }

    /// Closes the connection. Failing to say goodbye politely is not an error
    /// worth reporting — the socket goes either way.
    pub async fn disconnect(&self) {
        let _ = self
            .handle
            .lock()
            .await
            .disconnect(Disconnect::ByApplication, "", "en")
            .await;
        self.events
            .connection(&self.endpoint, ConnectionState::Disconnected);
    }
}

fn kind_of(file_type: FileType) -> EntryKind {
    match file_type {
        FileType::Dir => EntryKind::Directory,
        FileType::File => EntryKind::File,
        FileType::Symlink => EntryKind::Symlink,
        _ => EntryKind::Other,
    }
}

/// Turns an SFTP status into something the window can phrase.
fn path_error(path: &str, source: russh_sftp::client::error::Error) -> Error {
    use russh_sftp::protocol::StatusCode;

    let reason = match &source {
        russh_sftp::client::error::Error::Status(status) => match status.status_code {
            StatusCode::NoSuchFile => PathProblem::NotFound,
            StatusCode::PermissionDenied => PathProblem::PermissionDenied,
            _ => PathProblem::Unknown,
        },
        _ => PathProblem::Unknown,
    };

    Error::Path {
        path: path.to_string(),
        reason,
    }
}
