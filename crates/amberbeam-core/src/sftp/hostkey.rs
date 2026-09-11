//! Checking the server's key against `known_hosts`.
//!
//! Deliberately the same file the terminal uses, `~/.ssh/known_hosts`, so a
//! server trusted in one place is trusted in the other and nobody ends up
//! maintaining two lists.
//!
//! Three outcomes, and only the first connects on its own:
//!
//! * known and matching — connect
//! * unknown — stop, show the fingerprint, connect only after the user says so
//! * **changed — refuse.** Never a dialog that can be clicked away in passing:
//!   a changed host key is either a reinstalled server or somebody sitting in
//!   the middle, and the client cannot tell which.

use std::sync::{Arc, Mutex};

use russh::keys::{HashAlg, PublicKey};

use crate::error::Error;

/// What the user has already agreed to for this attempt.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum HostKeyDecision {
    /// Connect only if the key is already recorded. The state of every first
    /// attempt.
    #[default]
    KnownOnly,
    /// The user was shown this fingerprint and accepted it. Anything else is
    /// still refused — the decision belongs to one key, not to one host.
    Trust { fingerprint: String },
}

/// The fingerprint in the notation OpenSSH prints, e.g. `SHA256:0WY116…`.
pub fn fingerprint(key: &PublicKey) -> String {
    key.fingerprint(HashAlg::Sha256).to_string()
}

/// What the check decided, kept aside so the connection can explain itself
/// after russh has closed it.
#[derive(Debug, Default)]
pub struct Verdict {
    /// Why the key was refused, if it was.
    pub refusal: Mutex<Option<Error>>,
    /// A key the user accepted, to be written to `known_hosts` once the rest of
    /// the connection actually worked. Writing earlier would record a server
    /// that then failed to authenticate.
    pub to_learn: Mutex<Option<PublicKey>>,
}

impl Verdict {
    pub fn shared() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn take_refusal(&self) -> Option<Error> {
        self.refusal.lock().ok().and_then(|mut slot| slot.take())
    }

    pub fn take_to_learn(&self) -> Option<PublicKey> {
        self.to_learn.lock().ok().and_then(|mut slot| slot.take())
    }

    fn refuse(&self, error: Error) {
        if let Ok(mut slot) = self.refusal.lock() {
            *slot = Some(error);
        }
    }

    fn remember(&self, key: PublicKey) {
        if let Ok(mut slot) = self.to_learn.lock() {
            *slot = Some(key);
        }
    }
}

/// Decides whether this key may be used, and records why not.
///
/// `lookup` answers what `known_hosts` holds; the real one reads the file, the
/// tests hand in a list. Everything else about the decision is the same either
/// way, which is the point — this is the part that must not be wrong.
pub fn verify(
    host: &str,
    key: &PublicKey,
    decision: &HostKeyDecision,
    verdict: &Verdict,
    recorded: &[PublicKey],
) -> bool {
    let offered = fingerprint(key);

    if recorded.iter().any(|known| known == key) {
        return true;
    }

    // A recorded key for the same host that is not this one. The algorithm is
    // not compared: a server that suddenly answers with a different key type is
    // exactly as much of a question as one with a different key.
    if let Some(known) = recorded.first() {
        verdict.refuse(Error::HostKeyChanged {
            host: host.to_string(),
            fingerprint: offered,
            known_fingerprint: fingerprint(known),
        });
        return false;
    }

    match decision {
        HostKeyDecision::Trust {
            fingerprint: accepted,
        } if *accepted == offered => {
            verdict.remember(key.clone());
            true
        }
        _ => {
            verdict.refuse(Error::HostKeyUnknown {
                host: host.to_string(),
                fingerprint: offered,
            });
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    /// Three real Ed25519 public keys, written out so the tests neither need a
    /// random source nor produce a different key on every run.
    const KEYS: [&str; 3] = [
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIMzoeu1zYAjIT/oNaHRbkjXLzDRdplZkXUDe8T93J3tZ one",
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAINH6yEuCsH/3RPsjeOUod8ElN7x9cWbT/dGfdCTbFN/F two",
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOtBfs9ZFUTiJmMls0Jrq6jgLiswUt1l6nmgrhHMQndp three",
    ];

    fn key(index: usize) -> PublicKey {
        PublicKey::from_str(KEYS[index]).expect("parse test key")
    }

    #[test]
    fn fingerprints_match_what_ssh_keygen_prints() {
        // Checked against `ssh-keygen -lf`, so a change in the notation shows
        // up here rather than in a dialog the user is supposed to compare.
        assert_eq!(
            fingerprint(&key(0)),
            "SHA256:jVQP2dXZo1RnhVBNG4taowQS5nSg/wCI0K30hzuu90o"
        );
    }

    #[test]
    fn a_recorded_key_connects_without_asking() {
        let verdict = Verdict::default();
        assert!(verify(
            "example.org",
            &key(0),
            &HostKeyDecision::KnownOnly,
            &verdict,
            &[key(0)],
        ));
        assert!(verdict.take_refusal().is_none());
        assert!(verdict.take_to_learn().is_none());
    }

    #[test]
    fn an_unknown_key_stops_and_reports_its_fingerprint() {
        let verdict = Verdict::default();
        assert!(!verify(
            "example.org",
            &key(0),
            &HostKeyDecision::KnownOnly,
            &verdict,
            &[],
        ));
        match verdict.take_refusal().expect("a refusal") {
            Error::HostKeyUnknown { fingerprint, .. } => {
                assert_eq!(
                    fingerprint,
                    "SHA256:jVQP2dXZo1RnhVBNG4taowQS5nSg/wCI0K30hzuu90o"
                );
            }
            other => panic!("wrong refusal: {other:?}"),
        }
    }

    #[test]
    fn accepting_a_fingerprint_lets_exactly_that_key_through() {
        let verdict = Verdict::default();
        let decision = HostKeyDecision::Trust {
            fingerprint: fingerprint(&key(0)),
        };
        assert!(verify("example.org", &key(0), &decision, &verdict, &[]));
        assert_eq!(verdict.take_to_learn(), Some(key(0)));
    }

    #[test]
    fn accepting_one_fingerprint_does_not_accept_another_key() {
        let verdict = Verdict::default();
        let decision = HostKeyDecision::Trust {
            fingerprint: fingerprint(&key(0)),
        };
        assert!(!verify("example.org", &key(1), &decision, &verdict, &[]));
        assert!(matches!(
            verdict.take_refusal(),
            Some(Error::HostKeyUnknown { .. })
        ));
    }

    #[test]
    fn a_changed_key_is_refused_even_when_the_user_just_accepted_it() {
        let verdict = Verdict::default();
        // The user clicked "trust" on the new fingerprint. It still does not
        // connect: what is on file says something else, and that has to be
        // dealt with knowingly.
        let decision = HostKeyDecision::Trust {
            fingerprint: fingerprint(&key(1)),
        };
        assert!(!verify(
            "example.org",
            &key(1),
            &decision,
            &verdict,
            &[key(0)],
        ));
        match verdict.take_refusal().expect("a refusal") {
            Error::HostKeyChanged {
                fingerprint: shown,
                known_fingerprint,
                ..
            } => {
                assert_eq!(shown, fingerprint(&key(1)));
                assert_eq!(known_fingerprint, fingerprint(&key(0)));
            }
            other => panic!("wrong refusal: {other:?}"),
        }
        assert!(verdict.take_to_learn().is_none());
    }

    #[test]
    fn a_second_recorded_key_still_counts_as_known() {
        // A host may legitimately have several keys on file, one per algorithm.
        let verdict = Verdict::default();
        assert!(verify(
            "example.org",
            &key(2),
            &HostKeyDecision::KnownOnly,
            &verdict,
            &[key(0), key(2)],
        ));
    }
}
