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
use amberbeam_core::editing::Edits;
use amberbeam_core::endpoint::EndpointId;
use amberbeam_core::error::Error;
use amberbeam_core::events::RecvError;
use amberbeam_core::registry::Sessions;
use amberbeam_core::runner::Runner;
use amberbeam_core::secrets::{FileStore, MemoryStore, SecretStore};
use amberbeam_core::Events;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_core::Stream;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::AsyncWriteExt;
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

/// A failure as a person at a terminal should read it.
///
/// The core writes errors for logs, beginning with the key a window would look
/// up to find the sentence. There is no window here and nothing to look up, so
/// "error.other: the password file will not open" is a line explaining the
/// program to itself rather than to whoever is reading it.
fn plainly(why: &Error) -> String {
    match why {
        Error::Other { detail } => detail.clone(),
        other => other.to_string(),
    }
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
            eprintln!("{}", plainly(&why));
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

/// The one thing that may be asked for in an address.
#[derive(Deserialize)]
struct SocketQuery {
    token: Option<String>,
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
        edits: Edits::beneath_temp(),
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

    // What was chosen last time applies from the first second of this run.
    service
        .queue
        .set_clear_after(amberbeam_commands::clearing(&service.config.settings()))
        .await;
    // Copies left behind by a run that ended badly. The register is in
    // memory, so anything still in that directory belongs to nobody.
    service.edits.sweep();
    service.queue.start();
    amberbeam_commands::watch_edits(&service, events.clone());

    let web = env("AMBERBEAM_WEB").unwrap_or_else(|| "/web".into());
    let index = std::path::Path::new(&web).join("index.html");
    let app = Router::new()
        .route("/api/hello", get(hello))
        .route("/api/login", post(login))
        .route("/api/logout", post(logout))
        .route("/api/events", get(events_socket))
        .route("/api/download", get(download))
        .route("/api/upload", post(upload))
        // After the two above, so a command is never mistaken for one of them.
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
    Query(asked): Query<SocketQuery>,
    upgrade: WebSocketUpgrade,
) -> Response {
    // Checked before the upgrade, not after. A socket that opens and then
    // refuses to say anything looks like a bug from the other end.
    //
    // The token may arrive in the address here, which it may nowhere else. A
    // WebSocket cannot be opened with a header, so a client with no cookie has
    // nowhere else to put it. A browser on this service's own page sends the
    // cookie and never uses this.
    let Some(token) = token_of(&headers).or(asked.token) else {
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

/// Which file, and where it is.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileAt {
    endpoint: String,
    path: String,
}

/// Where an arriving file should land.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileInto {
    endpoint: String,
    directory: String,
    name: String,
}

/// Only the machine this service runs on.
///
/// Not a shortcut. Reading from or writing to a remote endpoint means holding
/// an FTP data connection open for as long as a browser takes, and the
/// server's verdict on that transfer arrives on the control connection
/// afterwards — a stream that ends when a download is cancelled would leave
/// the connection in a state nobody asked about. Moving a file between here
/// and a server is what the queue is for; this is only the last step to the
/// person looking at it.
fn only_local(endpoint: &str) -> Result<EndpointId, Box<Response>> {
    if endpoint != amberbeam_core::registry::LOCAL {
        return Err(Box::new(
            (
                StatusCode::BAD_REQUEST,
                Json(problem(
                    "only this machine's own files can be sent this way",
                )),
            )
                .into_response(),
        ));
    }
    Ok(EndpointId::new(endpoint))
}

/// Hands a file to the browser.
async fn download(
    State(shell): State<Arc<Shell>>,
    headers: HeaderMap,
    Query(asked): Query<FileAt>,
) -> Response {
    if let Some(refusal) = refuse(&shell, &headers) {
        return refusal;
    }
    let endpoint = match only_local(&asked.endpoint) {
        Ok(endpoint) => endpoint,
        Err(refusal) => return *refusal,
    };

    let session = match shell.service.sessions.find(&endpoint).await {
        Ok(session) => session,
        Err(why) => return failed(&why),
    };
    let (size, _) = match session.stat(&asked.path).await {
        Ok(found) => found,
        Err(why) => return failed(&why),
    };
    let (reader, hold) = match session.open_read(&asked.path, 0).await {
        Ok(opened) => opened,
        Err(why) => return failed(&why),
    };
    // A hold means a connection that has to be given back, and nothing here
    // can give it back once the body is streaming. Local files have none; if
    // that ever stops being true this refuses rather than leaking one.
    if hold.is_some() {
        return (
            StatusCode::BAD_REQUEST,
            Json(problem("that file cannot be sent this way")),
        )
            .into_response();
    }

    let name = asked.path.rsplit('/').next().unwrap_or("file").to_string();
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_string()),
            (header::CONTENT_LENGTH, size.to_string()),
            (
                header::CONTENT_DISPOSITION,
                // Encoded rather than quoted: a name with a quote, a semicolon
                // or an umlaut in it would otherwise arrive as something else,
                // or as a header a browser refuses to read.
                format!("attachment; filename*=UTF-8''{}", encoded(&name)),
            ),
        ],
        axum::body::Body::from_stream(tokio_util::io::ReaderStream::new(reader)),
    )
        .into_response()
}

/// Takes a file from the browser.
async fn upload(
    State(shell): State<Arc<Shell>>,
    headers: HeaderMap,
    Query(asked): Query<FileInto>,
    body: axum::body::Body,
) -> Response {
    if let Some(refusal) = refuse(&shell, &headers) {
        return refusal;
    }
    let endpoint = match only_local(&asked.endpoint) {
        Ok(endpoint) => endpoint,
        Err(refusal) => return *refusal,
    };

    // The name comes from somebody else's computer. A name carrying a
    // separator or `..` would land outside the directory they are looking at,
    // which is the one thing an upload must never be able to do.
    if !amberbeam_core::ops::is_usable_name(&asked.name) {
        return (
            StatusCode::BAD_REQUEST,
            Json(problem("that is not a file name")),
        )
            .into_response();
    }

    let session = match shell.service.sessions.find(&endpoint).await {
        Ok(session) => session,
        Err(why) => return failed(&why),
    };
    let path = match shell
        .service
        .sessions
        .join(&endpoint, &asked.directory, &asked.name)
        .await
    {
        Ok(path) => path,
        Err(why) => return failed(&why),
    };
    let (mut writer, hold) = match session.open_write(&path, 0).await {
        Ok(opened) => opened,
        Err(why) => return failed(&why),
    };
    if hold.is_some() {
        return (
            StatusCode::BAD_REQUEST,
            Json(problem("files cannot be put there this way")),
        )
            .into_response();
    }

    // Chunk by chunk rather than through a pile of combinators. The body may
    // be a hundred gigabytes, so none of it is ever held whole; and a loop
    // this short is easier to be sure about than the crate that would shorten
    // it further.
    let mut stream = body.into_data_stream();
    let mut moved: u64 = 0;
    loop {
        let next = std::future::poll_fn(|cx| std::pin::Pin::new(&mut stream).poll_next(cx)).await;
        match next {
            Some(Ok(bytes)) => {
                if let Err(why) = writer.write_all(&bytes).await {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(problem(&format!("writing failed: {why}"))),
                    )
                        .into_response();
                }
                moved += bytes.len() as u64;
            }
            Some(Err(why)) => {
                // The browser stopped sending. What was written stays as it
                // is: a half file that says so by its size is better than one
                // this quietly deletes while somebody is still uploading it.
                return (
                    StatusCode::BAD_REQUEST,
                    Json(problem(&format!("the file did not arrive whole: {why}"))),
                )
                    .into_response();
            }
            None => break,
        }
    }

    if let Err(why) = session.finish_write(writer, None).await {
        return failed(&why);
    }
    (StatusCode::OK, Json(serde_json::json!({ "bytes": moved }))).into_response()
}

/// Percent-encodes a file name for a header.
fn encoded(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for byte in name.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(*byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// The refusal a request gets when it carries no session, or none.
fn refuse(shell: &Arc<Shell>, headers: &HeaderMap) -> Option<Response> {
    match token_of(headers) {
        Some(token) if shell.door.holds(&token) => None,
        _ => Some((StatusCode::UNAUTHORIZED, Json(problem("not signed in"))).into_response()),
    }
}

/// A core failure, in the shape the window already reads.
fn failed(why: &Error) -> Response {
    let body = serde_json::to_value(why).unwrap_or_else(|_| problem("something failed"));
    (StatusCode::BAD_REQUEST, Json(body)).into_response()
}
