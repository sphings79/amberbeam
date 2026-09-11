//! Endpoints a transfer can read from or write to.
//!
//! This module carries the first of the three seams the concept paper asks for
//! (section 08). An endpoint is a place with a protocol, nothing more. The
//! local disk is one protocol among several, not a special case, which is why
//! server-to-server copying and copying within a single server need no separate
//! code path — and why FXP can later be added as one more case instead of as a
//! rebuild of the foundation.

use serde::{Deserialize, Serialize};

/// Protocol an endpoint speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    /// The file system the core itself runs on. On the desktop that is the
    /// user's Mac; in a container it is the mounted volumes of that container.
    Local,
    Sftp,
    Ftp,
    Ftps,
}

impl Protocol {
    /// How many transfers run at the same time unless a site entry says otherwise.
    ///
    /// SFTP channels are cheap, but OpenSSH caps them through `MaxSessions`,
    /// whose default is exactly 10 — so 8 leaves room instead of sitting on the
    /// edge. Every FTP transfer is a separate login, and shared hosters commonly
    /// allow three to eight per address, so 4 works everywhere. Both values are
    /// only a starting point; each site entry can raise them up to
    /// [`Protocol::MAX_CONCURRENCY`].
    pub const fn default_concurrency(self) -> u8 {
        match self {
            Protocol::Local => 4,
            Protocol::Sftp => 8,
            Protocol::Ftp | Protocol::Ftps => 4,
        }
    }

    /// Upper limit a user may configure per site. A client without a ceiling
    /// opens a connection per file and is refused by the server.
    pub const MAX_CONCURRENCY: u8 = 64;

    /// Port used when a site entry names none.
    pub const fn default_port(self) -> Option<u16> {
        match self {
            Protocol::Local => None,
            Protocol::Sftp => Some(22),
            Protocol::Ftp | Protocol::Ftps => Some(21),
        }
    }

    /// Whether traffic is encrypted. Plain FTP stays possible — many old
    /// servers speak nothing else — but has to be marked as such in the window.
    pub const fn is_encrypted(self) -> bool {
        match self {
            Protocol::Local => true,
            Protocol::Sftp | Protocol::Ftps => true,
            Protocol::Ftp => false,
        }
    }

    /// Whether a half written file can safely be given a temporary name and
    /// renamed when it is whole.
    ///
    /// Locally and over SFTP, rename is part of the deal. Over FTP it needs a
    /// right plenty of hosting accounts do not grant, and a transfer that fails
    /// at the very last step — leaving `index.html.ampart` and no index.html —
    /// would be worse than the risk it was meant to avoid.
    pub const fn rename_is_dependable(self) -> bool {
        match self {
            Protocol::Local | Protocol::Sftp => true,
            Protocol::Ftp | Protocol::Ftps => false,
        }
    }

    /// Stable identifier, used in configuration files and towards the frontend.
    pub const fn as_str(self) -> &'static str {
        match self {
            Protocol::Local => "local",
            Protocol::Sftp => "sftp",
            Protocol::Ftp => "ftp",
            Protocol::Ftps => "ftps",
        }
    }
}

/// How an FTP connection negotiates TLS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FtpSecurity {
    /// Plain FTP. Possible, but marked in the window.
    None,
    /// `AUTH TLS` on the regular port. The default for new site entries.
    Explicit,
    /// TLS from the first byte, historically on port 990.
    Implicit,
}

/// Identifier of an endpoint within one running core.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EndpointId(pub String);

impl EndpointId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A place transfers move data from or to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Endpoint {
    pub id: EndpointId,
    pub protocol: Protocol,
    /// Host name or address. Empty for [`Protocol::Local`].
    pub host: Option<String>,
    pub port: Option<u16>,
    pub user: Option<String>,
    /// TLS negotiation, only meaningful for FTP and FTPS.
    pub security: Option<FtpSecurity>,
    /// Transfers running at the same time against this endpoint.
    pub concurrency: u8,
}

impl Endpoint {
    /// An endpoint on the file system the core runs on.
    pub fn local(id: impl Into<String>) -> Self {
        Self {
            id: EndpointId::new(id),
            protocol: Protocol::Local,
            host: None,
            port: None,
            user: None,
            security: None,
            concurrency: Protocol::Local.default_concurrency(),
        }
    }

    /// A remote endpoint with the protocol's default port and concurrency.
    pub fn remote(id: impl Into<String>, protocol: Protocol, host: impl Into<String>) -> Self {
        Self {
            id: EndpointId::new(id),
            protocol,
            host: Some(host.into()),
            port: protocol.default_port(),
            user: None,
            security: match protocol {
                Protocol::Ftp | Protocol::Ftps => Some(FtpSecurity::Explicit),
                _ => None,
            },
            concurrency: protocol.default_concurrency(),
        }
    }

    /// Clamp a user supplied concurrency into the allowed range.
    pub fn set_concurrency(&mut self, requested: u8) {
        self.concurrency = requested.clamp(1, Protocol::MAX_CONCURRENCY);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concurrency_defaults_follow_the_protocol() {
        assert_eq!(Protocol::Sftp.default_concurrency(), 8);
        assert_eq!(Protocol::Ftp.default_concurrency(), 4);
        assert_eq!(Protocol::Ftps.default_concurrency(), 4);
    }

    #[test]
    fn concurrency_is_clamped_to_the_allowed_range() {
        let mut endpoint = Endpoint::remote("a", Protocol::Sftp, "example.org");
        endpoint.set_concurrency(0);
        assert_eq!(endpoint.concurrency, 1);
        endpoint.set_concurrency(255);
        assert_eq!(endpoint.concurrency, Protocol::MAX_CONCURRENCY);
    }

    #[test]
    fn new_ftp_endpoints_start_out_encrypted() {
        let endpoint = Endpoint::remote("a", Protocol::Ftp, "example.org");
        assert_eq!(endpoint.security, Some(FtpSecurity::Explicit));
        assert_eq!(endpoint.port, Some(21));
    }

    #[test]
    fn a_temporary_name_is_only_used_where_renaming_is_certain() {
        assert!(Protocol::Local.rename_is_dependable());
        assert!(Protocol::Sftp.rename_is_dependable());
        // FTP accounts often cannot rename, and a transfer that completes and
        // then fails to rename leaves nothing usable behind.
        assert!(!Protocol::Ftp.rename_is_dependable());
        assert!(!Protocol::Ftps.rename_is_dependable());
    }

    #[test]
    fn plain_ftp_is_the_only_protocol_that_is_not_encrypted() {
        assert!(!Protocol::Ftp.is_encrypted());
        assert!(Protocol::Ftps.is_encrypted());
        assert!(Protocol::Sftp.is_encrypted());
    }
}
