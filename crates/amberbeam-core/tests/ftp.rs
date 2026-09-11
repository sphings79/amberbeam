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
