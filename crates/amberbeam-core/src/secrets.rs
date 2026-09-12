//! Where passwords live, which is never a file of ours.
//!
//! Section 12 of the concept paper is short about this and means it: site
//! files sit on disk in the open so they can be read, edited and backed up,
//! and the only reason that is acceptable is that they hold no secret. A
//! password is referenced from a site entry by the entry's identifier and
//! fetched from the system's own store when a connection is opened.
//!
//! The store is behind a trait for one specific reason. The desktop has a
//! keychain; a container has no such thing, so it gets [`FileStore`] — one
//! encrypted file under a passphrase, using exactly the sealing an export
//! already uses. That is the same "build the door now, walk through it later"
//! pattern as the endpoints in [`crate::endpoint`] — and the trait has a
//! second use today, because a Linux build machine has no keychain either, so
//! the tests would otherwise have to be skipped exactly where they matter.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
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
/// Used by the tests, by anything that has to run where no credential store
/// exists, and — in the desktop shell — for a password somebody typed without
/// asking for it to be kept. It is not a fallback the user is ever silently
/// given in place of the real store: a password that seems to be remembered
/// and then is not would be worse than one that was never offered.
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

    /// A temporary directory that goes when the test does.
    fn somewhere(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("amberbeam-secrets-{name}"));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn a_file_store_keeps_what_it_was_given_across_being_reopened() {
        let home = somewhere("roundtrip");
        let path = home.join("secrets.sealed");

        let store = FileStore::open(&path, "tannenbaum").unwrap();
        store.set("site-1", Secret::Password, "eichhorn").unwrap();
        store
            .set("site-1", Secret::Passphrase, "mondschein")
            .unwrap();
        drop(store);

        let again = FileStore::open(&path, "tannenbaum").unwrap();
        assert_eq!(
            again.get("site-1", Secret::Password).unwrap().as_deref(),
            Some("eichhorn")
        );
        assert_eq!(
            again.get("site-1", Secret::Passphrase).unwrap().as_deref(),
            Some("mondschein")
        );
        assert_eq!(again.count(), 2);
    }

    /// The file on disk must not be readable. Obvious, and exactly the sort of
    /// obvious thing that is worth a test: a change to how it is written could
    /// leave the passwords in plain sight and nothing else would notice.
    #[test]
    fn what_lands_on_disk_is_not_the_password() {
        let home = somewhere("opaque");
        let path = home.join("secrets.sealed");
        let store = FileStore::open(&path, "tannenbaum").unwrap();
        store.set("site-1", Secret::Password, "eichhorn").unwrap();

        let bytes = std::fs::read(&path).unwrap();
        assert!(
            !String::from_utf8_lossy(&bytes).contains("eichhorn"),
            "the password is sitting in the file in the open"
        );
        assert!(crate::sealed::is_sealed(&bytes));
    }

    /// A wrong passphrase must fail loudly. Opening empty and carrying on
    /// would mean the next saved password overwrites everything that was in
    /// the file — a typo destroying exactly what it failed to read.
    #[test]
    fn a_wrong_passphrase_refuses_rather_than_starting_empty() {
        let home = somewhere("wrong");
        let path = home.join("secrets.sealed");
        FileStore::open(&path, "tannenbaum")
            .unwrap()
            .set("site-1", Secret::Password, "eichhorn")
            .unwrap();

        assert!(FileStore::open(&path, "something else").is_err());
        assert!(FileStore::open(&path, "").is_err());

        // And the file is still there, still holding what it held.
        let again = FileStore::open(&path, "tannenbaum").unwrap();
        assert_eq!(
            again.get("site-1", Secret::Password).unwrap().as_deref(),
            Some("eichhorn")
        );
    }

    #[test]
    fn forgetting_one_leaves_the_others() {
        let home = somewhere("forget");
        let path = home.join("secrets.sealed");
        let store = FileStore::open(&path, "tannenbaum").unwrap();
        store.set("site-1", Secret::Password, "eichhorn").unwrap();
        store.set("site-2", Secret::Password, "mondschein").unwrap();
        store.forget("site-1", Secret::Password).unwrap();
        // Forgetting what was never there is not an error and writes nothing.
        store.forget("site-9", Secret::Password).unwrap();

        let again = FileStore::open(&path, "tannenbaum").unwrap();
        assert!(again.get("site-1", Secret::Password).unwrap().is_none());
        assert_eq!(
            again.get("site-2", Secret::Password).unwrap().as_deref(),
            Some("mondschein")
        );
    }

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

/// One encrypted file, for a machine with no credential store of its own.
///
/// The container has no keychain, no Credential Manager and no Secret Service.
/// The honest alternatives were "no saved passwords at all" or "a file only a
/// passphrase opens", and this is the second.
///
/// The encryption is the one already in this program: PBKDF2-HMAC-SHA256 at
/// 600,000 rounds and ChaCha20-Poly1305, the same as a sealed export. Nothing
/// new was invented for it, which is the point — a second encryption scheme
/// would be a second thing to get wrong.
///
/// The whole file is rewritten on every change. It holds a handful of
/// passwords, not a database, and rewriting it whole means there is never a
/// half-written one to read back.
#[derive(Debug)]
pub struct FileStore {
    path: PathBuf,
    passphrase: String,
    held: Mutex<HashMap<String, String>>,
}

impl FileStore {
    /// Opens the file, or starts an empty one where there is none yet.
    ///
    /// A file that will not open is an error and never an empty store. Going
    /// on as if it were empty would mean the first saved password overwrites
    /// every password that was in it — a mistyped passphrase would destroy
    /// exactly what it failed to read.
    pub fn open(path: impl AsRef<Path>, passphrase: &str) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if passphrase.is_empty() {
            return Err(Error::other(
                "a passphrase is needed to open the password file",
            ));
        }

        let held = match std::fs::read(&path) {
            Ok(bytes) => {
                let plain = crate::sealed::open(&bytes, passphrase).map_err(|_| {
                    Error::other(
                        "the password file will not open with this passphrase; \
                         nothing has been changed",
                    )
                })?;
                serde_json::from_slice(&plain)
                    .map_err(|why| Error::other(format!("the password file is damaged: {why}")))?
            }
            Err(why) if why.kind() == std::io::ErrorKind::NotFound => HashMap::new(),
            Err(why) => return Err(Error::from(why)),
        };

        Ok(Self {
            path,
            passphrase: passphrase.to_string(),
            held: Mutex::new(held),
        })
    }

    /// How many secrets it holds, for a service that wants to say so at start.
    pub fn count(&self) -> usize {
        self.held.lock().map(|held| held.len()).unwrap_or_default()
    }

    fn write(&self, held: &HashMap<String, String>) -> Result<()> {
        let plain = serde_json::to_vec(held).map_err(Error::other)?;
        let sealed = crate::sealed::seal(&plain, &self.passphrase)?;

        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(Error::from)?;
        }
        // Beside it first, then moved into place. A crash halfway through a
        // write would otherwise leave a file that opens with no passphrase at
        // all, because it is no longer a sealed file.
        let beside = self.path.with_extension("writing");
        std::fs::write(&beside, sealed).map_err(Error::from)?;
        std::fs::rename(&beside, &self.path).map_err(Error::from)
    }
}

impl SecretStore for FileStore {
    fn get(&self, site_id: &str, secret: Secret) -> Result<Option<String>> {
        Ok(self
            .held
            .lock()
            .map_err(|_| Error::other("the password file was poisoned"))?
            .get(&account(site_id, secret))
            .cloned())
    }

    fn set(&self, site_id: &str, secret: Secret, value: &str) -> Result<()> {
        let mut held = self
            .held
            .lock()
            .map_err(|_| Error::other("the password file was poisoned"))?;
        held.insert(account(site_id, secret), value.to_string());
        self.write(&held)
    }

    fn forget(&self, site_id: &str, secret: Secret) -> Result<()> {
        let mut held = self
            .held
            .lock()
            .map_err(|_| Error::other("the password file was poisoned"))?;
        if held.remove(&account(site_id, secret)).is_none() {
            // Nothing was there, so nothing has to be written. Rewriting the
            // file anyway would mean forgetting something twice costs two
            // writes of everything else.
            return Ok(());
        }
        self.write(&held)
    }
}
