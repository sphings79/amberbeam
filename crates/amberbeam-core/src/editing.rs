//! Working on a file that lives on a server.
//!
//! A copy comes down, something edits it, and what comes back goes up again.
//! That is the whole idea, and everything here exists because of the three
//! ways it can go wrong quietly:
//!
//! * **The file is not text.** A program that hands a JPEG to a text editor
//!   gets a JPEG back with its bytes mangled, and nobody sees it until the
//!   image will not open.
//! * **The file is not UTF-8.** Plenty of servers still hold Latin-1, and
//!   reading one as UTF-8 and writing it back as UTF-8 rewrites every umlaut
//!   in the file. So the encoding is read off the bytes and written back as it
//!   was found — including the byte order mark and the line endings, which are
//!   the other two things an editor silently rewrites.
//! * **Somebody else changed it.** A copy taken twenty minutes ago and written
//!   back now would erase whatever happened in between. What the file looked
//!   like when it was taken is kept, and the write-back checks before it
//!   writes.
//!
//! The copies do not go through the queue. Every save would be a line in it,
//! and the queue is for things somebody asked to transfer — not for the
//! twenty times an editor writes a file in an afternoon.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::endpoint::EndpointId;
use crate::engine::Progress;
use crate::error::{Error, Result};
use crate::registry::{Sessions, TransferRun, LOCAL};

/// The most a file may be for editing to be offered.
///
/// Not a judgement about editors, which handle far more: the text travels
/// through a command as one string, and a person who asks to edit a 400 MB log
/// wants a different tool than this one.
pub const LARGEST: u64 = 20 * 1024 * 1024;

/// How much of a file decides whether it is text.
const SNIFFED: usize = 8 * 1024;

const BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];

/// What opens a file of a given kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OpenWith {
    /// AmberBeam's own editor.
    Own,
    /// Whatever this machine opens that kind of file with.
    System,
    /// One named program, in [`EditRule::program`].
    Program,
}

/// One line of the table that says what may be edited, and with what.
///
/// The same list answers both questions on purpose. "Which files are text" and
/// "what opens them" are one decision with two halves, and keeping them in two
/// lists would mean a file that is editable according to one and not according
/// to the other.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditRule {
    /// Extensions without the dot, or whole file names for the ones that have
    /// no extension at all.
    pub extensions: Vec<String>,
    pub open_with: OpenWith,
    /// The program, when that is what this line says. A path on the machine
    /// the program runs on, which is why it means nothing in a browser.
    #[serde(default)]
    pub program: Option<String>,
}

impl EditRule {
    /// What a fresh installation starts with.
    ///
    /// The files somebody opens a transfer program to fix: what a web server
    /// serves, what configures it, and what it logs. Everything else is added
    /// by hand, which is also how anything is taken away.
    pub fn shipped() -> Vec<Self> {
        let text = [
            "php",
            "phtml",
            "inc",
            "html",
            "htm",
            "twig",
            "css",
            "scss",
            "less",
            "js",
            "mjs",
            "cjs",
            "ts",
            "json",
            "xml",
            "yml",
            "yaml",
            "toml",
            "md",
            "txt",
            "ini",
            "conf",
            "cfg",
            "env",
            "sql",
            "sh",
            "bash",
            "py",
            "rb",
            "pl",
            "log",
            "csv",
            "svg",
            "htaccess",
            "gitignore",
            "dockerfile",
            "makefile",
        ];
        vec![Self {
            extensions: text.iter().map(|name| name.to_string()).collect(),
            open_with: OpenWith::Own,
            program: None,
        }]
    }
}

/// Which line of the table covers a file, if any.
///
/// The first that matches wins, so a line somebody added for one extension
/// beats the long one underneath it without having to be taken out of it.
pub fn how_to_open<'a>(rules: &'a [EditRule], name: &str) -> Option<&'a EditRule> {
    let lower = name.to_lowercase();
    let extension = extension_of(&lower);
    rules.iter().find(|rule| {
        rule.extensions.iter().any(|wanted| {
            let wanted = wanted.trim().trim_start_matches('.').to_lowercase();
            !wanted.is_empty() && (Some(wanted.as_str()) == extension.as_deref() || wanted == lower)
        })
    })
}

/// The part of a name that says what kind of file it is.
///
/// `.htaccess` counts as `htaccess`. A name that is nothing but a dot and a
/// word is the oldest kind of configuration file there is, and a program for
/// web servers that cannot open one would be missing the point.
fn extension_of(lower: &str) -> Option<String> {
    let (before, after) = lower.rsplit_once('.')?;
    if after.is_empty() {
        return None;
    }
    // A leading dot and nothing else before it: the word is the kind.
    let _ = before;
    Some(after.to_string())
}

/// What the bytes of a file turned out to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Encoding {
    Utf8,
    /// Everything that is not valid UTF-8, read one byte per character.
    ///
    /// Not a guess between a dozen code pages: Latin-1 is the one that still
    /// turns up on servers, and it is the one the existing per-server switch
    /// already names.
    Latin1,
}

/// One file taken off a server to be worked on.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Edit {
    pub id: String,
    pub endpoint: String,
    pub remote_path: String,
    pub name: String,
    /// Where the copy lies on this machine.
    pub local_path: String,
    pub encoding: Encoding,
    /// Seconds since the epoch, for showing the list in the order it grew.
    pub opened: i64,

    /// Whether the file began with a byte order mark.
    #[serde(skip)]
    bom: bool,
    /// Whether its first line ended with CR LF.
    #[serde(skip)]
    crlf: bool,
    /// Size and modification time of the server's file when the copy was
    /// taken, and again after each write-back. What "changed since" means.
    #[serde(skip)]
    taken: Stamp,
    /// Size and modification time of the copy after the last write this
    /// program made to it.
    ///
    /// So that a watcher can tell somebody else's save from our own. Without
    /// it, writing the copy would look like a reason to upload the copy.
    #[serde(skip)]
    written: Stamp,
}

/// Size and modification time, as far as an endpoint will say.
type Stamp = (u64, Option<i64>);

/// The files being worked on, and the copies they were taken into.
#[derive(Debug)]
pub struct Edits {
    root: PathBuf,
    open: Mutex<HashMap<String, Edit>>,
}

impl Edits {
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            open: Mutex::new(HashMap::new()),
        }
    }

    /// The usual place: a directory of our own under the system's temporary
    /// one, which is where a copy that is meant to be thrown away belongs.
    pub fn beneath_temp() -> Self {
        Self::at(std::env::temp_dir().join("amberbeam-edits"))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Everything open, oldest first.
    pub async fn list(&self) -> Vec<Edit> {
        let mut found: Vec<Edit> = self.open.lock().await.values().cloned().collect();
        found.sort_by_key(|edit| edit.opened);
        found
    }

    /// Takes a copy of a file and begins working on it.
    ///
    /// Asking twice for the same file on the same endpoint gives back the
    /// first copy rather than a second one. Two copies of one file, each
    /// unaware of the other, is a race with somebody's work as the prize.
    pub async fn begin(
        &self,
        sessions: &Sessions,
        endpoint: &EndpointId,
        path: &str,
        rules: &[EditRule],
    ) -> Result<Edit> {
        if let Some(already) = self.already(endpoint.as_str(), path).await {
            return Ok(already);
        }

        // The table decides what may be edited at all. Asked here rather than
        // in the window, because "what is a text file" must have one answer:
        // a window that offers a file the core will refuse, or refuses one the
        // core would take, is a window arguing with the program behind it.
        let name = leaf(path);
        if how_to_open(rules, &name).is_none() {
            return Err(Error::TypeNotEdited {
                path: path.to_string(),
                extension: extension_of(&name.to_lowercase()).unwrap_or_default(),
            });
        }

        let taken = sessions.stat_of(endpoint, path).await?;
        if taken.0 > LARGEST {
            return Err(Error::TooBigToEdit {
                path: path.to_string(),
                megabytes: LARGEST / (1024 * 1024),
            });
        }

        let id = new_id();
        // A directory per copy, and the file keeps the name it had. An editor
        // shows that name in its title bar and decides its highlighting by it,
        // and two files called config.php from two servers must not land on
        // top of one another.
        let folder = self.root.join(&id);
        std::fs::create_dir_all(&folder).map_err(Error::from)?;
        let local = folder.join(&name);
        let local_path = local.to_string_lossy().into_owned();

        let run = TransferRun {
            source_endpoint: endpoint.clone(),
            source_path: path.to_string(),
            target_endpoint: EndpointId::new(LOCAL),
            target_path: local_path.clone(),
            resume: None,
            // Deliberately not the server's time. The copy's own timestamp is
            // what says whether somebody has saved it since, and starting it
            // out at a time from last March would make the first save look
            // like no save at all.
            keep_modified: false,
            keep_permissions: false,
            use_temporary_name: true,
            source_permissions: None,
        };
        if let Err(why) = sessions
            .transfer(&run, &Progress::new(Some(taken.0), 0))
            .await
        {
            let _ = std::fs::remove_dir_all(&folder);
            return Err(why);
        }

        let bytes = match std::fs::read(&local) {
            Ok(bytes) => bytes,
            Err(why) => {
                let _ = std::fs::remove_dir_all(&folder);
                return Err(Error::from(why));
            }
        };
        if looks_binary(&bytes) {
            // The copy goes rather than lingering in a temporary directory
            // nobody will ever look in.
            let _ = std::fs::remove_dir_all(&folder);
            return Err(Error::NotTextToEdit {
                path: path.to_string(),
            });
        }

        let (encoding, bom, crlf) = shape_of(&bytes);
        let edit = Edit {
            id: id.clone(),
            endpoint: endpoint.as_str().to_string(),
            remote_path: path.to_string(),
            name,
            local_path,
            encoding,
            opened: now_seconds(),
            bom,
            crlf,
            taken,
            written: stamp_of(&local),
        };
        self.open.lock().await.insert(id, edit.clone());
        Ok(edit)
    }

    /// What the copy says, as text.
    pub async fn text(&self, id: &str) -> Result<String> {
        let edit = self.one(id).await?;
        let bytes = std::fs::read(&edit.local_path).map_err(Error::from)?;
        Ok(decode(&bytes, edit.encoding, edit.crlf))
    }

    /// Writes text into the copy, as the file it came from was written.
    pub async fn save(&self, id: &str, text: &str) -> Result<()> {
        let edit = self.one(id).await?;
        let bytes = encode(text, edit.encoding, edit.bom, edit.crlf)?;
        write_beside(Path::new(&edit.local_path), &bytes)?;
        self.note_our_own_write(id).await;
        Ok(())
    }

    /// Puts the copy back on the server.
    ///
    /// Refuses when the server's file is no longer the one the copy was taken
    /// from, unless told to go ahead anyway. That refusal is the whole point
    /// of keeping what it looked like: writing over somebody else's afternoon
    /// is not an error anybody notices until much later.
    pub async fn push(&self, sessions: &Sessions, id: &str, anyway: bool) -> Result<Edit> {
        let edit = self.one(id).await?;
        let endpoint = EndpointId::new(edit.endpoint.clone());

        if !anyway {
            match sessions.stat_of(&endpoint, &edit.remote_path).await {
                Ok(now) if !same_file(edit.taken, now) => {
                    return Err(Error::EditChangedOnServer {
                        path: edit.remote_path.clone(),
                    })
                }
                Ok(_) => {}
                // Not there any more. Writing it back puts it back, which is
                // what somebody with it open and unsaved work in hand means by
                // saving — and it is not the case this guard is about.
                Err(_) => {}
            }
        }

        let run = TransferRun {
            source_endpoint: EndpointId::new(LOCAL),
            source_path: edit.local_path.clone(),
            target_endpoint: endpoint.clone(),
            target_path: edit.remote_path.clone(),
            resume: None,
            keep_modified: false,
            keep_permissions: false,
            // The server sees the finished file appear under its own name or
            // nothing at all. A half-written config.php is a site that is down.
            use_temporary_name: true,
            source_permissions: None,
        };
        sessions.transfer(&run, &Progress::new(None, 0)).await?;

        // What is up there now is what the next comparison is against.
        let after = sessions
            .stat_of(&endpoint, &edit.remote_path)
            .await
            .unwrap_or(edit.taken);
        let mut open = self.open.lock().await;
        let Some(held) = open.get_mut(id) else {
            return Ok(edit);
        };
        held.taken = after;
        held.written = stamp_of(Path::new(&held.local_path));
        Ok(held.clone())
    }

    /// Stops working on one file, and throws the copy away or does not.
    pub async fn finish(&self, id: &str, delete_copy: bool) -> Option<Edit> {
        let edit = self.open.lock().await.remove(id)?;
        if delete_copy {
            let _ = std::fs::remove_dir_all(self.root.join(&edit.id));
        }
        Some(edit)
    }

    /// Stops working on everything, for the end of a session.
    pub async fn finish_all(&self, delete_copies: bool) -> Vec<Edit> {
        let all: Vec<Edit> = self
            .open
            .lock()
            .await
            .drain()
            .map(|(_, edit)| edit)
            .collect();
        if delete_copies {
            for edit in &all {
                let _ = std::fs::remove_dir_all(self.root.join(&edit.id));
            }
        }
        all
    }

    /// Copies somebody else has saved since this program last wrote them.
    ///
    /// The comparison is against our own last write and not against the time
    /// the copy was made, or saving through this program would look exactly
    /// like saving through another one and each save would upload twice.
    pub async fn saved_elsewhere(&self) -> Vec<Edit> {
        let mut moved = Vec::new();
        let mut open = self.open.lock().await;
        for edit in open.values_mut() {
            let now = stamp_of(Path::new(&edit.local_path));
            if now != (0, None) && now != edit.written {
                edit.written = now;
                moved.push(edit.clone());
            }
        }
        moved
    }

    async fn one(&self, id: &str) -> Result<Edit> {
        self.open
            .lock()
            .await
            .get(id)
            .cloned()
            .ok_or_else(|| Error::other("that file is not open for editing"))
    }

    async fn already(&self, endpoint: &str, path: &str) -> Option<Edit> {
        self.open
            .lock()
            .await
            .values()
            .find(|edit| edit.endpoint == endpoint && edit.remote_path == path)
            .cloned()
    }

    async fn note_our_own_write(&self, id: &str) {
        let mut open = self.open.lock().await;
        if let Some(edit) = open.get_mut(id) {
            edit.written = stamp_of(Path::new(&edit.local_path));
        }
    }
}

/// Whether two looks at a file found the same one.
///
/// The size has to match. The time only counts when both ends offered one:
/// plenty of FTP servers answer `MDTM` with nothing useful, and treating a
/// missing time as a difference would mean every write-back on such a server
/// asking a question nobody can answer.
fn same_file(before: Stamp, now: Stamp) -> bool {
    if before.0 != now.0 {
        return false;
    }
    match (before.1, now.1) {
        (Some(then), Some(since)) => then == since,
        _ => true,
    }
}

/// A file with a zero byte near the start is not text.
///
/// Crude on purpose. It is the one test that costs nothing and catches every
/// image, archive and compiled thing anybody might click on by mistake.
fn looks_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(SNIFFED).any(|&byte| byte == 0)
}

/// How a file was written: its encoding, its mark, and its line endings.
fn shape_of(bytes: &[u8]) -> (Encoding, bool, bool) {
    let bom = bytes.starts_with(&BOM);
    let body = if bom { &bytes[BOM.len()..] } else { bytes };
    let encoding = if std::str::from_utf8(body).is_ok() {
        Encoding::Utf8
    } else {
        Encoding::Latin1
    };
    (encoding, bom, first_ending_is_crlf(body))
}

/// The first line ending decides for the whole file.
///
/// A file of mixed endings has to be called one thing or the other, and the
/// first one is the only answer that does not depend on counting.
fn first_ending_is_crlf(bytes: &[u8]) -> bool {
    match bytes.iter().position(|&byte| byte == b'\n') {
        Some(0) | None => false,
        Some(at) => bytes[at - 1] == b'\r',
    }
}

/// The copy as text: no mark, and line endings an editor understands.
fn decode(bytes: &[u8], encoding: Encoding, crlf: bool) -> String {
    let body = if bytes.starts_with(&BOM) {
        &bytes[BOM.len()..]
    } else {
        bytes
    };
    let text = match encoding {
        // Lossy, and only reachable if the file changed underneath us between
        // being read as valid UTF-8 and being read again. Refusing to show a
        // file at that point helps nobody.
        Encoding::Utf8 => String::from_utf8_lossy(body).into_owned(),
        Encoding::Latin1 => body.iter().map(|&byte| byte as char).collect(),
    };
    if crlf {
        text.replace("\r\n", "\n")
    } else {
        text
    }
}

/// Text as the file was written, mark and line endings and all.
fn encode(text: &str, encoding: Encoding, bom: bool, crlf: bool) -> Result<Vec<u8>> {
    // Flattened first. Replacing every newline in text that already holds CR LF
    // would give CR CR LF, and a file that grows a carriage return per save is
    // a file whose every line shows as changed.
    let flat = text.replace("\r\n", "\n");
    let body = if crlf {
        flat.replace('\n', "\r\n")
    } else {
        flat
    };

    let mut out = Vec::with_capacity(body.len() + BOM.len());
    if bom {
        out.extend_from_slice(&BOM);
    }
    match encoding {
        Encoding::Utf8 => out.extend_from_slice(body.as_bytes()),
        Encoding::Latin1 => {
            for character in body.chars() {
                let point = character as u32;
                if point > 0xFF {
                    // Said rather than swallowed. The alternative is a question
                    // mark where somebody typed a word, found weeks later.
                    return Err(Error::TextDoesNotFit {
                        character: character.to_string(),
                    });
                }
                out.push(point as u8);
            }
        }
    }
    Ok(out)
}

/// Writes beside the target and renames over it, so an interrupted save leaves
/// the previous copy rather than half of the new one.
fn write_beside(path: &Path, bytes: &[u8]) -> Result<()> {
    let temporary = path.with_extension(format!(
        "{}.amberbeam-part",
        path.extension()
            .and_then(|it| it.to_str())
            .unwrap_or_default()
    ));
    std::fs::write(&temporary, bytes).map_err(Error::from)?;
    std::fs::rename(&temporary, path).map_err(Error::from)
}

fn stamp_of(path: &Path) -> Stamp {
    let Ok(data) = std::fs::metadata(path) else {
        return (0, None);
    };
    let modified = data
        .modified()
        .ok()
        .and_then(|when| when.duration_since(std::time::UNIX_EPOCH).ok())
        // Milliseconds, not seconds. Two saves in the same second are what an
        // editor does when somebody is working, and a watcher that cannot tell
        // them apart misses one.
        .map(|since| since.as_millis() as i64);
    (data.len(), modified)
}

fn leaf(path: &str) -> String {
    let name = path.trim_end_matches('/').rsplit('/').next().unwrap_or("");
    if name.is_empty() {
        "file".to_string()
    } else {
        name.to_string()
    }
}

fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs() as i64)
        .unwrap_or_default()
}

fn new_id() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_millis())
        .unwrap_or_default();
    format!("{now:x}-{:x}", COUNTER.fetch_add(1, Ordering::Relaxed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_of_bytes_is_not_offered_to_a_text_editor() {
        assert!(looks_binary(b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR"));
        assert!(!looks_binary(b"<?php\necho 'hallo';\n"));
        // A zero byte further in than anybody looks is not what this is for.
        let mut long = vec![b'a'; SNIFFED + 10];
        long[SNIFFED + 5] = 0;
        assert!(!looks_binary(&long));
    }

    #[test]
    fn a_file_that_is_not_utf8_is_read_as_latin1_and_written_back_as_latin1() {
        // "Grüße" in Latin-1, which is not valid UTF-8.
        let bytes = b"Gr\xfc\xdfe";
        let (encoding, bom, crlf) = shape_of(bytes);
        assert_eq!(encoding, Encoding::Latin1);
        assert!(!bom);

        let text = decode(bytes, encoding, crlf);
        assert_eq!(text, "Grüße");
        assert_eq!(
            encode(&text, encoding, bom, crlf).unwrap(),
            bytes,
            "what goes back is byte for byte what came out"
        );
    }

    #[test]
    fn a_mark_and_crlf_endings_survive_the_round_trip() {
        let mut bytes = BOM.to_vec();
        bytes.extend_from_slice(b"eins\r\nzwei\r\n");
        let (encoding, bom, crlf) = shape_of(&bytes);
        assert_eq!(encoding, Encoding::Utf8);
        assert!(bom, "a mark that was there stays there");
        assert!(crlf);

        let text = decode(&bytes, encoding, crlf);
        assert_eq!(text, "eins\nzwei\n", "an editor sees plain newlines");
        assert_eq!(encode(&text, encoding, bom, crlf).unwrap(), bytes);
    }

    #[test]
    fn a_unix_file_does_not_grow_carriage_returns() {
        let bytes = b"eins\nzwei\n";
        let (encoding, bom, crlf) = shape_of(bytes);
        assert!(!crlf);
        let text = decode(bytes, encoding, crlf);
        assert_eq!(encode(&text, encoding, bom, crlf).unwrap(), bytes);

        // And text that arrives with CR LF in it does not double up when the
        // file is a CR LF one.
        let windows = encode("eins\r\nzwei\n", encoding, bom, true).unwrap();
        assert_eq!(windows, b"eins\r\nzwei\r\n");
    }

    #[test]
    fn a_character_that_does_not_fit_the_files_encoding_is_refused() {
        let refused = encode("Grüße 😀", Encoding::Latin1, false, false);
        assert!(
            matches!(refused, Err(Error::TextDoesNotFit { .. })),
            "a question mark where somebody typed a word is the wrong kind of help"
        );
        // The same text in a UTF-8 file is no trouble at all.
        assert!(encode("Grüße 😀", Encoding::Utf8, false, false).is_ok());
    }

    #[test]
    fn a_missing_time_does_not_count_as_a_change() {
        assert!(same_file((10, Some(5)), (10, Some(5))));
        assert!(!same_file((10, Some(5)), (11, Some(5))));
        assert!(!same_file((10, Some(5)), (10, Some(6))));
        // A server that answers MDTM with nothing useful must not make every
        // write-back ask a question nobody can answer.
        assert!(same_file((10, None), (10, Some(6))));
        assert!(same_file((10, Some(5)), (10, None)));
    }

    #[test]
    fn the_table_says_what_may_be_edited_and_with_what() {
        let shipped = EditRule::shipped();
        assert!(how_to_open(&shipped, "index.php").is_some());
        assert!(
            how_to_open(&shipped, "INDEX.PHP").is_some(),
            "case is not a kind"
        );
        assert!(
            how_to_open(&shipped, ".htaccess").is_some(),
            "a dot and a word is the oldest configuration file there is"
        );
        assert!(
            how_to_open(&shipped, "Dockerfile").is_some(),
            "a whole name counts where there is no extension"
        );
        assert!(how_to_open(&shipped, "archive.tar.gz").is_none());
        assert!(how_to_open(&shipped, "photo.jpg").is_none());
        assert!(
            how_to_open(&shipped, "README").is_none(),
            "no kind, no rule"
        );

        // The first line that matches wins, so one added on top beats the
        // long one underneath without having to be cut out of it.
        let mut rules = vec![EditRule {
            extensions: vec!["php".into()],
            open_with: OpenWith::Program,
            program: Some("/Applications/Editor.app".into()),
        }];
        rules.extend(shipped);
        assert_eq!(
            how_to_open(&rules, "index.php").unwrap().open_with,
            OpenWith::Program
        );
        assert_eq!(
            how_to_open(&rules, "style.css").unwrap().open_with,
            OpenWith::Own
        );

        // A table somebody emptied stays empty, and then nothing is editable.
        assert!(how_to_open(&[], "index.php").is_none());
        // A line with a dot typed in front of the kind still works.
        let dotted = vec![EditRule {
            extensions: vec![".md".into()],
            open_with: OpenWith::Own,
            program: None,
        }];
        assert!(how_to_open(&dotted, "notes.md").is_some());
    }

    #[test]
    fn the_name_of_the_copy_is_the_name_of_the_file() {
        assert_eq!(leaf("/var/www/config.php"), "config.php");
        assert_eq!(leaf("config.php"), "config.php");
        assert_eq!(leaf("/"), "file");
    }

    #[tokio::test]
    async fn a_copy_is_thrown_away_or_kept_as_asked() {
        let root = std::env::temp_dir().join(format!("amberbeam-edits-test-{}", new_id()));
        let edits = Edits::at(&root);
        assert!(edits.list().await.is_empty());

        // Built by hand: taking a real copy needs a real endpoint, and that is
        // what the integration tests are for.
        let folder = root.join("abc");
        std::fs::create_dir_all(&folder).unwrap();
        let local = folder.join("config.php");
        std::fs::write(&local, b"<?php\n").unwrap();
        let edit = Edit {
            id: "abc".into(),
            endpoint: "remote".into(),
            remote_path: "/var/www/config.php".into(),
            name: "config.php".into(),
            local_path: local.to_string_lossy().into_owned(),
            encoding: Encoding::Utf8,
            opened: 1,
            bom: false,
            crlf: false,
            taken: (6, Some(1)),
            written: stamp_of(&local),
        };
        edits.open.lock().await.insert("abc".into(), edit);

        assert_eq!(edits.text("abc").await.unwrap(), "<?php\n");
        edits.save("abc", "<?php\necho 1;\n").await.unwrap();
        assert_eq!(edits.text("abc").await.unwrap(), "<?php\necho 1;\n");
        assert!(
            edits.saved_elsewhere().await.is_empty(),
            "this program's own write is not somebody else's save"
        );

        // Somebody else's editor writing the copy is.
        std::fs::write(&local, b"<?php\necho 2;\n").unwrap();
        // The stamp has millisecond resolution; make sure it moved.
        std::thread::sleep(std::time::Duration::from_millis(5));
        std::fs::write(&local, b"<?php\necho 3;\n").unwrap();
        assert_eq!(edits.saved_elsewhere().await.len(), 1);
        assert!(edits.saved_elsewhere().await.is_empty(), "and only once");

        assert!(edits.finish("abc", false).await.is_some());
        assert!(local.exists(), "a copy that was to be kept is still there");
        assert!(edits.list().await.is_empty());

        let _ = std::fs::remove_dir_all(&root);
    }
}
