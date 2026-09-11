//! Desktop shell of AmberBeam.
//!
//! This crate is deliberately thin. It owns the window, turns Tauri commands
//! into calls on `amberbeam-core`, and forwards the core's events into the
//! webview. It holds no logic of its own: the same set of commands is what the
//! headless HTTP and WebSocket service of milestone M7 will expose, which only
//! works as long as nothing of substance settles here.

use std::path::PathBuf;
use std::sync::Arc;

use amberbeam_core::bundle;
use amberbeam_core::config::{AuthKind, Config, QuickConnectEntry, Settings};
use amberbeam_core::endpoint::EndpointId;
use amberbeam_core::error::Error;
use amberbeam_core::events::{Event, RecvError};
use amberbeam_core::fs::Listing;
use amberbeam_core::ftp::tls::{CertificateDecision, Exceptions};
use amberbeam_core::ftp::{Encryption, FtpParams};
use amberbeam_core::import;
use amberbeam_core::ops::{is_usable_name, Measurement};
use amberbeam_core::queue::{Queue, Totals};
use amberbeam_core::registry::{Connected, Sessions};
use amberbeam_core::runner::{EnqueueRequest, Runner};
use amberbeam_core::secrets::{Secret, SecretStore, SystemStore};
use amberbeam_core::sftp::{AuthMethod, ConnectParams, HostKeyDecision};
use amberbeam_core::sites::Site;
use amberbeam_core::transfer::ConflictPolicy;
use amberbeam_core::{CoreInfo, Events};
use serde::Deserialize;
use tauri::{Emitter, Manager};

/// The name events arrive under in the webview. One channel for everything, so
/// the frontend has a single place to listen — which is also how the WebSocket
/// of M7 will look.
const EVENT_CHANNEL: &str = "amberbeam://event";

/// The label of the site manager window, and the channel the main window hears
/// its "open this one" on.
const SITES_WINDOW: &str = "sites";
const OPEN_SITE_CHANNEL: &str = "amberbeam://open-site";

/// What one window asks the other to do.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct OpenSite {
    id: String,
    side: String,
}

struct State {
    sessions: Arc<Sessions>,
    config: Config,
    queue: Arc<Runner>,
    /// Where passwords live. Behind the trait, so the container build of M7 can
    /// put an encrypted file here instead without anything above noticing.
    secrets: Box<dyn SecretStore>,
}

/// One row of the site list, as the window needs it.
///
/// The password itself is never here. Whether there is one is, because the
/// window has to be able to say so — a field that looks empty when a password
/// is stored invites somebody to type it again for nothing.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SiteRow {
    folder: String,
    #[serde(flatten)]
    site: Site,
    has_password: bool,
}

/// Which secret of an entry a command means.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum SecretKind {
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

/// What the window sends to open a connection.
///
/// The password travels from the window to the core and no further: it is not
/// written to the history, not to a site file, and not to the log. The system
/// credential store arrives with the site manager in M4.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConnectRequest {
    endpoint: String,
    /// Set when this connection comes from a site entry. It is how the stored
    /// password is found — which is why the window never has to hold one.
    site_id: Option<String>,
    /// Which protocol to speak. Absent means SFTP, which is what every request
    /// meant before there was a choice.
    #[serde(default = "sftp")]
    protocol: amberbeam_core::Protocol,
    host: String,
    port: u16,
    user: String,
    auth: AuthKind,
    password: Option<String>,
    key_path: Option<String>,
    passphrase: Option<String>,
    /// The fingerprint the user was shown and accepted, on a second attempt.
    accept_fingerprint: Option<String>,
    /// Transfers at once for this connection. Falls back to the settings, and
    /// then to what the protocol advises.
    concurrency: Option<u8>,
    /// Attempts before a broken job is paused. Falls back to the settings.
    retries: Option<u8>,
    /// Write through a temporary name on this server. Falls back to the
    /// settings.
    temporary_name: Option<bool>,
    /// FTP only: how the connection is encrypted.
    #[serde(default)]
    encryption: Encryption,
    /// FTP only. Passive is what works behind a router, so it is the default.
    passive: Option<bool>,
    /// FTP only: the server does not speak UTF-8.
    latin1: Option<bool>,
    /// FTP only: seconds between keep-alive commands on an idle connection.
    keep_alive: Option<u32>,
    /// The certificate fingerprint the user was shown and accepted, on a
    /// second attempt. Belongs to that one certificate, never to the host.
    accept_certificate: Option<String>,
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

/// Opens the site manager in a window of its own.
///
/// Its own window because somebody with thirty servers wants the list beside
/// the panes, not instead of them. Same bundle, a different view — so there is
/// one interface to maintain, not two.
#[tauri::command]
fn open_site_manager(app: tauri::AppHandle) -> Result<(), Error> {
    use tauri::{WebviewUrl, WebviewWindowBuilder};

    if let Some(existing) = app.get_webview_window(SITES_WINDOW) {
        // Already open: bring it forward rather than stack a second one.
        let _ = existing.unminimize();
        let _ = existing.set_focus();
        return Ok(());
    }

    WebviewWindowBuilder::new(
        &app,
        SITES_WINDOW,
        WebviewUrl::App("index.html?view=sites".into()),
    )
    .title("AmberBeam")
    .inner_size(980.0, 660.0)
    .min_inner_size(700.0, 460.0)
    .center()
    .build()
    .map_err(Error::other)?;
    Ok(())
}

/// Asks the main window to open a site on one side.
///
/// The site manager does not connect by itself on purpose: the questions a
/// connection can raise — an unknown host key, a certificate nobody vouches
/// for, a password that was not stored — all have their dialogs in the main
/// window, and asking them twice in two places is how two answers end up
/// disagreeing.
#[tauri::command]
fn open_site(app: tauri::AppHandle, id: String, side: String) -> Result<(), Error> {
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.unminimize();
        let _ = main.set_focus();
    }
    app.emit(OPEN_SITE_CHANNEL, OpenSite { id, side })
        .map_err(Error::other)
}

// --- Taking the list with you ----------------------------------------------

/// Writes the whole list to one file.
///
/// Two shapes and no third: without passwords it is plain JSON anybody can
/// read, and with them it is sealed under a passphrase. The core refuses the
/// combination that would be neither.
#[tauri::command]
fn export_sites(
    state: tauri::State<'_, Arc<State>>,
    path: PathBuf,
    with_passwords: bool,
    passphrase: Option<String>,
) -> Result<usize, Error> {
    let filed = state.config.sites().load();
    let made = bundle::gather(&filed, state.secrets.as_ref(), with_passwords);
    let bytes = bundle::to_bytes(&made, passphrase.as_deref().filter(|p| !p.is_empty()))?;

    std::fs::write(&path, bytes).map_err(Error::from)?;
    Ok(made.entries.len())
}

/// What an export holds, before any of it is taken over.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct BundlePreview {
    sealed: bool,
    entries: Vec<BundleRow>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct BundleRow {
    folder: String,
    name: String,
    host: String,
    user: String,
    port: u16,
    has_password: bool,
}

/// Looks into an export. Answers what is in it, never a password.
#[tauri::command]
fn bundle_preview(path: PathBuf, passphrase: Option<String>) -> Result<BundlePreview, Error> {
    let bytes = std::fs::read(&path).map_err(Error::from)?;
    let is_sealed = amberbeam_core::sealed::is_sealed(&bytes);

    // Asked before a passphrase is, so somebody opening a plain export is not
    // prompted for one that does not exist.
    if is_sealed && passphrase.as_deref().unwrap_or_default().is_empty() {
        return Ok(BundlePreview {
            sealed: true,
            entries: Vec::new(),
        });
    }

    let read = bundle::from_bytes(&bytes, passphrase.as_deref().filter(|p| !p.is_empty()))?;
    Ok(BundlePreview {
        sealed: is_sealed,
        entries: read
            .entries
            .iter()
            .map(|entry| BundleRow {
                folder: entry.folder.clone(),
                name: entry.site.name.clone(),
                host: entry.site.host.clone(),
                user: entry.site.user.clone(),
                port: entry.site.port,
                has_password: entry.password.is_some(),
            })
            .collect(),
    })
}

/// Takes the ticked entries of an export over.
#[tauri::command]
fn bundle_apply(
    state: tauri::State<'_, Arc<State>>,
    path: PathBuf,
    passphrase: Option<String>,
    chosen: Vec<usize>,
    into: String,
) -> Result<usize, Error> {
    let bytes = std::fs::read(&path).map_err(Error::from)?;
    let read = bundle::from_bytes(&bytes, passphrase.as_deref().filter(|p| !p.is_empty()))?;
    bundle::apply(
        &read,
        &chosen,
        &state.config.sites(),
        state.secrets.as_ref(),
        &into,
    )
}

// --- Importing somebody else's list ----------------------------------------

/// What one file holds, without the passwords.
///
/// The preview exists to be ticked through, and ticking needs a name and a
/// host, not a secret. The passwords stay in the core: [`import_apply`] reads
/// the file again and moves them straight into the credential store, so no
/// password ever travels to a window even once.
#[tauri::command]
fn import_candidates() -> Vec<import::Candidate> {
    import::discover()
}

#[tauri::command]
fn import_preview(source: import::Source, path: PathBuf) -> Result<import::Found, Error> {
    import::read(source, &path)
}

/// Takes the ticked entries over.
///
/// The file is read a second time rather than the preview being trusted: that
/// is what keeps the passwords out of the window, and it costs a few
/// milliseconds on a file of thirty servers.
#[tauri::command]
fn import_apply(
    state: tauri::State<'_, Arc<State>>,
    source: import::Source,
    path: PathBuf,
    chosen: Vec<usize>,
    expected: usize,
    take_passwords: bool,
    into: String,
) -> Result<usize, Error> {
    let found = import::read(source, &path)?;
    if found.entries.len() != expected {
        // The file changed between being looked at and being taken over, and
        // the ticks no longer point at what somebody ticked.
        return Err(Error::other("the file changed; look at it again"));
    }

    let sites = state.config.sites();
    let mut taken = 0;

    for index in chosen {
        let Some(entry) = found.entries.get(index) else {
            continue;
        };

        let site = Site {
            id: Site::new_id(),
            name: entry.name.clone(),
            protocol: entry.protocol,
            host: entry.host.clone(),
            port: entry.port,
            user: entry.user.clone(),
            auth: entry.auth,
            key_path: entry.key_path.clone(),
            remote_path: entry.remote_path.clone(),
            local_path: None,
            concurrency: entry.protocol.default_concurrency(),
            retries: None,
            temporary_name: None,
            encryption: entry.encryption,
            passive: None,
            latin1: None,
            keep_alive: None,
            remember_password: take_passwords && entry.has_password,
            colour: None,
        };

        let folder = match (into.trim(), entry.folder.as_str()) {
            ("", inner) => inner.to_string(),
            (outer, "") => outer.to_string(),
            (outer, inner) => format!("{outer}/{inner}"),
        };
        sites.save(&folder, &site)?;

        // Straight from the file into the credential store, with no stop in
        // between. This is the only place an imported password exists outside
        // the file it came from.
        if take_passwords {
            if let Some(password) = entry.password.as_deref() {
                state.secrets.set(&site.id, Secret::Password, password)?;
            }
        }
        taken += 1;
    }

    Ok(taken)
}

// --- The site manager ------------------------------------------------------
//
// The password is the one thing these commands never hand back. The window can
// ask whether an entry has one, set one and forget one; reading it is for the
// moment of connecting, and that happens down here where it does not have to
// travel through a webview to be useful.

#[tauri::command]
fn sites(state: tauri::State<'_, Arc<State>>) -> Vec<SiteRow> {
    let secrets = &state.secrets;
    state
        .config
        .sites()
        .load()
        .into_iter()
        .map(|filed| SiteRow {
            has_password: secrets
                .get(&filed.site.id, Secret::Password)
                .unwrap_or_default()
                .is_some(),
            folder: filed.folder,
            site: filed.site,
        })
        .collect()
}

#[tauri::command]
fn site_folders(state: tauri::State<'_, Arc<State>>) -> Vec<String> {
    state.config.sites().folders()
}

#[tauri::command]
fn save_site(
    state: tauri::State<'_, Arc<State>>,
    folder: String,
    mut site: Site,
) -> Result<String, Error> {
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
        let _ = state.secrets.forget(&site.id, Secret::Password);
        let _ = state.secrets.forget(&site.id, Secret::Passphrase);
    }

    state.config.sites().save(&folder, &site)?;
    Ok(site.id)
}

#[tauri::command]
fn delete_site(state: tauri::State<'_, Arc<State>>, id: String) -> Result<(), Error> {
    state.config.sites().delete(&id)?;
    // The password goes with the entry that explained what it was for.
    let _ = state.secrets.forget_all(&id);
    Ok(())
}

#[tauri::command]
fn create_site_folder(state: tauri::State<'_, Arc<State>>, folder: String) -> Result<(), Error> {
    state.config.sites().create_folder(&folder)
}

#[tauri::command]
fn rename_site_folder(
    state: tauri::State<'_, Arc<State>>,
    from: String,
    to: String,
) -> Result<(), Error> {
    state.config.sites().rename_folder(&from, &to)
}

#[tauri::command]
fn delete_site_folder(state: tauri::State<'_, Arc<State>>, folder: String) -> Result<(), Error> {
    state.config.sites().delete_folder(&folder)
}

#[tauri::command]
fn set_site_secret(
    state: tauri::State<'_, Arc<State>>,
    id: String,
    kind: SecretKind,
    value: String,
) -> Result<(), Error> {
    state.secrets.set(&id, kind.into(), &value)
}

#[tauri::command]
fn forget_site_secret(
    state: tauri::State<'_, Arc<State>>,
    id: String,
    kind: SecretKind,
) -> Result<(), Error> {
    state.secrets.forget(&id, kind.into())
}

#[tauri::command]
fn core_info() -> CoreInfo {
    CoreInfo::gather()
}

#[tauri::command]
async fn local_session(state: tauri::State<'_, Arc<State>>) -> Result<Connected, Error> {
    state.sessions.local().await
}

#[tauri::command]
async fn connect(
    state: tauri::State<'_, Arc<State>>,
    mut request: ConnectRequest,
) -> Result<Connected, Error> {
    // A site entry's secrets are fetched here rather than in the window. A
    // password that never reaches the webview cannot be read out of it, and the
    // window has no use for the value anyway — only for the connection.
    if let Some(site_id) = request.site_id.clone() {
        if request.password.is_none() {
            request.password = state.secrets.get(&site_id, Secret::Password)?;
        }
        if request.passphrase.is_none() {
            request.passphrase = state.secrets.get(&site_id, Secret::Passphrase)?;
        }
    }

    let endpoint = EndpointId::new(request.endpoint.clone());
    let settings = state.config.settings();
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
            let exceptions = state.config.certificate_exceptions();
            let session = state
                .sessions
                .connect_ftp(&endpoint, &request.into_ftp_params(&settings, &exceptions))
                .await?;

            // Written down only after it worked. A fingerprint accepted for a
            // server that then refused the login is not an exception anybody
            // wants kept.
            if let Some(fingerprint) = accepted_now {
                state.config.accept_certificate(&host, port, &fingerprint)?;
            }
            session
        }
        // Local needs no connecting, and asking for it here is a mistake worth
        // failing on rather than quietly turning into something else.
        _ => {
            state
                .sessions
                .connect_sftp(&endpoint, &request.into_params(&settings))
                .await?
        }
    };

    // Only a connection that worked is worth remembering. A typo in the host
    // name should not end up in the list the user picks from.
    let _ = state.config.remember_quick_connect(QuickConnectEntry {
        last_path: Some(connected.home.clone()),
        ..history
    });

    Ok(connected)
}

#[tauri::command]
async fn disconnect(state: tauri::State<'_, Arc<State>>, endpoint: String) -> Result<(), Error> {
    state.sessions.disconnect(&EndpointId::new(endpoint)).await;
    Ok(())
}

#[tauri::command]
async fn list_dir(
    state: tauri::State<'_, Arc<State>>,
    endpoint: String,
    path: String,
) -> Result<Listing, Error> {
    state
        .sessions
        .list_dir(&EndpointId::new(endpoint), &path)
        .await
}

#[tauri::command]
async fn parent_of(
    state: tauri::State<'_, Arc<State>>,
    endpoint: String,
    path: String,
) -> Result<Option<String>, Error> {
    state
        .sessions
        .parent(&EndpointId::new(endpoint), &path)
        .await
}

#[tauri::command]
async fn join_path(
    state: tauri::State<'_, Arc<State>>,
    endpoint: String,
    directory: String,
    name: String,
) -> Result<String, Error> {
    state
        .sessions
        .join(&EndpointId::new(endpoint), &directory, &name)
        .await
}

/// Joins a directory and a name, refusing anything that is not a plain name.
///
/// The name comes from a text box. A name carrying a separator or `..` would
/// land outside the directory the user is looking at — the same mistake as
/// trusting a path a server sent, which section 12 names outright.
async fn child_path(
    state: &Arc<State>,
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
    state.sessions.join(endpoint, directory, name).await
}

#[tauri::command]
async fn create_dir(
    state: tauri::State<'_, Arc<State>>,
    endpoint: String,
    directory: String,
    name: String,
) -> Result<(), Error> {
    let endpoint = EndpointId::new(endpoint);
    let path = child_path(&state, &endpoint, &directory, &name).await?;
    state.sessions.create_dir(&endpoint, &path).await
}

#[tauri::command]
async fn create_file(
    state: tauri::State<'_, Arc<State>>,
    endpoint: String,
    directory: String,
    name: String,
) -> Result<(), Error> {
    let endpoint = EndpointId::new(endpoint);
    let path = child_path(&state, &endpoint, &directory, &name).await?;
    state.sessions.create_file(&endpoint, &path).await
}

#[tauri::command]
async fn rename_entry(
    state: tauri::State<'_, Arc<State>>,
    endpoint: String,
    directory: String,
    from: String,
    to: String,
) -> Result<(), Error> {
    let endpoint = EndpointId::new(endpoint);
    let source = child_path(&state, &endpoint, &directory, &from).await?;
    let target = child_path(&state, &endpoint, &directory, &to).await?;
    state.sessions.rename(&endpoint, &source, &target).await
}

#[tauri::command]
async fn measure(
    state: tauri::State<'_, Arc<State>>,
    endpoint: String,
    path: String,
) -> Result<Measurement, Error> {
    state
        .sessions
        .measure(&EndpointId::new(endpoint), &path)
        .await
}

#[tauri::command]
async fn remove_entry(
    state: tauri::State<'_, Arc<State>>,
    endpoint: String,
    path: String,
) -> Result<(), Error> {
    state
        .sessions
        .remove(&EndpointId::new(endpoint), &path)
        .await
}

#[tauri::command]
async fn set_permissions(
    state: tauri::State<'_, Arc<State>>,
    endpoint: String,
    path: String,
    mode: u32,
    recursive: bool,
) -> Result<(), Error> {
    state
        .sessions
        .set_permissions(&EndpointId::new(endpoint), &path, mode, recursive)
        .await
}

/// What the window sends when something is dragged or the button is used.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EnqueueBody {
    source_endpoint: String,
    source_directory: String,
    names: Vec<String>,
    target_endpoint: String,
    target_directory: String,
}

#[tauri::command]
async fn enqueue(
    state: tauri::State<'_, Arc<State>>,
    request: EnqueueBody,
) -> Result<usize, Error> {
    let settings = state.config.settings();
    state
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

#[tauri::command]
async fn queue_snapshot(state: tauri::State<'_, Arc<State>>) -> Result<Queue, Error> {
    Ok(state.queue.snapshot().await)
}

#[tauri::command]
async fn queue_totals(state: tauri::State<'_, Arc<State>>) -> Result<Totals, Error> {
    Ok(state.queue.totals().await)
}

#[tauri::command]
async fn queue_pause(state: tauri::State<'_, Arc<State>>, paused: bool) -> Result<(), Error> {
    state.queue.set_paused(paused).await;
    Ok(())
}

#[tauri::command]
async fn queue_hold(state: tauri::State<'_, Arc<State>>, id: String) -> Result<(), Error> {
    state.queue.hold(&id).await;
    Ok(())
}

#[tauri::command]
async fn queue_resume(state: tauri::State<'_, Arc<State>>, id: String) -> Result<(), Error> {
    state.queue.resume(&id).await;
    Ok(())
}

#[tauri::command]
async fn queue_remove(state: tauri::State<'_, Arc<State>>, id: String) -> Result<(), Error> {
    state.queue.remove(&id).await;
    Ok(())
}

#[tauri::command]
async fn queue_clear_all(state: tauri::State<'_, Arc<State>>) -> Result<(), Error> {
    state.queue.clear_all().await;
    Ok(())
}

#[tauri::command]
async fn queue_clear_finished(state: tauri::State<'_, Arc<State>>) -> Result<(), Error> {
    state.queue.clear_finished().await;
    Ok(())
}

#[tauri::command]
async fn queue_move(
    state: tauri::State<'_, Arc<State>>,
    id: String,
    by: Option<i32>,
    to: Option<usize>,
) -> Result<(), Error> {
    if let Some(index) = to {
        state.queue.move_to(&id, index).await;
    } else if let Some(delta) = by {
        state.queue.move_by(&id, delta as isize).await;
    }
    Ok(())
}

#[tauri::command]
async fn queue_decide(
    state: tauri::State<'_, Arc<State>>,
    id: String,
    policy: ConflictPolicy,
    for_all: bool,
) -> Result<(), Error> {
    state.queue.decide(&id, policy, for_all).await;
    Ok(())
}

#[tauri::command]
fn quick_connect_history(state: tauri::State<'_, Arc<State>>) -> Vec<QuickConnectEntry> {
    state.config.quick_connect()
}

#[tauri::command]
fn forget_quick_connect(state: tauri::State<'_, Arc<State>>, id: String) -> Result<(), Error> {
    state.config.forget_quick_connect(&id)
}

#[tauri::command]
fn save_as_site(state: tauri::State<'_, Arc<State>>, id: String) -> Result<String, Error> {
    state
        .config
        .save_as_site(&id)
        .map(|path| path.to_string_lossy().into_owned())
}

#[tauri::command]
fn remember_path(
    state: tauri::State<'_, Arc<State>>,
    id: String,
    path: String,
) -> Result<(), Error> {
    let Some(entry) = state
        .config
        .quick_connect()
        .into_iter()
        .find(|entry| entry.id == id)
    else {
        return Ok(());
    };
    state.config.remember_quick_connect(QuickConnectEntry {
        last_path: Some(path),
        last_used: now(),
        ..entry
    })
}

/// Opens a web address in whatever the system uses for one.
///
/// A link in a webview goes nowhere on its own, and pulling in a plugin for
/// three lines of `open` would be a dependency for nothing. Only http and
/// https are accepted: this takes a string and hands it to the shell, and the
/// day something other than the window's own footer calls it, that check is
/// what stands between a link and a command line.
/// Judges an answer from GitHub against the version running.
///
/// The window makes the request — it needs no Rust HTTP client for one address,
/// and the content security policy names that one host. The judgement stays
/// here, where comparing version numbers is written once and tested: a text
/// comparison puts 0.10 before 0.9, and that is the release people would miss.
#[tauri::command]
fn newer_release(answer: String) -> Option<amberbeam_core::update::Release> {
    let release = amberbeam_core::update::read_answer(&answer)?;
    amberbeam_core::update::is_newer(env!("CARGO_PKG_VERSION"), &release.version).then_some(release)
}

/// Where to ask. Named by the core so the window cannot ask somewhere else.
#[tauri::command]
fn update_source() -> &'static str {
    amberbeam_core::update::LATEST_RELEASE
}

#[tauri::command]
fn open_url(url: String) -> Result<(), Error> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(Error::other("only http and https addresses are opened"));
    }

    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("open");
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = std::process::Command::new("cmd");
        command.args(["/C", "start", ""]);
        command
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = std::process::Command::new("xdg-open");

    command.arg(&url).spawn().map(|_| ()).map_err(Error::other)
}

/// Which pane of the system's settings a key scheme needs.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum SystemPane {
    /// Where the function keys are turned into function keys.
    FunctionKeys,
    /// Where Mission Control and Spotlight give up F3, F4 and F11.
    Shortcuts,
}

/// Opens the settings pane a key scheme needs, and nothing else.
///
/// A command of its own rather than widening [`open_url`]: that one accepts
/// http and https for a reason, and a program able to open any scheme the
/// system knows is a program that can be talked into opening a good deal more
/// than a settings pane.
///
/// It only opens the pane. Turning the setting on is the person's own doing —
/// no program should be able to change how somebody's keyboard behaves.
#[tauri::command]
fn open_system_keyboard(pane: SystemPane) -> Result<(), Error> {
    #[cfg(target_os = "macos")]
    {
        let target = match pane {
            SystemPane::FunctionKeys => {
                "x-apple.systempreferences:com.apple.Keyboard-Settings.extension"
            }
            SystemPane::Shortcuts => {
                "x-apple.systempreferences:com.apple.Keyboard-Settings.extension?Shortcuts"
            }
        };
        std::process::Command::new("open")
            .arg(target)
            .spawn()
            .map(|_| ())
            .map_err(Error::other)
    }

    #[cfg(not(target_os = "macos"))]
    {
        // Nowhere else does the operating system keep the function keys for
        // itself, so there is no pane to open and nothing to apologise for.
        let _ = pane;
        Ok(())
    }
}

/// Writes a text file the user picked, and reads one back.
///
/// Used by the key schemes, which are the window's own business — the core
/// stores them as opaque state and has no opinion about their shape.
#[tauri::command]
fn write_text_file(path: PathBuf, text: String) -> Result<(), Error> {
    std::fs::write(&path, text).map_err(Error::from)
}

#[tauri::command]
fn read_text_file(path: PathBuf) -> Result<String, Error> {
    std::fs::read_to_string(&path).map_err(Error::from)
}

#[tauri::command]
fn settings(state: tauri::State<'_, Arc<State>>) -> Settings {
    state.config.settings()
}

#[tauri::command]
fn set_settings(state: tauri::State<'_, Arc<State>>, value: Settings) -> Result<(), Error> {
    state.config.set_settings(&value)
}

#[tauri::command]
fn ui_state(state: tauri::State<'_, Arc<State>>) -> Option<serde_json::Value> {
    state.config.ui_state()
}

#[tauri::command]
fn set_ui_state(
    state: tauri::State<'_, Arc<State>>,
    value: serde_json::Value,
) -> Result<(), Error> {
    state.config.set_ui_state(&value)
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs() as i64)
        .unwrap_or_default()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let events = Events::new();
    let config = Config::default_location().unwrap_or_else(|_| Config::at("."));
    let sessions = Arc::new(Sessions::new(events.clone()));
    let queue_path = config.root().join("queue.json");
    let state = Arc::new(State {
        sessions: Arc::clone(&sessions),
        config,
        queue: Runner::new(sessions, events.clone(), queue_path),
        secrets: Box::new(SystemStore::default()),
    });

    let started = Arc::clone(&state);
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            // Inside an async block, so the queue's loops are spawned where a
            // runtime exists.
            let queue = Arc::clone(&started.queue);
            tauri::async_runtime::spawn(async move {
                queue.start();
            });

            // The core's event stream is pumped into the webview here. This is
            // the only place that knows both sides; everything else deals in
            // core events, which is what makes the M7 shell a transport swap.
            let handle = app.handle().clone();
            let mut listener = events.subscribe();
            tauri::async_runtime::spawn(async move {
                loop {
                    match listener.recv().await {
                        Ok(event) => {
                            let _ = handle.emit(EVENT_CHANNEL, &event);
                        }
                        // A window that fell behind loses log lines and carries
                        // on. Ending the pump here would silence it for good.
                        Err(RecvError::Lagged(missed)) => {
                            let _ = handle.emit(
                                EVENT_CHANNEL,
                                &Event::Log {
                                    endpoint: EndpointId::new("local"),
                                    direction: amberbeam_core::LogDirection::Note,
                                    text: format!("log fell behind, {missed} lines lost"),
                                },
                            );
                        }
                        Err(_) => break,
                    }
                }
            });
            Ok(())
        })
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            core_info,
            local_session,
            connect,
            disconnect,
            list_dir,
            parent_of,
            join_path,
            create_dir,
            create_file,
            rename_entry,
            measure,
            remove_entry,
            set_permissions,
            quick_connect_history,
            forget_quick_connect,
            save_as_site,
            open_site_manager,
            export_sites,
            bundle_preview,
            bundle_apply,
            import_candidates,
            import_preview,
            import_apply,
            open_site,
            sites,
            site_folders,
            save_site,
            delete_site,
            create_site_folder,
            rename_site_folder,
            delete_site_folder,
            set_site_secret,
            forget_site_secret,
            remember_path,
            ui_state,
            set_ui_state,
            settings,
            set_settings,
            open_url,
            open_system_keyboard,
            write_text_file,
            read_text_file,
            newer_release,
            update_source,
            enqueue,
            queue_snapshot,
            queue_totals,
            queue_pause,
            queue_hold,
            queue_resume,
            queue_remove,
            queue_clear_finished,
            queue_clear_all,
            queue_move,
            queue_decide,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
