//! What can be done to files, beyond reading them.
//!
//! Creating, renaming, deleting, setting permissions. Two rules run through
//! all of it:
//!
//! * **A symbolic link is a file, not the thing it points at.** Deleting a link
//!   removes the link; setting permissions on a tree skips links rather than
//!   quietly changing the target's. Anything else means a program that deletes
//!   somewhere the user was not looking.
//! * **Recursion is counted before it is carried out.** A deletion that says
//!   how much it is about to remove can be refused in time; one that starts
//!   and then reports cannot.

use serde::{Deserialize, Serialize};

/// What a recursive operation is about to touch.
///
/// Counting stops at [`Measurement::CAP`]. An exact count of a tree with a
/// million files costs more than the warning is worth, and "more than ten
/// thousand" answers the only question the user actually has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Measurement {
    pub files: u64,
    pub directories: u64,
    /// Links are counted separately, because they are what a careless delete
    /// gets wrong.
    pub symlinks: u64,
    pub bytes: u64,
    /// True when counting stopped early, so the window can say "more than".
    pub truncated: bool,
}

impl Measurement {
    /// How many entries are counted before the answer is good enough.
    pub const CAP: u64 = 10_000;

    pub fn total(&self) -> u64 {
        self.files + self.directories + self.symlinks
    }

    pub fn is_at_cap(&self) -> bool {
        self.truncated
    }

    pub(crate) fn add_file(&mut self, bytes: Option<u64>) {
        self.files += 1;
        self.bytes += bytes.unwrap_or(0);
    }

    pub(crate) fn add_directory(&mut self) {
        self.directories += 1;
    }

    pub(crate) fn add_symlink(&mut self) {
        self.symlinks += 1;
    }

    pub(crate) fn reached_cap(&self) -> bool {
        self.total() >= Self::CAP
    }
}

/// The nine permission bits, as a dialog with nine boxes thinks of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionBits {
    pub owner_read: bool,
    pub owner_write: bool,
    pub owner_execute: bool,
    pub group_read: bool,
    pub group_write: bool,
    pub group_execute: bool,
    pub other_read: bool,
    pub other_write: bool,
    pub other_execute: bool,
}

impl PermissionBits {
    pub fn from_mode(mode: u32) -> Self {
        Self {
            owner_read: mode & 0o400 != 0,
            owner_write: mode & 0o200 != 0,
            owner_execute: mode & 0o100 != 0,
            group_read: mode & 0o040 != 0,
            group_write: mode & 0o020 != 0,
            group_execute: mode & 0o010 != 0,
            other_read: mode & 0o004 != 0,
            other_write: mode & 0o002 != 0,
            other_execute: mode & 0o001 != 0,
        }
    }

    pub fn to_mode(self) -> u32 {
        let mut mode = 0;
        for (flag, bit) in [
            (self.owner_read, 0o400),
            (self.owner_write, 0o200),
            (self.owner_execute, 0o100),
            (self.group_read, 0o040),
            (self.group_write, 0o020),
            (self.group_execute, 0o010),
            (self.other_read, 0o004),
            (self.other_write, 0o002),
            (self.other_execute, 0o001),
        ] {
            if flag {
                mode |= bit;
            }
        }
        mode
    }
}

/// Rejects a name that is not a name.
///
/// Everything typed into a rename box or a "new folder" box ends up joined to a
/// directory and sent to a server. A name carrying a separator or `..` would
/// land somewhere else entirely, which is the same mistake as trusting a path a
/// server sent — section 12 names that one explicitly, and it holds in both
/// directions.
pub fn is_usable_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains('\0')
        && name.trim() == name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nine_boxes_and_an_octal_number_say_the_same_thing() {
        assert_eq!(PermissionBits::from_mode(0o755).to_mode(), 0o755);
        assert_eq!(PermissionBits::from_mode(0o640).to_mode(), 0o640);
        assert_eq!(PermissionBits::from_mode(0).to_mode(), 0);

        let bits = PermissionBits::from_mode(0o644);
        assert!(bits.owner_read && bits.owner_write && !bits.owner_execute);
        assert!(bits.group_read && !bits.group_write);
        assert!(bits.other_read && !bits.other_write);
    }

    #[test]
    fn bits_outside_the_nine_are_dropped() {
        // Setuid and the sticky bit are not something a nine box dialog can
        // represent, and silently carrying them through would be worse.
        assert_eq!(PermissionBits::from_mode(0o4755).to_mode(), 0o755);
    }

    #[test]
    fn a_name_that_is_really_a_path_is_refused() {
        assert!(is_usable_name("index.html"));
        assert!(is_usable_name(".htaccess"));
        assert!(is_usable_name("Größe & Maß.txt"));

        assert!(!is_usable_name(""));
        assert!(!is_usable_name("."));
        assert!(!is_usable_name(".."));
        assert!(!is_usable_name("../etc/passwd"));
        assert!(!is_usable_name("a/b"));
        assert!(!is_usable_name("a\\b"));
        assert!(!is_usable_name(" leading"));
        assert!(!is_usable_name("trailing "));
    }

    #[test]
    fn a_measurement_knows_when_it_gave_up_counting() {
        let mut measured = Measurement::default();
        for _ in 0..Measurement::CAP {
            measured.add_file(Some(10));
        }
        assert!(measured.reached_cap());
        assert_eq!(measured.bytes, Measurement::CAP * 10);
    }

    #[test]
    fn links_are_counted_apart_from_files() {
        let mut measured = Measurement::default();
        measured.add_file(Some(100));
        measured.add_symlink();
        measured.add_directory();
        assert_eq!(measured.total(), 3);
        // A link's target is not part of what a deletion removes.
        assert_eq!(measured.bytes, 100);
    }
}
