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
use amberbeam_core::endpoint::Protocol;
use amberbeam_core::engine::Progress;
use amberbeam_core::error::{Error, PathProblem};
use amberbeam_core::events::{Event, Events, LogDirection};
use amberbeam_core::fs::EntryKind;
use amberbeam_core::queue::QueuedJob;
use amberbeam_core::registry::{Sessions, TransferRun, LOCAL};
use amberbeam_core::runner::Runner;
use amberbeam_core::sftp::{AuthMethod, ConnectParams, HostKeyDecision, SftpSession};
use amberbeam_core::transfer::ResumeMarker;
use amberbeam_core::transfer::{ConflictPolicy, JobState};

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

/// The `known_hosts` every test shares that is not about host keys.
///
/// Accepting the fingerprint costs a connection of its own, and doing that
/// twelve times over means two dozen logins in a couple of seconds. Servers
/// and port forwards both dislike that — see the note at the top about running
/// one at a time — so the acceptance happens once and the rest connect
/// straight away.
fn shared_known_hosts() -> PathBuf {
    std::env::temp_dir().join("amberbeam-known-hosts-shared")
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
        concurrency: 4,
        retries: 5,
        temporary_name: true,
    }
}

fn password() -> AuthMethod {
    AuthMethod::Password {
        password: password_value(),
    }
}

/// Retries a connection that never reached the server.
///
/// Only ever on [`Error::Unreachable`], and only in the tests. A container port
/// forward stops accepting for a moment after a burst of connections — the
/// server itself logs nothing, because nothing arrives — and that has no
/// bearing on whether the code is right. Every other failure is passed
/// straight through: retrying a wrong password until it works is how test
/// suites start lying.
async fn with_retry<F, Fut>(attempt: F) -> Result<SftpSession, Error>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<SftpSession, Error>>,
{
    for _ in 0..4 {
        match attempt().await {
            Err(Error::Unreachable { .. }) => {
                tokio::time::sleep(std::time::Duration::from_millis(400)).await;
            }
            other => return other,
        }
    }
    attempt().await
}

/// Connects using the shared `known_hosts`, accepting the key the first time.
///
/// A key that no longer matches — the test container was recreated and has new
/// host keys — throws the shared file away and starts over, rather than failing
/// every test with a changed-key error that says nothing about the code.
async fn connect_ready(
    host: &str,
    port: u16,
    method: AuthMethod,
    events: &Events,
) -> Result<SftpSession, Error> {
    let shared = shared_known_hosts();
    let endpoint = EndpointId::new("test");
    let known_only = params(
        host,
        port,
        method.clone(),
        &shared,
        HostKeyDecision::KnownOnly,
    );

    match with_retry(|| SftpSession::connect(&known_only, &endpoint, events)).await {
        Ok(session) => Ok(session),
        Err(Error::HostKeyChanged { .. }) => {
            let _ = std::fs::remove_file(&shared);
            connect_trusting(host, port, method, &shared, events).await
        }
        Err(Error::HostKeyUnknown { .. }) => {
            connect_trusting(host, port, method, &shared, events).await
        }
        Err(other) => Err(other),
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
    // Built once and borrowed: a set of parameters created inside the closure
    // would not outlive the future that borrows it.
    let blind = params(
        host,
        port,
        method.clone(),
        known_hosts,
        HostKeyDecision::KnownOnly,
    );
    let first = with_retry(|| SftpSession::connect(&blind, &endpoint, events)).await;

    let fingerprint = match first {
        Ok(session) => return Ok(session),
        Err(Error::HostKeyUnknown { fingerprint, .. }) => fingerprint,
        Err(other) => return Err(other),
    };

    let trusting = params(
        host,
        port,
        method,
        known_hosts,
        HostKeyDecision::Trust { fingerprint },
    );
    with_retry(|| SftpSession::connect(&trusting, &endpoint, events)).await
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

    let session = connect_trusting(&host, port, password(), &known_hosts, &events)
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
    let events = Events::new();

    let error = connect_ready(
        &host,
        port,
        AuthMethod::Password {
            password: "not the password".into(),
        },
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
    let events = Events::new();

    let session = connect_ready(
        &host,
        port,
        AuthMethod::KeyFile {
            path: key_path("client"),
            passphrase: None,
        },
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
    let events = Events::new();

    let error = connect_ready(
        &host,
        port,
        AuthMethod::KeyFile {
            path: key_path("client-locked"),
            passphrase: Some("wrong".into()),
        },
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
    let events = Events::new();
    let session = connect_ready(&host, port, password(), &events)
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
    session.disconnect().await;
}

#[tokio::test]
async fn a_directory_without_permission_says_so() {
    let (host, port) = server_or_skip!("a_directory_without_permission");
    if !has_fixture() {
        eprintln!("skipping: pointed at a server without this repository's fixture");
        return;
    }
    let events = Events::new();
    let session = connect_ready(&host, port, password(), &events)
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
    session.disconnect().await;
}

#[tokio::test]
async fn a_missing_directory_says_so() {
    let (host, port) = server_or_skip!("a_missing_directory");
    let events = Events::new();
    let session = connect_ready(&host, port, password(), &events)
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
    session.disconnect().await;
}

#[tokio::test]
async fn the_home_directory_comes_back_absolute() {
    let (host, port) = server_or_skip!("the_home_directory");
    let events = Events::new();
    let session = connect_ready(&host, port, password(), &events)
        .await
        .expect("connect");

    let home = session.home().await.expect("home");
    assert!(home.starts_with('/'), "a pane cannot start from {home:?}");
    session.disconnect().await;
}

#[tokio::test]
async fn the_server_log_fills_while_connecting() {
    let (host, port) = server_or_skip!("the_server_log_fills");
    let events = Events::new();
    let mut listener = events.subscribe();

    let session = connect_ready(&host, port, password(), &events)
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
    session.disconnect().await;
}

#[tokio::test]
async fn every_row_of_a_real_listing_is_complete() {
    // Runs against whatever server is named, fixture or not: pointing the
    // tests at a real server is the only way to find out what real servers
    // actually send, and SFTP implementations differ.
    let (host, port) = server_or_skip!("every_row_of_a_real_listing");
    let events = Events::new();
    let session = connect_ready(&host, port, password(), &events)
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
    session.disconnect().await;
}

#[tokio::test]
async fn folders_and_files_can_be_made_renamed_and_removed() {
    let (host, port) = server_or_skip!("folders_and_files");
    let events = Events::new();
    let session = connect_ready(&host, port, password(), &events)
        .await
        .expect("connect");

    // A working directory of its own, so a failed run cannot take anything
    // else with it.
    let base = format!("{}/amberbeam-ops", testdata());
    let _ = session.remove(&base).await;
    session
        .create_dir(&base)
        .await
        .expect("create the work directory");

    session
        .create_dir(&format!("{base}/images"))
        .await
        .expect("create a folder");
    session
        .create_file(&format!("{base}/notes.txt"))
        .await
        .expect("create a file");

    let listing = session.list_dir(&base).await.expect("list");
    assert_eq!(listing.entries.len(), 2);

    // Creating over something that exists must not quietly replace it.
    assert!(matches!(
        session.create_file(&format!("{base}/notes.txt")).await,
        Err(Error::Path {
            reason: PathProblem::AlreadyExists,
            ..
        })
    ));

    session
        .rename(&format!("{base}/notes.txt"), &format!("{base}/readme.txt"))
        .await
        .expect("rename");
    let listing = session.list_dir(&base).await.expect("list");
    assert!(listing.entries.iter().any(|e| e.name == "readme.txt"));
    assert!(!listing.entries.iter().any(|e| e.name == "notes.txt"));

    // Renaming onto an existing name is refused, both files survive.
    session
        .create_file(&format!("{base}/other.txt"))
        .await
        .expect("create");
    assert!(matches!(
        session
            .rename(&format!("{base}/other.txt"), &format!("{base}/readme.txt"))
            .await,
        Err(Error::Path {
            reason: PathProblem::AlreadyExists,
            ..
        })
    ));
    assert_eq!(
        session.list_dir(&base).await.expect("list").entries.len(),
        3
    );

    session.remove(&base).await.expect("remove the tree");
    assert!(matches!(
        session.list_dir(&base).await,
        Err(Error::Path {
            reason: PathProblem::NotFound,
            ..
        })
    ));
    session.disconnect().await;
}

#[tokio::test]
async fn a_tree_is_measured_and_then_removed_whole() {
    let (host, port) = server_or_skip!("a_tree_is_measured");
    let events = Events::new();
    let session = connect_ready(&host, port, password(), &events)
        .await
        .expect("connect");

    let base = format!("{}/amberbeam-tree", testdata());
    let _ = session.remove(&base).await;
    session.create_dir(&base).await.expect("create");
    session
        .create_dir(&format!("{base}/inner"))
        .await
        .expect("create");
    session
        .create_file(&format!("{base}/one.txt"))
        .await
        .expect("create");
    session
        .create_file(&format!("{base}/inner/two.txt"))
        .await
        .expect("create");

    let measured = session.measure(&base).await.expect("measure");
    assert_eq!(measured.files, 2);
    assert_eq!(measured.directories, 2, "the folder itself counts too");
    assert!(!measured.is_at_cap());

    // SFTP has no recursive delete; the tree has to be emptied from the
    // inside out, and a directory that is not empty cannot be removed.
    session.remove(&base).await.expect("remove");
    assert!(matches!(
        session.list_dir(&base).await,
        Err(Error::Path {
            reason: PathProblem::NotFound,
            ..
        })
    ));
    session.disconnect().await;
}

#[tokio::test]
async fn permissions_are_set_through_a_tree() {
    let (host, port) = server_or_skip!("permissions_are_set");
    let events = Events::new();
    let session = connect_ready(&host, port, password(), &events)
        .await
        .expect("connect");

    let base = format!("{}/amberbeam-chmod", testdata());
    let _ = session.remove(&base).await;
    session.create_dir(&base).await.expect("create");
    session
        .create_dir(&format!("{base}/inner"))
        .await
        .expect("create");
    session
        .create_file(&format!("{base}/inner/file.txt"))
        .await
        .expect("create");

    session
        .set_permissions(&base, 0o750, true)
        .await
        .expect("set permissions");

    let listing = session
        .list_dir(&format!("{base}/inner"))
        .await
        .expect("list");
    let file = listing
        .entries
        .iter()
        .find(|e| e.name == "file.txt")
        .expect("the file");
    assert_eq!(
        file.permissions.map(|p| p.to_rwx()),
        Some("rwxr-x---".into())
    );

    // And not recursively, when not asked for.
    session
        .set_permissions(&format!("{base}/inner"), 0o700, false)
        .await
        .expect("set permissions");
    let listing = session
        .list_dir(&format!("{base}/inner"))
        .await
        .expect("list");
    let file = listing
        .entries
        .iter()
        .find(|e| e.name == "file.txt")
        .expect("the file");
    assert_eq!(
        file.permissions.map(|p| p.to_rwx()),
        Some("rwxr-x---".into()),
        "a single change must not reach into the folder"
    );

    session.remove(&base).await.expect("clean up");
    session.disconnect().await;
}

/// A registry with the local disk and one server, the way the window has it.
async fn two_endpoints(host: &str, port: u16, events: &Events) -> Option<(Sessions, EndpointId)> {
    let sessions = Sessions::new(events.clone());
    let server = EndpointId::new("server");
    let shared = shared_known_hosts();

    // The acceptance dance once, then hand the same decision to the registry.
    let probe = connect_ready(host, port, password(), events).await.ok()?;
    probe.disconnect().await;

    let mut params = params(host, port, password(), &shared, HostKeyDecision::KnownOnly);
    params.concurrency = 4;
    sessions.connect_sftp(&server, &params).await.ok()?;
    Some((sessions, server))
}

fn scratch_file(name: &str, contents: &[u8]) -> String {
    let path = std::env::temp_dir().join(format!("amberbeam-transfer-{name}"));
    std::fs::write(&path, contents).expect("write scratch file");
    path.to_string_lossy().into_owned()
}

#[tokio::test]
async fn a_file_travels_to_the_server_and_back() {
    let (host, port) = server_or_skip!("a_file_travels");
    let events = Events::new();
    let Some((sessions, server)) = two_endpoints(&host, port, &events).await else {
        panic!("could not open both endpoints");
    };
    let local = EndpointId::new(LOCAL);

    let contents = b"0123456789".repeat(5_000); // 50 kB, more than one chunk
    let source = scratch_file("up.bin", &contents);
    let remote = format!("{}/amberbeam-transfer.bin", testdata());
    let back = scratch_file("down.bin", b"");

    // Up.
    let progress = Progress::new(None, 0);
    let up = TransferRun {
        source_endpoint: local.clone(),
        source_path: source.clone(),
        target_endpoint: server.clone(),
        target_path: remote.clone(),
        resume: None,
        keep_modified: true,
        keep_permissions: false,
        source_permissions: None,
        use_temporary_name: true,
    };
    let done = sessions.transfer(&up, &progress).await.expect("upload");
    assert!(done.complete);
    assert_eq!(progress.done(), contents.len() as u64);

    // The partial name must be gone: a finished file wears its own name.
    let listing = sessions.list_dir(&server, &testdata()).await.expect("list");
    assert!(!listing.entries.iter().any(|e| e.name.ends_with(".ampart")));
    let uploaded = listing
        .entries
        .iter()
        .find(|e| e.name == "amberbeam-transfer.bin")
        .expect("the uploaded file");
    assert_eq!(uploaded.size, Some(contents.len() as u64));

    // And back down.
    let progress = Progress::new(None, 0);
    let down = TransferRun {
        source_endpoint: server.clone(),
        source_path: remote.clone(),
        target_endpoint: local.clone(),
        target_path: back.clone(),
        resume: None,
        keep_modified: true,
        keep_permissions: false,
        source_permissions: None,
        use_temporary_name: true,
    };
    sessions.transfer(&down, &progress).await.expect("download");
    assert_eq!(std::fs::read(&back).expect("read back"), contents);

    sessions.remove(&server, &remote).await.expect("clean up");
    let _ = std::fs::remove_file(&source);
    let _ = std::fs::remove_file(&back);
}

#[tokio::test]
async fn a_broken_download_carries_on_where_it_stopped() {
    let (host, port) = server_or_skip!("a_broken_download");
    let events = Events::new();
    let Some((sessions, server)) = two_endpoints(&host, port, &events).await else {
        panic!("could not open both endpoints");
    };
    let local = EndpointId::new(LOCAL);

    // Distinct bytes, so a wrong offset shows up as wrong content rather than
    // merely as a wrong length.
    let contents: Vec<u8> = (0..200_000_u32).map(|index| (index % 251) as u8).collect();
    let source = scratch_file("resume-source.bin", &contents);
    let remote = format!("{}/amberbeam-resume.bin", testdata());
    let target = scratch_file("resume-target.bin", b"");

    let run = TransferRun {
        source_endpoint: local.clone(),
        source_path: source.clone(),
        target_endpoint: server.clone(),
        target_path: remote.clone(),
        resume: None,
        keep_modified: true,
        keep_permissions: false,
        source_permissions: None,
        use_temporary_name: true,
    };
    sessions
        .transfer(&run, &Progress::new(None, 0))
        .await
        .expect("upload");

    // The state a broken download leaves behind, built by hand rather than by
    // racing a cancellation: the first part in the partial file, and a marker
    // describing the source as it was. Cancelling mid-flight is covered by the
    // engine's own tests, where no network can decide the timing.
    let offset = 70_000_u64;
    let session = sessions.find(&server).await.expect("session");
    let (size, modified) = session.stat(&remote).await.expect("stat");
    std::fs::write(format!("{target}.ampart"), &contents[..offset as usize]).expect("partial");

    let progress = Progress::new(None, offset);
    let rest = TransferRun {
        source_endpoint: server.clone(),
        source_path: remote.clone(),
        target_endpoint: local.clone(),
        target_path: target.clone(),
        resume: Some(ResumeMarker {
            offset,
            source_size: size,
            source_modified: modified,
        }),
        keep_modified: true,
        keep_permissions: false,
        source_permissions: None,
        use_temporary_name: true,
    };
    let finished = sessions.transfer(&rest, &progress).await.expect("resume");

    assert!(finished.complete);
    // The point of the whole exercise: only what was missing crossed the wire.
    assert_eq!(
        finished.moved,
        contents.len() as u64 - offset,
        "a resumed transfer moves the remainder, not the file again"
    );
    assert_eq!(
        std::fs::read(&target).expect("read"),
        contents,
        "the file has to be whole and identical, not merely the right length"
    );
    assert!(
        !std::path::Path::new(&format!("{target}.ampart")).exists(),
        "the partial name is gone once the file is whole"
    );
    assert_eq!(progress.done(), contents.len() as u64);

    sessions.remove(&server, &remote).await.expect("clean up");
    let _ = std::fs::remove_file(&source);
    let _ = std::fs::remove_file(&target);
}

#[tokio::test]
async fn a_cancelled_transfer_reports_where_to_pick_up() {
    let (host, port) = server_or_skip!("a_cancelled_transfer");
    let events = Events::new();
    let Some((sessions, server)) = two_endpoints(&host, port, &events).await else {
        panic!("could not open both endpoints");
    };
    let local = EndpointId::new(LOCAL);

    let source = scratch_file("cancel.bin", &b"y".repeat(50_000));
    let remote = format!("{}/amberbeam-cancel.bin", testdata());

    // Cancelled before a single chunk moves, which is the one moment the test
    // can name exactly.
    let progress = Progress::new(None, 0);
    progress.cancel();

    let run = TransferRun {
        source_endpoint: local.clone(),
        source_path: source.clone(),
        target_endpoint: server.clone(),
        target_path: remote.clone(),
        resume: None,
        keep_modified: false,
        keep_permissions: false,
        source_permissions: None,
        use_temporary_name: true,
    };
    let stopped = sessions.transfer(&run, &progress).await.expect("cancelled");

    assert!(!stopped.complete);
    let marker = stopped.resume.expect("a place to pick up from");
    assert_eq!(marker.offset, 0);
    assert_eq!(marker.source_size, 50_000);

    // Nothing under the final name: the file was never whole.
    let listing = sessions.list_dir(&server, &testdata()).await.expect("list");
    assert!(!listing
        .entries
        .iter()
        .any(|entry| entry.name == "amberbeam-cancel.bin"));

    let _ = sessions.remove(&server, &format!("{remote}.ampart")).await;
    let _ = std::fs::remove_file(&source);
}

#[tokio::test]
async fn a_source_that_changed_refuses_to_be_continued() {
    let (host, port) = server_or_skip!("a_source_that_changed");
    let events = Events::new();
    let Some((sessions, server)) = two_endpoints(&host, port, &events).await else {
        panic!("could not open both endpoints");
    };
    let local = EndpointId::new(LOCAL);

    let source = scratch_file("changed.bin", &b"a".repeat(10_000));
    let remote = format!("{}/amberbeam-changed.bin", testdata());

    let run = TransferRun {
        source_endpoint: local.clone(),
        source_path: source.clone(),
        target_endpoint: server.clone(),
        target_path: remote.clone(),
        // A marker from an earlier attempt, describing a source that no longer
        // matches what is on disk.
        resume: Some(ResumeMarker {
            offset: 4_000,
            source_size: 9_999,
            source_modified: Some(1),
        }),
        keep_modified: true,
        keep_permissions: false,
        source_permissions: None,
        use_temporary_name: true,
    };

    let error = sessions
        .transfer(&run, &Progress::new(None, 0))
        .await
        .expect_err("a changed source must not be appended to");
    assert!(
        matches!(error, Error::SourceChanged),
        "expected the source to be refused, got {error:?}"
    );

    let _ = sessions.remove(&server, &remote).await;
    let _ = std::fs::remove_file(&source);
}

#[tokio::test]
async fn the_connection_carries_several_transfers_at_once() {
    let (host, port) = server_or_skip!("several_transfers");
    let events = Events::new();
    let Some((sessions, server)) = two_endpoints(&host, port, &events).await else {
        panic!("could not open both endpoints");
    };
    let local = EndpointId::new(LOCAL);
    let sessions = std::sync::Arc::new(sessions);

    assert_eq!(
        sessions.find(&server).await.expect("session").protocol(),
        Protocol::Sftp
    );

    let mut running = Vec::new();
    for index in 0..4 {
        let sessions = std::sync::Arc::clone(&sessions);
        let local = local.clone();
        let server = server.clone();
        let source = scratch_file(&format!("parallel-{index}.bin"), &b"x".repeat(40_000));
        let remote = format!("{}/amberbeam-parallel-{index}.bin", testdata());
        running.push(tokio::spawn(async move {
            let run = TransferRun {
                source_endpoint: local,
                source_path: source.clone(),
                target_endpoint: server,
                target_path: remote.clone(),
                resume: None,
                keep_modified: false,
                keep_permissions: false,
                source_permissions: None,
                use_temporary_name: true,
            };
            let done = sessions
                .transfer(&run, &Progress::new(None, 0))
                .await
                .expect("parallel upload");
            let _ = std::fs::remove_file(&source);
            (done.complete, remote)
        }));
    }

    for task in running {
        let (complete, remote) = task.await.expect("task");
        assert!(complete);
        sessions.remove(&server, &remote).await.expect("clean up");
    }
}

#[tokio::test]
async fn the_temporary_name_can_be_switched_off() {
    let (host, port) = server_or_skip!("temporary_name_off");
    let events = Events::new();
    let Some((sessions, server)) = two_endpoints(&host, port, &events).await else {
        panic!("could not open both endpoints");
    };
    let local = EndpointId::new(LOCAL);

    let source = scratch_file("direct.bin", &b"z".repeat(20_000));
    let remote = format!("{}/amberbeam-direct.bin", testdata());

    // Cancelled before anything moves, with the temporary name switched off:
    // whatever exists on the server afterwards wears the final name.
    let progress = Progress::new(None, 0);
    progress.cancel();
    let run = TransferRun {
        source_endpoint: local.clone(),
        source_path: source.clone(),
        target_endpoint: server.clone(),
        target_path: remote.clone(),
        resume: None,
        keep_modified: false,
        keep_permissions: false,
        source_permissions: None,
        use_temporary_name: false,
    };
    sessions.transfer(&run, &progress).await.expect("cancelled");

    let listing = sessions.list_dir(&server, &testdata()).await.expect("list");
    assert!(
        listing
            .entries
            .iter()
            .any(|entry| entry.name == "amberbeam-direct.bin"),
        "without the temporary name the file is created under its own name"
    );
    assert!(
        !listing
            .entries
            .iter()
            .any(|entry| entry.name.ends_with(".ampart")),
        "and no temporary name is left behind"
    );

    let _ = sessions.remove(&server, &remote).await;
    let _ = std::fs::remove_file(&source);
}

#[tokio::test]
async fn the_queue_works_through_several_files_and_survives_a_restart() {
    let (host, port) = server_or_skip!("the_queue_works_through");
    let events = Events::new();
    let Some((sessions, server)) = two_endpoints(&host, port, &events).await else {
        panic!("could not open both endpoints");
    };
    let local = EndpointId::new(LOCAL);
    let sessions = std::sync::Arc::new(sessions);

    let queue_file = std::env::temp_dir().join("amberbeam-queue-live.json");
    let _ = std::fs::remove_file(&queue_file);
    let runner = Runner::new(
        std::sync::Arc::clone(&sessions),
        events.clone(),
        queue_file.clone(),
    );

    let mut names = Vec::new();
    let mut jobs = Vec::new();
    for index in 0..6 {
        let name = format!("amberbeam-queued-{index}.bin");
        let source = scratch_file(&name, &b"q".repeat(30_000));
        let target = format!("{}/{name}", testdata());
        names.push((source.clone(), target.clone()));
        jobs.push(QueuedJob {
            id: format!("job-{index}"),
            source_endpoint: local.clone(),
            source_path: source,
            target_endpoint: server.clone(),
            target_path: target,
            name: name.clone(),
            state: JobState::Queued,
            conflict_policy: ConflictPolicy::Overwrite,
            total_bytes: Some(30_000),
            done_bytes: 0,
            resume: None,
            keep_modified: true,
            keep_permissions: false,
            use_temporary_name: true,
            attempts: 0,
            retries: Some(3),
            failure: None,
            added: index,
        });
    }
    runner.add_all(jobs).await;

    // Wait for the queue to empty, but never forever: a test that hangs says
    // less than one that fails.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        let totals = runner.totals().await;
        if totals.done_jobs == 6 {
            break;
        }
        assert!(
            totals.failed_jobs == 0,
            "a job failed: {:?}",
            runner
                .snapshot()
                .await
                .jobs
                .iter()
                .map(|job| (job.name.clone(), job.state, job.failure.clone()))
                .collect::<Vec<_>>()
        );
        assert!(
            std::time::Instant::now() < deadline,
            "the queue did not finish"
        );
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    let listing = sessions.list_dir(&server, &testdata()).await.expect("list");
    for index in 0..6 {
        let name = format!("amberbeam-queued-{index}.bin");
        let entry = listing
            .entries
            .iter()
            .find(|entry| entry.name == name)
            .unwrap_or_else(|| panic!("{name} never arrived"));
        assert_eq!(entry.size, Some(30_000));
    }
    assert!(
        !listing.entries.iter().any(|e| e.name.ends_with(".ampart")),
        "no temporary names are left behind"
    );

    // The queue on disk is the queue in the window.
    let reopened = amberbeam_core::queue::load(&queue_file);
    assert_eq!(reopened.jobs.len(), 6);
    assert!(reopened.jobs.iter().all(|job| job.state == JobState::Done));

    for (source, target) in names {
        let _ = std::fs::remove_file(&source);
        sessions.remove(&server, &target).await.expect("clean up");
    }
    let _ = std::fs::remove_file(&queue_file);
}

#[tokio::test]
async fn a_held_job_keeps_its_place_and_its_offset() {
    let (host, port) = server_or_skip!("a_held_job");
    let events = Events::new();
    let Some((sessions, server)) = two_endpoints(&host, port, &events).await else {
        panic!("could not open both endpoints");
    };
    let local = EndpointId::new(LOCAL);
    let sessions = std::sync::Arc::new(sessions);

    let queue_file = std::env::temp_dir().join("amberbeam-queue-hold.json");
    let _ = std::fs::remove_file(&queue_file);
    let runner = Runner::new(
        std::sync::Arc::clone(&sessions),
        events.clone(),
        queue_file.clone(),
    );
    // Nothing starts while the queue is held, which is what makes the state
    // observable at all.
    runner.set_paused(true).await;

    let source = scratch_file("held.bin", &b"h".repeat(10_000));
    let target = format!("{}/amberbeam-held.bin", testdata());
    runner
        .add(QueuedJob {
            id: "held".into(),
            source_endpoint: local.clone(),
            source_path: source.clone(),
            target_endpoint: server.clone(),
            target_path: target.clone(),
            name: "held.bin".into(),
            state: JobState::Queued,
            conflict_policy: ConflictPolicy::Overwrite,
            total_bytes: Some(10_000),
            done_bytes: 0,
            resume: None,
            keep_modified: false,
            keep_permissions: false,
            use_temporary_name: true,
            attempts: 0,
            retries: Some(3),
            failure: None,
            added: 0,
        })
        .await;

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let totals = runner.totals().await;
    assert_eq!(totals.waiting_jobs, 1, "a held queue starts nothing");
    assert_eq!(totals.running_jobs, 0);

    // Let it go, and it finishes.
    runner.set_paused(false).await;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while runner.totals().await.done_jobs == 0 {
        assert!(std::time::Instant::now() < deadline, "nothing happened");
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    sessions.remove(&server, &target).await.expect("clean up");
    let _ = std::fs::remove_file(&source);
    let _ = std::fs::remove_file(&queue_file);
}
