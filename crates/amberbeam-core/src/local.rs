//! The file system the core itself runs on.
//!
//! On the desktop that is the user's own machine; in the container build of M7
//! it is whatever was mounted into the container, which is why this is one
//! endpoint among several and not "the local side". Nothing here is privileged.

use std::path::{Path, PathBuf};

use crate::error::{Error, PathProblem, Result};
use crate::fs::{DirEntry, EntryKind, Listing, Permissions};
use crate::ops::Measurement;
use crate::stream::{Reader, Writer};

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

    pub async fn create_dir(&self, path: &str) -> Result<()> {
        tokio::fs::create_dir(path)
            .await
            .map_err(|source| at(path, source))
    }

    /// Creates an empty file, and refuses to overwrite one that is there.
    pub async fn create_file(&self, path: &str) -> Result<()> {
        tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .await
            .map(|_| ())
            .map_err(|source| at(path, source))
    }

    pub async fn rename(&self, from: &str, to: &str) -> Result<()> {
        // Checked first: rename replaces the target without a word, and a
        // typo in a new name would then delete a file nobody mentioned.
        if tokio::fs::symlink_metadata(to).await.is_ok() {
            return Err(Error::Path {
                path: to.to_string(),
                reason: PathProblem::AlreadyExists,
            });
        }
        tokio::fs::rename(from, to)
            .await
            .map_err(|source| at(from, source))
    }

    /// Counts what a recursive delete would remove, up to the cap.
    pub async fn measure(&self, path: &str) -> Result<Measurement> {
        let mut measured = Measurement::default();
        let meta = tokio::fs::symlink_metadata(path)
            .await
            .map_err(|source| at(path, source))?;

        if meta.file_type().is_symlink() {
            measured.add_symlink();
            return Ok(measured);
        }
        if meta.is_file() {
            measured.add_file(Some(meta.len()));
            return Ok(measured);
        }

        let mut pending = vec![PathBuf::from(path)];
        while let Some(directory) = pending.pop() {
            measured.add_directory();
            if measured.reached_cap() {
                measured.truncated = true;
                return Ok(measured);
            }
            let Ok(mut reader) = tokio::fs::read_dir(&directory).await else {
                // A directory that cannot be read is still going to be part of
                // the attempt; refusing to count it must not refuse the warning.
                continue;
            };
            while let Ok(Some(entry)) = reader.next_entry().await {
                // symlink_metadata, not metadata: a link must be counted as a
                // link, not as whatever it points at.
                let Ok(meta) = tokio::fs::symlink_metadata(entry.path()).await else {
                    continue;
                };
                if meta.file_type().is_symlink() {
                    measured.add_symlink();
                } else if meta.is_dir() {
                    pending.push(entry.path());
                    continue;
                } else {
                    measured.add_file(Some(meta.len()));
                }
                if measured.reached_cap() {
                    measured.truncated = true;
                    return Ok(measured);
                }
            }
        }
        Ok(measured)
    }

    /// Removes a file, a link, or a whole directory.
    ///
    /// Links are removed as links. Following one would delete whatever it
    /// points at, which may be nowhere near what the user selected.
    pub async fn remove(&self, path: &str) -> Result<()> {
        let meta = tokio::fs::symlink_metadata(path)
            .await
            .map_err(|source| at(path, source))?;
        if meta.file_type().is_symlink() || !meta.is_dir() {
            tokio::fs::remove_file(path)
                .await
                .map_err(|source| at(path, source))
        } else {
            tokio::fs::remove_dir_all(path)
                .await
                .map_err(|source| at(path, source))
        }
    }

    /// Sets the nine permission bits, optionally through a whole tree.
    ///
    /// Links are skipped: there is no portable way to change a link's own bits,
    /// and changing the target's instead would reach outside the selection.
    pub async fn set_permissions(&self, path: &str, mode: u32, recursive: bool) -> Result<()> {
        let meta = tokio::fs::symlink_metadata(path)
            .await
            .map_err(|source| at(path, source))?;
        if meta.file_type().is_symlink() {
            return Ok(());
        }

        set_mode(path, mode).await?;
        if !recursive || !meta.is_dir() {
            return Ok(());
        }

        let mut pending = vec![PathBuf::from(path)];
        while let Some(directory) = pending.pop() {
            let Ok(mut reader) = tokio::fs::read_dir(&directory).await else {
                continue;
            };
            while let Ok(Some(entry)) = reader.next_entry().await {
                let child = entry.path();
                let Ok(meta) = tokio::fs::symlink_metadata(&child).await else {
                    continue;
                };
                if meta.file_type().is_symlink() {
                    continue;
                }
                let _ = set_mode(&child.to_string_lossy(), mode).await;
                if meta.is_dir() {
                    pending.push(child);
                }
            }
        }
        Ok(())
    }
}

impl LocalSession {
    /// Opens a file for reading, positioned at `offset`.
    pub async fn open_read(&self, path: &str, offset: u64) -> Result<Reader> {
        use tokio::io::AsyncSeekExt;

        let mut file = tokio::fs::File::open(path)
            .await
            .map_err(|source| at(path, source))?;
        if offset > 0 {
            file.seek(std::io::SeekFrom::Start(offset))
                .await
                .map_err(|source| at(path, source))?;
        }
        Ok(Reader::Local(file))
    }

    /// Opens a file for writing, positioned at `offset`.
    ///
    /// Writing at an offset never truncates: that is what continuing a broken
    /// transfer means. Starting from zero does truncate, because then the file
    /// is being replaced rather than continued.
    pub async fn open_write(&self, path: &str, offset: u64) -> Result<Writer> {
        use tokio::io::AsyncSeekExt;

        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(offset == 0)
            .open(path)
            .await
            .map_err(|source| at(path, source))?;
        if offset > 0 {
            file.seek(std::io::SeekFrom::Start(offset))
                .await
                .map_err(|source| at(path, source))?;
        }
        Ok(Writer::Local(file))
    }

    /// Renames over whatever is there.
    ///
    /// The opposite of [`LocalSession::rename`], which refuses to overwrite:
    /// this is the last step of a transfer, where replacing the old file is
    /// exactly the intention.
    pub async fn replace(&self, from: &str, to: &str) -> Result<()> {
        tokio::fs::rename(from, to)
            .await
            .map_err(|source| at(from, source))
    }

    /// Size and modification time, for deciding whether a resume is safe.
    pub async fn stat(&self, path: &str) -> Result<(u64, Option<i64>)> {
        let meta = tokio::fs::metadata(path)
            .await
            .map_err(|source| at(path, source))?;
        Ok((meta.len(), modified_seconds(&meta)))
    }

    /// Sets a file's modification time, for carrying it across a transfer.
    pub async fn set_modified(&self, path: &str, seconds: i64) -> Result<()> {
        let when = if seconds >= 0 {
            std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds as u64)
        } else {
            std::time::UNIX_EPOCH - std::time::Duration::from_secs(seconds.unsigned_abs())
        };
        let file = std::fs::File::options()
            .write(true)
            .open(path)
            .map_err(|source| at(path, source))?;
        file.set_modified(when).map_err(|source| at(path, source))
    }
}

#[cfg(unix)]
async fn set_mode(path: &str, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        .await
        .map_err(|source| at(path, source))
}

#[cfg(not(unix))]
async fn set_mode(_path: &str, _mode: u32) -> Result<()> {
    // Windows has no mode bits. Pretending to set them would be a lie the
    // window then displays back.
    Ok(())
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
    async fn a_folder_and_a_file_can_be_made_and_renamed() {
        let root = scratch("create");
        let session = LocalSession::new();
        let base = root.to_string_lossy().into_owned();

        session
            .create_dir(&session.join(&base, "images"))
            .await
            .unwrap();
        session
            .create_file(&session.join(&base, "notes.txt"))
            .await
            .unwrap();

        let listing = session.list_dir(&base).await.unwrap();
        assert_eq!(listing.entries.len(), 2);

        session
            .rename(
                &session.join(&base, "notes.txt"),
                &session.join(&base, "readme.txt"),
            )
            .await
            .unwrap();
        let listing = session.list_dir(&base).await.unwrap();
        assert!(listing.entries.iter().any(|e| e.name == "readme.txt"));
        assert!(!listing.entries.iter().any(|e| e.name == "notes.txt"));

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[tokio::test]
    async fn creating_something_that_is_there_does_not_overwrite_it() {
        let root = scratch("create-twice");
        let session = LocalSession::new();
        let path = session.join(&root.to_string_lossy(), "keep.txt");
        std::fs::write(&path, b"important").unwrap();

        let error = session.create_file(&path).await.expect_err("must refuse");
        assert!(matches!(
            error,
            Error::Path {
                reason: PathProblem::AlreadyExists,
                ..
            }
        ));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "important");

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[tokio::test]
    async fn renaming_onto_an_existing_name_is_refused() {
        let root = scratch("rename-clash");
        let session = LocalSession::new();
        let base = root.to_string_lossy().into_owned();
        std::fs::write(session.join(&base, "a.txt"), b"a").unwrap();
        std::fs::write(session.join(&base, "b.txt"), b"b").unwrap();

        let error = session
            .rename(&session.join(&base, "a.txt"), &session.join(&base, "b.txt"))
            .await
            .expect_err("must refuse");
        assert!(matches!(
            error,
            Error::Path {
                reason: PathProblem::AlreadyExists,
                ..
            }
        ));
        // Both are still there, with their own contents.
        assert_eq!(
            std::fs::read_to_string(session.join(&base, "b.txt")).unwrap(),
            "b"
        );

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn deleting_a_link_leaves_what_it_points_at_alone() {
        let root = scratch("delete-link");
        let session = LocalSession::new();
        let base = root.to_string_lossy().into_owned();
        let treasure = root.join("treasure");
        std::fs::create_dir(&treasure).unwrap();
        std::fs::write(treasure.join("gold.txt"), b"gold").unwrap();
        std::os::unix::fs::symlink(&treasure, root.join("shortcut")).unwrap();

        session
            .remove(&session.join(&base, "shortcut"))
            .await
            .unwrap();

        assert!(treasure.exists(), "the link's target must survive");
        assert!(treasure.join("gold.txt").exists());
        assert!(!root.join("shortcut").exists());

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[tokio::test]
    async fn a_measurement_counts_the_tree_before_it_is_removed() {
        let root = scratch("measure");
        let session = LocalSession::new();
        std::fs::create_dir_all(root.join("a/b")).unwrap();
        std::fs::write(root.join("one.txt"), b"12345").unwrap();
        std::fs::write(root.join("a/two.txt"), b"123").unwrap();
        std::fs::write(root.join("a/b/three.txt"), b"1").unwrap();

        let measured = session.measure(&root.to_string_lossy()).await.unwrap();
        assert_eq!(measured.files, 3);
        assert_eq!(measured.directories, 3, "the folder itself counts too");
        assert_eq!(measured.bytes, 9);
        assert!(!measured.is_at_cap());

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_measurement_does_not_walk_into_a_link() {
        let root = scratch("measure-link");
        let session = LocalSession::new();
        let elsewhere = root.join("elsewhere");
        std::fs::create_dir(&elsewhere).unwrap();
        for index in 0..5 {
            std::fs::write(elsewhere.join(format!("{index}.txt")), b"x").unwrap();
        }
        let here = root.join("here");
        std::fs::create_dir(&here).unwrap();
        std::os::unix::fs::symlink(&elsewhere, here.join("link")).unwrap();

        let measured = session.measure(&here.to_string_lossy()).await.unwrap();
        assert_eq!(measured.symlinks, 1);
        assert_eq!(measured.files, 0, "the files behind the link are not ours");

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn permissions_can_be_set_through_a_whole_tree_except_links() {
        use std::os::unix::fs::PermissionsExt;

        let root = scratch("chmod");
        let session = LocalSession::new();
        std::fs::create_dir_all(root.join("inner")).unwrap();
        std::fs::write(root.join("inner/file.txt"), b"x").unwrap();
        let outside = root.join("outside.txt");
        std::fs::write(&outside, b"x").unwrap();
        std::fs::set_permissions(&outside, std::fs::Permissions::from_mode(0o600)).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("inner/link")).unwrap();

        session
            .set_permissions(&root.join("inner").to_string_lossy(), 0o750, true)
            .await
            .unwrap();

        let mode = |p: &std::path::Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&root.join("inner")), 0o750);
        assert_eq!(mode(&root.join("inner/file.txt")), 0o750);
        // The link was skipped, so what it points at kept its own bits.
        assert_eq!(mode(&outside), 0o600);

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
