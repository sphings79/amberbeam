//! The server list as one file: `.amberbeam-sites`.
//!
//! Two shapes, and which one a file has is visible from the outside. Without
//! passwords it is plain JSON that anybody can read, diff and edit — the same
//! bargain as the site files themselves. With passwords it is sealed under a
//! passphrase, and there is no third option: a file that carries secrets and is
//! merely obscured is exactly what the importers in [`crate::import`] spend
//! their time undoing, and this program is not going to add to that pile.

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::sealed;
use crate::secrets::{Secret, SecretStore};
use crate::sites::{Filed, Site, Sites};

/// What the file says it is, so a later version can tell.
const VERSION: u32 = 1;

/// One entry in a bundle.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub folder: String,
    pub site: Site,
    /// Only ever present in a sealed file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    /// Likewise. A key file's passphrase is as much a secret as a password.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub passphrase: Option<String>,
}

/// Hand-written so nothing here can reach a log through a stray `{:?}`.
impl std::fmt::Debug for Entry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Entry")
            .field("folder", &self.folder)
            .field("name", &self.site.name)
            .field("host", &self.site.host)
            .field(
                "has_secrets",
                &(self.password.is_some() || self.passphrase.is_some()),
            )
            .finish_non_exhaustive()
    }
}

/// A whole exported list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bundle {
    pub version: u32,
    pub entries: Vec<Entry>,
}

impl Bundle {
    pub fn carries_secrets(&self) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.password.is_some() || entry.passphrase.is_some())
    }
}

/// Gathers the list into a bundle.
///
/// `with_secrets` decides whether the credential store is consulted at all. It
/// is a decision made once, here, rather than a flag carried around: an export
/// either holds secrets from the start or never touches them.
pub fn gather(filed: &[Filed], secrets: &dyn SecretStore, with_secrets: bool) -> Bundle {
    let entries = filed
        .iter()
        .map(|entry| {
            let (password, passphrase) = if with_secrets {
                (
                    secrets.get(&entry.site.id, Secret::Password).ok().flatten(),
                    secrets
                        .get(&entry.site.id, Secret::Passphrase)
                        .ok()
                        .flatten(),
                )
            } else {
                (None, None)
            };
            Entry {
                folder: entry.folder.clone(),
                site: entry.site.clone(),
                password,
                passphrase,
            }
        })
        .collect();

    Bundle {
        version: VERSION,
        entries,
    }
}

/// Turns a bundle into the bytes of a file.
///
/// Refuses to write secrets without a passphrase. Not as a nicety: the whole
/// point of the sealed shape is that there is no unsealed one carrying
/// passwords, and a flag somewhere that could turn that off would eventually
/// be turned off.
pub fn to_bytes(bundle: &Bundle, passphrase: Option<&str>) -> Result<Vec<u8>> {
    let text = serde_json::to_vec_pretty(bundle).map_err(Error::other)?;
    match passphrase {
        Some(passphrase) => sealed::seal(&text, passphrase),
        None if bundle.carries_secrets() => Err(Error::other(
            "an export with passwords needs a passphrase to seal it",
        )),
        None => Ok(text),
    }
}

/// Reads a file back, sealed or not.
///
/// Which it is can be seen from the bytes, so the caller is told to ask for a
/// passphrase rather than having to guess whether to prompt.
pub fn from_bytes(bytes: &[u8], passphrase: Option<&str>) -> Result<Bundle> {
    let text = if sealed::is_sealed(bytes) {
        let passphrase = passphrase.ok_or_else(|| Error::other("this export is sealed"))?;
        sealed::open(bytes, passphrase)?
    } else {
        bytes.to_vec()
    };

    let bundle: Bundle = serde_json::from_slice(&text)
        .map_err(|_| Error::other("this is not an AmberBeam export"))?;
    if bundle.version > VERSION {
        return Err(Error::other("this export was made by a newer version"));
    }
    Ok(bundle)
}

/// Writes a bundle into the server list.
///
/// Every entry gets a fresh identifier: the file may be somebody else's, or the
/// same list imported twice, and reusing the identifiers would have two entries
/// sharing one password in the credential store.
pub fn apply(
    bundle: &Bundle,
    chosen: &[usize],
    sites: &Sites,
    secrets: &dyn SecretStore,
    into: &str,
) -> Result<usize> {
    let mut taken = 0;
    for &index in chosen {
        let Some(entry) = bundle.entries.get(index) else {
            continue;
        };

        let site = Site {
            id: Site::new_id(),
            remember_password: entry.password.is_some(),
            ..entry.site.clone()
        };
        let folder = match (into.trim(), entry.folder.as_str()) {
            ("", inner) => inner.to_string(),
            (outer, "") => outer.to_string(),
            (outer, inner) => format!("{outer}/{inner}"),
        };
        sites.save(&folder, &site)?;

        if let Some(password) = entry.password.as_deref() {
            secrets.set(&site.id, Secret::Password, password)?;
        }
        if let Some(passphrase) = entry.passphrase.as_deref() {
            secrets.set(&site.id, Secret::Passphrase, passphrase)?;
        }
        taken += 1;
    }
    Ok(taken)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AuthKind;
    use crate::endpoint::Protocol;
    use crate::secrets::MemoryStore;

    fn site(name: &str) -> Site {
        Site {
            id: Site::new_id(),
            name: name.into(),
            protocol: Protocol::Sftp,
            host: "example.org".into(),
            port: 22,
            user: "dennis".into(),
            auth: AuthKind::Password,
            key_path: None,
            remote_path: None,
            local_path: None,
            concurrency: 8,
            retries: None,
            temporary_name: None,
            encryption: None,
            passive: None,
            latin1: None,
            keep_alive: None,
            remember_password: true,
            colour: None,
        }
    }

    fn filed() -> (Vec<Filed>, MemoryStore) {
        let entry = Filed {
            folder: "Kunden".into(),
            site: site("Webserver"),
        };
        let store = MemoryStore::default();
        store
            .set(&entry.site.id, Secret::Password, "tannenbaum")
            .unwrap();
        (vec![entry], store)
    }

    #[test]
    fn an_export_without_passwords_is_plain_and_readable() {
        let (list, store) = filed();
        let bundle = gather(&list, &store, false);
        assert!(!bundle.carries_secrets());

        let bytes = to_bytes(&bundle, None).unwrap();
        let text = String::from_utf8(bytes.clone()).unwrap();
        assert!(text.contains("Webserver"), "the point of the plain shape");
        assert!(!text.contains("tannenbaum"));

        let back = from_bytes(&bytes, None).unwrap();
        assert_eq!(back.entries.len(), 1);
        assert_eq!(back.entries[0].folder, "Kunden");
    }

    #[test]
    fn an_export_with_passwords_cannot_be_written_unsealed() {
        // There is no third shape. A file that carries secrets and is merely
        // obscured is what the importers spend their time undoing.
        let (list, store) = filed();
        let bundle = gather(&list, &store, true);
        assert!(bundle.carries_secrets());
        assert!(to_bytes(&bundle, None).is_err());
    }

    #[test]
    fn a_sealed_export_comes_back_whole() {
        let (list, store) = filed();
        let bundle = gather(&list, &store, true);
        let bytes = to_bytes(&bundle, Some("ein Kennwort")).unwrap();

        assert!(!bytes.windows(10).any(|w| w == b"tannenbaum"));
        assert!(!bytes.windows(9).any(|w| w == b"Webserver"));

        let back = from_bytes(&bytes, Some("ein Kennwort")).unwrap();
        assert_eq!(back.entries[0].password.as_deref(), Some("tannenbaum"));
        assert_eq!(back.entries[0].site.name, "Webserver");
    }

    #[test]
    fn a_sealed_export_says_so_before_asking_for_a_passphrase() {
        let (list, store) = filed();
        let bytes = to_bytes(&gather(&list, &store, true), Some("k")).unwrap();
        assert!(sealed::is_sealed(&bytes));
        assert!(from_bytes(&bytes, None).is_err());
        assert!(from_bytes(&bytes, Some("falsch")).is_err());
    }

    #[test]
    fn importing_a_list_twice_does_not_have_two_entries_share_a_password() {
        let (list, store) = filed();
        let bundle = gather(&list, &store, true);

        let root = std::env::temp_dir().join("amberbeam-bundle-twice");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let sites = Sites::at(&root);
        let target = MemoryStore::default();

        apply(&bundle, &[0], &sites, &target, "Erst").unwrap();
        apply(&bundle, &[0], &sites, &target, "Dann").unwrap();

        let written = sites.load();
        assert_eq!(written.len(), 2);
        assert_ne!(
            written[0].site.id, written[1].site.id,
            "two entries must not share one password in the credential store"
        );
        for entry in &written {
            assert_eq!(
                target
                    .get(&entry.site.id, Secret::Password)
                    .unwrap()
                    .as_deref(),
                Some("tannenbaum")
            );
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_folder_to_import_into_is_put_in_front() {
        let (list, store) = filed();
        let bundle = gather(&list, &store, false);

        let root = std::env::temp_dir().join("amberbeam-bundle-into");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let sites = Sites::at(&root);

        apply(&bundle, &[0], &sites, &MemoryStore::default(), "Übernommen").unwrap();
        assert_eq!(sites.load()[0].folder, "Übernommen/Kunden");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn nothing_in_an_entry_reaches_a_log_through_a_stray_debug() {
        let (list, store) = filed();
        let bundle = gather(&list, &store, true);
        let printed = format!("{:?}", bundle.entries[0]);
        assert!(!printed.contains("tannenbaum"), "{printed}");
        assert!(printed.contains("has_secrets: true"));
    }

    #[test]
    fn a_file_from_a_newer_version_is_refused_rather_than_half_read() {
        let text = br#"{"version":99,"entries":[]}"#;
        assert!(from_bytes(text, None).is_err());
    }
}
