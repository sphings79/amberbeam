//! Desktop shell of AmberBeam.
//!
//! This crate is deliberately thin, and since the container build it is
//! thinner still. Everything a command actually does lives in
//! `amberbeam-commands`, which both shells call; what is left here is the part
//! that only makes sense with a window in front of it.
//!
//! Two kinds of thing stay:
//!
//! * **Windows.** Opening the site manager, asking the main window to open a
//!   site, restarting after an update, opening a link or a settings pane in
//!   whatever the system uses for one.
//! * **Paths a person chose.** Exporting the list, reading an export,
//!   importing somebody else's file, and the two that read and write a text
//!   file for the key schemes. Each takes a path from the caller, and each is
//!   safe only because a file dialog put it there. Reached over a network the
//!   same commands would read and write any file the service can touch, so the
//!   shared crate does not carry them.

use std::path::PathBuf;
use std::sync::Arc;

use amberbeam_commands::Service;
use amberbeam_core::bundle;
use amberbeam_core::config::Config;
use amberbeam_core::editing::Edits;
use amberbeam_core::endpoint::EndpointId;
use amberbeam_core::error::Error;
use amberbeam_core::events::{Event, RecvError};
use amberbeam_core::import;
use amberbeam_core::registry::Sessions;
use amberbeam_core::runner::Runner;
use amberbeam_core::secrets::{MemoryStore, Secret, SystemStore};
use amberbeam_core::sites::Site;
use amberbeam_core::Events;
use serde::Deserialize;
use tauri::{Emitter, Manager};

/// The name events arrive under in the webview. One channel for everything, so
/// the frontend has a single place to listen — which is also how the WebSocket
/// of the container build looks.
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

/// Everything the window asks for that is not about windows or chosen paths.
///
/// One command rather than forty-odd, because the list of what those are lives
/// in the shared crate now. A second list here, kept in step by hand, is
/// exactly the drift this was built to end.
#[tauri::command]
async fn run_command(
    state: tauri::State<'_, Arc<Service>>,
    command: String,
    args: serde_json::Value,
) -> Result<serde_json::Value, Error> {
    amberbeam_commands::dispatch(&state, &command, args).await
}

/// Opens the site manager in a window of its own.
///
/// Its own window because somebody with thirty servers wants the list beside
/// the panes, not instead of them. Same bundle, a different view — so there is
/// one interface to maintain, not two.
///
/// Deliberately `async`, and that is the whole of why it works on Windows.
/// Tauri says so itself: building a webview from a synchronous command
/// deadlocks there, because WebView2 wants the main thread and the command is
/// already holding it. The window that came up frozen, and then blank, was
/// never a page that failed to load — it was a page that was never given the
/// chance to start. Nothing in it could report the fault, including the
/// reporter written for exactly that purpose.
#[tauri::command]
async fn open_site_manager(app: tauri::AppHandle) -> Result<(), Error> {
    use tauri::{WebviewUrl, WebviewWindowBuilder};

    if let Some(existing) = app.get_webview_window(SITES_WINDOW) {
        // Already open: bring it forward rather than stack a second one.
        let _ = existing.unminimize();
        let _ = existing.set_focus();
        return Ok(());
    }

    // The plain page, with no query string. `WebviewUrl::App` takes a
    // **path**, and a question mark is a perfectly ordinary character in a path
    // on macOS and an illegal one on Windows — so "index.html?view=sites"
    // loaded here and opened an empty, frozen window there. Which view this is
    // gets decided from the window's label instead, which is not a path and
    // cannot be mangled by one.
    WebviewWindowBuilder::new(&app, SITES_WINDOW, WebviewUrl::App("index.html".into()))
        // The window says what it is before the page loads, so nothing has to
        // call into Tauri at module scope to find out. Not the cure for the
        // blank window — that was the deadlock above — but one less thing that
        // has to have finished initialising before the first line of the view
        // can run.
        .initialization_script("window.__AMBERBEAM_VIEW__ = 'sites';")
        .title(format!("AmberBeam {}", env!("CARGO_PKG_VERSION")))
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

/// Starts the program again, which is what an installed update is waiting for.
#[tauri::command]
fn restart(app: tauri::AppHandle) {
    app.restart();
}

// --- Taking the list with you ----------------------------------------------

/// Writes the whole list to one file.
///
/// Two shapes and no third: without passwords it is plain JSON anybody can
/// read, and with them it is sealed under a passphrase. The core refuses the
/// combination that would be neither.
#[tauri::command]
fn export_sites(
    state: tauri::State<'_, Arc<Service>>,
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
    state: tauri::State<'_, Arc<Service>>,
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
async fn import_candidates() -> Result<Vec<import::Candidate>, Error> {
    Ok(import::discover())
}

#[tauri::command]
async fn import_preview(source: import::Source, path: PathBuf) -> Result<import::Found, Error> {
    import::read(source, &path)
}

/// Takes the ticked entries over.
///
/// The file is read a second time rather than the preview being trusted: that
/// is what keeps the passwords out of the window, and it costs a few
/// milliseconds on a file of thirty servers.
#[tauri::command]
fn import_apply(
    state: tauri::State<'_, Arc<Service>>,
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
            remember_path: false,
            local_path: None,
            concurrency: entry.protocol.default_concurrency(),
            retries: None,
            temporary_name: None,
            encryption: entry.encryption,
            passive: None,
            latin1: None,
            keep_alive: None,
            remember_password: take_passwords && entry.has_password,
            wastebasket: None,
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

/// Opens a web address in whatever the system uses for one.
///
/// A link in a webview goes nowhere on its own, and pulling in a plugin for
/// three lines of `open` would be a dependency for nothing. Only http and
/// https are accepted: this takes a string and hands it to the shell, and the
/// day something other than the window's own footer calls it, that check is
/// what stands between a link and a command line.
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let events = Events::new();
    let config = Config::default_location().unwrap_or_else(|_| Config::at("."));
    let sessions = Arc::new(Sessions::new(events.clone()));
    let queue_path = config.root().join("queue.json");
    let service = Arc::new(Service {
        sessions: Arc::clone(&sessions),
        config,
        queue: Runner::new(sessions, events.clone(), queue_path),
        secrets: Box::new(SystemStore::default()),
        session: MemoryStore::default(),
        edits: Edits::beneath_temp(),
        // The version of the program somebody is running, which is this crate
        // and not the shared one.
        version: env!("CARGO_PKG_VERSION").to_string(),
    });

    let started = Arc::clone(&service);
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        // Fetching and installing a new version without leaving the program.
        // Every update is checked against the public key in the config before
        // anything is written, which is the whole reason this can exist at
        // all: the program replaces itself, so "where did this come from" has
        // to have an answer that does not depend on trusting the network.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(move |app| {
            // The version belongs where somebody would look for it, and the
            // title bar is the one strip of the window that is always visible
            // whatever is open inside it. Set here rather than in the config
            // because the config cannot read the version it was built with,
            // and a number typed in twice is a number that will disagree.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_title(&format!("AmberBeam {}", env!("CARGO_PKG_VERSION")));
            }

            // Inside an async block, so the queue's loops are spawned where a
            // runtime exists.
            let queue = Arc::clone(&started.queue);
            let settings = started.config.settings();
            tauri::async_runtime::spawn(async move {
                // What was chosen last time applies from the first second of
                // this run, not from the next time the settings are touched.
                queue
                    .set_clear_after(amberbeam_commands::clearing(&settings))
                    .await;
                queue.start();
            });

            // The core's event stream is pumped into the webview here. This is
            // the only place that knows both sides; everything else deals in
            // core events, which is what makes the container shell a transport
            // swap.
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
        .manage(service)
        .invoke_handler(tauri::generate_handler![
            run_command,
            open_site_manager,
            open_site,
            restart,
            export_sites,
            bundle_preview,
            bundle_apply,
            import_candidates,
            import_preview,
            import_apply,
            open_url,
            open_system_keyboard,
            write_text_file,
            read_text_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
