//! A server that accepts the connection and then says nothing.
//!
//! This is not a hypothetical. It is what a firewall that swallows rather than
//! refuses looks like, what a port belonging to some other program looks like,
//! and what AmberBeam did about it until a Windows user sat watching
//! "connecting…" for as long as they were willing to.
//!
//! The listener below accepts and then holds the socket open without a word,
//! which is exactly that situation and needs no network at all.

use std::time::Instant;

use amberbeam_core::endpoint::{EndpointId, GREETING};
use amberbeam_core::error::Error;
use amberbeam_core::events::Events;

/// Accepts connections and answers none of them.
async fn silent_port() -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a port");
    let port = listener.local_addr().expect("an address").port();

    tokio::spawn(async move {
        let mut held = Vec::new();
        while let Ok((socket, _)) = listener.accept().await {
            // Kept, not dropped: dropping would close the connection and the
            // client would learn something. The point is that it learns nothing.
            held.push(socket);
        }
    });
    port
}

#[tokio::test(flavor = "multi_thread")]
async fn sftp_gives_up_instead_of_waiting_for_ever() {
    let port = silent_port().await;
    let started = Instant::now();

    let result = amberbeam_core::sftp::SftpSession::connect(
        &amberbeam_core::sftp::ConnectParams {
            host: "127.0.0.1".into(),
            port,
            user: "jemand".into(),
            method: amberbeam_core::sftp::AuthMethod::Password {
                password: "egal".into(),
            },
            host_key: amberbeam_core::sftp::HostKeyDecision::KnownOnly,
            known_hosts: None,
            concurrency: 4,
            retries: 1,
            temporary_name: true,
        },
        &EndpointId::new("sftp"),
        &Events::new(),
    )
    .await;

    match result {
        Err(Error::TimedOut {
            port: named,
            seconds,
            ..
        }) => {
            assert_eq!(named, port);
            assert_eq!(seconds, GREETING.as_secs());
        }
        other => panic!("expected a timeout, got {other:?}"),
    }

    // And it gave up roughly when it said it would, rather than early by luck.
    let waited = started.elapsed();
    assert!(waited >= GREETING, "gave up after only {waited:?}");
    assert!(waited < GREETING * 2, "took {waited:?}, far past the limit");
}

#[tokio::test(flavor = "multi_thread")]
async fn ftp_gives_up_too() {
    let port = silent_port().await;

    let result = amberbeam_core::ftp::FtpSession::connect(
        &amberbeam_core::ftp::FtpParams {
            host: "127.0.0.1".into(),
            port,
            user: "jemand".into(),
            password: "egal".into(),
            encryption: amberbeam_core::ftp::Encryption::None,
            passive: true,
            concurrency: 2,
            retries: 1,
            temporary_name: false,
            keep_alive: None,
            latin1: false,
            certificate: Default::default(),
        },
        &EndpointId::new("ftp"),
        &Events::new(),
    )
    .await;

    assert!(
        matches!(result, Err(Error::TimedOut { .. })),
        "expected a timeout, got {result:?}"
    );
}
