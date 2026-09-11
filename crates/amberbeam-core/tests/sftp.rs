//! SFTP against a real server.
//!
//! Start one with `dev/test-sftp-server.sh start`, then
//!
//! ```sh
//! AMBERBEAM_TEST_SFTP=127.0.0.1:2222 \
//!   cargo test -p amberbeam-core --test sftp -- --test-threads=1
//! ```
//!
//! **One at a time on purpose.** Cargo runs tests in parallel, and a dozen
//! logins arriving together walk straight into sshd's `MaxStartups`, which
//! drops connections — the failure then looks like "server unreachable" and
//! has nothing to do with the code. It is also a small demonstration of why
//! the queue in M2 has a per-server limit at all.
//!
//! Another server can be used instead:
//!
//! ```sh
//! AMBERBEAM_TEST_SFTP=192.168.0.10:2222 AMBERBEAM_TEST_USER=someone \
//!   AMBERBEAM_TEST_PASSWORD=secret AMBERBEAM_TEST_PATH=/upload \
//!   cargo test -p amberbeam-core --test sftp -- --test-threads=1
//! ```
//!
//! Tests that need this repository's own fixture or its test key skip
//! themselves when `AMBERBEAM_TEST_PATH` says the server is somebody else's.
//!
//! Without that variable every test here reports itself skipped and passes:
//! `cargo test` has to work on a machine with no container runtime, or nobody
//! will run it.
//!
//! Each test uses a `known_hosts` file of its own in a temporary directory.
//! Tests that write into the developer's real `~/.ssh/known_hosts` would be a
//! nasty surprise, and the ability to point somewhere else is needed by the
//! container build anyway.

use std::path::{Path, PathBuf};

use amberbeam_core::endpoint::EndpointId;
use amberbeam_core::error::{Error, PathProblem};
use amberbeam_core::events::{Event, Events, LogDirection};
use amberbeam_core::fs::EntryKind;
use amberbeam_core::sftp::{AuthMethod, ConnectParams, HostKeyDecision, SftpSession};

/// Defaults match the server `dev/test-sftp-server.sh` starts. Overridable, so
/// the same tests can be pointed at a real server elsewhere — which is worth
/// doing, because a container of one's own agrees with one's own assumptions.
fn user() -> String {
    std::env::var("AMBERBEAM_TEST_USER").unwrap_or_else(|_| "amberbeam".into())
}

fn password_value() -> String {
    std::env::var("AMBERBEAM_TEST_PASSWORD").unwrap_or_else(|_| "tannenbaum".into())
}

fn testdata() -> String {
    std::env::var("AMBERBEAM_TEST_PATH").unwrap_or_else(|_| "/config/testdata".into())
}

/// Tests that need what `dev/test-sftp-server.sh` sets up — the fixture
/// directory and the client key in `authorized_keys` — are skipped when
/// pointed at somebody else's server, which has neither.
fn has_fixture() -> bool {
    std::env::var("AMBERBEAM_TEST_PATH").is_err()
}

/// `Some((host, port))` when a server was named, otherwise a printed note.
fn server() -> Option<(String, u16)> {
    let address = std::env::var("AMBERBEAM_TEST_SFTP").ok()?;
    let (host, port) = address.rsplit_once(':')?;
    Some((host.to_string(), port.parse().ok()?))
}

macro_rules! server_or_skip {
    ($name:expr) => {
        match server() {
            Some(server) => server,
            None => {
                eprintln!("skipping {}: AMBERBEAM_TEST_SFTP is not set", $name);
                return;
            }
        }
    };
}

/// A `known_hosts` file nobody else touches.
fn scratch_known_hosts(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("amberbeam-known-hosts-{name}"));
    let _ = std::fs::remove_file(&path);
    path
}

fn key_path(name: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../dev/sftp-test/keys")
        .join(name)
        .canonicalize()
        .expect("test key exists — run dev/test-sftp-server.sh start")
        .to_string_lossy()
        .into_owned()
}

fn params(
    host: &str,
    port: u16,
    method: AuthMethod,
    known_hosts: &Path,
    host_key: HostKeyDecision,
) -> ConnectParams {
    ConnectParams {
        host: host.to_string(),
        port,
        user: user(),
        method,
        host_key,
        known_hosts: Some(known_hosts.to_string_lossy().into_owned()),
    }
}

fn password() -> AuthMethod {
    AuthMethod::Password {
        password: password_value(),
    }
}

/// Connects the way the window does: first attempt, accept the fingerprint,
/// second attempt.
async fn connect_trusting(
    host: &str,
    port: u16,
    method: AuthMethod,
    known_hosts: &Path,
    events: &Events,
) -> Result<SftpSession, Error> {
    let endpoint = EndpointId::new("test");
    let first = SftpSession::connect(
        &params(
            host,
            port,
            method.clone(),
            known_hosts,
            HostKeyDecision::KnownOnly,
        ),
        &endpoint,
        events,
    )
    .await;

    let fingerprint = match first {
        Ok(session) => return Ok(session),
        Err(Error::HostKeyUnknown { fingerprint, .. }) => fingerprint,
        Err(other) => return Err(other),
    };

    SftpSession::connect(
        &params(
            host,
            port,
            method,
            known_hosts,
            HostKeyDecision::Trust { fingerprint },
        ),
        &endpoint,
        events,
    )
    .await
}

#[tokio::test]
async fn an_unknown_server_is_refused_and_its_fingerprint_reported() {
    let (host, port) = server_or_skip!("an_unknown_server_is_refused");
    let known_hosts = scratch_known_hosts("unknown");
    let events = Events::new();

    let error = SftpSession::connect(
        &params(
            &host,
            port,
            password(),
            &known_hosts,
            HostKeyDecision::KnownOnly,
        ),
        &EndpointId::new("test"),
        &events,
    )
    .await
    .expect_err("an unrecorded server must not connect on its own");

    match error {
        Error::HostKeyUnknown { fingerprint, .. } => {
            assert!(fingerprint.starts_with("SHA256:"), "{fingerprint}");
        }
        other => panic!("expected an unknown host key, got {other:?}"),
    }
    // Nothing may have been written: the user never saw a fingerprint.
    assert!(!known_hosts.exists());
}

#[tokio::test]
async fn accepting_the_fingerprint_connects_and_is_remembered() {
    let (host, port) = server_or_skip!("accepting_the_fingerprint");
    let known_hosts = scratch_known_hosts("accept");
    let events = Events::new();

    let mut session = connect_trusting(&host, port, password(), &known_hosts, &events)
        .await
        .expect("connect after accepting the fingerprint");
    session.disconnect().await;

    assert!(known_hosts.exists(), "the accepted key has to be recorded");

    // And from now on it connects without anybody being asked again.
    let again = SftpSession::connect(
        &params(
            &host,
            port,
            password(),
            &known_hosts,
            HostKeyDecision::KnownOnly,
        ),
        &EndpointId::new("test"),
        &events,
    )
    .await;
    assert!(again.is_ok(), "a recorded server must connect unattended");
}

#[tokio::test]
async fn a_changed_server_key_blocks_the_connection() {
    let (host, port) = server_or_skip!("a_changed_server_key");
    let known_hosts = scratch_known_hosts("changed");
    // A key on file that the server cannot possibly present.
    std::fs::write(
        &known_hosts,
        format!(
            "[{host}]:{port} ssh-ed25519 \
             AAAAC3NzaC1lZDI1NTE5AAAAIMzoeu1zYAjIT/oNaHRbkjXLzDRdplZkXUDe8T93J3tZ\n"
        ),
    )
    .expect("write known_hosts");

    let events = Events::new();
    let error = SftpSession::connect(
        &params(
            &host,
            port,
            password(),
            &known_hosts,
            HostKeyDecision::KnownOnly,
        ),
        &EndpointId::new("test"),
        &events,
    )
    .await
    .expect_err("a changed key must not connect");

    assert!(
        matches!(error, Error::HostKeyChanged { .. }),
        "expected a changed host key, got {error:?}"
    );
}

#[tokio::test]
async fn a_wrong_password_is_reported_as_such() {
    let (host, port) = server_or_skip!("a_wrong_password");
    let known_hosts = scratch_known_hosts("wrong-password");
    let events = Events::new();

    let error = connect_trusting(
        &host,
        port,
        AuthMethod::Password {
            password: "not the password".into(),
        },
        &known_hosts,
        &events,
    )
    .await
    .expect_err("a wrong password must not connect");

    assert!(
        matches!(error, Error::AuthenticationFailed { .. }),
        "got {error:?}"
    );
}

#[tokio::test]
async fn a_key_file_connects() {
    let (host, port) = server_or_skip!("a_key_file_connects");
    if !has_fixture() {
        eprintln!("skipping: this repository's test key is not on that server");
        return;
    }
    let known_hosts = scratch_known_hosts("key-file");
    let events = Events::new();

    let mut session = connect_trusting(
        &host,
        port,
        AuthMethod::KeyFile {
            path: key_path("client"),
            passphrase: None,
        },
        &known_hosts,
        &events,
    )
    .await
    .expect("connect with a key file");
    session.disconnect().await;
}

#[tokio::test]
async fn an_encrypted_key_needs_its_passphrase() {
    let (host, port) = server_or_skip!("an_encrypted_key");
    if !has_fixture() {
        eprintln!("skipping: this repository's test key is not on that server");
        return;
    }
    let known_hosts = scratch_known_hosts("locked-key");
    let events = Events::new();

    let error = connect_trusting(
        &host,
        port,
        AuthMethod::KeyFile {
            path: key_path("client-locked"),
            passphrase: Some("wrong".into()),
        },
        &known_hosts,
        &events,
    )
    .await
    .expect_err("a wrong passphrase must not connect");

    // The difference matters: a wrong passphrase means type again, a broken
    // file means pick another one.
    assert!(
        matches!(error, Error::KeyPassphrase { .. }),
        "expected a passphrase problem, got {error:?}"
    );
}

#[tokio::test]
async fn a_listing_carries_everything_the_panes_show() {
    let (host, port) = server_or_skip!("a_listing_carries_everything");
    if !has_fixture() {
        eprintln!("skipping: pointed at a server without this repository's fixture");
        return;
    }
    let known_hosts = scratch_known_hosts("listing");
    let events = Events::new();
    let session = connect_trusting(&host, port, password(), &known_hosts, &events)
        .await
        .expect("connect");

    let listing = session.list_dir(&testdata()).await.expect("list testdata");
    let by_name = |name: &str| {
        listing
            .entries
            .iter()
            .find(|entry| entry.name == name)
            .unwrap_or_else(|| panic!("{name} missing from the listing"))
    };

    // "." and ".." are noise in a pane and must not arrive.
    assert!(!listing
        .entries
        .iter()
        .any(|e| e.name == "." || e.name == ".."));

    let file = by_name("index.html");
    assert_eq!(file.kind, EntryKind::File);
    assert_eq!(file.size, Some(15));
    assert!(file.modified.is_some());
    assert_eq!(
        file.permissions.map(|p| p.to_rwx()),
        Some("rw-r--r--".into())
    );
    assert!(file.owner.is_some(), "the owner column needs a value");

    let directory = by_name("images");
    assert!(directory.is_directory());

    // A name with spaces, an ampersand and an umlaut. Not exotic on a web
    // server, and a client that mangles it is useless.
    let awkward = by_name("Größe & Maß.txt");
    assert_eq!(awkward.kind, EntryKind::File);
    assert!(listing.entries.iter().any(|e| e.name == "with space.txt"));

    // Hidden files are listed; whether they are shown is the pane's business.
    assert!(by_name(".hidden").is_hidden());

    let link = by_name("current");
    assert_eq!(link.kind, EntryKind::Symlink);
    assert!(
        link.is_directory(),
        "a link into a directory can be entered"
    );
    assert!(link.link_target.is_some());

    let broken = by_name("broken");
    assert_eq!(broken.kind, EntryKind::Symlink);
    assert!(!broken.is_directory(), "a link into nothing leads nowhere");

    assert_eq!(
        by_name("secret.txt").permissions.map(|p| p.to_rwx()),
        Some("rw-------".into())
    );
}

#[tokio::test]
async fn a_directory_without_permission_says_so() {
    let (host, port) = server_or_skip!("a_directory_without_permission");
    if !has_fixture() {
        eprintln!("skipping: pointed at a server without this repository's fixture");
        return;
    }
    let known_hosts = scratch_known_hosts("denied");
    let events = Events::new();
    let session = connect_trusting(&host, port, password(), &known_hosts, &events)
        .await
        .expect("connect");

    let error = session
        .list_dir(&format!("{}/locked", testdata()))
        .await
        .expect_err("a directory with mode 000 cannot be read");
    assert!(
        matches!(
            error,
            Error::Path {
                reason: PathProblem::PermissionDenied,
                ..
            }
        ),
        "got {error:?}"
    );
}

#[tokio::test]
async fn a_missing_directory_says_so() {
    let (host, port) = server_or_skip!("a_missing_directory");
    let known_hosts = scratch_known_hosts("missing");
    let events = Events::new();
    let session = connect_trusting(&host, port, password(), &known_hosts, &events)
        .await
        .expect("connect");

    let error = session
        .list_dir("/definitely/not/here")
        .await
        .expect_err("must fail");
    assert!(
        matches!(
            error,
            Error::Path {
                reason: PathProblem::NotFound,
                ..
            }
        ),
        "got {error:?}"
    );
}

#[tokio::test]
async fn the_home_directory_comes_back_absolute() {
    let (host, port) = server_or_skip!("the_home_directory");
    let known_hosts = scratch_known_hosts("home");
    let events = Events::new();
    let session = connect_trusting(&host, port, password(), &known_hosts, &events)
        .await
        .expect("connect");

    let home = session.home().await.expect("home");
    assert!(home.starts_with('/'), "a pane cannot start from {home:?}");
}

#[tokio::test]
async fn the_server_log_fills_while_connecting() {
    let (host, port) = server_or_skip!("the_server_log_fills");
    let known_hosts = scratch_known_hosts("log");
    let events = Events::new();
    let mut listener = events.subscribe();

    let session = connect_trusting(&host, port, password(), &known_hosts, &events)
        .await
        .expect("connect");
    session.list_dir(&testdata()).await.expect("list");

    let mut lines = Vec::new();
    while let Ok(event) = listener.try_recv() {
        if let Event::Log {
            direction, text, ..
        } = event
        {
            lines.push((direction, text));
        }
    }

    assert!(
        lines.iter().any(|(_, text)| text.contains("connecting to")),
        "the log has to show the attempt: {lines:?}"
    );
    assert!(
        lines
            .iter()
            .any(|(direction, text)| *direction == LogDirection::Sent && text.contains("opendir")),
        "the log has to show the listing: {lines:?}"
    );
}

#[tokio::test]
async fn every_row_of_a_real_listing_is_complete() {
    // Runs against whatever server is named, fixture or not: pointing the
    // tests at a real server is the only way to find out what real servers
    // actually send, and SFTP implementations differ.
    let (host, port) = server_or_skip!("every_row_of_a_real_listing");
    let known_hosts = scratch_known_hosts("real-listing");
    let events = Events::new();
    let session = connect_trusting(&host, port, password(), &known_hosts, &events)
        .await
        .expect("connect");

    let listing = session.list_dir(&testdata()).await.expect("list");
    eprintln!("{} holds {} entries", listing.path, listing.entries.len());

    assert!(
        listing.path.starts_with('/'),
        "the pane needs an absolute path"
    );
    for entry in &listing.entries {
        assert!(
            !entry.name.is_empty(),
            "a row without a name cannot be drawn"
        );
        assert!(
            !entry.name.contains('/'),
            "a name is never a path: {}",
            entry.name
        );
        assert_ne!(entry.name, ".");
        assert_ne!(entry.name, "..");
        if entry.kind == EntryKind::File {
            // Without a size the queue of M2 cannot show progress, and a
            // resume cannot check whether the source changed.
            assert!(
                entry.size.is_some(),
                "{} arrived without a size",
                entry.name
            );
        }
    }
}
