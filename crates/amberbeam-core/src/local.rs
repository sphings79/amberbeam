//! The file system the core itself runs on.
//!
//! On the desktop that is the user's own machine; in the container build of M7
//! it is whatever was mounted into the container, which is why this is one
//! endpoint among several and not "the local side". Nothing here is privileged.

use std::path::{Path, PathBuf};

use crate::error::{Error, PathProblem, Result};
use crate::fs::{DirEntry, EntryKind, Listing, Permissions};

/// A session on the machine the core runs on. Holds nothing: there is no
/// connection to keep, and every call works from an absolute path.
#[derive(Debug, Default, Clone, Copy)]
pub struct LocalSession;

impl LocalSession {
    pub fn new() -> Self {
        Self
    }

    /// Where a pane starts when nothing else is known.
    pub fn home(&self) -> Result<String> {
        dirs::home_dir()
            .map(|path| path.to_string_lossy().into_owned())
            .ok_or_else(|| Error::Path {
                path: String::new(),
                reason: PathProblem::NotFound,
            })
    }

    pub async fn list_dir(&self, path: &str) -> Result<Listing> {
        let directory = PathBuf::from(path);
        let mut reader = tokio::fs::read_dir(&directory)
            .await
            .map_err(|source| at(path, source))?;

        let mut entries = Vec::new();
        while let Some(entry) = reader
            .next_entry()
            .await
            .map_err(|source| at(path, source))?
        {
            // A single unreadable entry must not lose the whole listing: a
            // directory full of files plus one broken symlink is still worth
            // showing.
            if let Some(row) = describe(&entry.path(), entry.file_name()).await {
                entries.push(row);
            }
        }

        Ok(Listing {
            path: directory.to_string_lossy().into_owned(),
            entries,
        })
    }

    /// The directory above, or `None` when already at the top.
    pub fn parent(&self, path: &str) -> Option<String> {
        Path::new(path)
            .parent()
            .map(|parent| parent.to_string_lossy().into_owned())
    }

    /// Joins in the notation of the system the core runs on, which is not the
    /// POSIX joining used for servers.
    pub fn join(&self, directory: &str, name: &str) -> String {
        Path::new(directory)
            .join(name)
            .to_string_lossy()
            .into_owned()
    }
}

fn at(path: &str, source: std::io::Error) -> Error {
    Error::Path {
        path: path.to_string(),
        reason: PathProblem::from(source.kind()),
    }
}

/// Turns one path into a row, or `None` when even its metadata is unreadable.
async fn describe(path: &Path, name: std::ffi::OsString) -> Option<DirEntry> {
    // `symlink_metadata` does not follow links, so a link shows up as a link.
    let meta = tokio::fs::symlink_metadata(path).await.ok()?;
    let file_type = meta.file_type();

    let (kind, link_target, kind_of_target) = if file_type.is_symlink() {
        let target = tokio::fs::read_link(path)
            .await
            .ok()
            .map(|target| target.to_string_lossy().into_owned());
        // Following the link is what tells the panes whether it can be entered.
        // A link into nothing is not an error, it simply leads nowhere.
        let resolved = tokio::fs::metadata(path).await.ok().map(|meta| {
            if meta.is_dir() {
                EntryKind::Directory
            } else if meta.is_file() {
                EntryKind::File
            } else {
                EntryKind::Other
            }
        });
        (EntryKind::Symlink, target, resolved)
    } else if file_type.is_dir() {
        (EntryKind::Directory, None, None)
    } else if file_type.is_file() {
        (EntryKind::File, None, None)
    } else {
        (EntryKind::Other, None, None)
    };

    Some(DirEntry {
        name: name.to_string_lossy().into_owned(),
        kind,
        size: (kind == EntryKind::File).then_some(meta.len()),
        modified: modified_seconds(&meta),
        permissions: permissions(&meta),
        owner: owner(&meta),
        group: group(&meta),
        link_target,
        kind_of_target,
    })
}

fn modified_seconds(meta: &std::fs::Metadata) -> Option<i64> {
    let time = meta.modified().ok()?;
    match time.duration_since(std::time::UNIX_EPOCH) {
        Ok(since) => i64::try_from(since.as_secs()).ok(),
        // Before 1970. Rare, but a file with a bogus date must not be dropped.
        Err(before) => i64::try_from(before.duration().as_secs())
            .ok()
            .map(|seconds| -seconds),
    }
}

#[cfg(unix)]
fn permissions(meta: &std::fs::Metadata) -> Option<Permissions> {
    use std::os::unix::fs::PermissionsExt;
    Some(Permissions(meta.permissions().mode() & 0o777))
}

#[cfg(not(unix))]
fn permissions(_meta: &std::fs::Metadata) -> Option<Permissions> {
    // Windows has no mode bits, and inventing some would be worse than an
    // empty column.
    None
}

#[cfg(unix)]
fn owner(meta: &std::fs::Metadata) -> Option<String> {
    use std::os::unix::fs::MetadataExt;
    // The numeric id, as SFTP reports it too. Resolving names needs the system
    // user database and a dependency this crate does not carry.
    Some(meta.uid().to_string())
}

#[cfg(not(unix))]
fn owner(_meta: &std::fs::Metadata) -> Option<String> {
    None
}

#[cfg(unix)]
fn group(meta: &std::fs::Metadata) -> Option<String> {
    use std::os::unix::fs::MetadataExt;
    Some(meta.gid().to_string())
}

#[cfg(not(unix))]
fn group(_meta: &std::fs::Metadata) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("amberbeam-test-{name}"));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("create scratch directory");
        path
    }

    #[tokio::test]
    async fn a_listing_names_files_and_directories() {
        let root = scratch("listing");
        std::fs::write(root.join("index.html"), b"<!doctype html>").unwrap();
        std::fs::create_dir(root.join("images")).unwrap();

        let listing = LocalSession::new()
            .list_dir(&root.to_string_lossy())
            .await
            .expect("list");

        let mut names: Vec<_> = listing.entries.iter().map(|e| e.name.as_str()).collect();
        names.sort_unstable();
        assert_eq!(names, ["images", "index.html"]);

        let file = listing
            .entries
            .iter()
            .find(|e| e.name == "index.html")
            .unwrap();
        assert_eq!(file.kind, EntryKind::File);
        assert_eq!(file.size, Some(15));
        assert!(file.modified.is_some());
        assert!(!file.is_directory());

        let directory = listing.entries.iter().find(|e| e.name == "images").unwrap();
        assert!(directory.is_directory());
        assert_eq!(directory.size, None);

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[tokio::test]
    async fn a_missing_directory_says_so_and_names_itself() {
        let error = LocalSession::new()
            .list_dir("/definitely/not/here")
            .await
            .expect_err("must fail");
        assert!(matches!(
            error,
            Error::Path {
                reason: PathProblem::NotFound,
                ref path,
            } if path == "/definitely/not/here"
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_symlink_to_a_directory_can_be_entered() {
        let root = scratch("symlink");
        std::fs::create_dir(root.join("releases")).unwrap();
        std::os::unix::fs::symlink(root.join("releases"), root.join("current")).unwrap();
        std::os::unix::fs::symlink(root.join("gone"), root.join("broken")).unwrap();

        let listing = LocalSession::new()
            .list_dir(&root.to_string_lossy())
            .await
            .expect("list");

        let link = listing
            .entries
            .iter()
            .find(|e| e.name == "current")
            .unwrap();
        assert_eq!(link.kind, EntryKind::Symlink);
        assert!(link.is_directory());
        assert!(link.link_target.is_some());

        // A link into nothing is still listed, and leads nowhere.
        let broken = listing.entries.iter().find(|e| e.name == "broken").unwrap();
        assert_eq!(broken.kind, EntryKind::Symlink);
        assert_eq!(broken.kind_of_target, None);
        assert!(!broken.is_directory());

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn permissions_and_owner_come_along() {
        use std::os::unix::fs::PermissionsExt;

        let root = scratch("permissions");
        let file = root.join("private.txt");
        std::fs::write(&file, b"x").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();

        let listing = LocalSession::new()
            .list_dir(&root.to_string_lossy())
            .await
            .expect("list");
        let entry = &listing.entries[0];
        assert_eq!(entry.permissions.unwrap().to_rwx(), "rw-------");
        assert!(entry.owner.is_some());

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[tokio::test]
    async fn names_that_are_not_plain_text_still_appear() {
        let root = scratch("names");
        for name in ["Größe & Maß.txt", "with space.txt", "ünïcode"] {
            std::fs::write(root.join(name), b"x").unwrap();
        }
        let listing = LocalSession::new()
            .list_dir(&root.to_string_lossy())
            .await
            .expect("list");
        assert_eq!(listing.entries.len(), 3);
        assert!(listing.entries.iter().any(|e| e.name.contains('&')));

        std::fs::remove_dir_all(&root).unwrap();
    }
}
