//! FTP against a real server.
//!
//! ```sh
//! AMBERBEAM_TEST_FTP=127.0.0.1:2121 AMBERBEAM_TEST_FTP_USER=someone \
//!   AMBERBEAM_TEST_FTP_PASSWORD=secret \
//!   cargo test -p amberbeam-core --test ftp -- --test-threads=1
//! ```
//!
//! Without that variable every test reports itself skipped and passes. One at
//! a time for the same reason as the SFTP tests: every simultaneous FTP
//! transfer is a separate login, and a burst of them is what a server's limit
//! is there to stop.

use amberbeam_core::endpoint::EndpointId;
use amberbeam_core::events::Events;
use amberbeam_core::ftp::tls::{CertificateDecision, CertificateProblem};
use amberbeam_core::ftp::{Encryption, FtpParams, FtpSession};

fn server() -> Option<(String, u16)> {
    let address = std::env::var("AMBERBEAM_TEST_FTP").ok()?;
    let (host, port) = address.rsplit_once(':')?;
    Some((host.to_string(), port.parse().ok()?))
}

macro_rules! server_or_skip {
    ($name:expr) => {
        match server() {
            Some(server) => server,
            None => {
                eprintln!("skipping {}: AMBERBEAM_TEST_FTP is not set", $name);
                return;
            }
        }
    };
}

fn params(host: &str, port: u16, encryption: Encryption) -> FtpParams {
    FtpParams {
        host: host.to_string(),
        port,
        user: std::env::var("AMBERBEAM_TEST_FTP_USER").unwrap_or_else(|_| "anonymous".into()),
        password: std::env::var("AMBERBEAM_TEST_FTP_PASSWORD").unwrap_or_default(),
        encryption,
        passive: true,
        concurrency: 2,
        retries: 3,
        temporary_name: false,
        keep_alive: None,
        latin1: false,
        certificate: CertificateDecision::TrustedOnly,
    }
}

#[tokio::test]
async fn plain_ftp_connects_and_says_what_it_can_do() {
    let (host, port) = server_or_skip!("plain_ftp_connects");
    let events = Events::new();
    let session = FtpSession::connect(
        &params(&host, port, Encryption::None),
        &EndpointId::new("ftp"),
        &events,
    )
    .await
    .expect("connect");

    let abilities = session.abilities();
    eprintln!(
        "MLSD {}, REST STREAM {}, UTF8 {}, SIZE {}",
        abilities.mlsd, abilities.rest, abilities.utf8, abilities.size
    );

    let home = session.home().await.expect("pwd");
    assert!(home.starts_with('/'), "a pane cannot start from {home:?}");
}

#[tokio::test]
async fn a_listing_comes_back_with_usable_rows() {
    let (host, port) = server_or_skip!("a_listing_comes_back");
    let events = Events::new();
    let session = FtpSession::connect(
        &params(&host, port, Encryption::None),
        &EndpointId::new("ftp"),
        &events,
    )
    .await
    .expect("connect");

    let home = session.home().await.expect("pwd");
    let listing = session.list_dir(&home).await.expect("list");
    eprintln!("{} holds {} entries", listing.path, listing.entries.len());

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
        // Whatever the dialect, a listing that answers nothing about dates
        // would break the comparison that guards a resumed transfer.
        if entry.kind == amberbeam_core::fs::EntryKind::File {
            assert!(
                entry.modified.is_some(),
                "{} arrived without a timestamp",
                entry.name
            );
        }
    }
}

#[tokio::test]
async fn several_logins_share_one_session() {
    let (host, port) = server_or_skip!("several_logins");
    let events = Events::new();
    let session = FtpSession::connect(
        &params(&host, port, Encryption::None),
        &EndpointId::new("ftp"),
        &events,
    )
    .await
    .expect("connect");

    let home = session.home().await.expect("pwd");
    // Two listings in a row reuse the pooled connection rather than logging in
    // again, which is what makes a queue of small files bearable.
    session.list_dir(&home).await.expect("first");
    session.list_dir(&home).await.expect("second");
    assert!(session.concurrency().await >= 1);
}

#[tokio::test]
async fn an_unknown_certificate_stops_the_connection_and_names_the_reason() {
    // The point of the test is the refusal, not the success: a client that
    // shrugs at an unknown certificate offers encryption without authenticity,
    // which is the part an attacker in the middle is counting on.
    let (host, port) = server_or_skip!("an_unknown_certificate_stops");
    let events = Events::new();
    let error = FtpSession::connect(
        &params(&host, port, Encryption::Explicit),
        &EndpointId::new("ftps"),
        &events,
    )
    .await
    .expect_err("a certificate nobody has accepted must not be trusted");

    match error {
        amberbeam_core::error::Error::CertificateUntrusted {
            host: named,
            fingerprint,
            reason,
            detail,
        } => {
            assert_eq!(named, host);
            assert_eq!(fingerprint.len(), 32 * 3 - 1, "a SHA-256 to compare by eye");
            // Not "certificate error": the window has to be able to say which.
            assert!(reason.starts_with("certificate."), "{reason}");
            assert!(!detail.is_empty(), "the log needs the original wording");
            eprintln!("refused: {reason} ({detail}), fingerprint {fingerprint}");
        }
        other => panic!("expected a certificate refusal, got {other:?}"),
    }
}

#[tokio::test]
async fn accepting_the_fingerprint_lets_the_connection_through() {
    let (host, port) = server_or_skip!("accepting_the_fingerprint");
    let events = Events::new();

    // First attempt: refused, and it hands back the fingerprint to show.
    let mut settings = params(&host, port, Encryption::Explicit);
    let error = FtpSession::connect(&settings, &EndpointId::new("ftps"), &events)
        .await
        .expect_err("first attempt");
    let amberbeam_core::error::Error::CertificateUntrusted { fingerprint, .. } = error else {
        panic!("expected a certificate refusal");
    };

    // Second attempt, with the same fingerprint accepted — as if the user had
    // compared it and pressed the button.
    settings.certificate = CertificateDecision::Trust {
        fingerprint: fingerprint.clone(),
    };
    let session = FtpSession::connect(&settings, &EndpointId::new("ftps"), &events)
        .await
        .expect("an accepted fingerprint connects");
    assert_eq!(session.encryption(), Encryption::Explicit);

    let home = session.home().await.expect("pwd");
    let listing = session.list_dir(&home).await.expect("list over TLS");
    eprintln!(
        "{} holds {} entries, over an encrypted data channel",
        listing.path,
        listing.entries.len()
    );

    // A different fingerprint on the same server asks again rather than
    // riding on the earlier decision.
    settings.certificate = CertificateDecision::Trust {
        fingerprint: "00:11:22:33".into(),
    };
    assert!(
        FtpSession::connect(&settings, &EndpointId::new("ftps"), &events)
            .await
            .is_err(),
        "accepting one certificate must not accept the next one"
    );
}

#[test]
fn self_signed_and_expired_are_never_the_same_message() {
    assert_ne!(
        CertificateProblem::UnknownIssuer.message_key(),
        CertificateProblem::Expired.message_key()
    );
}

/// A file put onto the server, read back, and removed again.
///
/// The round trip is the point: a transfer that reports success and leaves
/// different bytes behind is the failure this program exists to avoid, and only
/// reading them back proves it did not happen.
#[tokio::test]
async fn a_file_travels_up_and_comes_back_the_same() {
    use amberbeam_core::registry::TransferRun;
    use amberbeam_core::registry::{Sessions, LOCAL};

    let (host, port) = server_or_skip!("a_file_travels_up");
    let events = Events::new();
    let sessions = Sessions::new(events.clone());
    let remote = EndpointId::new("ftp");
    let local = EndpointId::new(LOCAL);

    let connected = sessions
        .connect_ftp(&remote, &params(&host, port, Encryption::None))
        .await
        .expect("connect");

    // Big enough that the copy loop runs more than once, small enough not to
    // sit on the test server.
    let written: Vec<u8> = (0..300_000_u32).map(|i| (i % 251) as u8).collect();
    let dir = std::env::temp_dir().join("amberbeam-ftp-test");
    std::fs::create_dir_all(&dir).expect("scratch directory");
    let source = dir.join("sent.bin");
    let back = dir.join("returned.bin");
    std::fs::write(&source, &written).expect("write the source");

    let run = |from: (&EndpointId, String), to: (&EndpointId, String)| TransferRun {
        source_endpoint: from.0.clone(),
        source_path: from.1,
        target_endpoint: to.0.clone(),
        target_path: to.1,
        resume: None,
        keep_modified: false,
        keep_permissions: false,
        use_temporary_name: false,
        source_permissions: None,
    };

    // Written where the account actually starts, rather than at a path this
    // test decided on: the server's idea of home is the only one that counts.
    let remote_path = format!(
        "{}/amberbeam-round-trip.bin",
        connected.home.trim_end_matches('/')
    );
    let progress = amberbeam_core::engine::Progress::default();

    let up = sessions
        .transfer(
            &run(
                (&local, source.to_string_lossy().into_owned()),
                (&remote, remote_path.clone()),
            ),
            &progress,
        )
        .await
        .expect("upload");
    assert!(up.complete);
    assert_eq!(up.moved, written.len() as u64, "every byte was sent");

    let progress = amberbeam_core::engine::Progress::default();
    let down = sessions
        .transfer(
            &run(
                (&remote, remote_path.clone()),
                (&local, back.to_string_lossy().into_owned()),
            ),
            &progress,
        )
        .await
        .expect("download");
    assert!(down.complete);

    let returned = std::fs::read(&back).expect("read what came back");
    assert_eq!(returned.len(), written.len(), "the file changed length");
    assert!(
        returned == written,
        "the bytes that came back are not the ones sent"
    );

    // Tidied up, so the next run starts from the same place.
    sessions
        .remove(&remote, &remote_path)
        .await
        .expect("remove the uploaded file");
    let _ = std::fs::remove_file(&source);
    let _ = std::fs::remove_file(&back);
}

/// A connection left sitting still is still usable afterwards.
///
/// Two seconds is short for a keep-alive and long for a test, which is the
/// compromise: long enough that the task really runs between the two listings,
/// short enough that nobody waits for it.
#[tokio::test]
async fn an_idle_connection_is_kept_alive() {
    let (host, port) = server_or_skip!("an_idle_connection");
    let events = Events::new();
    let mut settings = params(&host, port, Encryption::None);
    settings.keep_alive = Some(2);

    let session = FtpSession::connect(&settings, &EndpointId::new("ftp"), &events)
        .await
        .expect("connect");
    let home = session.home().await.expect("pwd");
    session.list_dir(&home).await.expect("first");

    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    // The connection the keep-alive has been holding is the one this listing
    // borrows, so a NOOP that left the control channel out of step would show
    // up here as a listing that answers the wrong question.
    let listing = session.list_dir(&home).await.expect("after sitting still");
    assert!(
        !listing.entries.is_empty(),
        "the second listing came back empty"
    );

    session.disconnect().await;
}

/// A transfer that was cut in half and picked up again.
///
/// This is the one failure that must never happen: a resumed transfer that
/// starts the file again at the server end while the client writes at the end
/// of what it already has. The result looks complete, has the right length, and
/// is wrong in the middle — which no progress bar and no size comparison would
/// ever reveal. Only reading the bytes back proves it.
#[tokio::test]
async fn a_transfer_cut_in_half_finishes_correctly() {
    use amberbeam_core::engine::Progress;
    use amberbeam_core::registry::{Sessions, TransferRun, LOCAL};
    use amberbeam_core::transfer::ResumeMarker;

    let (host, port) = server_or_skip!("a_transfer_cut_in_half");
    let events = Events::new();
    let sessions = Sessions::new(events.clone());
    let remote = EndpointId::new("ftp");
    let local = EndpointId::new(LOCAL);

    let connected = sessions
        .connect_ftp(&remote, &params(&host, port, Encryption::None))
        .await
        .expect("connect");

    // Bytes that differ everywhere, so a half from the wrong place shows up
    // wherever it lands rather than only at one seam.
    let written: Vec<u8> = (0..400_000_u32)
        .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8)
        .collect();
    let half = written.len() / 2;

    let dir = std::env::temp_dir().join("amberbeam-ftp-resume");
    std::fs::create_dir_all(&dir).expect("scratch directory");
    let source = dir.join("whole.bin");
    let target = dir.join("returned.bin");
    std::fs::write(&source, &written).expect("write the source");

    let remote_path = format!(
        "{}/amberbeam-resume.bin",
        connected.home.trim_end_matches('/')
    );

    // Put the whole file up first; the interesting half is the way back down.
    sessions
        .transfer(
            &TransferRun {
                source_endpoint: local.clone(),
                source_path: source.to_string_lossy().into_owned(),
                target_endpoint: remote.clone(),
                target_path: remote_path.clone(),
                resume: None,
                keep_modified: false,
                keep_permissions: false,
                use_temporary_name: false,
                source_permissions: None,
            },
            &Progress::default(),
        )
        .await
        .expect("upload");

    // A download that stopped in the middle, written by hand so the test does
    // not depend on catching a real one at the right moment.
    std::fs::write(&target, &written[..half]).expect("write the half");

    // The marker has to carry what the server says, not what this test
    // believes: the whole point of the check before a resume is that the two
    // are compared.
    let listed = sessions
        .list_dir(&remote, connected.home.trim_end_matches('/'))
        .await
        .expect("list the directory");
    let entry = listed
        .entries
        .iter()
        .find(|entry| entry.name == "amberbeam-resume.bin")
        .expect("the uploaded file is in the listing");
    let size = entry.size.expect("the server states a size");
    let modified = entry.modified;
    assert_eq!(
        size,
        written.len() as u64,
        "the server disagrees about length"
    );

    let rest = sessions
        .transfer(
            &TransferRun {
                source_endpoint: remote.clone(),
                source_path: remote_path.clone(),
                target_endpoint: local.clone(),
                target_path: target.to_string_lossy().into_owned(),
                resume: Some(ResumeMarker {
                    offset: half as u64,
                    source_size: size,
                    source_modified: modified,
                }),
                keep_modified: false,
                keep_permissions: false,
                use_temporary_name: false,
                source_permissions: None,
            },
            &Progress::default(),
        )
        .await
        .expect("the remainder");

    assert!(rest.complete);
    assert_eq!(
        rest.moved,
        (written.len() - half) as u64,
        "a resumed transfer moves the remainder, not the whole file again"
    );

    let returned = std::fs::read(&target).expect("read what came back");
    assert_eq!(returned.len(), written.len(), "the file changed length");
    assert!(
        returned == written,
        "the two halves do not join up — the second half came from the wrong offset"
    );

    sessions
        .remove(&remote, &remote_path)
        .await
        .expect("tidy up");
    let _ = std::fs::remove_file(&source);
    let _ = std::fs::remove_file(&target);
}

/// Names that survive the round trip through a listing.
///
/// Umlauts, an ampersand, a space and a leading dot. Each of them has broken a
/// file client at some point: the space by being treated as a separator in
/// `LIST` output, the dot by being filtered out on the way, the umlaut by being
/// read as Latin-1 when it was UTF-8.
///
/// Skipped where the fixture is absent, so this can be run against somebody's
/// own server without a directory being planted on it.
#[tokio::test]
async fn awkward_names_come_through_unharmed() {
    let (host, port) = server_or_skip!("awkward_names");
    let events = Events::new();
    let session = FtpSession::connect(
        &params(&host, port, Encryption::None),
        &EndpointId::new("ftp"),
        &events,
    )
    .await
    .expect("connect");

    let home = session.home().await.expect("pwd");
    let fixture = format!("{}/testdata", home.trim_end_matches('/'));
    let Ok(listing) = session.list_dir(&fixture).await else {
        eprintln!("skipping awkward_names: no {fixture} on this server");
        return;
    };

    let names: Vec<&str> = listing.entries.iter().map(|e| e.name.as_str()).collect();
    for expected in [
        "index.html",
        "Größe & Maß.txt",
        "with space.txt",
        ".hidden",
        "images",
    ] {
        assert!(
            names.contains(&expected),
            "{expected} is missing from {names:?}"
        );
    }
}
