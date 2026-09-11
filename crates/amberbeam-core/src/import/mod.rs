//! Reading somebody else's server list.
//!
//! Section 06 of the concept paper puts this plainly: the move decides itself
//! in the first ten minutes, and whoever has to retype thirty servers does not
//! come back. So the importers are part of version one, not an extra.
//!
//! Every source is read into the same shape, [`Imported`], and nothing is
//! written until the user has seen a preview and ticked what to take. The
//! passwords are the delicate part: several of these formats only obscure them,
//! which means they can be read — and the moment they are, they go into the
//! system's credential store and never into a file of ours.

pub mod commander;
pub mod filezilla;
pub mod ini;
pub mod openssh;
pub mod sites_dat;
pub mod sites_xml;
pub mod winscp;
pub mod xml;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::AuthKind;
use crate::endpoint::Protocol;
use crate::ftp::Encryption;

/// Where a list came from.
///
/// Named after the file rather than the program wherever the file has a name of
/// its own: it is a fact about what is being read, and it is what somebody sees
/// in their own folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Source {
    /// `~/.ssh/config` — no passwords, and by far the most useful of the five.
    SshConfig,
    /// `WinSCP.ini`
    WinScp,
    /// `wcx_ftp.ini`, the FTP plug-in of Total Commander.
    WcxFtp,
    /// `sitemanager.xml`
    FileZilla,
    /// `Sites.dat` of an older Windows client.
    SitesDat,
    /// The XML site export of an older Windows client, usually `.ftp`.
    SitesXml,
}

impl Source {
    /// The translation key naming this source in the window.
    pub const fn message_key(self) -> &'static str {
        match self {
            Source::SshConfig => "import.source.ssh-config",
            Source::WinScp => "import.source.winscp",
            Source::WcxFtp => "import.source.wcx-ftp",
            Source::FileZilla => "import.source.filezilla",
            Source::SitesDat => "import.source.sites-dat",
            Source::SitesXml => "import.source.sites-xml",
        }
    }

    /// File names that give a source away, for looking in the usual places.
    const fn file_names(self) -> &'static [&'static str] {
        match self {
            Source::SshConfig => &["config"],
            Source::WinScp => &["WinSCP.ini"],
            Source::WcxFtp => &["wcx_ftp.ini"],
            Source::FileZilla => &["sitemanager.xml"],
            Source::SitesDat => &["Sites.dat"],
            Source::SitesXml => &[],
        }
    }
}

/// One server, as it was read out of somebody else's file.
///
/// Not a [`crate::sites::Site`] yet: nothing has been written, no identifier
/// has been handed out, and the password — where there was one to read — is
/// still in hand rather than in the credential store.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Imported {
    pub name: String,
    /// Where the source filed it, if it had folders. Ours will match.
    pub folder: String,
    pub protocol: Protocol,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub auth: AuthKind,
    pub key_path: Option<String>,
    pub remote_path: Option<String>,
    pub encryption: Option<Encryption>,
    /// Whether a password was found and could be read. The value itself is
    /// deliberately not part of what the window is shown.
    pub has_password: bool,
    /// The password, when the source only obscured it. Skipped when this is
    /// sent anywhere: the window ticks entries, the core moves secrets.
    #[serde(skip)]
    pub password: Option<String>,
}

/// Hand-written so no password can reach a log through a stray `{:?}`.
impl std::fmt::Debug for Imported {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Imported")
            .field("name", &self.name)
            .field("folder", &self.folder)
            .field("protocol", &self.protocol)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("user", &self.user)
            .field("has_password", &self.has_password)
            .finish_non_exhaustive()
    }
}

impl Imported {
    /// A blank entry of the given name, for a reader to fill in.
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            folder: String::new(),
            protocol: Protocol::Sftp,
            host: String::new(),
            port: 22,
            user: String::new(),
            auth: AuthKind::Password,
            key_path: None,
            remote_path: None,
            encryption: None,
            has_password: false,
            password: None,
        }
    }

    pub fn with_password(mut self, password: Option<String>) -> Self {
        self.has_password = password.as_ref().is_some_and(|value| !value.is_empty());
        self.password = password.filter(|value| !value.is_empty());
        self
    }
}

/// What one file turned out to hold.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    pub source: Source,
    pub path: PathBuf,
    pub entries: Vec<Imported>,
    /// Translation keys for anything the reader could not do — a master
    /// password above all. Shown before anything is taken over, because an
    /// import that silently drops half the passwords is worse than one that
    /// says it cannot read them.
    pub warnings: Vec<String>,
}

impl Found {
    pub fn new(source: Source, path: impl Into<PathBuf>) -> Self {
        Self {
            source,
            path: path.into(),
            entries: Vec::new(),
            warnings: Vec::new(),
        }
    }
}

/// A file that looks like one of these lists.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub source: Source,
    pub path: PathBuf,
}

/// Looks where these files usually are.
///
/// Both halves matter. On the machine the program came from, the files sit
/// where their program put them; for somebody who has just moved to a Mac, they
/// sit in Downloads or on the desktop, copied off the old computer. Looking in
/// both and offering a file picker beside the result is one click fewer than
/// asking first whether to look.
pub fn discover() -> Vec<Candidate> {
    let mut found = Vec::new();
    let Some(home) = home_directory() else {
        return found;
    };

    // Where each program keeps its own file, on the system it runs on.
    let native: &[(Source, PathBuf)] = &[
        (Source::SshConfig, home.join(".ssh").join("config")),
        (
            Source::FileZilla,
            home.join(".config")
                .join("filezilla")
                .join("sitemanager.xml"),
        ),
        (
            Source::FileZilla,
            home.join("AppData")
                .join("Roaming")
                .join("FileZilla")
                .join("sitemanager.xml"),
        ),
        (
            Source::WinScp,
            home.join("AppData").join("Roaming").join("WinSCP.ini"),
        ),
        (
            Source::WcxFtp,
            home.join("AppData")
                .join("Roaming")
                .join("GHISLER")
                .join("wcx_ftp.ini"),
        ),
    ];
    for (source, path) in native {
        if path.is_file() {
            add(&mut found, *source, path);
        }
    }

    // And wherever somebody dropped them after changing computers.
    for folder in ["Downloads", "Desktop", "Documents"] {
        scan(&home.join(folder), 2, &mut found);
    }
    found
}

/// Looks through one directory for anything recognisable, `depth` levels deep.
fn scan(directory: &Path, depth: u8, into: &mut Vec<Candidate>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if depth > 0 {
                scan(&path, depth - 1, into);
            }
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if let Some(source) = source_of(name) {
            add(into, source, &path);
        }
    }
}

/// Which source a file name gives away, if any.
///
/// `config` is left out here on purpose: an SSH configuration is only
/// recognisable by where it lives, and treating every file called `config` as
/// one would fill the list with rubbish.
fn source_of(name: &str) -> Option<Source> {
    const SOURCES: [Source; 4] = [
        Source::WinScp,
        Source::WcxFtp,
        Source::FileZilla,
        Source::SitesDat,
    ];
    SOURCES.into_iter().find(|source| {
        source
            .file_names()
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(name))
    })
}

fn add(into: &mut Vec<Candidate>, source: Source, path: &Path) {
    let path = path.to_path_buf();
    if into.iter().any(|candidate| candidate.path == path) {
        return;
    }
    into.push(Candidate { source, path });
}

fn home_directory() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// Reads whichever kind of file this is.
pub fn read(source: Source, path: &Path) -> crate::error::Result<Found> {
    match source {
        Source::SshConfig => openssh::read(path),
        Source::WinScp => winscp::read(path),
        Source::WcxFtp => commander::read(path),
        Source::SitesDat => sites_dat::read(path),
        Source::FileZilla => filezilla::read(path),
        Source::SitesXml => sites_xml::read(path),
    }
}

/// Text out of a file that may not be UTF-8.
///
/// These formats predate the settled use of UTF-8 on Windows, and several are
/// written in the system's code page. Bytes that are not valid UTF-8 are read
/// as windows-1252, which is what that code page is on every western
/// installation — a name with an umlaut comes through rather than being
/// replaced by a question mark.
pub fn read_text(path: &Path) -> crate::error::Result<String> {
    let bytes = std::fs::read(path).map_err(crate::error::Error::from)?;
    Ok(decode(&bytes))
}

/// UTF-8 where it is UTF-8, windows-1252 where it is not.
pub fn decode(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => bytes.iter().map(|&byte| latin(byte)).collect(),
    }
}

/// One byte of windows-1252 as a character.
///
/// Identical to Latin-1 except for 0x80 to 0x9F, where Latin-1 has control
/// characters and windows-1252 has the punctuation people actually typed — the
/// euro sign, curly quotes, the dash. Thirty-two exceptions, written out,
/// because a table of thirty-two is not worth a dependency.
fn latin(byte: u8) -> char {
    const HIGH: [char; 32] = [
        '\u{20AC}', '\u{FFFD}', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}',
        '\u{2021}', '\u{02C6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{FFFD}',
        '\u{017D}', '\u{FFFD}', '\u{FFFD}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}',
        '\u{2022}', '\u{2013}', '\u{2014}', '\u{02DC}', '\u{2122}', '\u{0161}', '\u{203A}',
        '\u{0153}', '\u{FFFD}', '\u{017E}', '\u{0178}',
    ];
    match byte {
        0x80..=0x9F => HIGH[(byte - 0x80) as usize],
        other => other as char,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_read_as_utf8_when_it_is_utf8() {
        assert_eq!(decode("Größe & Maß".as_bytes()), "Größe & Maß");
        // And a byte-order mark does not become part of the first name.
        let mut with_bom = vec![0xEF, 0xBB, 0xBF];
        with_bom.extend_from_slice("Kunden".as_bytes());
        assert_eq!(decode(&with_bom), "Kunden");
    }

    #[test]
    fn text_that_is_not_utf8_is_read_as_windows_1252() {
        // "Größe" as a Windows program of that age would have written it.
        assert_eq!(decode(&[0x47, 0x72, 0xF6, 0xDF, 0x65]), "Größe");
        // The thirty-two that separate windows-1252 from Latin-1.
        assert_eq!(decode(&[0x80, 0xFF]), "€ÿ");
        assert_eq!(decode(&[0x93, 0x94]), "“”");
    }

    #[test]
    fn a_password_cannot_reach_a_log_through_a_stray_debug() {
        let entry = Imported::named("Webserver").with_password(Some("tannenbaum".into()));
        let printed = format!("{entry:?}");
        assert!(!printed.contains("tannenbaum"), "{printed}");
        assert!(printed.contains("has_password: true"));
    }

    #[test]
    fn a_password_is_not_serialised_towards_the_window() {
        // The window ticks which entries to take; the core moves the secrets.
        // Nothing in between needs the value, so nothing in between gets it.
        let entry = Imported::named("Webserver").with_password(Some("tannenbaum".into()));
        let text = serde_json::to_string(&entry).unwrap();
        assert!(!text.contains("tannenbaum"), "{text}");
        assert!(text.contains("\"hasPassword\":true"));
    }

    #[test]
    fn an_empty_password_is_no_password() {
        let entry = Imported::named("Webserver").with_password(Some(String::new()));
        assert!(!entry.has_password);
        assert!(entry.password.is_none());
    }

    #[test]
    fn every_source_names_a_translation_key() {
        for source in [
            Source::SshConfig,
            Source::WinScp,
            Source::WcxFtp,
            Source::FileZilla,
            Source::SitesDat,
            Source::SitesXml,
        ] {
            assert!(source.message_key().starts_with("import.source."));
        }
    }

    #[test]
    fn a_file_called_config_is_not_taken_for_an_ssh_configuration() {
        // Recognisable only by where it lives. Anything else fills the list
        // with every stray config file on the machine.
        assert_eq!(source_of("config"), None);
        assert_eq!(source_of("WinSCP.ini"), Some(Source::WinScp));
        assert_eq!(source_of("winscp.ini"), Some(Source::WinScp));
        assert_eq!(source_of("wcx_ftp.ini"), Some(Source::WcxFtp));
        assert_eq!(source_of("Sites.dat"), Some(Source::SitesDat));
        assert_eq!(source_of("holiday.jpg"), None);
    }
}
