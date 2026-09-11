//! Where passwords live, which is never a file of ours.
//!
//! Section 12 of the concept paper is short about this and means it: site
//! files sit on disk in the open so they can be read, edited and backed up,
//! and the only reason that is acceptable is that they hold no secret. A
//! password is referenced from a site entry by the entry's identifier and
//! fetched from the system's own store when a connection is opened.
//!
//! The store is behind a trait for one specific reason. The desktop has a
//! keychain; the container build of milestone M7 has no such thing and will
//! need an encrypted file with a master key. That is the same "build the door
//! now, walk through it later" pattern as the endpoints in [`crate::endpoint`]
//! — and it has a second use today, because a Linux build machine has no
//! keychain either, so the tests would otherwise have to be skipped exactly
//! where they matter.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::error::{Error, Result};

/// Which secret of an entry is meant.
///
/// A key file with a passphrase has two: the passphrase unlocks the key, and
/// the entry may still carry a password for something else. Keeping them apart
/// by name rather than by convention means neither can be fetched in place of
/// the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Secret {
    /// The account password.
    Password,
    /// The passphrase of a key file.
    Passphrase,
}

impl Secret {
    const fn suffix(self) -> &'static str {
        match self {
            Secret::Password => "password",
            Secret::Passphrase => "passphrase",
        }
    }
}

/// The name one secret is filed under.
///
/// The site's identifier rather than its name, so renaming a site does not
/// orphan its password — and its host and user are deliberately not part of it,
/// so moving a site to a new address does not either.
fn account(site_id: &str, secret: Secret) -> String {
    format!("{site_id}:{}", secret.suffix())
}

/// Somewhere to keep secrets.
///
/// Every method may fail, and a caller must be able to carry on when it does. A
/// locked keychain, a Linux session without a secret service, a user who said
/// no to the prompt — none of those are reasons for the program to stop; they
/// are reasons to ask for the password this once.
pub trait SecretStore: std::fmt::Debug + Send + Sync {
    fn get(&self, site_id: &str, secret: Secret) -> Result<Option<String>>;
    fn set(&self, site_id: &str, secret: Secret, value: &str) -> Result<()>;
    fn forget(&self, site_id: &str, secret: Secret) -> Result<()>;

    /// Removes everything filed under a site. Called when one is deleted, so a
    /// password does not outlive the entry that explained what it was for.
    fn forget_all(&self, site_id: &str) -> Result<()> {
        self.forget(site_id, Secret::Password)?;
        self.forget(site_id, Secret::Passphrase)
    }
}

/// The credential store of the operating system: Keychain on macOS, the
/// Credential Manager on Windows, the Secret Service on Linux.
#[derive(Debug)]
pub struct SystemStore {
    service: String,
}

impl SystemStore {
    /// `service` is the name the user sees beside the entry in Keychain Access
    /// and its counterparts, so it is the program's name and nothing cleverer.
    pub fn new(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }

    fn entry(&self, site_id: &str, secret: Secret) -> Result<keyring::Entry> {
        keyring::Entry::new(&self.service, &account(site_id, secret)).map_err(Error::other)
    }
}

impl Default for SystemStore {
    fn default() -> Self {
        Self::new("AmberBeam")
    }
}

impl SecretStore for SystemStore {
    fn get(&self, site_id: &str, secret: Secret) -> Result<Option<String>> {
        match self.entry(site_id, secret)?.get_password() {
            Ok(value) => Ok(Some(value)),
            // Nothing filed is not a failure. It is the ordinary state of an
            // entry whose owner chose not to have the password remembered.
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(other) => Err(Error::other(other)),
        }
    }

    fn set(&self, site_id: &str, secret: Secret, value: &str) -> Result<()> {
        self.entry(site_id, secret)?
            .set_password(value)
            .map_err(Error::other)
    }

    fn forget(&self, site_id: &str, secret: Secret) -> Result<()> {
        match self.entry(site_id, secret)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(other) => Err(Error::other(other)),
        }
    }
}

/// A store that keeps nothing past the end of the program.
///
/// Used by the tests, and by anything that has to run where no credential store
/// exists. It is not a fallback the user is ever silently given: a password
/// that seems to be remembered and then is not would be worse than one that was
/// never offered.
#[derive(Debug, Default)]
pub struct MemoryStore {
    held: Mutex<HashMap<String, String>>,
}

impl SecretStore for MemoryStore {
    fn get(&self, site_id: &str, secret: Secret) -> Result<Option<String>> {
        Ok(self
            .held
            .lock()
            .map_err(|_| Error::other("the secret store was poisoned"))?
            .get(&account(site_id, secret))
            .cloned())
    }

    fn set(&self, site_id: &str, secret: Secret, value: &str) -> Result<()> {
        self.held
            .lock()
            .map_err(|_| Error::other("the secret store was poisoned"))?
            .insert(account(site_id, secret), value.to_string());
        Ok(())
    }

    fn forget(&self, site_id: &str, secret: Secret) -> Result<()> {
        self.held
            .lock()
            .map_err(|_| Error::other("the secret store was poisoned"))?
            .remove(&account(site_id, secret));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_two_secrets_of_one_entry_do_not_collide() {
        let store = MemoryStore::default();
        store.set("site-1", Secret::Password, "tannenbaum").unwrap();
        store.set("site-1", Secret::Passphrase, "eichhorn").unwrap();

        assert_eq!(
            store.get("site-1", Secret::Password).unwrap().as_deref(),
            Some("tannenbaum")
        );
        assert_eq!(
            store.get("site-1", Secret::Passphrase).unwrap().as_deref(),
            Some("eichhorn")
        );
    }

    #[test]
    fn nothing_filed_is_not_a_failure() {
        // The ordinary state of an entry whose owner did not have the password
        // remembered, and the one case a caller must not treat as broken.
        let store = MemoryStore::default();
        assert_eq!(store.get("never-seen", Secret::Password).unwrap(), None);
        store.forget("never-seen", Secret::Password).unwrap();
    }

    #[test]
    fn deleting_an_entry_takes_both_its_secrets() {
        let store = MemoryStore::default();
        store.set("site-1", Secret::Password, "tannenbaum").unwrap();
        store.set("site-1", Secret::Passphrase, "eichhorn").unwrap();
        store
            .set("site-2", Secret::Password, "unbeteiligt")
            .unwrap();

        store.forget_all("site-1").unwrap();

        assert_eq!(store.get("site-1", Secret::Password).unwrap(), None);
        assert_eq!(store.get("site-1", Secret::Passphrase).unwrap(), None);
        // And nobody else's.
        assert_eq!(
            store.get("site-2", Secret::Password).unwrap().as_deref(),
            Some("unbeteiligt")
        );
    }

    #[test]
    fn a_secret_is_filed_under_the_identifier_and_nothing_else() {
        // Renaming a site, or moving it to a new address, must not orphan its
        // password — so neither the name nor the host may be part of the key.
        assert_eq!(account("abc123", Secret::Password), "abc123:password");
        assert_ne!(
            account("abc123", Secret::Password),
            account("abc123", Secret::Passphrase)
        );
    }
}
