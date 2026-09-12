//! Who is allowed in.
//!
//! One password for the whole service, and a token for everything after it.
//! Not accounts: this is a program somebody runs for themselves, and a user
//! table would be a login screen, a password reset, and a table to migrate —
//! all of it for a single person who already owns the machine.
//!
//! The password is not compared directly. Both sides are hashed first and the
//! hashes are compared, which is the ordinary way round this: a comparison
//! that stops at the first wrong byte would otherwise tell anybody patient
//! enough how much of their guess was right, and how long the real answer is.
//! Two digests are always the same length, and where they first differ says
//! nothing about the passwords behind them.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use ring::digest::{digest, SHA256};
use ring::rand::{SecureRandom, SystemRandom};

/// How long a session lasts without being used.
///
/// Long enough that a transfer somebody started before lunch is still theirs
/// when they come back, short enough that a browser left open on a train is
/// not a standing invitation.
const IDLE: Duration = Duration::from_secs(12 * 60 * 60);

/// The cookie a browser gets, and the header the desktop client sends.
pub const COOKIE: &str = "amberbeam_session";

pub struct Doorway {
    /// What the password must be, or `None` when the service was started
    /// without one.
    password: Option<String>,
    sessions: Mutex<HashMap<String, Instant>>,
    random: SystemRandom,
}

/// Why somebody is not getting in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The password was wrong.
    Wrong,
    /// There is no password set, so nothing can be let in.
    ///
    /// Deliberately not "everybody is welcome". A service that answers
    /// everything because its password was forgotten in the compose file is
    /// worse than one that answers nothing: the first looks like it is working.
    Closed,
}

impl Doorway {
    pub fn new(password: Option<String>) -> Self {
        Self {
            password: password.filter(|word| !word.is_empty()),
            sessions: Mutex::new(HashMap::new()),
            random: SystemRandom::new(),
        }
    }

    /// Whether a password was set at all.
    pub fn is_open(&self) -> bool {
        self.password.is_some()
    }

    /// Trades a password for a token.
    pub fn admit(&self, attempt: &str) -> Result<String, Refusal> {
        let Some(wanted) = self.password.as_deref() else {
            return Err(Refusal::Closed);
        };

        if digest(&SHA256, attempt.as_bytes()).as_ref()
            != digest(&SHA256, wanted.as_bytes()).as_ref()
        {
            return Err(Refusal::Wrong);
        }

        let mut bytes = [0u8; 32];
        self.random
            .fill(&mut bytes)
            .map_err(|_| Refusal::Wrong)
            .expect("the system has no randomness, which is not a thing to carry on after");
        let token: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();

        let mut sessions = self.sessions.lock().expect("the doorway was poisoned");
        sessions.insert(token.clone(), Instant::now());
        Ok(token)
    }

    /// Whether a token is still good, and marks it used if it is.
    pub fn holds(&self, token: &str) -> bool {
        let mut sessions = self.sessions.lock().expect("the doorway was poisoned");
        sessions.retain(|_, last| last.elapsed() < IDLE);
        match sessions.get_mut(token) {
            Some(last) => {
                *last = Instant::now();
                true
            }
            None => false,
        }
    }

    /// Ends one session, which is what signing out means.
    pub fn forget(&self, token: &str) {
        let mut sessions = self.sessions.lock().expect("the doorway was poisoned");
        sessions.remove(token);
    }
}

/// Reads one cookie out of a `Cookie:` header.
///
/// Written by hand rather than with a cookie crate: this reads one name from
/// one header, and a dependency for that would be more code arriving than
/// leaving.
pub fn cookie<'a>(header: &'a str, name: &str) -> Option<&'a str> {
    header.split(';').find_map(|part| {
        let (key, value) = part.split_once('=')?;
        (key.trim() == name).then(|| value.trim())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wrong_password_gets_nothing() {
        let door = Doorway::new(Some("tannenbaum".into()));
        assert_eq!(door.admit("eichhorn"), Err(Refusal::Wrong));
        // Nor does one that is merely a prefix of the right answer, which is
        // what a comparison stopping early would reward.
        assert_eq!(door.admit("tannen"), Err(Refusal::Wrong));
        assert_eq!(door.admit("tannenbaumer"), Err(Refusal::Wrong));
    }

    #[test]
    fn the_right_password_gets_a_token_that_works_once_it_exists() {
        let door = Doorway::new(Some("tannenbaum".into()));
        let token = door.admit("tannenbaum").expect("the password was right");
        assert!(door.holds(&token));
        assert!(!door.holds("something else"));
        door.forget(&token);
        assert!(!door.holds(&token));
    }

    #[test]
    fn two_sessions_are_two_tokens() {
        let door = Doorway::new(Some("tannenbaum".into()));
        let first = door.admit("tannenbaum").unwrap();
        let second = door.admit("tannenbaum").unwrap();
        assert_ne!(first, second, "a token that repeats is not a token");
        assert!(door.holds(&first) && door.holds(&second));
    }

    /// Without a password nothing is let in, and that is not the same as
    /// letting everything in.
    #[test]
    fn no_password_means_nobody_rather_than_everybody() {
        let door = Doorway::new(None);
        assert!(!door.is_open());
        assert_eq!(door.admit(""), Err(Refusal::Closed));
        assert_eq!(door.admit("anything"), Err(Refusal::Closed));
    }

    #[test]
    fn an_empty_password_is_no_password() {
        assert!(!Doorway::new(Some(String::new())).is_open());
    }

    #[test]
    fn one_cookie_is_read_out_of_several() {
        let header = "theme=dark; amberbeam_session=abc123; other=1";
        assert_eq!(cookie(header, COOKIE), Some("abc123"));
        assert_eq!(cookie(header, "theme"), Some("dark"));
        assert_eq!(cookie(header, "missing"), None);
        assert_eq!(cookie("", COOKIE), None);
    }
}
