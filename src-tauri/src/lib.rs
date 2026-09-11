//! Desktop shell of AmberBeam.
//!
//! This crate is deliberately thin. It owns the window, turns Tauri commands
//! into calls on `amberbeam-core`, and forwards the core's events into the
//! webview. It holds no logic of its own: the same set of commands is what the
//! headless HTTP and WebSocket service of milestone M7 will expose, which only
//! works as long as nothing of substance settles here.

use std::sync::Arc;

use amberbeam_core::config::{AuthKind, Config, QuickConnectEntry, Settings};
use amberbeam_core::endpoint::EndpointId;
use amberbeam_core::error::Error;
use amberbeam_core::events::{Event, RecvError};
use amberbeam_core::fs::Listing;
use amberbeam_core::ops::{is_usable_name, Measurement};
use amberbeam_core::queue::{Queue, Totals};
use amberbeam_core::registry::{Connected, Sessions};
use amberbeam_core::runner::{EnqueueRequest, Runner};
use amberbeam_core::sftp::{AuthMethod, ConnectParams, HostKeyDecision};
use amberbeam_core::transfer::ConflictPolicy;
use amberbeam_core::{CoreInfo, Events};
use serde::Deserialize;
use tauri::Emitter;

/// The name events arrive under in the webview. One channel for everything, so
/// the frontend has a single place to listen — which is also how the WebSocket
/// of M7 will look.
const EVENT_CHANNEL: &str = "amberbeam://event";

struct State {
    sessions: Arc<Sessions>,
    config: Config,
    queue: Arc<Runner>,
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
    request: ConnectRequest,
) -> Result<Connected, Error> {
    let endpoint = EndpointId::new(request.endpoint.clone());
    let settings = state.config.settings();
    let history = QuickConnectEntry {
        id: QuickConnectEntry::id_for(&request.user, &request.host, request.port),
        protocol: amberbeam_core::Protocol::Sftp,
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
    };

    let connected = state
        .sessions
        .connect_sftp(&endpoint, &request.into_params(&settings))
        .await?;

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
    });

    let started = Arc::clone(&state);
    tauri::Builder::default()
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
            remember_path,
            ui_state,
            set_ui_state,
            settings,
            set_settings,
            open_url,
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
