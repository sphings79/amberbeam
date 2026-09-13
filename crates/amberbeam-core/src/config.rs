//! What AmberBeam keeps between starts.
//!
//! Three things, in three files under the application directory, all readable
//! and editable by hand:
//!
//! * `quick-connect.json` — the servers reached without a site entry
//! * `ui-state.json` — window size, pane split, last local directory
//! * `sites/*.json` — one file per site entry, the format of section 06
//! * `visited.json` — where each site was last left, for the entries that
//!   asked to be remembered
//!
//! **Never a password.** Those belong in the system's credential store, which
//! arrives with the site manager in M4; until then they live in memory for the
//! length of a session and nowhere else.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::endpoint::Protocol;
use crate::error::{Error, PathProblem, Result};
use crate::ftp::tls::Exceptions;
use crate::ftp::Encryption;
use crate::sites::{Site, Sites};

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
    /// What to do when a file is already there, decided once and for good.
    ///
    /// `None` is asking, which is the default and stays the default: writing
    /// over somebody's file without a word is the kind of help nobody wants.
    /// This exists because answering the same question forty times while a
    /// folder walks is not consent, it is wearing somebody down.
    #[serde(default)]
    pub conflict_policy: Option<crate::transfer::ConflictPolicy>,
    /// Show what a comparison found before anything is queued.
    ///
    /// On by default. A comparison that queues its own findings is a program
    /// deciding for somebody what "the same" means, and four hundred files
    /// arriving in the queue unasked is not a decision anybody made.
    #[serde(default = "yes")]
    pub review_comparison: bool,
    /// Carry a deletion across: a file gone here goes there too.
    ///
    /// Off, and it stays off unless somebody says otherwise. A checkout, a
    /// build that cleans up after itself or a stray move would otherwise take
    /// files off a server, and the person would find out from the server.
    #[serde(default)]
    pub delete_along: bool,
    /// Whether a program driving this one may connect to a server that is not
    /// in the list, by being handed the details.
    ///
    /// Off. The entries somebody saved are the ones they meant; a server
    /// nobody wrote down is one nobody vouched for.
    #[serde(default)]
    pub mcp_quick_connect: bool,
    /// Whether such a program may write new entries into the server list.
    ///
    /// Off. An entry it creates is one it may use — a server it made and is
    /// then forbidden to touch would be pointless — which is exactly why this
    /// is a decision somebody makes once, knowingly.
    #[serde(default)]
    pub mcp_create_sites: bool,
    /// What may be edited where it lies, and what opens it.
    ///
    /// One table answering both, because they are one decision: see
    /// [`crate::editing::EditRule`]. Emptied by hand it stays empty, and then
    /// nothing is editable — which is a setting somebody made, not a state to
    /// be repaired behind their back.
    #[serde(default = "crate::editing::EditRule::shipped")]
    pub editing: Vec<crate::editing::EditRule>,
    /// Take a finished line out of the queue by itself, after a moment.
    ///
    /// Off by default. A queue that empties itself is tidy right up until
    /// somebody wants to know whether the thing they started actually
    /// happened.
    #[serde(default)]
    pub clear_finished: bool,
}

/// For `serde(default)` on a field whose default is true.
fn yes() -> bool {
    true
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
            conflict_policy: None,
            clear_finished: false,
            editing: crate::editing::EditRule::shipped(),
            review_comparison: true,
            delete_along: false,
            mcp_quick_connect: false,
            mcp_create_sites: false,
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
    /// FTP only: how this connection was encrypted. Kept because the port does
    /// not answer it — explicit FTPS on a port of somebody's choosing looks
    /// exactly like implicit FTPS would, and reconnecting to the wrong one
    /// either hangs or sends the login in the clear.
    #[serde(default)]
    pub encryption: Option<Encryption>,
    /// FTP only. `None` means passive, which is what works behind a router.
    #[serde(default)]
    pub passive: Option<bool>,
    /// FTP only: the server does not speak UTF-8.
    #[serde(default)]
    pub latin1: Option<bool>,
    /// FTP only: seconds between keep-alive commands on an idle connection.
    #[serde(default)]
    pub keep_alive: Option<u32>,
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

    /// The server list. Its own module, because folders, moving and renaming
    /// are a good deal more than "a directory of files".
    pub fn sites(&self) -> Sites {
        Sites::at(self.root.join("sites"))
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
            id: Site::new_id(),
            name: format!("{}@{}", entry.user, entry.host),
            protocol: entry.protocol,
            host: entry.host.clone(),
            port: entry.port,
            user: entry.user.clone(),
            auth: entry.auth,
            key_path: entry.key_path.clone(),
            remote_path: entry.last_path.clone(),
            remember_path: false,
            local_path: None,
            concurrency: entry
                .concurrency
                .unwrap_or_else(|| entry.protocol.default_concurrency()),
            retries: entry.retries,
            temporary_name: entry.temporary_name,
            encryption: entry.encryption,
            passive: entry.passive,
            latin1: entry.latin1,
            keep_alive: entry.keep_alive,
            // A password the history never held cannot be carried over, so the
            // new entry starts without one rather than claiming to have it.
            remember_password: false,
            wastebasket: None,
            excludes: Vec::new(),
            mcp: false,
            colour: None,
        };

        // At the top level: a history entry has no folder, and guessing one
        // would put servers where nobody filed them.
        let path = self.sites().save("", &site)?;

        self.remember_quick_connect(QuickConnectEntry {
            saved_as_site: true,
            ..entry
        })?;
        Ok(path)
    }

    /// Where a site was last left, or `None` if it has not been anywhere.
    ///
    /// Its own file rather than a field in the site entry, for two reasons.
    /// The site manager is a window of its own that writes an entry whole, so
    /// a directory recorded here every few seconds would be overwritten by its
    /// next save — or overwrite it. And a site file is meant to be read,
    /// versioned and edited by hand, which a line that rewrites itself as
    /// somebody walks about is not.
    ///
    /// Whether a site wants this is [`Site::remember_path`], and that question
    /// is not asked here: this only stores and returns.
    pub fn visited(&self, id: &str) -> Option<String> {
        self.visits().remove(id)
    }

    /// Records where a site is now.
    pub fn remember_visit(&self, id: &str, path: &str) -> Result<()> {
        let mut visits = self.visits();
        if visits.get(id).map(String::as_str) == Some(path) {
            // Asked on every move of either pane, and most of those moves are
            // the other pane's. Writing the same line again for each of them
            // would be a file rewritten for nothing.
            return Ok(());
        }
        visits.insert(id.to_string(), path.to_string());
        write_json(&self.visited_path(), &visits)
    }

    /// Drops what was remembered for a site, when it is deleted or when it
    /// stops asking to be remembered.
    pub fn forget_visit(&self, id: &str) -> Result<()> {
        let mut visits = self.visits();
        if visits.remove(id).is_none() {
            return Ok(());
        }
        write_json(&self.visited_path(), &visits)
    }

    fn visits(&self) -> BTreeMap<String, String> {
        read_json(&self.visited_path()).unwrap_or_default()
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

    /// Certificates the user accepted by hand, in their own file.
    ///
    /// Kept beside the rest as plain text on purpose: an exception to
    /// certificate checking is exactly the sort of thing that should be
    /// readable, and removable, without this program's help. The same reasoning
    /// as OpenSSH's `known_hosts`.
    pub fn certificate_exceptions(&self) -> Exceptions {
        read_json(&self.certificates_path()).unwrap_or_default()
    }

    /// Records one accepted certificate for one host and port.
    pub fn accept_certificate(&self, host: &str, port: u16, fingerprint: &str) -> Result<()> {
        let mut exceptions = self.certificate_exceptions();
        exceptions.accept(host, port, fingerprint);
        write_json(&self.certificates_path(), &exceptions)
    }

    pub fn forget_certificate(&self, host: &str, port: u16) -> Result<()> {
        let mut exceptions = self.certificate_exceptions();
        exceptions.forget(host, port);
        write_json(&self.certificates_path(), &exceptions)
    }

    fn certificates_path(&self) -> PathBuf {
        self.root.join("accepted-certificates.json")
    }

    fn quick_connect_path(&self) -> PathBuf {
        self.root.join("quick-connect.json")
    }

    fn visited_path(&self) -> PathBuf {
        self.root.join("visited.json")
    }
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

#[cfg(test)]
mod tests {
    #[test]
    fn an_accepted_certificate_is_still_accepted_after_a_restart() {
        // "Restart" here is a second Config over the same directory, which is
        // exactly what the next start of the program is.
        let root = std::env::temp_dir().join(format!(
            "amberbeam-certs-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|since| since.as_nanos())
                .unwrap_or_default()
        ));
        let config = super::Config::at(&root);
        config
            .accept_certificate("example.org", 21, "AA:BB:CC")
            .expect("write the exception");

        let later = super::Config::at(&root);
        assert_eq!(
            later
                .certificate_exceptions()
                .decision_for("example.org", 21),
            crate::ftp::tls::CertificateDecision::Trust {
                fingerprint: "AA:BB:CC".into()
            }
        );
        // Still only that one server, and only that one port.
        assert_eq!(
            later
                .certificate_exceptions()
                .decision_for("example.org", 990),
            crate::ftp::tls::CertificateDecision::TrustedOnly
        );

        later
            .forget_certificate("example.org", 21)
            .expect("remove the exception");
        assert_eq!(
            super::Config::at(&root)
                .certificate_exceptions()
                .decision_for("example.org", 21),
            crate::ftp::tls::CertificateDecision::TrustedOnly
        );

        let _ = std::fs::remove_dir_all(&root);
    }

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
            encryption: None,
            passive: None,
            latin1: None,
            keep_alive: None,
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

        let filed = config.sites().load();
        assert_eq!(filed.len(), 1);
        assert_eq!(filed[0].folder, "", "a history entry has no folder");
        assert_eq!(filed[0].site.host, "example.org");
        assert_eq!(filed[0].site.remote_path.as_deref(), Some("/var/www"));
        assert_eq!(
            filed[0].site.concurrency, 8,
            "SFTP starts at eight at a time"
        );
        // What a site file may and may not hold is guarded where the type
        // lives, in `sites`.

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
        config.save_as_site(&one.id).unwrap();
        assert_eq!(
            config.sites().load()[0].site.concurrency,
            2,
            "the login's own value, not the protocol's"
        );
    }

    #[test]
    fn where_a_site_was_left_survives_a_restart_and_can_be_dropped() {
        let config = scratch("visited");
        assert_eq!(config.visited("abc"), None, "nowhere yet");

        config.remember_visit("abc", "/var/www").unwrap();
        config.remember_visit("def", "/srv").unwrap();
        // Twice in the same place is not a second place.
        config.remember_visit("abc", "/var/www").unwrap();

        // A second Config over the same directory is the next start.
        let later = Config::at(config.root());
        assert_eq!(later.visited("abc").as_deref(), Some("/var/www"));
        assert_eq!(later.visited("def").as_deref(), Some("/srv"));

        later.forget_visit("abc").unwrap();
        assert_eq!(Config::at(config.root()).visited("abc"), None);
        assert_eq!(
            Config::at(config.root()).visited("def").as_deref(),
            Some("/srv"),
            "forgetting one entry is not forgetting the file"
        );
        // Forgetting what was never there is not an error.
        later.forget_visit("abc").unwrap();
    }
}
