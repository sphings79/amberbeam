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

use crate::endpoint::EndpointId;
use crate::error::{Error, PathProblem, Result};
use crate::events::{ConnectionState, Events, LogDirection};
use crate::fs::{join_remote, DirEntry, EntryKind, Listing, Permissions};

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
pub struct SftpSession {
    handle: Handle<Verifier>,
    sftp: RawSftp,
    endpoint: EndpointId,
    events: Events,
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

        Ok(Self {
            handle,
            sftp,
            endpoint: endpoint.clone(),
            events: events.clone(),
        })
    }

    /// Where the server puts us when we do not say otherwise.
    pub async fn home(&self) -> Result<String> {
        self.events
            .log(&self.endpoint, LogDirection::Sent, "realpath .");
        self.sftp
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
            .sftp
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
                let target = self.sftp.read_link(&full).await.ok();
                let resolved = self
                    .sftp
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

    /// Closes the connection. Failing to say goodbye politely is not an error
    /// worth reporting — the socket goes either way.
    pub async fn disconnect(&mut self) {
        let _ = self
            .handle
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
