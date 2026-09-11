//! The XML site export of an older Windows client, usually saved as `.ftp`.
//!
//! The cleanest of the six to read and the most dangerous to leave lying about:
//! the password is written out **in the clear**. Anyone who exports their sites
//! and forgets the file in Downloads has left every server password in a text
//! file. That is not this program's doing, but it is worth saying while
//! importing, and it is the reason the password goes straight into the
//! credential store and the file is never copied anywhere.
//!
//! `<GROUP>\Kunden\Müller\</GROUP>` carries the folder, and the declared
//! encoding is windows-1252 rather than UTF-8.

use std::path::Path;

use crate::config::AuthKind;
use crate::endpoint::Protocol;
use crate::error::Result;
use crate::ftp::Encryption;

use super::xml::{self, Step};
use super::{Found, Imported, Source};

pub fn read(path: &Path) -> Result<Found> {
    let text = super::read_text(path)?;
    Ok(parse(&text, path))
}

pub fn parse(text: &str, path: &Path) -> Found {
    let mut found = Found::new(Source::SitesXml, path);
    let mut current: Option<Site> = None;
    let mut cleartext = false;

    for step in xml::walk(text) {
        match step {
            Step::Open(element) => {
                if element.name.eq_ignore_ascii_case("SITE") {
                    current = Some(Site {
                        name: element
                            .attribute("NAME")
                            .unwrap_or_default()
                            .trim()
                            .to_string(),
                        ..Site::default()
                    });
                } else if let Some(site) = current.as_mut() {
                    site.set(&element.name, element.text.trim());
                }
            }
            Step::Close(name) => {
                if name.eq_ignore_ascii_case("SITE") {
                    if let Some(site) = current.take() {
                        cleartext |= site.password.is_some();
                        if let Some(entry) = site.into_entry() {
                            found.entries.push(entry);
                        }
                    }
                }
            }
        }
    }

    if cleartext {
        // Said while importing, because the file will otherwise sit in
        // Downloads with every password in it and nobody thinking about it.
        found.warnings.push("import.warning.cleartext".into());
    }
    found
}

#[derive(Default)]
struct Site {
    name: String,
    group: String,
    protocol: String,
    address: String,
    port: Option<u16>,
    user: String,
    password: Option<String>,
    remote_path: Option<String>,
    ssl: String,
}

impl Site {
    fn set(&mut self, field: &str, value: &str) {
        let field = field.to_ascii_uppercase();
        match field.as_str() {
            "GROUP" => self.group = value.to_string(),
            "PROTOCOL" => self.protocol = value.to_ascii_uppercase(),
            "ADDRESS" => self.address = value.to_string(),
            "PORT" => self.port = value.parse().ok(),
            "USERNAME" => self.user = value.to_string(),
            "PASSWORD" => self.password = (!value.is_empty()).then(|| value.to_string()),
            "REMOTEPATH" => {
                self.remote_path = (!value.is_empty()).then(|| value.to_string());
            }
            "SSL" => self.ssl = value.to_ascii_uppercase(),
            _ => {}
        }
    }

    fn into_entry(self) -> Option<Imported> {
        if self.address.is_empty() {
            return None;
        }

        let (protocol, encryption) = if self.protocol.contains("SFTP") {
            (Protocol::Sftp, None)
        } else {
            match self.ssl.as_str() {
                "" | "NONE" => (Protocol::Ftp, None),
                "IMPLICIT" => (Protocol::Ftps, Some(Encryption::Implicit)),
                // "AUTH TLS", "AUTH SSL", "EXPLICIT" and whatever else the
                // version wrote: all of them mean the explicit kind.
                _ => (Protocol::Ftps, Some(Encryption::Explicit)),
            }
        };

        let name = if self.name.is_empty() {
            self.address.clone()
        } else {
            self.name.clone()
        };

        let mut entry = Imported::named(name);
        // `\Kunden\Müller\` in the file, `Kunden/Müller` here.
        entry.folder = self
            .group
            .split('\\')
            .filter(|part| !part.trim().is_empty())
            .collect::<Vec<_>>()
            .join("/");
        entry.protocol = protocol;
        entry.encryption = encryption;
        entry.port = self.port.unwrap_or(match protocol {
            Protocol::Sftp => 22,
            _ => 21,
        });
        entry.host = self.address;
        entry.user = self.user;
        entry.auth = AuthKind::Password;
        entry.remote_path = self.remote_path;
        Some(entry.with_password(self.password))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="windows-1252"?>
<!--1-8-2012 14:17:43-->
<SITES VERSION="1.2">
  <SITE NAME="Kunde M&#252;ller">
    <GROUP>\Kunden\M&#252;ller\</GROUP>
    <PROTOCOL>FTP</PROTOCOL>
    <ADDRESS>example.org</ADDRESS>
    <PORT>2121</PORT>
    <USERNAME>dennis</USERNAME>
    <PASSWORD>tannenbaum</PASSWORD>
    <REMOTEPATH>/public_html/</REMOTEPATH>
    <PASSIVE>DEFAULT</PASSIVE>
    <SSL>AUTH TLS</SSL>
  </SITE>
  <SITE NAME="Ohne Port">
    <PROTOCOL>FTP</PROTOCOL>
    <ADDRESS>zwei.example</ADDRESS>
    <USERNAME>jemand</USERNAME>
    <SSL>NONE</SSL>
  </SITE>
</SITES>
"#;

    fn parsed() -> Found {
        parse(SAMPLE, &PathBuf::from("sites.ftp"))
    }

    #[test]
    fn a_group_becomes_a_folder() {
        let found = parsed();
        assert_eq!(found.entries.len(), 2);

        let first = &found.entries[0];
        assert_eq!(first.name, "Kunde Müller");
        assert_eq!(first.folder, "Kunden/Müller");
        assert_eq!(first.host, "example.org");
        assert_eq!(first.port, 2121);
        assert_eq!(first.remote_path.as_deref(), Some("/public_html/"));
        assert_eq!(first.protocol, Protocol::Ftps);
        assert_eq!(first.encryption, Some(Encryption::Explicit));

        // No group, no folder, and no port means the protocol's own.
        assert_eq!(found.entries[1].folder, "");
        assert_eq!(found.entries[1].port, 21);
        assert_eq!(found.entries[1].protocol, Protocol::Ftp);
    }

    #[test]
    fn a_password_in_the_clear_is_read_and_said_out_loud() {
        let found = parsed();
        assert_eq!(found.entries[0].password.as_deref(), Some("tannenbaum"));
        assert!(
            found
                .warnings
                .contains(&"import.warning.cleartext".to_string()),
            "a file with every password in it should not be left lying about unmentioned"
        );
    }

    #[test]
    fn a_file_with_nothing_in_the_clear_says_nothing() {
        let text = r#"<SITES><SITE NAME="A"><ADDRESS>a.example</ADDRESS></SITE></SITES>"#;
        let found = parse(text, &PathBuf::from("x.ftp"));
        assert_eq!(found.entries.len(), 1);
        assert!(found.warnings.is_empty());
    }
}
