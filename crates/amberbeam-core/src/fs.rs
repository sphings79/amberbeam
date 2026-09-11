//! What a directory listing looks like, wherever it came from.
//!
//! One shape for the local disk and for every server: the panes draw the same
//! rows either way, and the transfer engine of M2 takes the same entries no
//! matter which side they were read from.

use serde::{Deserialize, Serialize};

/// What an entry is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EntryKind {
    File,
    Directory,
    /// A symbolic link. Where it points is in [`DirEntry::link_target`], and
    /// `kind_of_target` says what waits at the other end when that is known.
    Symlink,
    /// Sockets, fifos, devices. Listed, never transferred.
    Other,
}

/// Access bits as a server reports them, in the shape `ls` prints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Permissions(pub u32);

impl Permissions {
    /// The nine rwx bits as text, without the leading type character.
    pub fn to_rwx(self) -> String {
        const FLAGS: [(u32, char); 9] = [
            (0o400, 'r'),
            (0o200, 'w'),
            (0o100, 'x'),
            (0o040, 'r'),
            (0o020, 'w'),
            (0o010, 'x'),
            (0o004, 'r'),
            (0o002, 'w'),
            (0o001, 'x'),
        ];
        FLAGS
            .iter()
            .map(
                |(bit, letter)| {
                    if self.0 & bit != 0 {
                        *letter
                    } else {
                        '-'
                    }
                },
            )
            .collect()
    }
}

/// One row in a file list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirEntry {
    /// File name only, never a path. Kept as the server sent it.
    pub name: String,
    pub kind: EntryKind,
    /// Size in bytes. `None` where the endpoint reports none, as for most
    /// directories.
    pub size: Option<u64>,
    /// Modification time in seconds since the Unix epoch. `None` when the
    /// endpoint reports none — and then a resume must never trust it, see
    /// `ResumeMarker`.
    pub modified: Option<i64>,
    pub permissions: Option<Permissions>,
    /// Owner as text where the endpoint gives a name, otherwise the numeric id.
    /// SFTP hands out numbers, so both cases occur.
    pub owner: Option<String>,
    pub group: Option<String>,
    /// Where a symlink points, verbatim.
    pub link_target: Option<String>,
    /// What a symlink points at, when following it was cheap enough to find
    /// out. Directories have to be recognised or the panes cannot be entered.
    pub kind_of_target: Option<EntryKind>,
}

impl DirEntry {
    /// Whether entering this row leads into a directory.
    pub fn is_directory(&self) -> bool {
        match self.kind {
            EntryKind::Directory => true,
            EntryKind::Symlink => self.kind_of_target == Some(EntryKind::Directory),
            _ => false,
        }
    }

    /// Whether the entry is hidden by convention. Filtering is the frontend's
    /// business; recognising it is not.
    pub fn is_hidden(&self) -> bool {
        self.name.starts_with('.')
    }
}

/// A finished listing of one directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    /// Absolute path that was read, in the endpoint's own notation.
    pub path: String,
    pub entries: Vec<DirEntry>,
}

/// Joins a directory and a name the way a server expects it.
///
/// SFTP paths are POSIX, whatever the operating system underneath. Local paths
/// on Windows are not, which is why the local session does its joining with
/// `PathBuf` instead of calling this.
pub fn join_remote(directory: &str, name: &str) -> String {
    if directory.ends_with('/') {
        format!("{directory}{name}")
    } else {
        format!("{directory}/{name}")
    }
}

/// The directory above a POSIX path, or `None` at the root.
pub fn parent_remote(path: &str) -> Option<String> {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    match trimmed.rfind('/') {
        Some(0) => Some("/".to_string()),
        Some(index) => Some(trimmed[..index].to_string()),
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, kind: EntryKind) -> DirEntry {
        DirEntry {
            name: name.into(),
            kind,
            size: None,
            modified: None,
            permissions: None,
            owner: None,
            group: None,
            link_target: None,
            kind_of_target: None,
        }
    }

    #[test]
    fn permissions_read_like_ls() {
        assert_eq!(Permissions(0o755).to_rwx(), "rwxr-xr-x");
        assert_eq!(Permissions(0o640).to_rwx(), "rw-r-----");
        assert_eq!(Permissions(0).to_rwx(), "---------");
    }

    #[test]
    fn a_symlink_counts_as_a_directory_only_when_it_points_at_one() {
        let mut link = entry("current", EntryKind::Symlink);
        assert!(!link.is_directory());
        link.kind_of_target = Some(EntryKind::Directory);
        assert!(link.is_directory());
    }

    #[test]
    fn joining_never_doubles_the_separator() {
        assert_eq!(join_remote("/var/www", "index.html"), "/var/www/index.html");
        assert_eq!(join_remote("/", "etc"), "/etc");
    }

    #[test]
    fn walking_up_ends_at_the_root() {
        assert_eq!(parent_remote("/var/www/html"), Some("/var/www".into()));
        assert_eq!(parent_remote("/var"), Some("/".into()));
        assert_eq!(parent_remote("/"), None);
    }

    #[test]
    fn a_trailing_separator_does_not_confuse_the_way_up() {
        assert_eq!(parent_remote("/var/www/"), Some("/var".into()));
    }
}
