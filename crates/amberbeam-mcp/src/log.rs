//! What happened, written where somebody can read it afterwards.
//!
//! Nobody is watching this shell work. There is no window, no server log
//! panel, and standard output is the protocol — so the only honest place for
//! "what did it actually do" is a file beside the configuration.
//!
//! Everything a tool is asked for goes in, including what was refused. A line
//! per call, appended, never rotated here: this is a program somebody runs on
//! their own machine, and a log that deletes its own past is a log that cannot
//! answer the question it exists for.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use amberbeam_core::config::Config;

/// The file, and the lock that keeps two calls from interleaving in it.
#[derive(Debug)]
pub struct Journal {
    path: PathBuf,
    open: Mutex<()>,
    /// Whether anything that goes in should also go to standard error, where
    /// whatever started this process can see it.
    aloud: bool,
}

impl Journal {
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            open: Mutex::new(()),
            aloud: true,
        }
    }

    /// Beside the configuration, which is where somebody would look — and the
    /// path is the core's to name, because the window reads the same file.
    pub fn beside(config: &Config) -> Self {
        Self::at(config.mcp_log())
    }

    /// Quiet on standard error. For tests, which have no use for it.
    pub fn quiet(mut self) -> Self {
        self.aloud = false;
        self
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// A tool call, with anything secret in it taken out first.
    ///
    /// The arguments are written down because that is the whole point of this
    /// file — but a quick connection is handed a password, and a log that
    /// keeps one is a worse leak than the thing it was written to guard
    /// against. The value goes; the fact that there was one stays, because
    /// "a password was given" is itself worth being able to read afterwards.
    pub fn call(&self, name: &str, arguments: &serde_json::Value) {
        self.note(&format!("call {name} {}", redact(arguments)));
    }

    pub fn note(&self, what: &str) {
        let line = format!("{} {what}\n", stamp());
        if self.aloud {
            let _ = std::io::stderr().write_all(line.as_bytes());
        }

        let _guard = self.open.lock();
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        // A log that cannot be written is not a reason to stop working, and
        // there is nowhere to complain to: standard output belongs to the
        // protocol.
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        {
            let _ = file.write_all(line.as_bytes());
        }
    }
}

/// The same arguments, with the secrets replaced.
///
/// By name, and the names are the ones this program's own tools use. Anything
/// a future tool calls a secret has to be added here — which is why the list
/// sits next to the log rather than somewhere it can be forgotten.
fn redact(arguments: &serde_json::Value) -> String {
    const SECRET: [&str; 4] = ["password", "passphrase", "token", "secret"];

    let Some(fields) = arguments.as_object() else {
        return arguments.to_string();
    };
    let mut copy = fields.clone();
    for (key, value) in copy.iter_mut() {
        if SECRET.iter().any(|name| key.to_lowercase().contains(name)) {
            *value = serde_json::Value::String("(given, not written down)".into());
        }
    }
    serde_json::Value::Object(copy).to_string()
}

/// `2026-09-13 11:42:07`, in local time, because whoever reads this is here.
fn stamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs() as i64)
        .unwrap_or_default();
    let (year, month, day) = civil_from_days(now.div_euclid(86_400));
    let rest = now.rem_euclid(86_400);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}",
        rest / 3600,
        (rest % 3600) / 60,
        rest % 60
    )
}

/// Days since the epoch as a date. Howard Hinnant's civil_from_days, by hand,
/// because a date library for one line of a log file is not a trade worth
/// making.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_call_ends_up_in_the_file() {
        let path =
            std::env::temp_dir().join(format!("amberbeam-mcp-log-{}.log", std::process::id()));
        let _ = std::fs::remove_file(&path);

        let journal = Journal::at(&path).quiet();
        journal.note("started");
        journal.note("call list_servers {}");

        let written = std::fs::read_to_string(&path).expect("the log was written");
        assert_eq!(written.lines().count(), 2);
        assert!(written.contains("call list_servers"));
        // A line begins with a date somebody can read.
        assert!(written.starts_with("20"), "{written}");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_password_is_never_written_down() {
        let arguments = serde_json::json!({
            "host": "example.org",
            "user": "someone",
            "password": "hunter2",
            "passphrase": "also secret",
        });
        let written = redact(&arguments);
        assert!(!written.contains("hunter2"), "{written}");
        assert!(!written.contains("also secret"), "{written}");
        // And what is not a secret is still readable, or the log would be
        // useless for the thing it exists for.
        assert!(written.contains("example.org"));
        assert!(written.contains("someone"));
        // That there was one is worth knowing.
        assert!(written.contains("given, not written down"));
    }

    #[test]
    fn the_date_is_the_one_it_says() {
        // 2026-09-13 is 20_709 days after the epoch, which is a thing to look
        // up rather than to count on the fingers: the first try was a day out.
        assert_eq!(civil_from_days(20_709), (2026, 9, 13));
        assert_eq!(civil_from_days(20_710), (2026, 9, 14));
        // A leap day, which is where a hand-written calendar goes wrong.
        assert_eq!(civil_from_days(19_782), (2024, 2, 29));
        assert_eq!(civil_from_days(0), (1970, 1, 1));
    }
}
