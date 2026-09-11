//! What the core can say about itself.
//!
//! The window of milestone M0 shows exactly this, and it shows it through the
//! frontend's bridge module. That makes the wiring visible: user interface →
//! bridge → shell → core. If this text appears on screen, all three seams hold.

use serde::{Deserialize, Serialize};

use crate::endpoint::Protocol;

/// A protocol the core carries, along with the defaults it starts out with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolInfo {
    pub protocol: Protocol,
    pub default_port: Option<u16>,
    pub default_concurrency: u8,
    pub encrypted: bool,
}

/// Version and capabilities of the running core.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreInfo {
    /// Version of the core crate, not of the shell around it.
    pub version: &'static str,
    /// Architecture the core was compiled for, such as `aarch64`.
    pub arch: &'static str,
    /// Operating system the core runs on, such as `macos`.
    pub os: &'static str,
    pub protocols: Vec<ProtocolInfo>,
}

impl CoreInfo {
    pub fn gather() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION"),
            arch: std::env::consts::ARCH,
            os: std::env::consts::OS,
            protocols: [
                Protocol::Sftp,
                Protocol::Ftp,
                Protocol::Ftps,
                Protocol::Local,
            ]
            .into_iter()
            .map(|protocol| ProtocolInfo {
                protocol,
                default_port: protocol.default_port(),
                default_concurrency: protocol.default_concurrency(),
                encrypted: protocol.is_encrypted(),
            })
            .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_core_reports_every_protocol_of_version_one() {
        let info = CoreInfo::gather();
        assert_eq!(info.protocols.len(), 4);
        assert!(!info.version.is_empty());
        assert!(!info.arch.is_empty());
        assert!(!info.os.is_empty());
    }

    #[test]
    fn core_info_survives_the_trip_through_json() {
        let info = CoreInfo::gather();
        let text = serde_json::to_string(&info).expect("serialise");
        assert!(text.contains("\"defaultConcurrency\":8"));
    }
}
