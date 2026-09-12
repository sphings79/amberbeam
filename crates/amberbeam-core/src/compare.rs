//! Telling two directories apart.
//!
//! Both sides are listed, matched up by name, and every pair is judged. What
//! comes out is a list somebody can look at before anything moves — which is
//! the point: a comparison that queued its own findings would be a program
//! that decides for you what "the same" means.
//!
//! That decision is the hard part, and it is deliberately in the open. Two
//! files of the same length are not the same file, and two timestamps that
//! differ by a second usually are. Every rule here is a compromise between
//! reading everything and trusting what a server says about itself.

use serde::{Deserialize, Serialize};

use crate::endpoint::EndpointId;
use crate::error::{Error, Result};
use crate::events::{Event, Events};
use crate::fs::{DirEntry, EntryKind};
use crate::registry::Sessions;

/// How much two modification times may differ and still count as one moment.
///
/// FTP reports `MDTM` to the second at best, plenty of servers round to the
/// minute, and a file written across a network arrives a moment after it was
/// read. Two seconds is the smallest window that does not report a file as
/// changed for having been copied.
const TOLERANCE: i64 = 2;

/// The most directories one comparison will read.
///
/// A recursive compare walks both trees, and a tree with a `node_modules` in
/// it is tens of thousands of directories and twice as many listings. The cap
/// is high enough for anything anybody keeps on a web server and low enough
/// that a wrong turn ends rather than runs all night.
const MOST_DIRECTORIES: usize = 5_000;

/// What the two sides are judged by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum How {
    /// Size alone. Cheap, and blind to any change that keeps the length.
    Size,
    /// Size, and then the modification time within [`TOLERANCE`].
    SizeAndTime,
    /// Size, and then what the bytes actually are.
    ///
    /// The timestamps are not consulted at all: a digest is better evidence
    /// than a clock, and a file copied about has the wrong time far more often
    /// than it has the wrong contents. It reads both files end to end, which
    /// over a server is the cost of transferring them.
    Checksum,
}

/// What was found about one name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Difference {
    /// On the left only.
    OnlyHere,
    /// On the right only.
    OnlyThere,
    /// On both, and not the same.
    Different,
    /// On both, as far as the chosen rule can tell.
    Same,
}

/// One side of one row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Seen {
    pub size: Option<u64>,
    pub modified: Option<i64>,
}

/// One name, as both sides have it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    /// Where it sits below the two directories being compared, with forward
    /// slashes whatever either side writes. Both sides are addressed by
    /// joining this onto their own root, so neither notation leaks into the
    /// other.
    pub path: String,
    pub name: String,
    pub kind: EntryKind,
    pub state: Difference,
    pub here: Option<Seen>,
    pub there: Option<Seen>,
}

/// What a comparison found.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Comparison {
    pub rows: Vec<Row>,
    /// How many directories were listed, on both sides together.
    pub directories: usize,
    /// True when the walk stopped at [`MOST_DIRECTORIES`] rather than at the
    /// end. Said out loud: a list that is quietly incomplete is worse than no
    /// list, because everything missing from it looks like agreement.
    pub cut_short: bool,
}

/// What one comparison was asked to do.
#[derive(Debug, Clone)]
pub struct Asking {
    pub here: (EndpointId, String),
    pub there: (EndpointId, String),
    pub recursive: bool,
    pub how: How,
    /// Names never looked at, as patterns. See [`excluded`].
    pub excludes: Vec<String>,
}

/// Walks both sides and says what differs.
pub async fn compare(sessions: &Sessions, it: &Asking, events: &Events) -> Result<Comparison> {
    let mut found = Comparison::default();
    walk(sessions, it, "", &mut found, events).await?;
    // Deepest last, and alphabetical within a directory: the order somebody
    // reads a tree in, and the order a transfer has to happen in anyway.
    found.rows.sort_by_key(|row| row.path.to_lowercase());
    Ok(found)
}

async fn walk(
    sessions: &Sessions,
    it: &Asking,
    below: &str,
    found: &mut Comparison,
    events: &Events,
) -> Result<()> {
    if found.cut_short {
        return Ok(());
    }
    if found.directories >= MOST_DIRECTORIES {
        found.cut_short = true;
        return Ok(());
    }

    let here_path = beneath(sessions, &it.here.0, &it.here.1, below).await?;
    let there_path = beneath(sessions, &it.there.0, &it.there.1, below).await?;

    // A side that cannot be read is not a side that is empty. Everything in
    // the other one would come out as "only here", and acting on that would
    // upload a whole tree because a directory was briefly unreadable.
    let here = sessions.list_dir(&it.here.0, &here_path).await?;
    let there = match sessions.list_dir(&it.there.0, &there_path).await {
        Ok(listing) => listing,
        // Except when it is simply not there yet, which is the ordinary case
        // for a directory that has never been uploaded.
        Err(Error::Path { .. }) => crate::fs::Listing {
            path: there_path.clone(),
            entries: Vec::new(),
        },
        Err(other) => return Err(other),
    };
    found.directories += 2;
    events.emit(Event::Comparing {
        directories: found.directories,
        rows: found.rows.len(),
    });

    let mut deeper: Vec<String> = Vec::new();

    for entry in here.entries.iter().filter(|entry| worth_it(entry, it)) {
        let path = join(below, &entry.name);
        let twin = there
            .entries
            .iter()
            .find(|other| other.name == entry.name && worth_it(other, it));

        match twin {
            None => {
                found
                    .rows
                    .push(row(&path, entry, Difference::OnlyHere, None));
                // A directory that is only here still has to be walked, or
                // "upload what differs" would make the directory and leave it
                // empty.
                if it.recursive && entry.is_directory() {
                    deeper.push(path);
                }
            }
            Some(twin) => {
                let both_directories = entry.is_directory() && twin.is_directory();
                if both_directories {
                    found
                        .rows
                        .push(row(&path, entry, Difference::Same, Some(seen(twin))));
                    if it.recursive {
                        deeper.push(path);
                    }
                    continue;
                }
                // One a directory and the other a file is not something to
                // resolve by copying. It is said and left alone.
                let state = if entry.is_directory() != twin.is_directory() {
                    Difference::Different
                } else {
                    judge(sessions, it, entry, twin, &here_path, &there_path).await?
                };
                found.rows.push(row(&path, entry, state, Some(seen(twin))));
            }
        }
    }

    for entry in there.entries.iter().filter(|entry| worth_it(entry, it)) {
        if here.entries.iter().any(|other| other.name == entry.name) {
            continue;
        }
        let path = join(below, &entry.name);
        let mut only = row(&path, entry, Difference::OnlyThere, Some(seen(entry)));
        only.here = None;
        found.rows.push(only);
    }

    for path in deeper {
        Box::pin(walk(sessions, it, &path, found, events)).await?;
    }
    Ok(())
}

/// Judges two files that exist on both sides.
///
/// Size first, whatever the rule: two files of different lengths are
/// different, and nothing is gained by reading either of them to find that
/// out.
async fn judge(
    sessions: &Sessions,
    it: &Asking,
    here: &DirEntry,
    there: &DirEntry,
    here_dir: &str,
    there_dir: &str,
) -> Result<Difference> {
    if let Some(settled) = by_listing(it.how, here, there) {
        return Ok(settled);
    }

    match it.how {
        How::Checksum => {
            let one = sessions
                .digest(
                    &it.here.0,
                    &sessions.join(&it.here.0, here_dir, &here.name).await?,
                )
                .await?;
            let other = sessions
                .digest(
                    &it.there.0,
                    &sessions.join(&it.there.0, there_dir, &there.name).await?,
                )
                .await?;
            Ok(if one == other {
                Difference::Same
            } else {
                Difference::Different
            })
        }
        // Everything else was settled by the listings alone.
        _ => Ok(Difference::Same),
    }
}

/// The judgement as far as the two listings can take it.
///
/// `None` means the rule cannot be settled without reading the files, which
/// only [`How::Checksum`] ever asks for. Separate so that what "the same"
/// means can be tested without a server on the other end of it.
fn by_listing(how: How, here: &DirEntry, there: &DirEntry) -> Option<Difference> {
    if here.size != there.size {
        return Some(Difference::Different);
    }
    match how {
        How::Size => Some(Difference::Same),
        How::SizeAndTime => Some(match (here.modified, there.modified) {
            (Some(one), Some(other)) if (one - other).abs() > TOLERANCE => Difference::Different,
            // A side that reports no time at all cannot disagree about one.
            // Calling that "different" would queue every file on a server
            // whose listing has no dates, every single time.
            _ => Difference::Same,
        }),
        // Same size is where a digest becomes worth reading for.
        How::Checksum => None,
    }
}

/// Whether an entry takes part in the comparison at all.
fn worth_it(entry: &DirEntry, it: &Asking) -> bool {
    // Sockets, fifos and devices are listed everywhere and transferred
    // nowhere. A comparison that reported them would be a list of rows nobody
    // can act on.
    if matches!(entry.kind, EntryKind::Other) {
        return false;
    }
    !excluded(&entry.name, &it.excludes)
}

/// Whether a name matches any of the patterns somebody typed.
///
/// `*` stands for any run of characters and is the only thing understood.
/// These are patterns written by a person into a settings field, so the match
/// ignores case: somebody who typed `.ds_store` meant the file that is
/// actually called `.DS_Store`.
pub fn excluded(name: &str, patterns: &[String]) -> bool {
    let name = name.to_lowercase();
    patterns.iter().any(|pattern| {
        let pattern = pattern.trim().to_lowercase();
        !pattern.is_empty() && matches(&name, &pattern)
    })
}

fn matches(name: &str, pattern: &str) -> bool {
    let mut parts = pattern.split('*');
    let Some(first) = parts.next() else {
        return false;
    };
    if !name.starts_with(first) {
        return false;
    }
    let mut at = first.len();
    let mut last: Option<&str> = None;
    for part in parts {
        last = Some(part);
        if part.is_empty() {
            continue;
        }
        match name[at..].find(part) {
            Some(found) => at += found + part.len(),
            None => return false,
        }
    }
    match last {
        // No star at all: the whole name had to be the pattern.
        None => name.len() == first.len(),
        // A trailing star matches whatever is left; anything else had to land
        // at the very end.
        Some("") => true,
        Some(tail) => name.ends_with(tail) && at <= name.len(),
    }
}

fn seen(entry: &DirEntry) -> Seen {
    Seen {
        size: entry.size,
        modified: entry.modified,
    }
}

fn row(path: &str, entry: &DirEntry, state: Difference, there: Option<Seen>) -> Row {
    Row {
        path: path.to_string(),
        name: entry.name.clone(),
        kind: entry.kind,
        state,
        here: Some(seen(entry)),
        there,
    }
}

fn join(below: &str, name: &str) -> String {
    if below.is_empty() {
        name.to_string()
    } else {
        format!("{below}/{name}")
    }
}

/// The path of a relative place on one side, in that side's own notation.
async fn beneath(
    sessions: &Sessions,
    endpoint: &EndpointId,
    root: &str,
    below: &str,
) -> Result<String> {
    let mut path = root.to_string();
    for part in below.split('/').filter(|part| !part.is_empty()) {
        path = sessions.join(endpoint, &path, part).await?;
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str, size: u64, modified: Option<i64>) -> DirEntry {
        DirEntry {
            name: name.into(),
            kind: EntryKind::File,
            size: Some(size),
            modified,
            permissions: None,
            owner: None,
            group: None,
            link_target: None,
            kind_of_target: None,
        }
    }

    #[test]
    fn a_pattern_matches_the_names_somebody_meant() {
        let list = |patterns: &[&str]| patterns.iter().map(|p| p.to_string()).collect::<Vec<_>>();

        assert!(excluded(".git", &list(&[".git"])));
        assert!(
            !excluded(".gitignore", &list(&[".git"])),
            "not a prefix match"
        );
        assert!(excluded(".gitignore", &list(&[".git*"])));
        assert!(excluded("error.log", &list(&["*.log"])));
        assert!(!excluded("logbook", &list(&["*.log"])));
        assert!(excluded("node_modules", &list(&["node_modules", "*.tmp"])));

        // Typed by a person into a field, so the case they typed is not the
        // case the file has.
        assert!(excluded(".DS_Store", &list(&[".ds_store"])));
        assert!(excluded("Thumbs.db", &list(&["thumbs.db"])));

        // A star on its own would exclude everything, and an empty line in a
        // list is not a pattern at all.
        assert!(excluded("anything", &list(&["*"])));
        assert!(!excluded("anything", &list(&["", "   "])));
    }

    #[test]
    fn what_counts_as_the_same_file() {
        // Size alone: blind on purpose, and cheap.
        assert_eq!(
            by(How::Size, 10, Some(100), 10, Some(9_000)),
            Difference::Same
        );
        assert_eq!(by(How::Size, 10, None, 11, None), Difference::Different);

        // Size and time, with a window wide enough that copying a file does
        // not make it look changed.
        assert_eq!(
            by(How::SizeAndTime, 10, Some(1_000), 10, Some(1_002)),
            Difference::Same,
            "two seconds is within one moment"
        );
        assert_eq!(
            by(How::SizeAndTime, 10, Some(1_000), 10, Some(1_003)),
            Difference::Different
        );
        // A listing with no dates in it must not report every file as changed.
        assert_eq!(
            by(How::SizeAndTime, 10, Some(1_000), 10, None),
            Difference::Same
        );
        assert_eq!(by(How::SizeAndTime, 10, None, 10, None), Difference::Same);

        // With checksums the listings settle only the easy half: a different
        // length needs no reading, and the same length is exactly when the
        // bytes have to be looked at.
        assert_eq!(
            by_listing(
                How::Checksum,
                &file("a", 10, Some(1)),
                &file("a", 11, Some(1))
            ),
            Some(Difference::Different)
        );
        assert_eq!(
            by_listing(
                How::Checksum,
                &file("a", 10, Some(1)),
                &file("a", 10, Some(9_000))
            ),
            None,
            "a clock is not evidence when a digest was asked for"
        );
    }

    /// The real rule, asked the way the walk asks it.
    fn by(how: How, one: u64, when: Option<i64>, other: u64, then: Option<i64>) -> Difference {
        by_listing(
            how,
            &file("index.php", one, when),
            &file("index.php", other, then),
        )
        .expect("these rules need no file read")
    }
}
