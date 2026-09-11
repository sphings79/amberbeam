//! What AmberBeam keeps between starts.
//!
//! Three things, in three files under the application directory, all readable
//! and editable by hand:
//!
//! * `quick-connect.json` — the servers reached without a site entry
//! * `ui-state.json` — window size, pane split, last local directory
//! * `sites/*.json` — one file per site entry, the format of section 06
//!
//! **Never a password.** Those belong in the system's credential store, which
//! arrives with the site manager in M4; until then they live in memory for the
//! length of a session and nowhere else.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::endpoint::Protocol;
use crate::error::{Error, PathProblem, Result};

/// Where everything lives. Settable, because the container build of M7 has a
/// mounted directory rather than a home, and tests need their own.
#[derive(Debug, Clone)]
pub struct Config {
    root: PathBuf,
}

/// How a quick connect entry authenticates. No secret is part of this — the
/// point of the file is that it can sit on disk in the open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuthKind {
    Password,
    KeyFile,
    Agent,
}

/// The settings that apply when a connection says nothing of its own.
///
/// Per connection beats global, and the site manager of M4 will carry its own
/// values too — this is the floor everything falls back to, not the only place
/// a number can live.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Transfers at once. `None` means the protocol decides: eight for SFTP,
    /// four for FTP — see `Protocol::default_concurrency` for why.
    pub concurrency: Option<u8>,
    /// Attempts before a broken job is paused rather than retried again.
    pub retries: u8,
    /// Carry the source's modification time across.
    pub keep_modified: bool,
    /// Carry the source's permission bits across. Off by default: bits that
    /// made sense on one machine often make a mess on another.
    pub keep_permissions: bool,
    /// Write to a temporary name and rename when the file is whole.
    ///
    /// On by default, and the same thing WinSCP does with its `.filepart`.
    /// Worth switching off where something on the other side watches the
    /// directory and trips over a name it does not expect.
    pub temporary_name: bool,
    /// Ask GitHub now and then whether a newer release exists.
    ///
    /// On by default and switchable off. It is the only request AmberBeam
    /// makes that the user did not ask for, so what it does is written out in
    /// the settings rather than left to be assumed.
    pub check_for_updates: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            concurrency: None,
            retries: 5,
            keep_modified: true,
            keep_permissions: false,
            temporary_name: true,
            check_for_updates: true,
        }
    }
}

/// One server reached through quick connect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickConnectEntry {
    /// `user@host:port` — one entry per login, so reconnecting updates rather
    /// than piles up.
    pub id: String,
    pub protocol: Protocol,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub auth: AuthKind,
    /// Path to the key file, when that is how it authenticates.
    pub key_path: Option<String>,
    /// The directory this server was last showing, so it opens there again.
    pub last_path: Option<String>,
    /// Seconds since the Unix epoch, for sorting the list.
    pub last_used: i64,
    /// Whether this login has already been written to the site manager.
    pub saved_as_site: bool,
    /// Transfers at once for this login. `None` falls back to the settings,
    /// and then to the protocol.
    #[serde(default)]
    pub concurrency: Option<u8>,
    /// Attempts for this login before a job is paused. `None` uses the
    /// settings.
    #[serde(default)]
    pub retries: Option<u8>,
    /// Write through a temporary name on this server. `None` uses the settings.
    #[serde(default)]
    pub temporary_name: Option<bool>,
}

impl QuickConnectEntry {
    pub fn id_for(user: &str, host: &str, port: u16) -> String {
        format!("{user}@{host}:{port}")
    }
}

impl Config {
    /// The usual place: `~/Library/Application Support/AmberBeam` on macOS,
    /// `%APPDATA%\AmberBeam` on Windows, `~/.config/amberbeam` on Linux.
    pub fn default_location() -> Result<Self> {
        let root = dirs::config_dir()
            .ok_or(Error::Path {
                path: String::new(),
                reason: PathProblem::NotFound,
            })?
            .join("AmberBeam");
        Ok(Self { root })
    }

    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn sites_dir(&self) -> PathBuf {
        self.root.join("sites")
    }

    /// The quick connect history, most recently used first.
    pub fn quick_connect(&self) -> Vec<QuickConnectEntry> {
        let mut entries: Vec<QuickConnectEntry> =
            read_json(&self.quick_connect_path()).unwrap_or_default();
        entries.sort_by_key(|entry| std::cmp::Reverse(entry.last_used));
        entries
    }

    /// Records a connection, replacing the entry for the same login.
    pub fn remember_quick_connect(&self, entry: QuickConnectEntry) -> Result<()> {
        let mut entries = self.quick_connect();
        // The previous entry for this login may already be marked as saved to
        // the site manager; that fact belongs to the login, not to the attempt.
        let saved = entries
            .iter()
            .find(|existing| existing.id == entry.id)
            .map(|existing| existing.saved_as_site)
            .unwrap_or(false);
        entries.retain(|existing| existing.id != entry.id);
        entries.insert(
            0,
            QuickConnectEntry {
                saved_as_site: entry.saved_as_site || saved,
                ..entry
            },
        );
        write_json(&self.quick_connect_path(), &entries)
    }

    pub fn forget_quick_connect(&self, id: &str) -> Result<()> {
        let mut entries = self.quick_connect();
        entries.retain(|entry| entry.id != id);
        write_json(&self.quick_connect_path(), &entries)
    }

    /// Writes a site entry in the format of section 06 and marks the history
    /// entry as taken over.
    ///
    /// The site manager that edits these arrives in M4; the file is written in
    /// its final shape now so nothing has to be migrated later. It carries no
    /// password — that reference to the credential store comes with M4 too.
    pub fn save_as_site(&self, id: &str) -> Result<PathBuf> {
        let entry = self
            .quick_connect()
            .into_iter()
            .find(|entry| entry.id == id)
            .ok_or(Error::Path {
                path: id.to_string(),
                reason: PathProblem::NotFound,
            })?;

        let site = Site {
            name: format!("{}@{}", entry.user, entry.host),
            protocol: entry.protocol,
            host: entry.host.clone(),
            port: entry.port,
            user: entry.user.clone(),
            auth: entry.auth,
            key_path: entry.key_path.clone(),
            remote_path: entry.last_path.clone(),
            local_path: None,
            concurrency: entry
                .concurrency
                .unwrap_or_else(|| entry.protocol.default_concurrency()),
            colour: None,
        };

        std::fs::create_dir_all(self.sites_dir()).map_err(Error::from)?;
        let path = self
            .sites_dir()
            .join(format!("{}.json", safe_file_name(&site.name)));
        write_json(&path, &site)?;

        self.remember_quick_connect(QuickConnectEntry {
            saved_as_site: true,
            ..entry
        })?;
        Ok(path)
    }

    pub fn settings(&self) -> Settings {
        read_json(&self.root.join("settings.json")).unwrap_or_default()
    }

    pub fn set_settings(&self, settings: &Settings) -> Result<()> {
        write_json(&self.root.join("settings.json"), settings)
    }

    /// Whatever the window wants to find again on the next start. The core does
    /// not look inside: pane splits and column widths are the frontend's
    /// business, and a core that knew about them would have to change whenever
    /// the window does.
    pub fn ui_state(&self) -> Option<serde_json::Value> {
        read_json(&self.root.join("ui-state.json"))
    }

    pub fn set_ui_state(&self, state: &serde_json::Value) -> Result<()> {
        write_json(&self.root.join("ui-state.json"), state)
    }

    fn quick_connect_path(&self) -> PathBuf {
        self.root.join("quick-connect.json")
    }
}

/// A site entry as it lands on disk. One file per server, readable, versionable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Site {
    pub name: String,
    pub protocol: Protocol,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub auth: AuthKind,
    pub key_path: Option<String>,
    /// Directory the server side opens in.
    pub remote_path: Option<String>,
    /// Directory the local side opens in.
    pub local_path: Option<String>,
    pub concurrency: u8,
    /// Colour marking in the site list.
    pub colour: Option<String>,
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    let text = std::fs::read_to_string(path).ok()?;
    // A file somebody edited by hand into nonsense must not stop the program
    // from starting. Losing a history is survivable; refusing to open is not.
    serde_json::from_str(&text).ok()
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(Error::from)?;
    }
    let text = serde_json::to_string_pretty(value).map_err(Error::other)?;

    // Written beside the target and renamed over it, so a crash midway leaves
    // the previous file intact instead of half of the new one.
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, text).map_err(Error::from)?;
    std::fs::rename(&temporary, path).map_err(Error::from)
}

/// Keeps a name usable as a file name without inventing a different one.
fn safe_file_name(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            other => other,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> Config {
        let root = std::env::temp_dir().join(format!("amberbeam-config-{name}"));
        let _ = std::fs::remove_dir_all(&root);
        Config::at(root)
    }

    fn entry(user: &str, host: &str) -> QuickConnectEntry {
        QuickConnectEntry {
            id: QuickConnectEntry::id_for(user, host, 22),
            protocol: Protocol::Sftp,
            host: host.into(),
            port: 22,
            user: user.into(),
            auth: AuthKind::Password,
            key_path: None,
            last_path: Some("/var/www".into()),
            last_used: 1_700_000_000,
            saved_as_site: false,
            concurrency: None,
            retries: None,
            temporary_name: None,
        }
    }

    #[test]
    fn an_empty_history_is_not_an_error() {
        assert!(scratch("empty").quick_connect().is_empty());
    }

    #[test]
    fn the_same_login_updates_instead_of_piling_up() {
        let config = scratch("dedupe");
        config
            .remember_quick_connect(entry("dennis", "example.org"))
            .unwrap();
        config
            .remember_quick_connect(QuickConnectEntry {
                last_path: Some("/srv".into()),
                last_used: 1_700_000_500,
                ..entry("dennis", "example.org")
            })
            .unwrap();

        let history = config.quick_connect();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].last_path.as_deref(), Some("/srv"));
    }

    #[test]
    fn the_most_recent_login_comes_first() {
        let config = scratch("order");
        config
            .remember_quick_connect(entry("dennis", "old.example"))
            .unwrap();
        config
            .remember_quick_connect(QuickConnectEntry {
                last_used: 1_800_000_000,
                ..entry("dennis", "new.example")
            })
            .unwrap();
        let history = config.quick_connect();
        assert_eq!(history[0].host, "new.example");
    }

    #[test]
    fn an_entry_can_be_deleted() {
        let config = scratch("delete");
        let one = entry("dennis", "example.org");
        config.remember_quick_connect(one.clone()).unwrap();
        config.forget_quick_connect(&one.id).unwrap();
        assert!(config.quick_connect().is_empty());
    }

    #[test]
    fn taking_an_entry_into_the_site_manager_writes_a_site_file() {
        let config = scratch("site");
        let one = entry("dennis", "example.org");
        config.remember_quick_connect(one.clone()).unwrap();

        let path = config.save_as_site(&one.id).expect("write site");
        assert!(path.exists());

        let site: Site = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(site.host, "example.org");
        assert_eq!(site.remote_path.as_deref(), Some("/var/www"));
        assert_eq!(site.concurrency, 8, "SFTP starts at eight at a time");

        // No field may hold a secret. The value "password" is allowed — it is
        // the name of a method — but a field called password or passphrase is
        // not, and that is what this watches for.
        let raw: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        for field in raw.as_object().expect("an object").keys() {
            let field = field.to_lowercase();
            assert!(
                !field.contains("pass") && !field.contains("secret"),
                "a site file must not carry {field}"
            );
        }

        // And the history knows it has been taken over.
        assert!(config.quick_connect()[0].saved_as_site);
    }

    #[test]
    fn being_saved_to_the_site_manager_survives_the_next_connection() {
        let config = scratch("saved-flag");
        let one = entry("dennis", "example.org");
        config.remember_quick_connect(one.clone()).unwrap();
        config.save_as_site(&one.id).unwrap();

        // Connecting again writes the entry afresh, and must not quietly forget
        // that this login already has a site entry.
        config.remember_quick_connect(one.clone()).unwrap();
        assert!(config.quick_connect()[0].saved_as_site);
    }

    #[test]
    fn a_history_file_full_of_nonsense_does_not_stop_the_program() {
        let config = scratch("broken");
        std::fs::create_dir_all(config.root()).unwrap();
        std::fs::write(config.root().join("quick-connect.json"), "{not json").unwrap();
        assert!(config.quick_connect().is_empty());
    }

    #[test]
    fn the_window_state_is_kept_as_the_frontend_left_it() {
        let config = scratch("ui");
        let state = serde_json::json!({ "split": 0.5, "theme": "dark" });
        config.set_ui_state(&state).unwrap();
        assert_eq!(config.ui_state(), Some(state));
    }

    #[test]
    fn settings_start_at_the_values_the_paper_decided() {
        let settings = Settings::default();
        assert_eq!(
            settings.concurrency, None,
            "the protocol decides by default"
        );
        assert_eq!(settings.retries, 5);
        assert!(
            settings.keep_modified,
            "a timestamp is cheap and usually wanted"
        );
        assert!(
            !settings.keep_permissions,
            "bits from one machine often make a mess on another"
        );
        assert!(
            settings.temporary_name,
            "a half file should not wear a finished name unless asked"
        );
    }

    #[test]
    fn settings_survive_a_restart() {
        let config = scratch("settings");
        let settings = Settings {
            concurrency: Some(3),
            keep_permissions: true,
            ..Default::default()
        };
        config.set_settings(&settings).unwrap();
        assert_eq!(config.settings(), settings);
    }

    #[test]
    fn a_login_may_carry_its_own_numbers() {
        let config = scratch("per-login");
        let one = QuickConnectEntry {
            concurrency: Some(2),
            retries: Some(9),
            ..entry("dennis", "example.org")
        };
        config.remember_quick_connect(one.clone()).unwrap();
        let path = config.save_as_site(&one.id).unwrap();
        let site: Site = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            site.concurrency, 2,
            "the login's own value, not the protocol's"
        );
    }

    #[test]
    fn a_name_with_a_separator_cannot_escape_the_sites_directory() {
        assert_eq!(
            safe_file_name("dennis@example.org/../evil"),
            "dennis@example.org-..-evil"
        );
    }
}
