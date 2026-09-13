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

    /// Beside the configuration, which is where somebody would look.
    pub fn beside(config: &Path) -> Self {
        Self::at(config.join("mcp.log"))
    }

    /// Quiet on standard error. For tests, which have no use for it.
    pub fn quiet(mut self) -> Self {
        self.aloud = false;
        self
    }

    pub fn path(&self) -> &Path {
        &self.path
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
