//! The commands every AmberBeam shell offers, and nothing about how they are
//! reached.
//!
//! Section 09 of the concept paper draws the second seam between the window
//! and the core. This crate is where that seam has its knot: the desktop shell
//! hands a command name and its arguments in through Tauri, the container
//! shell hands the same name and arguments in over HTTP, and neither knows
//! anything the other does not.
//!
//! It exists because the alternative was two lists of forty-odd commands kept
//! in step by hand. They would have drifted — not loudly, but one argument at
//! a time, and always in the shell nobody was looking at.
//!
//! ## What is deliberately not here
//!
//! Every command that takes a path from the caller and reads or writes it:
//! exporting the site list, looking into an export, importing somebody else's
//! file, and the two that read and write a text file for the key schemes.
//!
//! In the desktop program those paths come from a file dialog the person
//! opened themselves, which is what makes them safe. Reached over a network
//! the same commands would read and write any file the service can touch, and
//! a shared list is exactly the wrong place to put something whose safety
//! depends on which shell is calling. They stay in the shell that can prove a
//! human chose the path.

use std::sync::Arc;

use amberbeam_core::config::{AuthKind, Config, QuickConnectEntry, Settings};
use amberbeam_core::endpoint::EndpointId;
use amberbeam_core::error::Error;
use amberbeam_core::ftp::tls::{CertificateDecision, Exceptions};
use amberbeam_core::ftp::{Encryption, FtpParams};
use amberbeam_core::ops::{is_usable_name, Measurement};
use amberbeam_core::registry::{Connected, Sessions};
use amberbeam_core::runner::{EnqueueRequest, Runner};
use amberbeam_core::secrets::{MemoryStore, Secret, SecretStore};
use amberbeam_core::sftp::{AuthMethod, ConnectParams, HostKeyDecision};
use amberbeam_core::sites::Site;
use amberbeam_core::transfer::ConflictPolicy;
use amberbeam_core::CoreInfo;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Everything a command needs, and the only thing a shell has to build.
pub struct Service {
    pub sessions: Arc<Sessions>,
    pub config: Config,
    pub queue: Arc<Runner>,
    /// Where passwords live. Behind the trait, so the container build can put
    /// an encrypted file here instead without anything above noticing.
    pub secrets: Box<dyn SecretStore>,
    /// Passwords for this run only, for somebody who typed one but did not ask
    /// for it to be kept. It has to be here rather than in the window that
    /// took it: the site list is a window of its own, and what it holds cannot
    /// be reached from the one that connects. Never written anywhere; it goes
    /// when the program does.
    pub session: MemoryStore,
    /// The version the shell was built as, for judging a release against.
    ///
    /// Passed in rather than read here: `CARGO_PKG_VERSION` in this crate is
    /// this crate's version, and what matters is the version of the program
    /// somebody is running.
    pub version: String,
}

/// One row of the site list, as a window needs it.
///
/// The password itself is never here. Whether there is one is, because the
/// window has to be able to say so — a field that looks empty when a password
/// is stored invites somebody to type it again for nothing.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteRow {
    pub folder: String,
    #[serde(flatten)]
    pub site: Site,
    pub has_password: bool,
    /// One that was typed but not kept. Shown apart from the stored one,
    /// because "until you quit" and "until you delete it" are not the same
    /// promise and the window must not blur them.
    pub has_session_password: bool,
}

/// Which secret of an entry a command means.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SecretKind {
    Password,
    Passphrase,
}

impl From<SecretKind> for Secret {
    fn from(kind: SecretKind) -> Self {
        match kind {
            SecretKind::Password => Secret::Password,
            SecretKind::Passphrase => Secret::Passphrase,
        }
    }
}

/// What a window sends to open a connection.
///
/// The password travels from the window to the core and no further: it is not
/// written to the history, not to a site file, and not to the log.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectRequest {
    pub endpoint: String,
    /// Set when this connection comes from a site entry. It is how the stored
    /// password is found — which is why the window never has to hold one.
    pub site_id: Option<String>,
    /// Which protocol to speak. Absent means SFTP, which is what every request
    /// meant before there was a choice.
    #[serde(default = "sftp")]
    pub protocol: amberbeam_core::Protocol,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub auth: AuthKind,
    pub password: Option<String>,
    pub key_path: Option<String>,
    pub passphrase: Option<String>,
    /// The fingerprint the user was shown and accepted, on a second attempt.
    pub accept_fingerprint: Option<String>,
    /// Transfers at once for this connection. Falls back to the settings, and
    /// then to what the protocol advises.
    pub concurrency: Option<u8>,
    /// Attempts before a broken job is paused. Falls back to the settings.
    pub retries: Option<u8>,
    /// Write through a temporary name on this server. Falls back to the
    /// settings.
    pub temporary_name: Option<bool>,
    /// FTP only: how the connection is encrypted.
    #[serde(default)]
    pub encryption: Encryption,
    /// FTP only. Passive is what works behind a router, so it is the default.
    pub passive: Option<bool>,
    /// FTP only: the server does not speak UTF-8.
    pub latin1: Option<bool>,
    /// FTP only: seconds between keep-alive commands on an idle connection.
    pub keep_alive: Option<u32>,
    /// The certificate fingerprint the user was shown and accepted, on a
    /// second attempt. Belongs to that one certificate, never to the host.
    pub accept_certificate: Option<String>,
}

/// The protocol a request means when it does not say.
fn sftp() -> amberbeam_core::Protocol {
    amberbeam_core::Protocol::Sftp
}

impl ConnectRequest {
    fn into_params(self, settings: &Settings) -> ConnectParams {
        let method = match self.auth {
            AuthKind::Password => AuthMethod::Password {
                password: self.password.unwrap_or_default(),
            },
            AuthKind::KeyFile => AuthMethod::KeyFile {
                path: self.key_path.unwrap_or_default(),
                passphrase: self.passphrase,
            },
            AuthKind::Agent => AuthMethod::Agent,
        };
        ConnectParams {
            host: self.host,
            port: self.port,
            user: self.user,
            method,
            host_key: match self.accept_fingerprint {
                Some(fingerprint) => HostKeyDecision::Trust { fingerprint },
                None => HostKeyDecision::KnownOnly,
            },
            known_hosts: None,
            concurrency: self
                .concurrency
                .or(settings.concurrency)
                .unwrap_or_else(|| amberbeam_core::Protocol::Sftp.default_concurrency()),
            retries: self.retries.unwrap_or(settings.retries),
            temporary_name: self.temporary_name.unwrap_or(settings.temporary_name),
        }
    }

    fn into_ftp_params(self, settings: &Settings, stored: &Exceptions) -> FtpParams {
        let protocol = self.protocol;
        let (host, port) = (self.host.clone(), self.port);
        FtpParams {
            host: self.host,
            port: self.port,
            user: self.user,
            password: self.password.unwrap_or_default(),
            encryption: match protocol {
                // Plain FTP is plain whatever else the request says; anything
                // else would encrypt a connection the user asked to be open,
                // or leave open one they asked to be encrypted.
                amberbeam_core::Protocol::Ftp => Encryption::None,
                _ => self.encryption,
            },
            passive: self.passive.unwrap_or(true),
            concurrency: self
                .concurrency
                .or(settings.concurrency)
                .unwrap_or_else(|| protocol.default_concurrency()),
            retries: self.retries.unwrap_or(settings.retries),
            temporary_name: self.temporary_name.unwrap_or(settings.temporary_name),
            keep_alive: self.keep_alive,
            latin1: self.latin1.unwrap_or(false),
            certificate: match self.accept_certificate {
                Some(fingerprint) => CertificateDecision::Trust { fingerprint },
                // Nothing accepted in this attempt, so whatever was accepted in
                // an earlier one still counts — for that one certificate on
                // that one host and port, and nothing else.
                None => stored.decision_for(&host, port),
            },
        }
    }
}

/// What a window sends when something is dragged or the button is used.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnqueueBody {
    pub source_endpoint: String,
    pub source_directory: String,
    pub names: Vec<String>,
    pub target_endpoint: String,
    pub target_directory: String,
}

// --- The shapes arguments arrive in ----------------------------------------
//
// One per shape rather than one per command: most commands ask for an endpoint
// and a path, and forty near-identical structs would be forty places for a
// field name to be typed differently.

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct At {
    endpoint: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AtPath {
    endpoint: String,
    path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InDirectory {
    endpoint: String,
    directory: String,
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Renaming {
    endpoint: String,
    directory: String,
    from: String,
    to: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Permissions {
    endpoint: String,
    path: String,
    mode: u32,
    recursive: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Joining {
    endpoint: String,
    directory: String,
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Searching {
    endpoint: String,
    root: String,
    needle: String,
    limit: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Raw {
    endpoint: String,
    command: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ById {
    id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Secreting {
    id: String,
    kind: SecretKind,
    #[serde(default)]
    value: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Saving {
    folder: String,
    site: Site,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Folder {
    folder: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Renaming2 {
    from: String,
    to: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Remembering {
    id: String,
    path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Paused {
    paused: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Moving {
    id: String,
    by: Option<i32>,
    to: Option<usize>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Deciding {
    id: String,
    policy: ConflictPolicy,
    for_all: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Answering {
    answer: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Valued<T> {
    value: T,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Requesting<T> {
    request: T,
}

/// Every command this crate answers to, in one place.
///
/// Exported so a shell can be checked against it rather than trusted to keep
/// up. A name that is here and nowhere else is a command nothing can reach; a
/// name asked for that is not here answers with a refusal, not a panic.
pub const COMMANDS: &[&str] = &[
    "core_info",
    "local_session",
    "connect",
    "disconnect",
    "list_dir",
    "parent_of",
    "join_path",
    "create_dir",
    "create_file",
    "rename_entry",
    "measure",
    "remove_entry",
    "set_permissions",
    "search",
    "raw_command",
    "quick_connect_history",
    "forget_quick_connect",
    "save_as_site",
    "remember_path",
    "sites",
    "site_folders",
    "save_site",
    "delete_site",
    "create_site_folder",
    "rename_site_folder",
    "delete_site_folder",
    "set_site_secret",
    "forget_site_secret",
    "set_session_secret",
    "forget_session_secret",
    "settings",
    "set_settings",
    "ui_state",
    "set_ui_state",
    "newer_release",
    "update_source",
    "enqueue",
    "queue_snapshot",
    "queue_totals",
    "queue_pause",
    "queue_hold",
    "queue_resume",
    "queue_remove",
    "queue_clear_all",
    "queue_clear_finished",
    "queue_move",
    "queue_decide",
];

fn out<T: Serialize>(value: T) -> Result<Value, Error> {
    serde_json::to_value(value).map_err(Error::other)
}

fn taking<T: DeserializeOwned>(command: &str, args: Value) -> Result<T, Error> {
    serde_json::from_value(args)
        .map_err(|why| Error::other(format!("{command} was called wrongly: {why}")))
}

/// Joins a directory and a name, refusing anything that is not a plain name.
///
/// The name comes from a text box. A name carrying a separator or `..` would
/// land outside the directory the user is looking at — the same mistake as
/// trusting a path a server sent, which section 12 names outright.
async fn child_path(
    service: &Service,
    endpoint: &EndpointId,
    directory: &str,
    name: &str,
) -> Result<String, Error> {
    if !is_usable_name(name) {
        return Err(Error::Path {
            path: name.to_string(),
            reason: amberbeam_core::error::PathProblem::Unknown,
        });
    }
    service.sessions.join(endpoint, directory, name).await
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs() as i64)
        .unwrap_or_default()
}

/// Runs one command.
///
/// Arguments arrive as they were sent, in the window's own spelling, and are
/// read into a shape here. A name nothing knows is an error and not a panic:
/// this is reachable from a network in the container build, and a service that
/// can be stopped by a made-up word is a service that will be.
pub async fn dispatch(service: &Arc<Service>, command: &str, args: Value) -> Result<Value, Error> {
    match command {
        "core_info" => out(CoreInfo::gather()),
        "update_source" => out(amberbeam_core::update::LATEST_RELEASE),

        "newer_release" => {
            let it: Answering = taking(command, args)?;
            let release = amberbeam_core::update::read_answer(&it.answer);
            out(release.filter(|release| {
                amberbeam_core::update::is_newer(&service.version, &release.version)
            }))
        }

        // --- Connections ---
        "local_session" => out(service.sessions.local().await?),
        "connect" => {
            let it: Requesting<ConnectRequest> = taking(command, args)?;
            out(connect(service, it.request).await?)
        }
        "disconnect" => {
            let it: At = taking(command, args)?;
            service
                .sessions
                .disconnect(&EndpointId::new(it.endpoint))
                .await;
            out(())
        }

        // --- Looking around ---
        "list_dir" => {
            let it: AtPath = taking(command, args)?;
            out(service
                .sessions
                .list_dir(&EndpointId::new(it.endpoint), &it.path)
                .await?)
        }
        "parent_of" => {
            let it: AtPath = taking(command, args)?;
            out(service
                .sessions
                .parent(&EndpointId::new(it.endpoint), &it.path)
                .await?)
        }
        "join_path" => {
            let it: Joining = taking(command, args)?;
            out(service
                .sessions
                .join(&EndpointId::new(it.endpoint), &it.directory, &it.name)
                .await?)
        }
        "measure" => {
            let it: AtPath = taking(command, args)?;
            let found: Measurement = service
                .sessions
                .measure(&EndpointId::new(it.endpoint), &it.path)
                .await?;
            out(found)
        }
        "search" => {
            let it: Searching = taking(command, args)?;
            out(service
                .sessions
                .search(
                    &EndpointId::new(it.endpoint),
                    &it.root,
                    &it.needle,
                    it.limit.clamp(1, 2_000),
                )
                .await?)
        }
        "raw_command" => {
            let it: Raw = taking(command, args)?;
            out(service
                .sessions
                .raw(&EndpointId::new(it.endpoint), &it.command)
                .await?)
        }

        // --- Changing things ---
        "create_dir" => {
            let it: InDirectory = taking(command, args)?;
            let endpoint = EndpointId::new(it.endpoint);
            let path = child_path(service, &endpoint, &it.directory, &it.name).await?;
            out(service.sessions.create_dir(&endpoint, &path).await?)
        }
        "create_file" => {
            let it: InDirectory = taking(command, args)?;
            let endpoint = EndpointId::new(it.endpoint);
            let path = child_path(service, &endpoint, &it.directory, &it.name).await?;
            out(service.sessions.create_file(&endpoint, &path).await?)
        }
        "rename_entry" => {
            let it: Renaming = taking(command, args)?;
            let endpoint = EndpointId::new(it.endpoint);
            let source = child_path(service, &endpoint, &it.directory, &it.from).await?;
            let target = child_path(service, &endpoint, &it.directory, &it.to).await?;
            out(service.sessions.rename(&endpoint, &source, &target).await?)
        }
        "remove_entry" => {
            let it: AtPath = taking(command, args)?;
            out(service
                .sessions
                .remove(&EndpointId::new(it.endpoint), &it.path)
                .await?)
        }
        "set_permissions" => {
            let it: Permissions = taking(command, args)?;
            out(service
                .sessions
                .set_permissions(
                    &EndpointId::new(it.endpoint),
                    &it.path,
                    it.mode,
                    it.recursive,
                )
                .await?)
        }

        // --- The list of servers ---
        "sites" => out(sites(service)),
        "site_folders" => out(service.config.sites().folders()),
        "save_site" => {
            let it: Saving = taking(command, args)?;
            out(save_site(service, it.folder, it.site)?)
        }
        "delete_site" => {
            let it: ById = taking(command, args)?;
            service.config.sites().delete(&it.id)?;
            // The password goes with the entry that explained what it was for.
            let _ = service.secrets.forget_all(&it.id);
            let _ = service.session.forget_all(&it.id);
            out(())
        }
        "create_site_folder" => {
            let it: Folder = taking(command, args)?;
            out(service.config.sites().create_folder(&it.folder)?)
        }
        "rename_site_folder" => {
            let it: Renaming2 = taking(command, args)?;
            out(service.config.sites().rename_folder(&it.from, &it.to)?)
        }
        "delete_site_folder" => {
            let it: Folder = taking(command, args)?;
            out(service.config.sites().delete_folder(&it.folder)?)
        }

        // --- Secrets, which only ever travel inwards ---
        "set_site_secret" => {
            let it: Secreting = taking(command, args)?;
            // Keeping it for good settles the question, so the copy that was
            // only for this run goes. Two answers to one question is how a
            // changed password ends up being the old one at the next
            // connection.
            let _ = service.session.forget(&it.id, it.kind.into());
            out(service.secrets.set(&it.id, it.kind.into(), &it.value)?)
        }
        "forget_site_secret" => {
            let it: Secreting = taking(command, args)?;
            out(service.secrets.forget(&it.id, it.kind.into())?)
        }
        "set_session_secret" => {
            let it: Secreting = taking(command, args)?;
            out(service.session.set(&it.id, it.kind.into(), &it.value)?)
        }
        "forget_session_secret" => {
            let it: Secreting = taking(command, args)?;
            out(service.session.forget(&it.id, it.kind.into())?)
        }

        // --- What was connected to before ---
        "quick_connect_history" => out(service.config.quick_connect()),
        "forget_quick_connect" => {
            let it: ById = taking(command, args)?;
            out(service.config.forget_quick_connect(&it.id)?)
        }
        "save_as_site" => {
            let it: ById = taking(command, args)?;
            out(service
                .config
                .save_as_site(&it.id)
                .map(|path| path.to_string_lossy().into_owned())?)
        }
        "remember_path" => {
            let it: Remembering = taking(command, args)?;
            out(remember_path(service, it.id, it.path)?)
        }

        // --- Settings and whatever the window keeps ---
        "settings" => out(service.config.settings()),
        "set_settings" => {
            let it: Valued<Settings> = taking(command, args)?;
            out(service.config.set_settings(&it.value)?)
        }
        "ui_state" => out(service.config.ui_state()),
        "set_ui_state" => {
            let it: Valued<Value> = taking(command, args)?;
            out(service.config.set_ui_state(&it.value)?)
        }

        // --- The queue ---
        "enqueue" => {
            let it: Requesting<EnqueueBody> = taking(command, args)?;
            out(enqueue(service, it.request).await?)
        }
        "queue_snapshot" => out(service.queue.snapshot().await),
        "queue_totals" => out(service.queue.totals().await),
        "queue_pause" => {
            let it: Paused = taking(command, args)?;
            service.queue.set_paused(it.paused).await;
            out(())
        }
        "queue_hold" => {
            let it: ById = taking(command, args)?;
            service.queue.hold(&it.id).await;
            out(())
        }
        "queue_resume" => {
            let it: ById = taking(command, args)?;
            service.queue.resume(&it.id).await;
            out(())
        }
        "queue_remove" => {
            let it: ById = taking(command, args)?;
            service.queue.remove(&it.id).await;
            out(())
        }
        "queue_clear_all" => {
            service.queue.clear_all().await;
            out(())
        }
        "queue_clear_finished" => {
            service.queue.clear_finished().await;
            out(())
        }
        "queue_move" => {
            let it: Moving = taking(command, args)?;
            if let Some(index) = it.to {
                service.queue.move_to(&it.id, index).await;
            } else if let Some(delta) = it.by {
                service.queue.move_by(&it.id, delta as isize).await;
            }
            out(())
        }
        "queue_decide" => {
            let it: Deciding = taking(command, args)?;
            service.queue.decide(&it.id, it.policy, it.for_all).await;
            out(())
        }

        other => Err(Error::other(format!("no command called {other}"))),
    }
}

/// The list, without opening the credential store once.
///
/// It used to ask the store for every entry's password just to answer whether
/// there was one — and on macOS each of those is a separate request to the
/// keychain, so a list of ten servers meant ten password prompts, every time
/// the window was opened. "Always allow" does not help: an ad-hoc signed
/// build has a different signature after every rebuild, and the permission
/// was granted to the old one.
///
/// So the entry's own "remember this" is what the list goes by. It is the
/// intention rather than the fact, and the difference shows only if something
/// removed the password from the store behind our back — in which case
/// connecting asks for it, which is what it would have done anyway.
fn sites(service: &Service) -> Vec<SiteRow> {
    service
        .config
        .sites()
        .load()
        .into_iter()
        .map(|filed| SiteRow {
            has_password: filed.site.remember_password,
            has_session_password: service
                .session
                .get(&filed.site.id, Secret::Password)
                .unwrap_or_default()
                .is_some(),
            folder: filed.folder,
            site: filed.site,
        })
        .collect()
}

fn save_site(service: &Service, folder: String, mut site: Site) -> Result<String, Error> {
    // A new entry gets its identifier here rather than in the window. It is
    // what the credential store files the password under, and one place has to
    // be in charge of it.
    if site.id.is_empty() {
        site.id = Site::new_id();
    }

    // An entry that no longer wants its password remembered loses it here
    // rather than at some later tidy-up, so the window and the keychain never
    // disagree about what is stored.
    if !site.remember_password {
        let _ = service.secrets.forget(&site.id, Secret::Password);
        let _ = service.secrets.forget(&site.id, Secret::Passphrase);
    }

    service.config.sites().save(&folder, &site)?;
    Ok(site.id)
}

fn remember_path(service: &Service, id: String, path: String) -> Result<(), Error> {
    let Some(entry) = service
        .config
        .quick_connect()
        .into_iter()
        .find(|entry| entry.id == id)
    else {
        return Ok(());
    };
    service.config.remember_quick_connect(QuickConnectEntry {
        last_path: Some(path),
        last_used: now(),
        ..entry
    })
}

async fn enqueue(service: &Service, request: EnqueueBody) -> Result<usize, Error> {
    let settings = service.config.settings();
    service
        .queue
        .enqueue(&EnqueueRequest {
            source_endpoint: EndpointId::new(request.source_endpoint),
            source_directory: request.source_directory,
            names: request.names,
            target_endpoint: EndpointId::new(request.target_endpoint),
            target_directory: request.target_directory,
            // Asking is the default: overwriting somebody's file without a word
            // is the kind of help nobody wants.
            conflict_policy: ConflictPolicy::Ask,
            keep_modified: settings.keep_modified,
            keep_permissions: settings.keep_permissions,
            use_temporary_name: settings.temporary_name,
            retries: Some(settings.retries),
        })
        .await
}

async fn connect(service: &Service, mut request: ConnectRequest) -> Result<Connected, Error> {
    // A site entry's secrets are fetched here rather than in the window. A
    // password that never reaches the webview cannot be read out of it, and the
    // window has no use for the value anyway — only for the connection.
    if let Some(site_id) = request.site_id.clone() {
        // The kept one first, then the one for this run. They cannot both be
        // there — keeping one clears the other — but the order says which
        // would win if that ever stopped being true.
        if request.password.is_none() {
            request.password = service.secrets.get(&site_id, Secret::Password)?;
        }
        if request.password.is_none() {
            request.password = service.session.get(&site_id, Secret::Password)?;
        }
        if request.passphrase.is_none() {
            request.passphrase = service.secrets.get(&site_id, Secret::Passphrase)?;
        }
        if request.passphrase.is_none() {
            request.passphrase = service.session.get(&site_id, Secret::Passphrase)?;
        }
    }

    let endpoint = EndpointId::new(request.endpoint.clone());
    let settings = service.config.settings();
    let remote_ftp = matches!(
        request.protocol,
        amberbeam_core::Protocol::Ftp | amberbeam_core::Protocol::Ftps
    );
    let history = QuickConnectEntry {
        id: QuickConnectEntry::id_for(&request.user, &request.host, request.port),
        protocol: request.protocol,
        host: request.host.clone(),
        port: request.port,
        user: request.user.clone(),
        auth: request.auth,
        key_path: request.key_path.clone(),
        last_path: None,
        last_used: now(),
        saved_as_site: false,
        concurrency: request.concurrency,
        retries: request.retries,
        temporary_name: request.temporary_name,
        // Kept only where they mean something. An SFTP entry carrying an FTP
        // encryption mode would be answering a question nobody asked.
        encryption: match request.protocol {
            amberbeam_core::Protocol::Ftps => Some(request.encryption),
            _ => None,
        },
        passive: remote_ftp.then_some(request.passive.unwrap_or(true)),
        latin1: remote_ftp.then_some(request.latin1.unwrap_or(false)),
        keep_alive: request.keep_alive,
    };

    let accepted_now = request.accept_certificate.clone();
    let (host, port) = (request.host.clone(), request.port);

    let connected = match request.protocol {
        amberbeam_core::Protocol::Ftp | amberbeam_core::Protocol::Ftps => {
            let exceptions = service.config.certificate_exceptions();
            let session = service
                .sessions
                .connect_ftp(&endpoint, &request.into_ftp_params(&settings, &exceptions))
                .await?;

            // Written down only after it worked. A fingerprint accepted for a
            // server that then refused the login is not an exception anybody
            // wants kept.
            if let Some(fingerprint) = accepted_now {
                service
                    .config
                    .accept_certificate(&host, port, &fingerprint)?;
            }
            session
        }
        // Local needs no connecting, and asking for it here is a mistake worth
        // failing on rather than quietly turning into something else.
        _ => {
            service
                .sessions
                .connect_sftp(&endpoint, &request.into_params(&settings))
                .await?
        }
    };

    // Only a connection that worked is worth remembering. A typo in the host
    // name should not end up in the list the user picks from.
    let _ = service.config.remember_quick_connect(QuickConnectEntry {
        last_path: Some(connected.home.clone()),
        ..history
    });

    Ok(connected)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The list and the match have to agree in both directions, or a command
    /// is either unreachable or invisible to whatever checks the shells.
    #[tokio::test]
    async fn every_listed_command_is_answered() {
        let service = Arc::new(Service {
            sessions: Arc::new(Sessions::new(amberbeam_core::Events::new())),
            config: Config::at(std::env::temp_dir().join("amberbeam-commands-test")),
            queue: Runner::new(
                Arc::new(Sessions::new(amberbeam_core::Events::new())),
                amberbeam_core::Events::new(),
                std::env::temp_dir().join("amberbeam-commands-test/queue.json"),
            ),
            secrets: Box::new(MemoryStore::default()),
            session: MemoryStore::default(),
            version: "0.0.0".into(),
        });

        for name in COMMANDS {
            let answer = dispatch(&service, name, Value::Null).await;
            // Most will refuse for want of arguments, which is the point: what
            // must not happen is "no command called …".
            if let Err(Error::Other { detail }) = &answer {
                assert!(
                    !detail.starts_with("no command called"),
                    "{name} is listed but nothing answers to it"
                );
            }
        }
    }

    #[tokio::test]
    async fn a_name_nobody_knows_is_refused_rather_than_fatal() {
        let service = Arc::new(Service {
            sessions: Arc::new(Sessions::new(amberbeam_core::Events::new())),
            config: Config::at(std::env::temp_dir().join("amberbeam-commands-test2")),
            queue: Runner::new(
                Arc::new(Sessions::new(amberbeam_core::Events::new())),
                amberbeam_core::Events::new(),
                std::env::temp_dir().join("amberbeam-commands-test2/queue.json"),
            ),
            secrets: Box::new(MemoryStore::default()),
            session: MemoryStore::default(),
            version: "0.0.0".into(),
        });

        let answer = dispatch(&service, "drop_everything", Value::Null).await;
        assert!(answer.is_err());
    }
}
