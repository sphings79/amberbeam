//! Desktop shell of AmberBeam.
//!
//! This crate is deliberately thin. It owns the window, turns Tauri commands
//! into calls on `amberbeam-core`, and forwards the core's events into the
//! webview. It holds no logic of its own: the same set of commands is what the
//! headless HTTP and WebSocket service of milestone M7 will expose, which only
//! works as long as nothing of substance settles here.

use std::sync::Arc;

use amberbeam_core::config::{AuthKind, Config, QuickConnectEntry};
use amberbeam_core::endpoint::EndpointId;
use amberbeam_core::error::Error;
use amberbeam_core::events::{Event, RecvError};
use amberbeam_core::fs::Listing;
use amberbeam_core::registry::{Connected, Sessions};
use amberbeam_core::sftp::{AuthMethod, ConnectParams, HostKeyDecision};
use amberbeam_core::{CoreInfo, Events};
use serde::Deserialize;
use tauri::Emitter;

/// The name events arrive under in the webview. One channel for everything, so
/// the frontend has a single place to listen — which is also how the WebSocket
/// of M7 will look.
const EVENT_CHANNEL: &str = "amberbeam://event";

struct State {
    sessions: Sessions,
    config: Config,
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
}

impl ConnectRequest {
    fn into_params(self) -> ConnectParams {
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
    };

    let connected = state
        .sessions
        .connect_sftp(&endpoint, &request.into_params())
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
    let state = Arc::new(State {
        sessions: Sessions::new(events.clone()),
        config,
    });

    tauri::Builder::default()
        .setup(move |app| {
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
            quick_connect_history,
            forget_quick_connect,
            save_as_site,
            remember_path,
            ui_state,
            set_ui_state,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
