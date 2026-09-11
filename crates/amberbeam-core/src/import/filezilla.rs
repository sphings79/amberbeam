//! `sitemanager.xml`.
//!
//! Folders nest as elements, and a folder's name is loose text inside it,
//! sitting in front of the servers it holds. That maps onto our folders
//! directly, which is why a FileZilla tree arrives as a tree.
//!
//! The password is Base64 — encoding, not encryption, and reversible by
//! anybody. Since FileZilla 3.26 there is a master password as well, and then
//! the value really is encrypted; the importer says so rather than handing over
//! something that will not log in.

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
    let mut found = Found::new(Source::FileZilla, path);
    let mut folders: Vec<String> = Vec::new();
    let mut current: Option<Server> = None;

    for step in xml::walk(text) {
        match step {
            Step::Open(element) => match element.name.as_str() {
                "Folder" => folders.push(element.text.trim().to_string()),
                "Server" => current = Some(Server::default()),
                other => {
                    // The walk has already gathered the text of an element by
                    // the time it is reported, so a field is complete when it
                    // opens.
                    if let Some(server) = current.as_mut() {
                        server.set(other, element.text.trim(), element.attribute("encoding"));
                    }
                }
            },
            Step::Close(name) => match name.as_str() {
                "Folder" => {
                    folders.pop();
                }
                "Server" => {
                    if let Some(server) = current.take() {
                        if server.master_password {
                            let warning = "import.warning.master-password".to_string();
                            if !found.warnings.contains(&warning) {
                                found.warnings.push(warning);
                            }
                        }
                        if let Some(entry) = server.into_entry(&folders) {
                            found.entries.push(entry);
                        }
                    }
                }
                _ => {}
            },
        }
    }

    found
}

#[derive(Default)]
struct Server {
    host: String,
    port: Option<u16>,
    protocol: i32,
    logon_type: i32,
    user: String,
    password: Option<String>,
    master_password: bool,
    name: String,
    key_file: Option<String>,
    remote_dir: Option<String>,
}

impl Server {
    fn set(&mut self, field: &str, value: &str, encoding: Option<&str>) {
        match field {
            "Host" => self.host = value.to_string(),
            "Port" => self.port = value.parse().ok(),
            "Protocol" => self.protocol = value.parse().unwrap_or(0),
            "Logontype" => self.logon_type = value.parse().unwrap_or(1),
            "User" => self.user = value.to_string(),
            "Name" => self.name = value.to_string(),
            "Keyfile" => {
                self.key_file = (!value.is_empty()).then(|| value.to_string());
            }
            "RemoteDir" => {
                self.remote_dir = safe_path(value);
            }
            "Pass" => match encoding {
                // Real encryption behind a master password. Not readable, and
                // not to be claimed as readable.
                Some("crypt") => self.master_password = true,
                Some("base64") => {
                    self.password = xml::from_base64(value).map(|bytes| super::decode(&bytes));
                }
                // Older files, and files somebody edited: the value as it
                // stands.
                _ => self.password = (!value.is_empty()).then(|| value.to_string()),
            },
            _ => {}
        }
    }

    fn into_entry(self, folders: &[String]) -> Option<Imported> {
        if self.host.is_empty() {
            return None;
        }
        // Values from FileZilla's own enumeration, which its source says may
        // never change or people's saved sites break.
        let (protocol, encryption) = match self.protocol {
            1 => (Protocol::Sftp, None),
            3 => (Protocol::Ftps, Some(Encryption::Implicit)),
            4 => (Protocol::Ftps, Some(Encryption::Explicit)),
            6 => (Protocol::Ftp, None),
            // 0 is "FTP, attempts AUTH TLS", which is what we call explicit
            // FTPS. Anything else is a cloud service this program does not
            // speak, and an entry nobody can connect to is worse than none.
            0 => (Protocol::Ftps, Some(Encryption::Explicit)),
            _ => return None,
        };

        let auth = match self.logon_type {
            // 0 anonymous, 5 key file. The rest ask for a password one way or
            // another, which for us is the same kind of entry.
            0 => AuthKind::Password,
            5 => AuthKind::KeyFile,
            _ => AuthKind::Password,
        };

        let name = if self.name.is_empty() {
            self.host.clone()
        } else {
            self.name.clone()
        };

        let mut entry = Imported::named(name);
        entry.folder = folders
            .iter()
            .filter(|folder| !folder.is_empty())
            .cloned()
            .collect::<Vec<_>>()
            .join("/");
        entry.protocol = protocol;
        entry.encryption = encryption;
        entry.port = self.port.unwrap_or(match protocol {
            Protocol::Sftp => 22,
            _ => 21,
        });
        entry.host = self.host;
        entry.user = if self.logon_type == 0 {
            "anonymous".to_string()
        } else {
            self.user
        };
        entry.auth = auth;
        entry.key_path = self.key_file;
        entry.remote_path = self.remote_dir;
        Some(entry.with_password(self.password))
    }
}

/// FileZilla's own notation for a remote directory.
///
/// `<type> <prefix length>[ <prefix>]` and then ` <length> <segment>` for each
/// part. The lengths are there because a directory name may contain spaces,
/// which is exactly why this cannot be split on whitespace and hoped for.
fn safe_path(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let chars: Vec<char> = value.chars().collect();
    let mut at = 0_usize;

    let number = |at: &mut usize| -> Option<usize> {
        let start = *at;
        while *at < chars.len() && chars[*at].is_ascii_digit() {
            *at += 1;
        }
        if start == *at {
            return None;
        }
        let text: String = chars[start..*at].iter().collect();
        // The space after a number, where there is one.
        if *at < chars.len() && chars[*at] == ' ' {
            *at += 1;
        }
        text.parse().ok()
    };

    let _type = number(&mut at)?;
    let prefix = number(&mut at)?;
    at += prefix;
    if at > chars.len() {
        return None;
    }
    if prefix > 0 && at < chars.len() && chars[at] == ' ' {
        at += 1;
    }

    let mut segments = Vec::new();
    while at < chars.len() {
        let length = number(&mut at)?;
        if at + length > chars.len() {
            return None;
        }
        segments.push(chars[at..at + length].iter().collect::<String>());
        at += length;
        if at < chars.len() && chars[at] == ' ' {
            at += 1;
        }
    }

    (!segments.is_empty()).then(|| format!("/{}", segments.join("/")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const SAMPLE: &str = r#"<?xml version="1.0"?>
<FileZilla3 version="3.66.0" platform="*nix">
  <Servers>
    <Folder expanded="1">Kunden
      <Folder expanded="0">M&#252;ller
        <Server>
          <Host>example.org</Host>
          <Port>2222</Port>
          <Protocol>1</Protocol>
          <User>dennis</User>
          <Pass encoding="base64">dGFubmVuYmF1bQ==</Pass>
          <Logontype>1</Logontype>
          <Name>Webserver</Name>
          <RemoteDir>1 0 3 var 3 www</RemoteDir>
        </Server>
      </Folder>
    </Folder>
    <Server>
      <Host>ftp.example</Host>
      <Protocol>6</Protocol>
      <User>anonymous</User>
      <Logontype>0</Logontype>
      <Name>Offen</Name>
    </Server>
  </Servers>
</FileZilla3>
"#;

    fn parsed() -> Found {
        parse(SAMPLE, &PathBuf::from("sitemanager.xml"))
    }

    #[test]
    fn a_tree_arrives_as_a_tree() {
        let found = parsed();
        assert_eq!(found.entries.len(), 2);

        let web = &found.entries[0];
        assert_eq!(web.name, "Webserver");
        assert_eq!(web.folder, "Kunden/Müller");
        assert_eq!(web.protocol, Protocol::Sftp);
        assert_eq!(web.port, 2222);
        assert_eq!(web.remote_path.as_deref(), Some("/var/www"));
        assert_eq!(web.password.as_deref(), Some("tannenbaum"));

        // And the one at the top level stays at the top level.
        assert_eq!(found.entries[1].folder, "");
        assert_eq!(found.entries[1].protocol, Protocol::Ftp);
    }

    #[test]
    fn a_master_password_is_said_out_loud() {
        let text = r#"<FileZilla3><Servers><Server>
            <Host>example.org</Host><Protocol>1</Protocol><User>dennis</User>
            <Pass encoding="crypt">AAAA</Pass><Logontype>1</Logontype>
            </Server></Servers></FileZilla3>"#;
        let found = parse(text, &PathBuf::from("sitemanager.xml"));

        assert!(found
            .warnings
            .contains(&"import.warning.master-password".to_string()));
        assert_eq!(found.entries.len(), 1, "the server is still worth having");
        assert!(!found.entries[0].has_password);
    }

    #[test]
    fn the_protocol_numbers_are_filezillas_own() {
        let one = |protocol: i32| {
            let text = format!(
                "<FileZilla3><Servers><Server><Host>a.example</Host>\
                 <Protocol>{protocol}</Protocol><Logontype>1</Logontype></Server></Servers></FileZilla3>"
            );
            parse(&text, &PathBuf::from("x")).entries.pop()
        };

        assert_eq!(one(0).unwrap().encryption, Some(Encryption::Explicit));
        assert_eq!(one(1).unwrap().protocol, Protocol::Sftp);
        assert_eq!(one(3).unwrap().encryption, Some(Encryption::Implicit));
        assert_eq!(one(4).unwrap().encryption, Some(Encryption::Explicit));
        assert_eq!(one(6).unwrap().protocol, Protocol::Ftp);
        // A cloud service this program does not speak.
        assert!(one(9).is_none());
    }

    #[test]
    fn a_directory_name_with_a_space_survives_its_own_notation() {
        // The lengths in that notation are there for exactly this, which is why
        // it cannot be split on whitespace and hoped for.
        assert_eq!(
            safe_path("1 0 3 var 8 www neue"),
            Some("/var/www neue".into())
        );
        assert_eq!(safe_path("1 0 3 var 3 www"), Some("/var/www".into()));
        assert_eq!(safe_path(""), None);
        assert_eq!(safe_path("nonsense"), None);
    }
}
