//! Every open session, and the one door to them.
//!
//! This is what both shells talk to: the desktop through Tauri's channel, the
//! container build of M7 over HTTP and WebSocket. Neither knows anything about
//! sessions beyond what is offered here, which is the only reason the second
//! shell is a matter of a different transport rather than a second program.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::endpoint::{EndpointId, Protocol};
use crate::engine::{self, Progress, ResumeVerdict};
use crate::error::{Error, PathProblem, Result};
use crate::events::{Events, LogDirection};
use crate::fs::Listing;
use crate::ftp::tls::CertificateDecision;
use crate::ftp::{Encryption, FtpParams, FtpSession};
use crate::local::LocalSession;
use crate::ops::Measurement;
use crate::session::Session;
use crate::sftp::{ConnectParams, SftpSession};
use crate::transfer::ResumeMarker;

/// The identifier the local file system always has. The panes address it like
/// any other endpoint — in the container build it is the container's own disk,
/// not the user's.
pub const LOCAL: &str = "local";

/// One transfer, as the engine needs it.
#[derive(Debug, Clone)]
pub struct TransferRun {
    pub source_endpoint: EndpointId,
    pub source_path: String,
    pub target_endpoint: EndpointId,
    pub target_path: String,
    /// Where a previous attempt stopped, when there was one.
    pub resume: Option<ResumeMarker>,
    pub keep_modified: bool,
    pub keep_permissions: bool,
    /// Whether to write through a temporary name. The endpoint and the protocol
    /// can still say no.
    pub use_temporary_name: bool,
    /// The source's bits, when they are to be carried across.
    pub source_permissions: Option<u32>,
}

/// One file found while resolving what was selected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expanded {
    pub source_path: String,
    /// Path relative to what was selected, so the structure is kept at the
    /// other end. `None` size means a directory that exists only to be created.
    pub relative: String,
    pub size: Option<u64>,
}

/// How a transfer ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transferred {
    pub complete: bool,
    /// Bytes moved in this run. After a resume that is the remainder, not the
    /// whole file.
    pub moved: u64,
    /// Where to pick up, when it was stopped rather than finished.
    pub resume: Option<ResumeMarker>,
}

/// What a freshly opened session reports back.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Connected {
    pub endpoint: EndpointId,
    pub protocol: Protocol,
    /// Directory the pane should open, canonical.
    pub home: String,
    /// Whether this connection stands on a certificate somebody accepted by
    /// hand rather than one an authority vouches for. The window marks the
    /// pane for as long as that holds.
    pub certificate_accepted: bool,
}

/// All open sessions.
///
/// The map is held only long enough to find a session; nothing is locked while
/// one is used. A session governs its own concurrency through its channels, so
/// listing a directory does not wait behind eight running transfers — which was
/// the whole reason for opening more than one channel.
#[derive(Debug)]
pub struct Sessions {
    open: Mutex<HashMap<EndpointId, Arc<Session>>>,
    events: Events,
}

impl Sessions {
    /// Starts with the local file system already open — there is nothing to
    /// connect to.
    pub fn new(events: Events) -> Self {
        let mut open = HashMap::new();
        open.insert(
            EndpointId::new(LOCAL),
            Arc::new(Session::Local(LocalSession::new())),
        );
        Self {
            open: Mutex::new(open),
            events,
        }
    }

    pub fn events(&self) -> &Events {
        &self.events
    }

    /// Opens an SFTP connection under `endpoint`, replacing whatever was there.
    pub async fn connect_sftp(
        &self,
        endpoint: &EndpointId,
        params: &ConnectParams,
    ) -> Result<Connected> {
        // Reconnecting a pane that already holds a session closes the old one
        // first, or the count of open connections only ever grows.
        self.disconnect(endpoint).await;

        let session = SftpSession::connect(params, endpoint, &self.events).await?;
        let home = session.home().await?;

        self.open
            .lock()
            .await
            .insert(endpoint.clone(), Arc::new(Session::Sftp(Box::new(session))));

        Ok(Connected {
            endpoint: endpoint.clone(),
            protocol: Protocol::Sftp,
            home,
            certificate_accepted: false,
        })
    }

    /// Opens an FTP or FTPS connection.
    ///
    /// Separate from the SFTP door because the two need genuinely different
    /// things — a key and a channel limit against a certificate and a login
    /// count — and folding them into one set of parameters would mean half of
    /// it being meaningless in either case.
    pub async fn connect_ftp(
        &self,
        endpoint: &EndpointId,
        params: &FtpParams,
    ) -> Result<Connected> {
        self.disconnect(endpoint).await;

        let session = FtpSession::connect(params, endpoint, &self.events).await?;
        let home = session.home().await?;
        let protocol = match params.encryption {
            Encryption::None => Protocol::Ftp,
            Encryption::Explicit | Encryption::Implicit => Protocol::Ftps,
        };

        self.open
            .lock()
            .await
            .insert(endpoint.clone(), Arc::new(Session::Ftp(Box::new(session))));

        Ok(Connected {
            endpoint: endpoint.clone(),
            protocol,
            home,
            certificate_accepted: params.certificate != CertificateDecision::TrustedOnly,
        })
    }

    /// Reports the local session the way a connection would, so a pane can
    /// treat both the same.
    pub async fn local(&self) -> Result<Connected> {
        let endpoint = EndpointId::new(LOCAL);
        let session = self.find(&endpoint).await?;
        let home = session.home().await?;
        Ok(Connected {
            endpoint,
            protocol: Protocol::Local,
            home,
            certificate_accepted: false,
        })
    }

    /// Sends one hand-typed command to an endpoint.
    pub async fn raw(&self, endpoint: &EndpointId, command: &str) -> Result<crate::ftp::RawReply> {
        self.find(endpoint).await?.raw(command).await
    }

    pub async fn list_dir(&self, endpoint: &EndpointId, path: &str) -> Result<Listing> {
        let session = self.find(endpoint).await?;
        let listing = session.list_dir(path).await?;
        self.events.emit(crate::events::Event::Listed {
            endpoint: endpoint.clone(),
            path: listing.path.clone(),
        });
        Ok(listing)
    }

    pub async fn parent(&self, endpoint: &EndpointId, path: &str) -> Result<Option<String>> {
        let session = self.find(endpoint).await?;
        let parent = session.parent(path);
        Ok(parent)
    }

    pub async fn join(&self, endpoint: &EndpointId, directory: &str, name: &str) -> Result<String> {
        let session = self.find(endpoint).await?;
        let joined = session.join(directory, name);
        Ok(joined)
    }

    pub async fn create_dir(&self, endpoint: &EndpointId, path: &str) -> Result<()> {
        let session = self.find(endpoint).await?;
        session.create_dir(path).await
    }

    pub async fn create_file(&self, endpoint: &EndpointId, path: &str) -> Result<()> {
        let session = self.find(endpoint).await?;
        session.create_file(path).await
    }

    pub async fn rename(&self, endpoint: &EndpointId, from: &str, to: &str) -> Result<()> {
        let session = self.find(endpoint).await?;
        session.rename(from, to).await
    }

    pub async fn measure(&self, endpoint: &EndpointId, path: &str) -> Result<Measurement> {
        let session = self.find(endpoint).await?;
        session.measure(path).await
    }

    pub async fn remove(&self, endpoint: &EndpointId, path: &str) -> Result<()> {
        let session = self.find(endpoint).await?;
        session.remove(path).await
    }

    pub async fn set_permissions(
        &self,
        endpoint: &EndpointId,
        path: &str,
        mode: u32,
        recursive: bool,
    ) -> Result<()> {
        let session = self.find(endpoint).await?;
        session.set_permissions(path, mode, recursive).await
    }

    /// Moves one file from one endpoint to another.
    ///
    /// Neither side is privileged: a download, an upload and a copy between two
    /// servers are the same call with different endpoints. What differs is what
    /// the target protocol can promise about renaming, which decides whether
    /// the file is written under a temporary name first.
    pub async fn transfer(&self, job: &TransferRun, progress: &Progress) -> Result<Transferred> {
        let source = self.find(&job.source_endpoint).await?;
        let target = self.find(&job.target_endpoint).await?;

        let (size, modified) = source.stat(&job.source_path).await?;
        progress.set_total(size);

        // Nothing is appended to before the source has been shown to be what it
        // was. A file stitched together from two versions looks complete and is
        // not, which is the one failure this program must never produce.
        // Both ends have to be able to do it. A server that cannot continue a
        // download and one that cannot continue an upload rule it out just the
        // same, and the transfer starts again rather than failing.
        let resumable = source.can_resume() && target.can_resume();
        let verdict = if resumable {
            engine::may_resume(job.resume.as_ref(), size, modified)
        } else {
            ResumeVerdict::Fresh
        };
        let offset = match verdict {
            ResumeVerdict::Safe => job.resume.as_ref().map_or(0, |marker| marker.offset),
            ResumeVerdict::Changed => return Err(Error::SourceChanged),
            ResumeVerdict::Fresh => 0,
        };
        progress.set_done(offset);

        // Three things have to agree: the job, the endpoint's own setting, and
        // what the protocol can actually promise about renaming.
        let use_partial = job.use_temporary_name
            && target.temporary_name()
            && target.protocol().rename_is_dependable();
        let write_path = if use_partial {
            engine::partial_name(&job.target_path)
        } else {
            job.target_path.clone()
        };

        // Both sides are opened at the offset, so a transfer that stopped at
        // 10 of 20 MB reads and writes the remaining 10 and not one byte more.
        let moved = {
            let (mut reader, source_hold) = source.open_read(&job.source_path, offset).await?;
            let (mut writer, target_hold) = target.open_write(&write_path, offset).await?;
            let moved = engine::copy(&mut reader, &mut writer, progress).await?;

            // The writing side first: the target has to accept the file before
            // there is any point asking the source whether it sent it all.
            // Either may still refuse, and a refusal here means the transfer
            // failed however many bytes went across.
            target.finish_write(writer, target_hold).await?;
            source.finish_read(reader, source_hold).await?;
            moved
        };
        debug_assert!(
            offset + moved <= size.max(offset + moved),
            "a resumed transfer never re-reads what it already has"
        );

        if progress.is_cancelled() {
            return Ok(Transferred {
                complete: false,
                moved,
                // No marker where nothing can act on one. Handing back an
                // offset that the next attempt has to ignore is how a resumed
                // transfer ends up reading from the start and writing at the
                // end.
                resume: resumable.then(|| ResumeMarker {
                    offset: progress.done(),
                    source_size: size,
                    source_modified: modified,
                }),
            });
        }

        if use_partial {
            // The final name appears only now, when the file behind it is
            // whole. Anything watching the directory never sees a half file
            // wearing a finished name.
            target.replace(&write_path, &job.target_path).await?;
        }

        if job.keep_modified {
            if let Some(seconds) = modified {
                // A timestamp that cannot be set is not worth failing a
                // transfer that otherwise worked.
                let _ = target.set_modified(&job.target_path, seconds).await;
            }
        }
        if let (true, Some(mode)) = (job.keep_permissions, job.source_permissions) {
            let _ = target.set_permissions(&job.target_path, mode, false).await;
        }

        Ok(Transferred {
            complete: true,
            moved,
            resume: None,
        })
    }

    /// Closes a session. The local one stays: there is nothing to close, and a
    /// pane pointing at it must not end up pointing at nothing.
    pub async fn disconnect(&self, endpoint: &EndpointId) {
        if endpoint.as_str() == LOCAL {
            return;
        }
        let session = self.open.lock().await.remove(endpoint);
        if let Some(session) = session {
            session.disconnect().await;
            self.events
                .log(endpoint, LogDirection::Note, "disconnected");
        }
    }

    /// Creates a directory and everything above it that is missing.
    ///
    /// A queue that keeps the structure of what was dragged has to put the
    /// folders there first, and the file being transferred is no place to find
    /// out that its directory does not exist.
    pub async fn ensure_dir(&self, endpoint: &EndpointId, path: &str) -> Result<()> {
        let session = self.find(endpoint).await?;
        if session.list_dir(path).await.is_ok() {
            return Ok(());
        }
        if let Some(parent) = session.parent(path) {
            if parent != path {
                Box::pin(self.ensure_dir(endpoint, &parent)).await?;
            }
        }
        match session.create_dir(path).await {
            Ok(()) => Ok(()),
            // Another job in the same queue may have made it in the meantime,
            // which is a race worth losing quietly.
            Err(Error::Path {
                reason: PathProblem::AlreadyExists,
                ..
            }) => Ok(()),
            Err(error) if session.list_dir(path).await.is_ok() => {
                let _ = error;
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    /// Everything below one entry, as files with their paths on both sides.
    ///
    /// Directories are resolved here rather than while transferring: the window
    /// shows how many files are coming, and relative paths are worked out once
    /// instead of per job.
    pub async fn expand(
        &self,
        endpoint: &EndpointId,
        directory: &str,
        name: &str,
        target_directory: &str,
    ) -> Result<Vec<Expanded>> {
        let session = self.find(endpoint).await?;
        let source = session.join(directory, name);

        let listing = match session.list_dir(&source).await {
            // Not a directory: one file, and the name is kept as it is.
            Err(_) => {
                let (size, _) = session.stat(&source).await.unwrap_or((0, None));
                return Ok(vec![Expanded {
                    source_path: source,
                    relative: name.to_string(),
                    size: Some(size),
                }]);
            }
            Ok(listing) => listing,
        };

        let _ = target_directory;
        let mut out = Vec::new();
        let mut pending = vec![(source.clone(), name.to_string(), listing)];

        while let Some((directory, relative, listing)) = pending.pop() {
            // An empty folder is worth carrying across on its own; without this
            // it would simply vanish from the copy.
            if listing.entries.is_empty() {
                out.push(Expanded {
                    source_path: directory.clone(),
                    relative: relative.clone(),
                    size: None,
                });
            }
            for entry in listing.entries {
                let child = session.join(&directory, &entry.name);
                let child_relative = format!("{relative}/{}", entry.name);
                if entry.is_directory() {
                    if let Ok(inner) = session.list_dir(&child).await {
                        pending.push((child, child_relative, inner));
                    }
                } else {
                    out.push(Expanded {
                        source_path: child,
                        relative: child_relative,
                        size: entry.size,
                    });
                }
            }
        }
        Ok(out)
    }

    /// Every endpoint that currently has a session.
    pub async fn open_endpoints(&self) -> Vec<EndpointId> {
        self.open.lock().await.keys().cloned().collect()
    }

    pub async fn is_connected(&self, endpoint: &EndpointId) -> bool {
        self.open.lock().await.contains_key(endpoint)
    }

    /// Finds a session without holding the map while it is used.
    pub async fn find(&self, endpoint: &EndpointId) -> Result<Arc<Session>> {
        self.open
            .lock()
            .await
            .get(endpoint)
            .cloned()
            .ok_or(Error::NotConnected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_local_file_system_is_open_from_the_start() {
        let sessions = Sessions::new(Events::new());
        let local = sessions.local().await.expect("local session");
        assert_eq!(local.protocol, Protocol::Local);
        assert!(local.home.starts_with('/') || local.home.contains(':'));
        assert!(sessions.is_connected(&EndpointId::new(LOCAL)).await);
    }

    #[tokio::test]
    async fn a_pane_pointing_nowhere_is_told_so() {
        let sessions = Sessions::new(Events::new());
        let error = sessions
            .list_dir(&EndpointId::new("server-a"), "/")
            .await
            .expect_err("nothing is connected there");
        assert!(matches!(error, Error::NotConnected));
    }

    #[tokio::test]
    async fn the_local_session_cannot_be_closed_away() {
        let sessions = Sessions::new(Events::new());
        let local = EndpointId::new(LOCAL);
        sessions.disconnect(&local).await;
        assert!(sessions.is_connected(&local).await);
    }
}
