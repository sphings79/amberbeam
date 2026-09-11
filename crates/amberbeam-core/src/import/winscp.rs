//! `WinSCP.ini`.
//!
//! Sessions live in sections called `Sessions\name`, where the name carries the
//! folder as well: `Sessions\Kunden/Müller` is an entry filed under `Kunden`.
//! That maps straight onto our own folders, which is the whole reason the
//! importer bothers with it rather than dropping everything at the top level.
//!
//! The stored password is obscured rather than encrypted, and the method is
//! published. Reading it is the point: a password that stays in the old
//! program's file is a password somebody has to look up and type again, while
//! one that is read here goes straight into the system's credential store and
//! out of every file.
//!
//! A configuration with a master password is a different matter — that is real
//! encryption, and the importer says so instead of producing nonsense.

use std::path::Path;

use crate::config::AuthKind;
use crate::endpoint::Protocol;
use crate::error::Result;
use crate::ftp::Encryption;

use super::ini;
use super::{Found, Imported, Source};

/// The two constants of the published method.
const MAGIC: u8 = 0xA3;
const FLAG: u8 = 0xFF;

pub fn read(path: &Path) -> Result<Found> {
    let text = super::read_text(path)?;
    Ok(parse(&text, path))
}

pub fn parse(text: &str, path: &Path) -> Found {
    let mut found = Found::new(Source::WinScp, path);
    let sections = ini::parse(text);

    let master = sections
        .iter()
        .any(|section| section.get("UseMasterPassword").map(str::trim) == Some("1"));
    if master {
        // Not a shortcoming of this reader: with a master password the stored
        // passwords really are encrypted, and nothing short of that password
        // opens them.
        found.warnings.push("import.warning.master-password".into());
    }

    for section in &sections {
        let Some(raw_name) = section.name.strip_prefix("Sessions\\") else {
            continue;
        };
        let full = ini::unescape(raw_name);
        // Anything before the last slash is where it was filed.
        let (folder, name) = match full.rsplit_once('/') {
            Some((folder, name)) => (folder.to_string(), name.to_string()),
            None => (String::new(), full.clone()),
        };
        if name.is_empty() {
            continue;
        }

        let protocol = match section.number::<u8>("FSProtocol").unwrap_or(1) {
            // 0 SCP only, 1 SFTP with SCP fallback, 2 SFTP only. All three are
            // an SSH login, which is the one thing this program can offer.
            0..=2 => Protocol::Sftp,
            5 => match section.number::<u8>("FtpSecure").unwrap_or(0) {
                0 => Protocol::Ftp,
                _ => Protocol::Ftps,
            },
            // WebDAV and S3 are not protocols this program speaks, and an entry
            // that cannot be connected to is worse than one that is missing.
            _ => continue,
        };

        let encryption = match (protocol, section.number::<u8>("FtpSecure").unwrap_or(0)) {
            (Protocol::Ftps, 1) => Some(Encryption::Implicit),
            (Protocol::Ftps, _) => Some(Encryption::Explicit),
            _ => None,
        };

        let host = section.get("HostName").unwrap_or_default().to_string();
        if host.is_empty() {
            continue;
        }
        let key_path = section
            .get("PublicKeyFile")
            .map(ini::unescape)
            .filter(|value| !value.is_empty());
        let user = section
            .get("UserName")
            .map(ini::unescape)
            .unwrap_or_default();

        let password = if master {
            None
        } else {
            section
                .get("Password")
                .and_then(|stored| decode(stored, &user, &host))
        };

        let mut entry = Imported::named(name);
        entry.folder = folder;
        entry.protocol = protocol;
        entry.encryption = encryption;
        entry.port = section
            .number::<u16>("PortNumber")
            .unwrap_or(default_port(protocol));
        entry.host = host;
        entry.user = user;
        entry.auth = if key_path.is_some() {
            AuthKind::KeyFile
        } else {
            AuthKind::Password
        };
        entry.key_path = key_path;
        entry.remote_path = section
            .get("RemoteDirectory")
            .map(ini::unescape)
            .filter(|value| !value.is_empty());

        found.entries.push(entry.with_password(password));
    }

    found
}

const fn default_port(protocol: Protocol) -> u16 {
    match protocol {
        Protocol::Sftp => 22,
        _ => 21,
    }
}

/// Turns a stored password back into the password.
///
/// The published method, written out: the value is hex, every pair of nibbles
/// is exclusive-ored against a constant and complemented, and the result begins
/// with a length and — in the newer form — the user name and host, which are
/// there so the same password stored for two servers does not look the same.
fn decode(stored: &str, user: &str, host: &str) -> Option<String> {
    let mut nibbles: Vec<u8> = stored
        .trim()
        .chars()
        .map(|c| c.to_digit(16).map(|value| value as u8))
        .collect::<Option<Vec<u8>>>()?;
    if nibbles.len() < 2 {
        return None;
    }
    nibbles.reverse();

    let mut next = || -> Option<u8> {
        let high = nibbles.pop()?;
        let low = nibbles.pop()?;
        Some(!(((high << 4) | low) ^ MAGIC))
    };

    let flag = next()?;
    let length = if flag == FLAG {
        next()?;
        next()?
    } else {
        flag
    };

    // A run of filler whose length is stored, there to move the real value off
    // the start of the string.
    let skip = next()?;
    for _ in 0..skip {
        next()?;
    }

    let mut bytes = Vec::with_capacity(length as usize);
    for _ in 0..length {
        bytes.push(next()?);
    }

    let text = super::decode(&bytes);
    if flag == FLAG {
        // The newer form puts the user and host in front. They have to match,
        // or this value belongs to a different server and is not a password.
        let key = format!("{user}{host}");
        return text.strip_prefix(&key).map(str::to_string);
    }
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// The same method in reverse, for the tests only.
    ///
    /// This proves the decoder matches the published method, which is as far as
    /// a test without the program itself can go. What it does not prove is that
    /// the published method is right — that is what the fixture file and a real
    /// import are for.
    fn encode(password: &str, user: &str, host: &str) -> String {
        let mut plain = format!("{user}{host}").into_bytes();
        let length = plain.len() + password.len();
        plain.extend_from_slice(password.as_bytes());

        let mut bytes = vec![FLAG, 0x00, length as u8, 0x00];
        bytes.extend_from_slice(&plain);
        bytes
            .into_iter()
            .map(|byte| format!("{:02X}", (!byte) ^ MAGIC))
            .collect()
    }

    #[test]
    fn a_stored_password_comes_back() {
        let stored = encode("tannenbaum", "dennis", "example.org");
        assert_eq!(
            decode(&stored, "dennis", "example.org").as_deref(),
            Some("tannenbaum")
        );
    }

    #[test]
    fn a_password_stored_for_another_server_is_not_accepted() {
        // The user and host are in the value precisely so this can be told.
        let stored = encode("tannenbaum", "dennis", "example.org");
        assert_eq!(decode(&stored, "dennis", "other.example"), None);
        assert_eq!(decode(&stored, "jemand", "example.org"), None);
    }

    #[test]
    fn nonsense_decodes_to_nothing_rather_than_to_rubbish() {
        assert_eq!(decode("", "u", "h"), None);
        assert_eq!(decode("zz", "u", "h"), None);
        assert_eq!(decode("A3", "u", "h"), None);
    }

    #[test]
    fn a_session_name_carries_its_folder() {
        let text = format!(
            "[Sessions\\Kunden/M%C3%BCller/Web]\n\
             HostName=example.org\n\
             UserName=dennis\n\
             PortNumber=2222\n\
             FSProtocol=2\n\
             Password={}\n",
            encode("tannenbaum", "dennis", "example.org")
        );
        let found = parse(&text, &PathBuf::from("WinSCP.ini"));

        assert_eq!(found.entries.len(), 1);
        let entry = &found.entries[0];
        assert_eq!(entry.name, "Web");
        assert_eq!(entry.folder, "Kunden/Müller");
        assert_eq!(entry.protocol, Protocol::Sftp);
        assert_eq!(entry.port, 2222);
        assert!(entry.has_password);
        assert_eq!(entry.password.as_deref(), Some("tannenbaum"));
    }

    #[test]
    fn the_protocol_and_its_encryption_come_from_the_file() {
        let text = "\
[Sessions\\Plain]
HostName=a.example
FSProtocol=5
[Sessions\\Implicit]
HostName=b.example
FSProtocol=5
FtpSecure=1
[Sessions\\Explicit]
HostName=c.example
FSProtocol=5
FtpSecure=3
[Sessions\\Cloud]
HostName=d.example
FSProtocol=7
";
        let found = parse(text, &PathBuf::from("WinSCP.ini"));
        let by = |name: &str| found.entries.iter().find(|e| e.name == name).cloned();

        assert_eq!(by("Plain").unwrap().protocol, Protocol::Ftp);
        assert_eq!(by("Plain").unwrap().port, 21);
        assert_eq!(
            by("Implicit").unwrap().encryption,
            Some(Encryption::Implicit)
        );
        assert_eq!(
            by("Explicit").unwrap().encryption,
            Some(Encryption::Explicit)
        );
        // S3 is not a protocol this program speaks, and an entry that cannot be
        // connected to is worse than one that is missing.
        assert!(by("Cloud").is_none());
    }

    #[test]
    fn a_master_password_is_said_out_loud_and_nothing_is_guessed() {
        let text = "\
[Configuration\\Security]
UseMasterPassword=1
[Sessions\\Web]
HostName=example.org
UserName=dennis
Password=A35C1122
";
        let found = parse(text, &PathBuf::from("WinSCP.ini"));
        assert!(found
            .warnings
            .contains(&"import.warning.master-password".to_string()));
        assert_eq!(found.entries.len(), 1, "the server is still worth having");
        assert!(
            !found.entries[0].has_password,
            "a password behind a master password is not readable, and must not be claimed"
        );
    }
}
