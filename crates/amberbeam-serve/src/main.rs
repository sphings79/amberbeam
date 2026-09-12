//! The container shell of AmberBeam.
//!
//! The same program, reached over HTTP instead of through a window. It holds
//! no commands of its own: it takes a name and its arguments off a request,
//! hands them to `amberbeam-commands`, and sends back what comes out. The
//! desktop shell does exactly the same through Tauri, which is what section 09
//! of the concept paper means by the second seam.
//!
//! What it adds is what a network needs and a window does not: a password at
//! the door, a token afterwards, and one WebSocket carrying the core's events
//! to every browser that is looking.
//!
//! ## How it is told what to do
//!
//! Everything through the environment, because that is what a container reads:
//!
//! | Variable | What it is | Without it |
//! |---|---|---|
//! | `AMBERBEAM_PASSWORD` | the one password | nothing is let in |
//! | `AMBERBEAM_PASSWORD_FILE` | a file holding it, for a docker secret | — |
//! | `AMBERBEAM_ADDRESS` | where to listen | `0.0.0.0:2122` |
//! | `AMBERBEAM_CONFIG` | where the site list and settings live | `/config` |
//! | `AMBERBEAM_WEB` | the built interface to serve | `/web` |
//! | `AMBERBEAM_BEHIND_TLS` | a proxy in front terminates TLS | assumed not |
//! | `AMBERBEAM_SECRET_PASSPHRASE` | opens the password file | no saved passwords |
//! | `AMBERBEAM_SECRET_PASSPHRASE_FILE` | a file holding it | — |
//!
//! ## Why it does not speak TLS itself
//!
//! Because a reverse proxy does it better. It renews certificates, which a
//! container cannot; it speaks HTTP/2; and it is the piece already standing in
//! front of everything else on the machine. Terminating TLS here would mean
//! two more dependencies to do a worse job twice.
//!
//! `AMBERBEAM_BEHIND_TLS` is how it is told that a proxy is doing it, and the
//! only thing that changes is the session cookie: marked `Secure`, a browser
//! will not send it back over plain HTTP. Setting it wrongly either way gives
//! a login that appears to work and then does not, which is why it is asked
//! rather than guessed.
//!
//! The local side starts wherever `HOME` points, which the image sets to
//! `/data`. That is a starting point and not a fence — see the container page
//! in the documentation, which says so plainly rather than implying otherwise.

mod auth;

use std::net::SocketAddr;
use std::sync::Arc;

use amberbeam_commands::Service;
use amberbeam_core::config::Config;
use amberbeam_core::events::RecvError;
use amberbeam_core::registry::Sessions;
use amberbeam_core::runner::Runner;
use amberbeam_core::secrets::{FileStore, MemoryStore, SecretStore};
use amberbeam_core::Events;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tower_http::services::{ServeDir, ServeFile};

use auth::{Doorway, Refusal};

struct Shell {
    service: Arc<Service>,
    door: Doorway,
    events: Events,
    /// Whether something in front of this is terminating TLS, which decides
    /// whether the session cookie may be marked `Secure`. Marked so on plain
    /// HTTP it is a cookie the browser never sends back; left off behind a
    /// proxy it is a cookie that would travel in the clear if anybody ever
    /// reached the service directly.
    secure: bool,
}

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

/// The password, from the variable or from the file a docker secret puts it in.
///
/// The file wins. Somebody who went to the trouble of mounting a secret means
/// it, and an old variable left in a compose file should not quietly override
/// them.
fn password() -> Option<String> {
    if let Some(path) = env("AMBERBEAM_PASSWORD_FILE") {
        match std::fs::read_to_string(&path) {
            Ok(text) => return Some(text.trim().to_string()).filter(|word| !word.is_empty()),
            Err(why) => {
                eprintln!("AMBERBEAM_PASSWORD_FILE points at {path}, which cannot be read: {why}");
                return None;
            }
        }
    }
    env("AMBERBEAM_PASSWORD")
}

/// The passphrase the password file is kept under.
///
/// Same two ways as the login password, and for the same reason: a file for a
/// docker secret, a variable for somebody who would rather not bother.
fn passphrase() -> Option<String> {
    if let Some(path) = env("AMBERBEAM_SECRET_PASSPHRASE_FILE") {
        match std::fs::read_to_string(&path) {
            Ok(text) => return Some(text.trim().to_string()).filter(|word| !word.is_empty()),
            Err(why) => {
                eprintln!(
                    "AMBERBEAM_SECRET_PASSPHRASE_FILE points at {path}, which cannot be read: {why}"
                );
                return None;
            }
        }
    }
    env("AMBERBEAM_SECRET_PASSPHRASE")
}

/// Where saved passwords go, and what to say when they cannot go anywhere.
///
/// A container has no credential store, so there are two honest answers and no
/// third: an encrypted file under a passphrase, or nothing saved at all. What
/// it must never be is a store that quietly forgets — somebody ticking
/// "remember" and finding out at the next restart that it did not.
fn secrets(root: &std::path::Path) -> Box<dyn SecretStore> {
    let Some(passphrase) = passphrase() else {
        eprintln!(
            "No AMBERBEAM_SECRET_PASSPHRASE, so no password can be saved. Connections still \
             work; you will be asked each time."
        );
        return Box::new(MemoryStore::default());
    };

    match FileStore::open(root.join("secrets.sealed"), &passphrase) {
        Ok(store) => {
            println!("password file open, holding {}", store.count());
            Box::new(store)
        }
        Err(why) => {
            // Deliberately fatal. Carrying on with an empty store would mean
            // the first saved password overwrites every password in the file,
            // so a mistyped passphrase would destroy what it failed to read.
            eprintln!("{why}");
            std::process::exit(1);
        }
    }
}

#[derive(Deserialize)]
struct Login {
    password: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Welcome {
    token: String,
}

/// What the service says about itself before anybody has logged in.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Greeting {
    /// Whether a password was set. A browser needs to know so it can say "this
    /// service was started without one" rather than "wrong password" for ever.
    guarded: bool,
    version: &'static str,
}

#[tokio::main]
async fn main() {
    let events = Events::new();
    let config = Config::at(env("AMBERBEAM_CONFIG").unwrap_or_else(|| "/config".into()));
    let sessions = Arc::new(Sessions::new(events.clone()));
    let queue_path = config.root().join("queue.json");
    let secrets = secrets(config.root());
    let service = Arc::new(Service {
        sessions: Arc::clone(&sessions),
        config,
        queue: Runner::new(sessions, events.clone(), queue_path),
        secrets,
        session: MemoryStore::default(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    });

    let door = Doorway::new(password());
    if !door.is_open() {
        eprintln!(
            "No password is set, so nothing will be let in. Set AMBERBEAM_PASSWORD or \
             AMBERBEAM_PASSWORD_FILE and start again."
        );
    }

    let shell = Arc::new(Shell {
        service: Arc::clone(&service),
        door,
        events: events.clone(),
        secure: env("AMBERBEAM_BEHIND_TLS").is_some_and(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        }),
    });

    service.queue.start();

    let web = env("AMBERBEAM_WEB").unwrap_or_else(|| "/web".into());
    let index = std::path::Path::new(&web).join("index.html");
    let app = Router::new()
        .route("/api/hello", get(hello))
        .route("/api/login", post(login))
        .route("/api/logout", post(logout))
        .route("/api/events", get(events_socket))
        .route("/api/{command}", post(command))
        // Anything that is not the API is the interface itself. A path the
        // build did not produce falls back to index.html, because the window
        // decides what it shows and the server has no opinion about it.
        .fallback_service(ServeDir::new(&web).fallback(ServeFile::new(index)))
        .with_state(Arc::clone(&shell));

    let address: SocketAddr = env("AMBERBEAM_ADDRESS")
        .unwrap_or_else(|| "0.0.0.0:2122".into())
        .parse()
        .expect("AMBERBEAM_ADDRESS is not an address to listen on");

    let listener = tokio::net::TcpListener::bind(address)
        .await
        .unwrap_or_else(|why| panic!("cannot listen on {address}: {why}"));
    println!(
        "AmberBeam {} is listening on {address}",
        env!("CARGO_PKG_VERSION")
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
            println!("stopping");
        })
        .await
        .expect("the server stopped badly");
}

async fn hello(State(shell): State<Arc<Shell>>) -> Json<Greeting> {
    Json(Greeting {
        guarded: shell.door.is_open(),
        version: env!("CARGO_PKG_VERSION"),
    })
}

async fn login(State(shell): State<Arc<Shell>>, Json(body): Json<Login>) -> Response {
    match shell.door.admit(&body.password) {
        Ok(token) => {
            // Both at once, and on purpose. A browser wants the cookie and
            // cannot hold a header; the desktop client talking to another
            // machine wants the token and cannot rely on a cookie surviving
            // the trip. One login, two ways of carrying the answer.
            let cookie = format!(
                "{}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age=43200{}",
                auth::COOKIE,
                if shell.secure { "; Secure" } else { "" }
            );
            (
                StatusCode::OK,
                [(header::SET_COOKIE, cookie)],
                Json(Welcome { token }),
            )
                .into_response()
        }
        Err(Refusal::Closed) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(problem(
                "this service was started without a password, so nothing is let in",
            )),
        )
            .into_response(),
        Err(Refusal::Wrong) => (
            StatusCode::UNAUTHORIZED,
            Json(problem("that is not the password")),
        )
            .into_response(),
    }
}

async fn logout(State(shell): State<Arc<Shell>>, headers: HeaderMap) -> Response {
    if let Some(token) = token_of(&headers) {
        shell.door.forget(&token);
    }
    let cookie = format!("{}=; Path=/; HttpOnly; Max-Age=0", auth::COOKIE);
    (
        StatusCode::OK,
        [(header::SET_COOKIE, cookie)],
        Json(Value::Null),
    )
        .into_response()
}

/// The token a request carries, from the header or from the cookie.
fn token_of(headers: &HeaderMap) -> Option<String> {
    if let Some(value) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    {
        if let Some(token) = value.strip_prefix("Bearer ") {
            return Some(token.trim().to_string());
        }
    }
    headers
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|header| auth::cookie(header, auth::COOKIE))
        .map(|token| token.to_string())
}

/// A failure in the shape the window already knows how to read.
///
/// The same tagged object the core rejects with, so both halves of the
/// interface handle one shape and the container build needs no error handling
/// of its own.
fn problem(detail: &str) -> Value {
    serde_json::json!({ "kind": "other", "detail": detail })
}

async fn command(
    State(shell): State<Arc<Shell>>,
    headers: HeaderMap,
    Path(command): Path<String>,
    body: Option<Json<Value>>,
) -> Response {
    let Some(token) = token_of(&headers) else {
        return (StatusCode::UNAUTHORIZED, Json(problem("not signed in"))).into_response();
    };
    if !shell.door.holds(&token) {
        return (StatusCode::UNAUTHORIZED, Json(problem("not signed in"))).into_response();
    }

    let args = body.map(|Json(value)| value).unwrap_or(Value::Null);
    match amberbeam_commands::dispatch(&shell.service, &command, args).await {
        Ok(value) => (StatusCode::OK, Json(value)).into_response(),
        Err(why) => {
            // The core's own error, serialised as it is. The window reads the
            // same object here as it does on the desktop, which is the point
            // of having one shape for a failure.
            let body = serde_json::to_value(&why).unwrap_or_else(|_| problem("something failed"));
            (StatusCode::BAD_REQUEST, Json(body)).into_response()
        }
    }
}

async fn events_socket(
    State(shell): State<Arc<Shell>>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    // Checked before the upgrade, not after. A socket that opens and then
    // refuses to say anything looks like a bug from the other end.
    let Some(token) = token_of(&headers) else {
        return (StatusCode::UNAUTHORIZED, Json(problem("not signed in"))).into_response();
    };
    if !shell.door.holds(&token) {
        return (StatusCode::UNAUTHORIZED, Json(problem("not signed in"))).into_response();
    }

    upgrade.on_upgrade(move |socket| pump(socket, shell))
}

/// Carries the core's events to one browser for as long as it is listening.
async fn pump(mut socket: WebSocket, shell: Arc<Shell>) {
    let mut listener = shell.events.subscribe();
    loop {
        match listener.recv().await {
            Ok(event) => {
                let Ok(text) = serde_json::to_string(&event) else {
                    continue;
                };
                if socket.send(Message::Text(text.into())).await.is_err() {
                    // The browser is gone. Nothing to report: closing a tab is
                    // not a failure.
                    break;
                }
            }
            // A listener that fell behind loses log lines and carries on.
            // Ending here would silence that browser for good.
            Err(RecvError::Lagged(missed)) => {
                let note = serde_json::json!({
                    "kind": "log",
                    "endpoint": "local",
                    "direction": "note",
                    "text": format!("log fell behind, {missed} lines lost"),
                });
                if socket
                    .send(Message::Text(note.to_string().into()))
                    .await
                    .is_err()
                {
                    break;
                }
            }
            Err(_) => break,
        }
    }
}
