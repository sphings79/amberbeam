//! The server list: one readable JSON file per entry, in folders that are
//! folders.
//!
//! Section 06 of the concept paper asks for a tree with folders, and for a
//! storage format that can be versioned, backed up and edited by hand. Those
//! two wishes point at the same answer: the tree on screen *is* the tree on
//! disk. A folder is a directory under `sites/`, an entry is a file in it.
//! Making an empty folder is `mkdir`, moving an entry is one rename, and
//! somebody poking around in Finder sees exactly what they see in the program.
//!
//! **Never a password in any of these files.** A site refers to its secrets by
//! [`Site::id`], and [`crate::secrets`] fetches them from the system's own
//! store. The id is why renaming an entry — which does rename its file — does
//! not lose its password.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::AuthKind;
use crate::endpoint::Protocol;
use crate::error::{Error, PathProblem, Result};
use crate::ftp::Encryption;

/// A site entry as it lands on disk.
///
/// Everything a connection needs and nothing it does not: no password, no
/// passphrase, and no field that might one day hold either.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Site {
    /// Stable for the life of the entry, and never shown. It is what the
    /// credential store files the password under, which is why renaming an
    /// entry or moving it to a new address does not orphan the password.
    pub id: String,
    pub name: String,
    pub protocol: Protocol,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub auth: AuthKind,
    pub key_path: Option<String>,
    /// Directory the server side opens in.
    pub remote_path: Option<String>,
    /// Open where this server was last left instead of at [`Site::remote_path`].
    ///
    /// Off by default. Somebody who typed a starting directory meant it, and a
    /// program that quietly stops honouring a setting after the first
    /// connection is a program whose settings nobody can trust.
    ///
    /// The path itself is not kept here. It changes with every directory
    /// somebody walks into, and this file is written whole by a second window
    /// and read by people in an editor; neither wants a line that rewrites
    /// itself every few seconds. It lives in `visited.json` instead — see
    /// [`crate::config::Config::visited`].
    #[serde(default)]
    pub remember_path: bool,
    /// Directory the local side opens in.
    pub local_path: Option<String>,
    pub concurrency: u8,
    #[serde(default)]
    pub retries: Option<u8>,
    #[serde(default)]
    pub temporary_name: Option<bool>,
    /// FTP only: how the connection is encrypted.
    #[serde(default)]
    pub encryption: Option<Encryption>,
    /// FTP only.
    #[serde(default)]
    pub passive: Option<bool>,
    /// FTP only: the server does not speak UTF-8.
    #[serde(default)]
    pub latin1: Option<bool>,
    /// FTP only: seconds between keep-alive commands on an idle connection.
    #[serde(default)]
    pub keep_alive: Option<u32>,
    /// Whether this entry's password is kept in the credential store at all.
    /// Off by default: remembering a password is a choice, not a default.
    #[serde(default)]
    pub remember_password: bool,
    /// A directory on the server that deleted files are moved into instead.
    ///
    /// `None` deletes for good, which stays the default: a program that
    /// quietly kept everything somebody deleted would be filling a disk they
    /// thought they were clearing.
    ///
    /// It carries no secret — a path on a server whose address is already in
    /// this file.
    #[serde(default)]
    pub wastebasket: Option<String>,
    /// Whether to count what a delete would take before asking about it.
    ///
    /// On, and it should stay on where it can: "three files" and "eleven
    /// thousand files" deserve different answers to the same question, and the
    /// only way to tell them apart is to look.
    ///
    /// Looking means one listing per directory, and on a server every listing
    /// is a round trip — so a deep tree can leave somebody waiting on a count
    /// they did not want for a delete they were sure about. Off, the question
    /// is still asked; it just cannot say how much.
    ///
    /// Per server because that is where the cost lives. The local side always
    /// counts: it reads a directory in the time a server takes to say hello.
    #[serde(default = "yes")]
    pub count_before_delete: bool,
    /// What a program driving AmberBeam over MCP may do on this server.
    ///
    /// Six switches rather than one, all off, because "may use this server" is
    /// not one question. Reading a configuration file, putting one back,
    /// tidying a directory and emptying one are four different amounts of
    /// trust, and somebody handing out the first has not agreed to the last.
    ///
    /// A server with none of them on does not exist as far as those tools are
    /// concerned: not listed and refused, but absent, unnameable, unreachable.
    ///
    /// They replace an older three — see it, change it, delete on it — and
    /// nothing is carried over from those. A permission given once under a
    /// coarser name is not consent to the finer ones underneath it.
    #[serde(default)]
    pub mcp_see: bool,
    /// Put a file there.
    #[serde(default)]
    pub mcp_upload: bool,
    /// Take a file from there, which also writes to this machine: the local
    /// side has switches of its own and both have to allow it.
    #[serde(default)]
    pub mcp_download: bool,
    /// Make a directory there, and anything above it that is missing.
    #[serde(default)]
    pub mcp_create: bool,
    /// Rename something there, within the directory it is in.
    #[serde(default)]
    pub mcp_rename: bool,
    /// Delete there.
    ///
    /// The last one anybody should turn on. A program acting on what it read
    /// can misread; everything else it does can be undone by doing it again,
    /// and this cannot.
    #[serde(default)]
    pub mcp_remove: bool,
    /// Names a comparison and a watch never look at, as patterns.
    ///
    /// Per server because that is where it belongs: what counts as noise in a
    /// web project is not what counts as noise in a backup, and the person who
    /// set up the server is the one who knows. Carries no secret — they are
    /// file names, and the file already holds the address they sit at.
    #[serde(default)]
    pub excludes: Vec<String>,
    /// Colour marking in the list, one of the interface's accents.
    pub colour: Option<String>,
}

/// For `serde(default)` on a field whose default is true.
fn yes() -> bool {
    true
}

impl Site {
    /// A fresh identifier.
    ///
    /// Time and a counter rather than a random number: this has to be unique
    /// among one person's site entries, not unguessable, and a value that
    /// sorts by age is easier to look at in a credential store than a UUID.
    pub fn new_id() -> String {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|since| since.as_millis())
            .unwrap_or_default();
        let count = COUNTER.fetch_add(1, Ordering::Relaxed);
        format!("{now:x}-{count:x}")
    }
}

/// One entry together with where it sits in the tree.
///
/// The folder is not part of [`Site`] because it is not a property of the
/// server — it is where somebody filed it, and on disk it is the directory the
/// file happens to be in. Keeping it out of the file means moving an entry
/// cannot leave the file and its own idea of its place disagreeing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Filed {
    /// `""` for the top level, otherwise `Kunden/Müller`, always with forward
    /// slashes whatever the system writes on disk.
    pub folder: String,
    pub site: Site,
}

/// The server list.
#[derive(Debug, Clone)]
pub struct Sites {
    root: PathBuf,
}

impl Sites {
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Every entry, deepest folder last, each folder's entries by name.
    ///
    /// A file that cannot be read as a site is skipped rather than fatal. Half
    /// a list is still a list somebody can connect from; refusing to show any
    /// of it because one file was edited into nonsense is not.
    pub fn load(&self) -> Vec<Filed> {
        let mut found = Vec::new();
        self.walk(&self.root, "", &mut found);
        found.sort_by(|a, b| {
            a.folder
                .to_lowercase()
                .cmp(&b.folder.to_lowercase())
                .then_with(|| a.site.name.to_lowercase().cmp(&b.site.name.to_lowercase()))
        });
        found
    }

    fn walk(&self, directory: &Path, folder: &str, into: &mut Vec<Filed>) {
        let Ok(entries) = std::fs::read_dir(directory) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if path.is_dir() {
                let deeper = if folder.is_empty() {
                    name.to_string()
                } else {
                    format!("{folder}/{name}")
                };
                self.walk(&path, &deeper, into);
            } else if name.ends_with(".json") {
                if let Some(site) = read_site(&path) {
                    into.push(Filed {
                        folder: folder.to_string(),
                        site,
                    });
                }
            }
        }
    }

    /// Every folder that exists, including the ones holding nothing.
    ///
    /// An empty folder is a directory with no files in it, which is why it
    /// survives a restart without a register of its own.
    pub fn folders(&self) -> Vec<String> {
        let mut found = Vec::new();
        collect_folders(&self.root, "", &mut found);
        found.sort_by_key(|folder| folder.to_lowercase());
        found
    }

    /// Writes an entry, moving or renaming its file when either changed.
    ///
    /// The old file is removed only after the new one is written. A crash in
    /// between leaves two copies of an entry, which somebody can sort out; the
    /// other order leaves none, which nobody can.
    pub fn save(&self, folder: &str, site: &Site) -> Result<PathBuf> {
        let target = self.path_for(folder, &site.name);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(Error::from)?;
        }

        let previous = self.find(&site.id).map(|(_, path)| path);
        write_json(&target, site)?;

        if let Some(previous) = previous {
            if previous != target {
                let _ = std::fs::remove_file(&previous);
                // A folder emptied by the move goes with it, or moving the last
                // entry out of a folder would leave a folder nobody made.
                self.prune_empty(previous.parent());
            }
        }
        Ok(target)
    }

    /// Where an entry lives now, if it does.
    pub fn find(&self, id: &str) -> Option<(Filed, PathBuf)> {
        let mut found = Vec::new();
        self.walk(&self.root, "", &mut found);
        found
            .into_iter()
            .find(|filed| filed.site.id == id)
            .map(|filed| {
                let path = self.path_for(&filed.folder, &filed.site.name);
                (filed, path)
            })
    }

    /// Removes an entry. Its secrets are not this module's to remove — the
    /// caller does that, because only it holds the credential store.
    pub fn delete(&self, id: &str) -> Result<()> {
        let Some((_, path)) = self.find(id) else {
            return Err(Error::Path {
                path: id.to_string(),
                reason: PathProblem::NotFound,
            });
        };
        std::fs::remove_file(&path).map_err(Error::from)?;
        self.prune_empty(path.parent());
        Ok(())
    }

    pub fn create_folder(&self, folder: &str) -> Result<()> {
        let path = self.folder_path(folder);
        if path == self.root {
            return Err(Error::other("a folder needs a name"));
        }
        std::fs::create_dir_all(&path).map_err(Error::from)
    }

    /// Renames a folder, and with it everything inside.
    ///
    /// One rename of a directory, because the tree on screen is the tree on
    /// disk. Nothing inside has to be rewritten: no entry records where it is.
    pub fn rename_folder(&self, from: &str, to: &str) -> Result<()> {
        let (source, target) = (self.folder_path(from), self.folder_path(to));
        if source == self.root || target == self.root {
            return Err(Error::other("a folder needs a name"));
        }
        if target.exists() {
            return Err(Error::Path {
                path: to.to_string(),
                reason: PathProblem::AlreadyExists,
            });
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(Error::from)?;
        }
        std::fs::rename(&source, &target).map_err(Error::from)
    }

    /// Removes a folder, but only an empty one.
    ///
    /// Deleting a folder full of servers is a different decision from deleting
    /// a folder, and one that deserves its own question rather than being
    /// carried along by this one.
    pub fn delete_folder(&self, folder: &str) -> Result<()> {
        let path = self.folder_path(folder);
        if path == self.root {
            return Err(Error::other("a folder needs a name"));
        }
        let empty = std::fs::read_dir(&path)
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(false);
        if !empty {
            return Err(Error::other("this folder is not empty"));
        }
        std::fs::remove_dir(&path).map_err(Error::from)
    }

    fn path_for(&self, folder: &str, name: &str) -> PathBuf {
        self.folder_path(folder)
            .join(format!("{}.json", safe_segment(name)))
    }

    fn folder_path(&self, folder: &str) -> PathBuf {
        let mut path = self.root.clone();
        for segment in folder.split('/').filter(|part| !part.is_empty()) {
            path.push(safe_segment(segment));
        }
        path
    }

    /// Removes directories that the last move or delete left empty, up to but
    /// never including the root.
    fn prune_empty(&self, from: Option<&Path>) {
        let mut current = from.map(Path::to_path_buf);
        while let Some(path) = current {
            if path == self.root || !path.starts_with(&self.root) {
                return;
            }
            if std::fs::remove_dir(&path).is_err() {
                // Not empty, or not there. Either way there is nothing above
                // it worth trying.
                return;
            }
            current = path.parent().map(Path::to_path_buf);
        }
    }
}

fn collect_folders(directory: &Path, folder: &str, into: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let deeper = if folder.is_empty() {
            name.to_string()
        } else {
            format!("{folder}/{name}")
        };
        into.push(deeper.clone());
        collect_folders(&path, &deeper, into);
    }
}

fn read_site(path: &Path) -> Option<Site> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let text = serde_json::to_string_pretty(value).map_err(Error::other)?;
    // Written beside the target and renamed over it, so a crash midway leaves
    // the previous file intact instead of half of the new one.
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, text).map_err(Error::from)?;
    std::fs::rename(&temporary, path).map_err(Error::from)
}

/// Keeps a name usable as one path segment without inventing a different one.
///
/// Only the characters a file system would refuse, plus the two names that mean
/// something else entirely. A name is otherwise left alone, umlauts and spaces
/// and all: it is what the person typed.
fn safe_segment(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            c if (c as u32) < 0x20 => '-',
            other => other,
        })
        .collect();
    let trimmed = cleaned.trim().trim_end_matches('.');
    match trimmed {
        "" | "." | ".." => "unnamed".to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> Sites {
        let root = std::env::temp_dir().join(format!("amberbeam-sites-{name}"));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        Sites::at(root)
    }

    fn site(name: &str) -> Site {
        Site {
            id: Site::new_id(),
            name: name.into(),
            protocol: Protocol::Sftp,
            host: "example.org".into(),
            port: 22,
            user: "someone".into(),
            auth: AuthKind::Password,
            key_path: None,
            remote_path: Some("/var/www".into()),
            remember_path: false,
            local_path: None,
            concurrency: 8,
            retries: None,
            temporary_name: None,
            encryption: None,
            passive: None,
            latin1: None,
            keep_alive: None,
            remember_password: false,
            wastebasket: None,
            count_before_delete: true,
            excludes: Vec::new(),
            mcp_see: false,
            mcp_upload: false,
            mcp_download: false,
            mcp_create: false,
            mcp_rename: false,
            mcp_remove: false,
            colour: None,
        }
    }

    #[test]
    fn the_tree_on_screen_is_the_tree_on_disk() {
        let sites = scratch("tree");
        sites.save("Kunden/Müller", &site("Webserver")).unwrap();
        sites.save("Kunden", &site("Hausanschluss")).unwrap();
        sites.save("", &site("Zuhause")).unwrap();

        // Somebody looking in Finder sees the same folders as in the window.
        assert!(sites.root().join("Kunden/Müller/Webserver.json").exists());
        assert!(sites.root().join("Zuhause.json").exists());

        let loaded = sites.load();
        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded[0].folder, "", "the top level sorts first");
        assert_eq!(loaded[0].site.name, "Zuhause");
        assert_eq!(loaded[2].folder, "Kunden/Müller");

        let mut folders = sites.folders();
        folders.sort();
        assert_eq!(folders, vec!["Kunden", "Kunden/Müller"]);
    }

    #[test]
    fn an_empty_folder_survives_without_a_register_of_its_own() {
        let sites = scratch("empty");
        sites.create_folder("Später/Vielleicht").unwrap();

        assert!(sites.folders().contains(&"Später/Vielleicht".to_string()));
        assert!(sites.load().is_empty());

        sites.delete_folder("Später/Vielleicht").unwrap();
        assert!(!sites.folders().contains(&"Später/Vielleicht".to_string()));
    }

    #[test]
    fn a_folder_with_something_in_it_is_not_deleted_by_accident() {
        let sites = scratch("full");
        sites.save("Kunden", &site("Webserver")).unwrap();
        assert!(
            sites.delete_folder("Kunden").is_err(),
            "deleting a folder full of servers is a different decision"
        );
        assert_eq!(sites.load().len(), 1);
    }

    #[test]
    fn moving_an_entry_leaves_exactly_one_copy() {
        let sites = scratch("move");
        let mut entry = site("Webserver");
        sites.save("Alt", &entry).unwrap();

        // Moved and renamed at once, which is the awkward case: two reasons for
        // the file to be somewhere else.
        entry.name = "Neuer Name".into();
        sites.save("Neu/Tiefer", &entry).unwrap();

        let loaded = sites.load();
        assert_eq!(loaded.len(), 1, "a move must not leave a second copy");
        assert_eq!(loaded[0].folder, "Neu/Tiefer");
        assert_eq!(loaded[0].site.name, "Neuer Name");
        // The identifier did not move, which is what keeps the password.
        assert_eq!(loaded[0].site.id, entry.id);
        // And the folder it left is gone, rather than lingering empty.
        assert!(!sites.folders().contains(&"Alt".to_string()));
    }

    #[test]
    fn renaming_a_folder_takes_everything_in_it() {
        let sites = scratch("rename");
        sites.save("Kunden/Müller", &site("Web")).unwrap();
        sites.save("Kunden/Müller", &site("Datenbank")).unwrap();

        sites
            .rename_folder("Kunden/Müller", "Kunden/Müller GmbH")
            .unwrap();

        let loaded = sites.load();
        assert_eq!(loaded.len(), 2);
        assert!(loaded.iter().all(|f| f.folder == "Kunden/Müller GmbH"));
    }

    #[test]
    fn a_file_edited_into_nonsense_costs_one_entry_not_the_list() {
        let sites = scratch("nonsense");
        sites.save("", &site("Gut")).unwrap();
        std::fs::write(sites.root().join("Kaputt.json"), "{ not json").unwrap();

        let loaded = sites.load();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].site.name, "Gut");
    }

    #[test]
    fn a_name_that_would_not_survive_a_file_system_is_made_to() {
        // The characters a file system refuses, and the two names that mean
        // something else. Everything else is left alone.
        assert_eq!(safe_segment("Kunde: Müller/Web"), "Kunde- Müller-Web");
        assert_eq!(safe_segment(".."), "unnamed");
        assert_eq!(safe_segment("   "), "unnamed");
        assert_eq!(safe_segment("Größe & Maß"), "Größe & Maß");
    }

    #[test]
    fn every_field_of_a_site_file_has_been_looked_at() {
        // A site file may hold exactly these. Adding a field fails this test
        // until somebody has decided it carries no secret — which is the point,
        // because these files sit on disk in the open.
        const ALLOWED: [&str; 29] = [
            // A path on a server whose address is already in this file, so it
            // gives away nothing that was not already here.
            "wastebasket",
            "id",
            "name",
            "protocol",
            "host",
            "port",
            "user",
            "auth",
            "keyPath",
            "remotePath",
            // A directory on that same server, and only kept at all when the
            // entry asked for it.
            "rememberPath",
            "localPath",
            "concurrency",
            "retries",
            "temporaryName",
            "encryption",
            "passive",
            "latin1",
            "keepAlive",
            // File names, on a server whose address is already in this file.
            "excludes",
            "countBeforeDelete",
            // Six switches, and the ones that decide what a program driving
            // this one may do with the entry. None of them carries a secret;
            // each of them decides whether one tool answers at all.
            "mcpSee",
            "mcpUpload",
            "mcpDownload",
            "mcpCreate",
            "mcpRename",
            "mcpRemove",
            "rememberPassword",
            "colour",
        ];
        let written = serde_json::to_value(site("Webserver")).unwrap();
        let object = written.as_object().expect("an object");
        for field in object.keys() {
            assert!(
                ALLOWED.contains(&field.as_str()),
                "a site file grew a field nobody has vetted: {field}"
            );
        }
        assert_eq!(
            object.len(),
            ALLOWED.len(),
            "the allowed list and the file have drifted apart"
        );
    }
}
