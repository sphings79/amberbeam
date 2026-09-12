//! What can go wrong, in a form the user interface can translate.
//!
//! No variant carries a finished sentence. A message assembled here would be
//! English text baked into the core, and section 11 of the concept paper has
//! exactly one rule about that: no text stands in the code. What travels to the
//! frontend is a kind plus the few details that belong in the sentence, and the
//! language file decides how it reads.

use std::fmt;

use serde::{Deserialize, Serialize};

/// The kind of failure, which is what a translation key is chosen by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Error {
    /// The address could not be reached at all.
    Unreachable { host: String, port: u16 },
    /// The server refused every credential offered.
    AuthenticationFailed { user: String },
    /// A key file could not be read or parsed.
    KeyFile { path: String },
    /// A key file is encrypted and the passphrase was wrong or missing.
    KeyPassphrase { path: String },
    /// No agent was reachable, or it held nothing the server accepted.
    Agent,
    /// The server is not in `known_hosts`. The connection waits for a decision.
    HostKeyUnknown {
        host: String,
        /// SHA-256 fingerprint in the notation OpenSSH prints.
        fingerprint: String,
    },
    /// The server's key differs from the one in `known_hosts`. Blocked.
    HostKeyChanged {
        host: String,
        fingerprint: String,
        known_fingerprint: String,
    },
    /// The server's TLS certificate was not accepted. Carries what the window
    /// needs to ask about it, including *why* it was refused.
    CertificateUntrusted {
        host: String,
        fingerprint: String,
        /// The translation key naming the problem — self-signed, expired and
        /// wrong-name are told apart rather than lumped together.
        reason: String,
        /// What the TLS library said, for the log and the small print.
        detail: String,
    },
    /// The server took the connection and then said nothing.
    ///
    /// Not the same as unreachable: something answered. A firewall that
    /// swallows rather than refuses looks exactly like this, and so does a port
    /// that belongs to a different program.
    TimedOut {
        host: String,
        port: u16,
        seconds: u64,
    },
    /// The server would not encrypt at all, or would not encrypt the data
    /// channel. Never silently downgraded: a data channel without `PROT P` is
    /// not half secure, it is in the clear.
    EncryptionRefused { detail: String },
    /// The server will not talk at all without TLS, and this connection has
    /// none.
    ///
    /// Its own kind rather than a failed login, because it is not one: the
    /// password was never looked at. Told apart because the remedy is a
    /// setting the person can change in five seconds, and "the server refused
    /// the login" sends them to check a password that was never wrong.
    EncryptionRequired { host: String, detail: String },
    /// A path could not be listed, read or written.
    Path { path: String, reason: PathProblem },
    /// The source changed since the transfer broke off, so continuing would
    /// stitch two versions together. The window has to ask before anything
    /// else happens.
    SourceChanged,
    /// The connection was lost while something was in flight.
    Disconnected,
    /// The endpoint asked for is not connected.
    NotConnected,
    /// Anything the core did not foresee. `detail` is for the log, not for a
    /// sentence shown to the user.
    Other { detail: String },
}

/// Why a path did not work out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PathProblem {
    NotFound,
    PermissionDenied,
    NotADirectory,
    AlreadyExists,
    Unknown,
}

impl Error {
    /// Wraps anything that has no better variant.
    pub fn other(detail: impl fmt::Display) -> Self {
        Error::Other {
            detail: detail.to_string(),
        }
    }

    /// The translation key the user interface looks up for this failure.
    ///
    /// Kept beside the variants on purpose: adding a variant without a key is
    /// then a compile error rather than a raw key appearing in the window.
    pub const fn message_key(&self) -> &'static str {
        match self {
            Error::Unreachable { .. } => "error.unreachable",
            Error::AuthenticationFailed { .. } => "error.authentication-failed",
            Error::KeyFile { .. } => "error.key-file",
            Error::KeyPassphrase { .. } => "error.key-passphrase",
            Error::Agent => "error.agent",
            Error::HostKeyUnknown { .. } => "error.host-key-unknown",
            Error::HostKeyChanged { .. } => "error.host-key-changed",
            Error::CertificateUntrusted { .. } => "error.certificate-untrusted",
            Error::TimedOut { .. } => "error.timed-out",
            Error::EncryptionRefused { .. } => "error.encryption-refused",
            Error::EncryptionRequired { .. } => "error.encryption-required",
            Error::Path { .. } => "error.path",
            Error::SourceChanged => "error.source-changed",
            Error::Disconnected => "error.disconnected",
            Error::NotConnected => "error.not-connected",
            Error::Other { .. } => "error.other",
        }
    }
}

impl fmt::Display for Error {
    /// For logs and `Result` in tests. The window never shows this text.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message_key())?;
        if let Error::Other { detail } = self {
            write!(f, ": {detail}")?;
        }
        Ok(())
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(source: std::io::Error) -> Self {
        Error::Path {
            path: String::new(),
            reason: PathProblem::from(source.kind()),
        }
    }
}

impl From<std::io::ErrorKind> for PathProblem {
    fn from(kind: std::io::ErrorKind) -> Self {
        match kind {
            std::io::ErrorKind::NotFound => PathProblem::NotFound,
            std::io::ErrorKind::PermissionDenied => PathProblem::PermissionDenied,
            std::io::ErrorKind::NotADirectory => PathProblem::NotADirectory,
            std::io::ErrorKind::AlreadyExists => PathProblem::AlreadyExists,
            _ => PathProblem::Unknown,
        }
    }
}

/// Shorthand for the whole crate.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_variant_names_a_translation_key() {
        let samples = [
            Error::Unreachable {
                host: "example.org".into(),
                port: 22,
            },
            Error::AuthenticationFailed {
                user: "dennis".into(),
            },
            Error::Agent,
            Error::Disconnected,
            Error::NotConnected,
            Error::other("anything"),
        ];
        for error in samples {
            assert!(error.message_key().starts_with("error."));
        }
    }

    #[test]
    fn io_errors_keep_their_reason() {
        let error: Error = std::io::Error::from(std::io::ErrorKind::PermissionDenied).into();
        assert!(matches!(
            error,
            Error::Path {
                reason: PathProblem::PermissionDenied,
                ..
            }
        ));
    }

    #[test]
    fn a_failure_travels_as_a_tagged_object() {
        let text = serde_json::to_string(&Error::Unreachable {
            host: "example.org".into(),
            port: 2222,
        })
        .expect("serialise");
        assert!(text.contains("\"kind\":\"unreachable\""));
        assert!(text.contains("2222"));
    }
}
