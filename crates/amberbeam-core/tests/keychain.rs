//! The system credential store, against the real thing.
//!
//! Skipped unless AMBERBEAM_TEST_KEYCHAIN is set, because a build machine has
//! no keychain and a locked one asks the person sitting in front of it. The
//! store is exercised by the unit tests through its in-memory twin; this is
//! here to prove the twin is not the only thing that works.
use amberbeam_core::secrets::{Secret, SecretStore, SystemStore};

#[test]
fn a_password_survives_a_round_trip_through_the_system_store() {
    if std::env::var("AMBERBEAM_TEST_KEYCHAIN").is_err() {
        eprintln!("skipping: AMBERBEAM_TEST_KEYCHAIN is not set");
        return;
    }
    // A service name of its own, so a test never touches what the program keeps.
    let store = SystemStore::new("AmberBeam test");
    let id = format!("test-{}", std::process::id());

    store.set(&id, Secret::Password, "tannenbaum").expect("set");
    assert_eq!(
        store.get(&id, Secret::Password).expect("get").as_deref(),
        Some("tannenbaum")
    );

    store.forget_all(&id).expect("forget");
    assert_eq!(store.get(&id, Secret::Password).expect("get"), None);
}
