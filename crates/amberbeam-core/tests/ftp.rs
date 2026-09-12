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

/// Implicit FTPS pointed at a port that speaks plain FTP.
///
/// The honest limit of this test: no test server offers implicit FTPS —
/// pure-ftpd does not support it, and it is a deprecated mode nobody sets up on
/// purpose any more. So what is checked here is not that it works but that it
/// fails the right way. A client that expects TLS from the first byte and gets
/// `220 Welcome` instead must say so and let go, not sit waiting for a
/// handshake that will never come. A hang is the worst of the failures
/// available here, because nothing on screen would ever change.
#[tokio::test]
async fn implicit_ftps_on_a_plain_port_gives_up_instead_of_hanging() {
    let (host, port) = server_or_skip!("implicit_ftps_on_a_plain_port");
    let events = Events::new();

    let outcome = tokio::time::timeout(
        std::time::Duration::from_secs(20),
        FtpSession::connect(
            &params(&host, port, Encryption::Implicit),
            &EndpointId::new("ftps"),
            &events,
        ),
    )
    .await;

    match outcome {
        Ok(Ok(_)) => panic!("a plain FTP port answered an implicit TLS handshake"),
        Ok(Err(error)) => eprintln!("gave up, as it should: {error:?}"),
        Err(_) => panic!("implicit FTPS hung instead of reporting a failure"),
    }
}

/// Hand-typed commands, and what happens to the connection afterwards.
///
/// The second half is the point. A command that opens a data connection leaves
/// the control connection mid-sentence, and the rule is that such a connection
/// is closed rather than put back in the pool. What this proves is that the
/// next command still gets an answer to its own question — which is exactly
/// what would stop being true if the rule were dropped.
#[tokio::test]
async fn a_raw_command_answers_and_does_not_poison_the_next_one() {
    let (host, port) = server_or_skip!("a_raw_command");
    let events = Events::new();
    let session = FtpSession::connect(
        &params(&host, port, Encryption::None),
        &EndpointId::new("ftp"),
        &events,
    )
    .await
    .expect("connect");

    // An ordinary command: answered, and the connection goes back to the pool.
    let feat = session.raw("FEAT").await.expect("FEAT");
    assert_eq!(feat.code, 211, "FEAT answers 211 with the feature list");
    assert!(!feat.connection_dropped);
    assert!(feat.text.to_uppercase().contains("MLSD"), "{}", feat.text);

    // A command the server refuses is still an answer, not a failure of ours.
    let nonsense = session.raw("XYZZY").await.expect("a reply, not an error");
    assert!(
        nonsense.code >= 500,
        "an unknown command answers in the 500s, got {}",
        nonsense.code
    );

    // And one that opens a data connection: sent, answered, and the connection
    // thrown away rather than reused.
    let listing = session.raw("PASV").await.expect("PASV");
    assert!(
        listing.connection_dropped,
        "a data-channel command must not leave its connection in the pool"
    );

    // The question this test exists for: does the next command get its own
    // answer? If the poisoned connection had been reused, this would come back
    // with whatever was left over from the one before.
    let home = session.home().await.expect("PWD after a data command");
    assert!(home.starts_with('/'), "got {home:?}");
    let again = session.raw("FEAT").await.expect("FEAT again");
    assert_eq!(again.code, 211);
}

/// Looking for a name through a whole tree.
#[tokio::test]
async fn a_search_walks_the_tree_and_says_what_it_cost() {
    use amberbeam_core::registry::{Sessions, LOCAL};

    let (host, port) = server_or_skip!("a_search_walks");
    let events = Events::new();
    let sessions = Sessions::new(events.clone());
    let remote = EndpointId::new("ftp");
    let _ = LOCAL;

    let connected = sessions
        .connect_ftp(&remote, &params(&host, port, Encryption::None))
        .await
        .expect("connect");
    let home = connected.home.trim_end_matches('/').to_string();
    let root = if home.is_empty() {
        "/".to_string()
    } else {
        home
    };

    // The fixture holds index.html inside testdata, one level down.
    let Ok(found) = sessions.search(&remote, &root, "index", 100).await else {
        eprintln!("skipping: this server refused the search");
        return;
    };
    eprintln!(
        "{} matches after reading {} directories",
        found.matches.len(),
        found.directories
    );

    if found.matches.is_empty() {
        eprintln!("skipping the rest: no fixture on this server");
        return;
    }
    assert!(
        found.matches.iter().any(|m| m.name.contains("index")),
        "a match must actually contain what was looked for"
    );
    assert!(
        found.matches.iter().all(|m| m.path.starts_with('/')),
        "a match has to be reachable, so its path is the whole path"
    );
    assert!(found.directories >= 1);

    // Case is not remembered by anybody looking for a file.
    let upper = sessions
        .search(&remote, &root, "INDEX", 100)
        .await
        .expect("search");
    assert_eq!(upper.matches.len(), found.matches.len());

    // A bound that is reached is admitted rather than hidden.
    let one = sessions
        .search(&remote, &root, "index", 1)
        .await
        .expect("search");
    assert_eq!(one.matches.len(), 1);
    assert!(one.truncated, "a search that stopped early has to say so");

    // And nothing to look for is a refusal, not an empty answer that looks
    // like "there is nothing there".
    assert!(sessions.search(&remote, &root, "   ", 10).await.is_err());
}

/// A file edited where it lies: taken, changed, written back.
///
/// The part worth a real server is the last third. Whether a write-back is
/// refused depends on what the server says a file's size and time are, and
/// `MDTM` answers differently from one server to the next — a unit test with
/// numbers in it would prove nothing about the case this guard exists for.
#[tokio::test]
async fn a_file_is_edited_where_it_lies() {
    use amberbeam_core::editing::{Edits, Encoding};
    use amberbeam_core::error::Error;
    use amberbeam_core::registry::{Sessions, TransferRun, LOCAL};

    let (host, port) = server_or_skip!("a_file_is_edited_where_it_lies");
    let events = Events::new();
    let sessions = Sessions::new(events.clone());
    let remote = EndpointId::new("ftp");
    let local = EndpointId::new(LOCAL);

    let connected = sessions
        .connect_ftp(&remote, &params(&host, port, Encryption::None))
        .await
        .expect("connect");
    let home = connected.home.trim_end_matches('/').to_string();

    // Latin-1 on purpose: it is the case that goes wrong silently.
    let put = |name: &str, bytes: &[u8]| {
        let dir = std::env::temp_dir().join("amberbeam-edit-test");
        std::fs::create_dir_all(&dir).expect("scratch directory");
        let path = dir.join(name);
        std::fs::write(&path, bytes).expect("write the source");
        path.to_string_lossy().into_owned()
    };
    let upload = |from: String, to: String| TransferRun {
        source_endpoint: local.clone(),
        source_path: from,
        target_endpoint: remote.clone(),
        target_path: to,
        resume: None,
        keep_modified: false,
        keep_permissions: false,
        use_temporary_name: false,
        source_permissions: None,
    };

    let there = format!("{home}/amberbeam-edit.php");
    let source = put("amberbeam-edit.php", b"<?php\n// Gr\xfc\xdfe\n");
    sessions
        .transfer(
            &upload(source.clone(), there.clone()),
            &amberbeam_core::engine::Progress::default(),
        )
        .await
        .expect("put the file there");

    let edits = Edits::at(std::env::temp_dir().join("amberbeam-edit-test/copies"));
    let rules = amberbeam_core::editing::EditRule::shipped();
    let edit = edits
        .begin(&sessions, &remote, &there, &rules)
        .await
        .expect("take a copy");
    assert_eq!(edit.name, "amberbeam-edit.php");
    assert_eq!(
        edit.encoding,
        Encoding::Latin1,
        "a file that is not UTF-8 must not be read as though it were"
    );
    assert_eq!(edits.text(&edit.id).await.unwrap(), "<?php\n// Grüße\n");

    // Asking again gives the same copy, not a second one racing it.
    let again = edits
        .begin(&sessions, &remote, &there, &rules)
        .await
        .unwrap();
    assert_eq!(again.id, edit.id);
    assert_eq!(edits.list().await.len(), 1);

    // Saved and sent, and what arrives is Latin-1 again.
    edits
        .save(&edit.id, "<?php\n// Grüße, Welt\n")
        .await
        .expect("save the copy");
    edits
        .push(&sessions, &edit.id, false)
        .await
        .expect("write it back");

    let check = put("check.php", b"");
    sessions
        .transfer(
            &TransferRun {
                source_endpoint: remote.clone(),
                source_path: there.clone(),
                target_endpoint: local.clone(),
                target_path: check.clone(),
                resume: None,
                keep_modified: false,
                keep_permissions: false,
                use_temporary_name: false,
                source_permissions: None,
            },
            &amberbeam_core::engine::Progress::default(),
        )
        .await
        .expect("read it back");
    assert_eq!(
        std::fs::read(&check).unwrap(),
        b"<?php\n// Gr\xfc\xdfe, Welt\n".to_vec(),
        "what is on the server is byte for byte what an editor typed, in the file's own encoding"
    );

    // Now somebody else changes it. Long enough after that a server keeping
    // times to the second reports a different one.
    tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
    let meddled = put("meddled.php", b"<?php\n// jemand anderes war hier\n");
    sessions
        .transfer(
            &upload(meddled, there.clone()),
            &amberbeam_core::engine::Progress::default(),
        )
        .await
        .expect("somebody else writes the file");

    edits.save(&edit.id, "<?php\n// noch mehr\n").await.unwrap();
    let refused = edits.push(&sessions, &edit.id, false).await;
    assert!(
        matches!(refused, Err(Error::EditChangedOnServer { .. })),
        "writing back over somebody else's work has to be asked about, not done: {refused:?}"
    );
    assert_eq!(
        edits.text(&edit.id).await.unwrap(),
        "<?php\n// noch mehr\n",
        "the typing survives the refusal, or the question could not be answered"
    );

    // Told to go ahead, it goes ahead.
    edits
        .push(&sessions, &edit.id, true)
        .await
        .expect("write it back anyway");
    // And the refusal does not repeat: what is up there now is the new starting
    // point.
    edits
        .save(&edit.id, "<?php\n// und noch mehr\n")
        .await
        .unwrap();
    edits
        .push(&sessions, &edit.id, false)
        .await
        .expect("the next save is not asked about again");

    let copy = edit.local_path.clone();
    edits.finish(&edit.id, true).await;
    assert!(
        !std::path::Path::new(&copy).exists(),
        "a copy that was to be thrown away is gone"
    );
    assert!(edits.list().await.is_empty());

    sessions.remove(&remote, &there).await.expect("tidy up");
    let _ = std::fs::remove_dir_all(std::env::temp_dir().join("amberbeam-edit-test"));
}

/// Two directories compared against a real server.
///
/// The rules are unit-tested; what needs a server is everything around them —
/// a side that is not there yet, a listing whose dates come from `MLSD`, and a
/// digest read back over a data connection.
#[tokio::test]
async fn two_directories_are_told_apart() {
    use amberbeam_core::compare::{compare, Asking, Difference, How};
    use amberbeam_core::registry::{Sessions, TransferRun, LOCAL};

    let (host, port) = server_or_skip!("two_directories_are_told_apart");
    let events = Events::new();
    let sessions = Sessions::new(events.clone());
    let remote = EndpointId::new("ftp");
    let local = EndpointId::new(LOCAL);

    let connected = sessions
        .connect_ftp(&remote, &params(&host, port, Encryption::None))
        .await
        .expect("connect");
    let there = format!("{}/compare", connected.home.trim_end_matches('/'));

    // A directory on this machine, and the same one on the server with three
    // deliberate differences in it.
    let here = std::env::temp_dir().join("amberbeam-compare-test");
    let _ = std::fs::remove_dir_all(&here);
    std::fs::create_dir_all(here.join("bilder")).expect("scratch directory");
    std::fs::write(here.join("index.php"), b"<?php\n// eins\n").unwrap();
    std::fs::write(here.join("gleich.txt"), b"gleich\n").unwrap();
    std::fs::write(here.join("nur-hier.txt"), b"nur hier\n").unwrap();
    std::fs::write(here.join("bilder/foto.jpg"), b"nicht wirklich\n").unwrap();
    std::fs::write(here.join("error.log"), b"weggelassen\n").unwrap();

    let put = |from: std::path::PathBuf, to: String| TransferRun {
        source_endpoint: local.clone(),
        source_path: from.to_string_lossy().into_owned(),
        target_endpoint: remote.clone(),
        target_path: to,
        resume: None,
        keep_modified: false,
        keep_permissions: false,
        use_temporary_name: false,
        source_permissions: None,
    };
    let progress = || amberbeam_core::engine::Progress::default();

    // Made rather than ensured: this server answers MLSD of a directory that
    // does not exist with an empty listing and no error, so "can it be listed"
    // is not the same question as "is it there". See the note in the commit.
    let _ = sessions.remove(&remote, &there).await;
    sessions.create_dir(&remote, &there).await.expect("make it");
    // The same length as index.php here, and not the same bytes: the case
    // that size alone cannot see and a digest can.
    let other = here.join("other.php");
    std::fs::write(&other, b"<?php\n// ZWEI\n").unwrap();
    sessions
        .transfer(
            &put(other.clone(), format!("{there}/index.php")),
            &progress(),
        )
        .await
        .expect("upload the changed one");
    sessions
        .transfer(
            &put(here.join("gleich.txt"), format!("{there}/gleich.txt")),
            &progress(),
        )
        .await
        .expect("upload the same one");
    let only_there = here.join("nur-dort.txt");
    std::fs::write(&only_there, b"nur dort\n").unwrap();
    sessions
        .transfer(
            &put(only_there.clone(), format!("{there}/nur-dort.txt")),
            &progress(),
        )
        .await
        .expect("upload the extra one");
    std::fs::remove_file(&only_there).unwrap();
    std::fs::remove_file(&other).unwrap();

    let asking = |how, recursive| Asking {
        here: (local.clone(), here.to_string_lossy().into_owned()),
        there: (remote.clone(), there.clone()),
        recursive,
        how,
        excludes: vec!["*.log".into()],
    };

    // By size the two index.php files look identical, because they are the
    // same length. That is the blind spot, and it is the reason the checksum
    // option exists.
    let found = compare(&sessions, &asking(How::Size, false), &events)
        .await
        .expect("compare");
    let state = |rows: &Vec<amberbeam_core::compare::Row>, name: &str| {
        rows.iter()
            .find(|row| row.name == name)
            .unwrap_or_else(|| panic!("{name} is not in the comparison"))
            .state
    };
    assert_eq!(state(&found.rows, "index.php"), Difference::Same);
    assert_eq!(state(&found.rows, "nur-hier.txt"), Difference::OnlyHere);
    assert_eq!(state(&found.rows, "nur-dort.txt"), Difference::OnlyThere);
    assert_eq!(state(&found.rows, "gleich.txt"), Difference::Same);
    assert!(
        !found.rows.iter().any(|row| row.name == "error.log"),
        "an excluded name is not a row"
    );
    assert!(
        !found.rows.iter().any(|row| row.path.contains('/')),
        "nothing below the top level without being asked"
    );
    assert!(!found.cut_short);

    // By checksum the same pair is what it really is.
    let found = compare(&sessions, &asking(How::Checksum, false), &events)
        .await
        .expect("compare by checksum");
    assert_eq!(
        state(&found.rows, "index.php"),
        Difference::Different,
        "same length, different bytes: what the digest is for"
    );
    assert_eq!(state(&found.rows, "gleich.txt"), Difference::Same);

    // And recursively, a directory that is only here is walked rather than
    // reported and left — otherwise "upload what differs" would make an empty
    // directory and stop.
    let found = compare(&sessions, &asking(How::Checksum, true), &events)
        .await
        .expect("compare recursively");
    assert_eq!(state(&found.rows, "bilder"), Difference::OnlyHere);
    assert_eq!(state(&found.rows, "foto.jpg"), Difference::OnlyHere);
    assert!(
        found.rows.iter().any(|row| row.path == "bilder/foto.jpg"),
        "a row below the top level is named by where it sits"
    );
    assert!(found.directories >= 4, "both sides of both directories");

    sessions.remove(&remote, &there).await.expect("tidy up");
    let _ = std::fs::remove_dir_all(&here);
}

/// A folder uploaded into a directory that does not exist yet.
///
/// This server answers `MLSD` of a missing directory with a success and no
/// rows, so "can it be listed" and "is it there" are different questions.
/// Asking the first one meant creating nothing and then failing every file
/// that was to go into it, which is what dragging a folder onto such a server
/// did.
#[tokio::test]
async fn a_directory_that_is_not_there_is_made_rather_than_assumed() {
    use amberbeam_core::registry::{Sessions, TransferRun, LOCAL};

    let (host, port) = server_or_skip!("a_directory_that_is_not_there");
    let events = Events::new();
    let sessions = Sessions::new(events.clone());
    let remote = EndpointId::new("ftp");
    let local = EndpointId::new(LOCAL);

    let connected = sessions
        .connect_ftp(&remote, &params(&host, port, Encryption::None))
        .await
        .expect("connect");
    let home = connected.home.trim_end_matches('/').to_string();
    let deep = format!("{home}/nicht-da/auch-nicht/tiefer");
    let _ = sessions.remove(&remote, &format!("{home}/nicht-da")).await;

    // The listing lies, which is the whole point of this test.
    assert!(
        sessions.list_dir(&remote, &deep).await.is_ok(),
        "this server lists a directory that does not exist; if that ever \
         changes, the reason for is_there goes with it"
    );

    sessions
        .ensure_dir(&remote, &deep)
        .await
        .expect("make the whole chain");

    let source = std::env::temp_dir().join("amberbeam-ensure-test.txt");
    std::fs::write(&source, b"hinein\n").unwrap();
    sessions
        .transfer(
            &TransferRun {
                source_endpoint: local.clone(),
                source_path: source.to_string_lossy().into_owned(),
                target_endpoint: remote.clone(),
                target_path: format!("{deep}/datei.txt"),
                resume: None,
                keep_modified: false,
                keep_permissions: false,
                use_temporary_name: false,
                source_permissions: None,
            },
            &amberbeam_core::engine::Progress::default(),
        )
        .await
        .expect("the file lands in a directory that was actually created");

    // And asking again for a directory that is now there and empty must not
    // create it a second time or fail.
    sessions
        .ensure_dir(&remote, &deep)
        .await
        .expect("asking twice is not an error");

    sessions
        .remove(&remote, &format!("{home}/nicht-da"))
        .await
        .expect("tidy up");
    let _ = std::fs::remove_file(&source);
}
