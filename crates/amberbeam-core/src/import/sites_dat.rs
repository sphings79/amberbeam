//! `Sites.dat` of an older Windows client.
//!
//! An INI file despite the extension: one section per server, with `IP`, `user`
//! and `pass` among the keys. The password is hex, obscured against a fixed
//! string that ships with the program — not a key, not a secret, and published
//! for years.
//!
//! This one is read **as well as it can be**, which is less of a promise than
//! the others. The format changed across versions, the obscuring varies with
//! it, and there is no version stamp worth trusting. So a value that does not
//! come back as text is dropped rather than guessed at, and the entry arrives
//! without a password instead of with a wrong one.

use std::path::Path;

use crate::config::AuthKind;
use crate::endpoint::Protocol;
use crate::error::Result;
use crate::ftp::Encryption;

use super::ini;
use super::{Found, Imported, Source};

/// The fixed string the method runs against. Not a key: it is the same in every
/// copy of the program, and has been printed in public since 2011.
const SCRAMBLE: &[u8] = b"yA36zA48dEhfrvghGRg57h5UlDv3";

pub fn read(path: &Path) -> Result<Found> {
    let text = super::read_text(path)?;
    Ok(parse(&text, path))
}

pub fn parse(text: &str, path: &Path) -> Found {
    let mut found = Found::new(Source::SitesDat, path);
    let mut unreadable = 0_usize;

    for section in ini::parse(text) {
        if section.name.is_empty() {
            continue;
        }
        let Some(host) = section.get("IP").filter(|value| !value.is_empty()) else {
            continue;
        };

        // The name may carry the folder the same way the tree showed it.
        let full = section.name.replace('\\', "/");
        let (folder, name) = match full.trim_matches('/').rsplit_once('/') {
            Some((folder, name)) => (folder.to_string(), name.to_string()),
            None => (String::new(), full.trim_matches('/').to_string()),
        };
        if name.is_empty() {
            continue;
        }

        // `SSL` is written as a number whose meaning changed between versions;
        // anything other than nothing means the connection was secured, and
        // explicit is what to try first.
        let secure = section.number::<u8>("SSL").is_some_and(|value| value != 0);

        let mut entry = Imported::named(name);
        entry.folder = folder;
        entry.protocol = if secure {
            Protocol::Ftps
        } else {
            Protocol::Ftp
        };
        entry.encryption = secure.then_some(Encryption::Explicit);
        entry.host = host.to_string();
        entry.port = section.number::<u16>("port").unwrap_or(21);
        entry.user = section.get("user").unwrap_or_default().to_string();
        entry.auth = AuthKind::Password;
        entry.remote_path = section
            .get("path")
            .map(str::to_string)
            .filter(|value| !value.is_empty());

        let stored = section.get("pass").filter(|value| !value.is_empty());
        let password = stored.and_then(decode);
        if stored.is_some() && password.is_none() {
            unreadable += 1;
        }
        found.entries.push(entry.with_password(password));
    }

    if unreadable > 0 {
        // Said plainly rather than left as a surprise at the first connection.
        found.warnings.push("import.warning.some-passwords".into());
    }
    found
}

/// Turns a stored password back into the password.
///
/// Each byte is worked out from its neighbour and the fixed string, which is
/// why the value is one byte longer than the password it holds.
fn decode(stored: &str) -> Option<String> {
    let hex: String = stored.chars().filter(|c| !c.is_whitespace()).collect();
    if hex.len() < 4 || hex.len() % 2 != 0 {
        return None;
    }
    let cipher: Vec<u8> = hex
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok())
        .collect::<Option<Vec<u8>>>()?;

    let mut bytes = Vec::with_capacity(cipher.len() - 1);
    for index in 0..cipher.len() - 1 {
        let key = *SCRAMBLE.get(index)?;
        bytes.push((cipher[index + 1] ^ key).wrapping_sub(cipher[index]));
    }

    // Text or nothing. A value from a version whose method differs comes out as
    // bytes, and handing somebody a password that silently fails to log in is
    // worse than telling them it could not be read.
    let text = String::from_utf8(bytes).ok()?;
    (!text.is_empty() && !text.chars().any(|c| c.is_control())).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// The method forwards, for the tests only.
    fn encode(password: &str) -> String {
        let plain = password.as_bytes();
        let mut cipher = vec![0x42_u8];
        for (index, byte) in plain.iter().enumerate() {
            let key = SCRAMBLE[index];
            cipher.push((byte.wrapping_add(cipher[index])) ^ key);
        }
        cipher.iter().map(|byte| format!("{byte:02X}")).collect()
    }

    #[test]
    fn a_stored_password_comes_back() {
        for password in ["tannenbaum", "x", "kurz&knapp"] {
            assert_eq!(decode(&encode(password)).as_deref(), Some(password));
        }
    }

    #[test]
    fn a_value_that_is_not_text_is_dropped_rather_than_handed_over() {
        // A file from a version whose method differs. Better no password than
        // one that silently fails to log in.
        assert_eq!(decode("00112233445566778899"), None);
        assert_eq!(decode(""), None);
        assert_eq!(decode("AB"), None);
    }

    #[test]
    fn a_section_becomes_a_server_with_its_folder() {
        let text = format!(
            "[Kunden\\Müller]\n\
             created=40668.9448318287\n\
             IP=example.org\n\
             port=2121\n\
             user=dennis\n\
             pass={}\n\
             path=/var/www\n",
            encode("tannenbaum")
        );
        let found = parse(&text, &PathBuf::from("Sites.dat"));

        assert_eq!(found.entries.len(), 1);
        let entry = &found.entries[0];
        assert_eq!(entry.name, "Müller");
        assert_eq!(entry.folder, "Kunden");
        assert_eq!(entry.host, "example.org");
        assert_eq!(entry.port, 2121);
        assert_eq!(entry.remote_path.as_deref(), Some("/var/www"));
        assert_eq!(entry.password.as_deref(), Some("tannenbaum"));
        assert!(found.warnings.is_empty());
    }

    #[test]
    fn passwords_that_could_not_be_read_are_said_out_loud() {
        let text = "[Web]\nIP=example.org\nuser=dennis\npass=00112233445566778899\n";
        let found = parse(text, &PathBuf::from("Sites.dat"));

        assert_eq!(found.entries.len(), 1, "the server is still worth having");
        assert!(!found.entries[0].has_password);
        assert!(found
            .warnings
            .contains(&"import.warning.some-passwords".to_string()));
    }
}
