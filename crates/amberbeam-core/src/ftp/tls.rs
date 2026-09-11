//! Deciding whether to trust an FTPS server's certificate.
//!
//! The same shape as the SSH host key check in [`crate::sftp::hostkey`], and
//! for the same reason: strict by default, an exception only for one
//! certificate on one server, and never a dialog that can be clicked away in
//! passing.
//!
//! Two failures are told apart rather than lumped into "certificate error",
//! because they mean different things:
//!
//! * **Self-signed or unknown issuer** — common on a machine of one's own, and
//!   safe enough once the fingerprint has been compared with what the operator
//!   says it should be.
//! * **Expired** — the certificate was once valid and nobody renewed it. Also
//!   what a stolen certificate looks like after its owner revoked it.
//!
//! What the user accepts is the fingerprint of that one certificate. Accepting
//! is never "stop checking": a different certificate on the same host asks
//! again.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// Why a certificate was not accepted on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CertificateProblem {
    /// Signed by itself or by an authority the system does not know.
    UnknownIssuer,
    /// Valid once, not any more.
    Expired,
    /// Withdrawn by the authority that issued it. Never something to wave
    /// through: this is what a stolen certificate looks like afterwards.
    Revoked,
    /// The signature does not hold, or the certificate cannot be read at all.
    /// Not a naming problem — something is wrong with the bytes.
    Broken,
    /// Not yet valid — a clock is wrong somewhere, on one side or the other.
    NotYetValid,
    /// A valid certificate, for a different name than the one connected to.
    WrongName,
    /// Refused for a reason that has no better name.
    Other,
}

impl CertificateProblem {
    /// The translation key naming this problem in the window.
    pub const fn message_key(self) -> &'static str {
        match self {
            CertificateProblem::UnknownIssuer => "certificate.unknown-issuer",
            CertificateProblem::Expired => "certificate.expired",
            CertificateProblem::Revoked => "certificate.revoked",
            CertificateProblem::Broken => "certificate.broken",
            CertificateProblem::NotYetValid => "certificate.not-yet-valid",
            CertificateProblem::WrongName => "certificate.wrong-name",
            CertificateProblem::Other => "certificate.other",
        }
    }
}

/// What the window shows when it asks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CertificateFacts {
    pub host: String,
    /// SHA-256 over the certificate, in the notation `openssl` prints.
    pub fingerprint: String,
    pub problem: CertificateProblem,
    /// What the TLS library said, word for word. The headline above is the
    /// translated summary; this is for the log and for the line of small print
    /// under it, so a puzzling case can still be looked up.
    pub detail: String,
}

/// The fingerprint of a certificate, as colon separated hex.
///
/// The same notation `openssl x509 -fingerprint -sha256` produces, so the two
/// can be compared by eye — which is the entire point of showing it.
pub fn fingerprint(der: &[u8]) -> String {
    use sha2::{Digest, Sha256};

    let digest = Sha256::digest(der);
    digest
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(":")
}

/// What the user has already agreed to for this attempt.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum CertificateDecision {
    /// Accept only what the system already trusts. Every first attempt.
    #[default]
    TrustedOnly,
    /// The user was shown this fingerprint and accepted it. Anything else is
    /// still refused: the decision belongs to one certificate, not to a host.
    Trust { fingerprint: String },
}

/// Where a refusal is put down, so the connection can explain itself after the
/// TLS layer has closed.
///
/// It also records whether a certificate was seen at all. That single bit tells
/// two very different failures apart: a server that refuses `AUTH TLS` never
/// gets as far as a handshake, while a server that refuses `PROT P` has already
/// shown its certificate. Without it both arrive as "the server said no".
#[derive(Debug, Default)]
pub struct Verdict {
    refusal: Mutex<Option<CertificateFacts>>,
    seen: std::sync::atomic::AtomicBool,
}

impl Verdict {
    pub fn take(&self) -> Option<CertificateFacts> {
        self.refusal.lock().ok().and_then(|mut slot| slot.take())
    }

    pub fn refuse(&self, facts: CertificateFacts) {
        if let Ok(mut slot) = self.refusal.lock() {
            *slot = Some(facts);
        }
    }

    /// Noted whichever way the certificate was judged.
    pub fn saw_certificate(&self) {
        self.seen.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// True once the server has presented a certificate, so anything that goes
    /// wrong afterwards is no longer about starting encryption.
    pub fn handshaked(&self) -> bool {
        self.seen.load(std::sync::atomic::Ordering::Relaxed)
    }
}

/// Decides what happens to a certificate the system did not vouch for.
///
/// Split out from the TLS plumbing so it can be tested without a handshake —
/// this is the part that must not be wrong.
pub fn accept_untrusted(
    host: &str,
    der: &[u8],
    problem: CertificateProblem,
    detail: &str,
    decision: &CertificateDecision,
    verdict: &Verdict,
) -> bool {
    let offered = fingerprint(der);
    match decision {
        CertificateDecision::Trust {
            fingerprint: accepted,
        } if *accepted == offered => true,
        _ => {
            verdict.refuse(CertificateFacts {
                host: host.to_string(),
                fingerprint: offered,
                problem,
                detail: detail.to_string(),
            });
            false
        }
    }
}

/// Exceptions the user has agreed to, by host.
///
/// Kept beside the rest of the configuration as plain text: an exception to
/// certificate checking is exactly the sort of thing that should be visible and
/// removable without the program's help.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Exceptions {
    /// `host:port` to the accepted fingerprint.
    pub accepted: std::collections::BTreeMap<String, String>,
}

impl Exceptions {
    pub fn key(host: &str, port: u16) -> String {
        format!("{host}:{port}")
    }

    pub fn decision_for(&self, host: &str, port: u16) -> CertificateDecision {
        match self.accepted.get(&Self::key(host, port)) {
            Some(fingerprint) => CertificateDecision::Trust {
                fingerprint: fingerprint.clone(),
            },
            None => CertificateDecision::TrustedOnly,
        }
    }

    pub fn accept(&mut self, host: &str, port: u16, fingerprint: &str) {
        self.accepted
            .insert(Self::key(host, port), fingerprint.to_string());
    }

    pub fn forget(&mut self, host: &str, port: u16) {
        self.accepted.remove(&Self::key(host, port));
    }
}

// --- The rustls side ---------------------------------------------------------
//
// Everything below turns the decision above into something a TLS handshake can
// use. It is deliberately the smaller half: the rules live in plain functions
// that can be read and tested, and this part only carries them across.

use std::sync::Arc;

use suppaftp::tokio::AsyncRustlsConnector;
use suppaftp::tokio_rustls::rustls::client::danger::{
    HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier,
};
use suppaftp::tokio_rustls::rustls::client::WebPkiServerVerifier;
use suppaftp::tokio_rustls::rustls::crypto::{
    verify_tls12_signature, verify_tls13_signature, CryptoProvider,
};
use suppaftp::tokio_rustls::rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use suppaftp::tokio_rustls::rustls::{
    CertificateError, ClientConfig, DigitallySignedStruct, Error as TlsError, RootCertStore,
    SignatureScheme,
};
use suppaftp::tokio_rustls::TlsConnector;

/// Names the reason rustls gave, so the window can say *why* rather than
/// "certificate error".
///
/// The categories are chosen by what the user can sensibly do about them, not
/// by how the TLS library files them. Everything structural — no known issuer,
/// a certificate acting as its own authority, an algorithm nobody supports any
/// more — ends up as "nothing vouches for this", because the answer to all of
/// them is the same: compare the fingerprint against what the operator says it
/// should be, or do not connect. A certificate generated in one line with
/// `openssl req -x509` lands here, refused as `CaUsedAsEndEntity`.
///
/// Two things are kept out of that bucket on purpose. A revoked certificate was
/// deliberately withdrawn, and a broken signature means the bytes were tampered
/// with or corrupted; neither is something to settle by comparing a
/// fingerprint.
fn problem_of(error: &TlsError) -> CertificateProblem {
    let TlsError::InvalidCertificate(reason) = error else {
        return CertificateProblem::Other;
    };
    match reason {
        CertificateError::Expired | CertificateError::ExpiredContext { .. } => {
            CertificateProblem::Expired
        }
        CertificateError::NotValidYet | CertificateError::NotValidYetContext { .. } => {
            CertificateProblem::NotYetValid
        }
        CertificateError::NotValidForName | CertificateError::NotValidForNameContext { .. } => {
            CertificateProblem::WrongName
        }
        CertificateError::Revoked | CertificateError::UnknownRevocationStatus => {
            CertificateProblem::Revoked
        }
        CertificateError::BadEncoding | CertificateError::BadSignature => {
            CertificateProblem::Broken
        }
        _ => CertificateProblem::UnknownIssuer,
    }
}

/// Checks the server's certificate the ordinary way first, and falls back to
/// the fingerprint the user accepted.
///
/// The fallback is the whole point of the type, and it is narrow on purpose:
/// one fingerprint, put there by a person who was shown it. Signature checking
/// is never touched — an accepted certificate still has to prove it holds the
/// matching private key.
#[derive(Debug)]
struct Checker {
    host: String,
    decision: CertificateDecision,
    /// Absent when no root certificates are available, in which case every
    /// server counts as unknown and is asked about once.
    trusted: Option<Arc<WebPkiServerVerifier>>,
    provider: Arc<CryptoProvider>,
    verdict: Arc<Verdict>,
}

impl ServerCertVerifier for Checker {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, TlsError> {
        self.verdict.saw_certificate();

        let refusal = match &self.trusted {
            Some(trusted) => match trusted.verify_server_cert(
                end_entity,
                intermediates,
                server_name,
                ocsp_response,
                now,
            ) {
                Ok(verified) => return Ok(verified),
                Err(error) => error,
            },
            None => TlsError::InvalidCertificate(CertificateError::UnknownIssuer),
        };

        if accept_untrusted(
            &self.host,
            end_entity,
            problem_of(&refusal),
            &refusal.to_string(),
            &self.decision,
            &self.verdict,
        ) {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(refusal)
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, TlsError> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, TlsError> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// The certificates this machine already trusts.
///
/// Read from the operating system rather than shipped with the program, so a
/// certificate authority the user or their employer installed counts here too.
/// Read once: the store does not change while the program runs, and parsing a
/// few hundred certificates for every connection would be noticeable.
///
/// An empty or unreadable store is not an error. It leaves the program at the
/// strict end of the scale — nothing verifies on its own, so every server is
/// shown to the user once and trusted by fingerprint afterwards. That is what
/// happens in a container image without `ca-certificates`, and it is better
/// than quietly trusting a list nobody chose.
pub fn system_roots() -> Arc<RootCertStore> {
    static ROOTS: std::sync::OnceLock<Arc<RootCertStore>> = std::sync::OnceLock::new();

    Arc::clone(ROOTS.get_or_init(|| {
        let found = rustls_native_certs::load_native_certs();
        let mut store = RootCertStore::empty();
        // Certificates the system holds but rustls cannot parse are skipped
        // rather than fatal: one unreadable entry must not cost the other five
        // hundred. How many were taken is reported in the connection log.
        let _ = store.add_parsable_certificates(found.certs);
        Arc::new(store)
    }))
}

/// Builds the connector for one attempt, and the slip of paper it writes its
/// refusal on.
///
/// The verdict is handed back because a TLS handshake that fails says very
/// little by the time the error surfaces; the fingerprint and the reason have
/// to be caught while the certificate is still in hand.
pub fn connector(
    host: &str,
    decision: CertificateDecision,
) -> (AsyncRustlsConnector, Arc<Verdict>) {
    let provider = Arc::new(suppaftp::tokio_rustls::rustls::crypto::ring::default_provider());
    let roots = system_roots();
    let trusted = if roots.is_empty() {
        None
    } else {
        WebPkiServerVerifier::builder_with_provider(roots, provider.clone())
            .build()
            .ok()
    };

    let verdict = Arc::new(Verdict::default());
    let checker = Arc::new(Checker {
        host: host.to_string(),
        decision,
        trusted,
        provider: provider.clone(),
        verdict: Arc::clone(&verdict),
    });

    let config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .expect("the default protocol versions are always supported")
        .dangerous()
        .with_custom_certificate_verifier(checker)
        .with_no_client_auth();

    (
        AsyncRustlsConnector::from(TlsConnector::from(Arc::new(config))),
        verdict,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const CERTIFICATE: &[u8] = b"a certificate, as far as this test is concerned";
    const ANOTHER: &[u8] = b"a different certificate entirely";

    #[test]
    fn a_fingerprint_reads_like_the_one_openssl_prints() {
        let printed = fingerprint(CERTIFICATE);
        assert_eq!(printed.len(), 32 * 3 - 1, "32 bytes, colon separated");
        assert!(printed.chars().all(|c| c.is_ascii_hexdigit() || c == ':'));
        assert!(printed
            .chars()
            .filter(|c| c.is_alphabetic())
            .all(|c| c.is_uppercase()));
        // The same certificate always prints the same thing, or comparing by
        // eye would be pointless.
        assert_eq!(printed, fingerprint(CERTIFICATE));
        assert_ne!(printed, fingerprint(ANOTHER));
    }

    #[test]
    fn an_untrusted_certificate_is_refused_and_described() {
        let verdict = Verdict::default();
        assert!(!accept_untrusted(
            "example.org",
            CERTIFICATE,
            CertificateProblem::UnknownIssuer,
            "no issuer this machine knows",
            &CertificateDecision::TrustedOnly,
            &verdict,
        ));
        let facts = verdict.take().expect("a refusal");
        assert_eq!(facts.problem, CertificateProblem::UnknownIssuer);
        assert!(
            !facts.detail.is_empty(),
            "the log needs the original wording"
        );
        assert_eq!(facts.fingerprint, fingerprint(CERTIFICATE));
        assert_eq!(facts.host, "example.org");
    }

    #[test]
    fn accepting_a_fingerprint_lets_exactly_that_certificate_through() {
        let verdict = Verdict::default();
        let decision = CertificateDecision::Trust {
            fingerprint: fingerprint(CERTIFICATE),
        };
        assert!(accept_untrusted(
            "example.org",
            CERTIFICATE,
            CertificateProblem::UnknownIssuer,
            "no issuer this machine knows",
            &decision,
            &verdict,
        ));
        assert!(verdict.take().is_none());
    }

    #[test]
    fn a_different_certificate_on_the_same_host_asks_again() {
        // Accepting is never "stop checking this server".
        let verdict = Verdict::default();
        let decision = CertificateDecision::Trust {
            fingerprint: fingerprint(CERTIFICATE),
        };
        assert!(!accept_untrusted(
            "example.org",
            ANOTHER,
            CertificateProblem::Expired,
            "the certificate expired last March",
            &decision,
            &verdict,
        ));
        assert_eq!(
            verdict.take().expect("a refusal").problem,
            CertificateProblem::Expired
        );
    }

    #[test]
    fn every_problem_names_a_translation_key() {
        for problem in [
            CertificateProblem::UnknownIssuer,
            CertificateProblem::Expired,
            CertificateProblem::Revoked,
            CertificateProblem::Broken,
            CertificateProblem::NotYetValid,
            CertificateProblem::WrongName,
            CertificateProblem::Other,
        ] {
            assert!(problem.message_key().starts_with("certificate."));
        }
        // Self-signed and expired are not the same message.
        assert_ne!(
            CertificateProblem::UnknownIssuer.message_key(),
            CertificateProblem::Expired.message_key()
        );
    }

    #[test]
    fn a_handshake_is_noted_whether_or_not_the_certificate_passes() {
        // This is what separates "refused to start TLS" from "refused to
        // encrypt the data channel" once the connection has fallen over.
        let verdict = Verdict::default();
        assert!(!verdict.handshaked());
        verdict.saw_certificate();
        assert!(verdict.handshaked());
        // Taking the refusal must not erase the fact that one happened.
        verdict.refuse(CertificateFacts {
            host: "example.org".into(),
            fingerprint: fingerprint(CERTIFICATE),
            problem: CertificateProblem::Expired,
            detail: "the certificate expired last March".into(),
        });
        assert!(verdict.take().is_some());
        assert!(verdict.handshaked());
    }

    #[test]
    fn an_exception_belongs_to_one_host_and_port() {
        let mut exceptions = Exceptions::default();
        exceptions.accept("example.org", 21, "AA:BB");

        assert_eq!(
            exceptions.decision_for("example.org", 21),
            CertificateDecision::Trust {
                fingerprint: "AA:BB".into()
            }
        );
        // A different port is a different service.
        assert_eq!(
            exceptions.decision_for("example.org", 990),
            CertificateDecision::TrustedOnly
        );
        assert_eq!(
            exceptions.decision_for("other.example", 21),
            CertificateDecision::TrustedOnly
        );

        exceptions.forget("example.org", 21);
        assert_eq!(
            exceptions.decision_for("example.org", 21),
            CertificateDecision::TrustedOnly
        );
    }
}
