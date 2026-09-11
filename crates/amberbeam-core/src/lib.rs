//! Core of AmberBeam: connections, directory listings, queue and transfers.
//!
//! This crate holds everything that is not a user interface and not tied to a
//! platform shell. It builds and tests on any operating system, so the parts of
//! AmberBeam that carry the actual risk can be developed and verified without a
//! Mac — and so the same core can later sit behind an HTTP/WebSocket service
//! (milestone M7) instead of behind Tauri.
//!
//! Two rules hold for every module in here:
//!
//! 1. A transfer has a source and a target, and both are ordinary endpoints.
//!    No code may assume that either side is the local machine.
//! 2. Nothing in this crate produces text for the user. Messages are values
//!    the user interface translates; see the `i18n` module of the frontend.

pub mod config;
pub mod endpoint;
pub mod engine;
pub mod error;
pub mod events;
pub mod fs;
pub mod local;
pub mod ops;
pub mod registry;
pub mod session;
pub mod sftp;
pub mod stream;
pub mod system;
pub mod transfer;

pub use config::{AuthKind, Config, QuickConnectEntry, Settings, Site};
pub use endpoint::{Endpoint, EndpointId, FtpSecurity, Protocol};
pub use error::{Error, PathProblem, Result};
pub use events::{ConnectionState, Event, Events, LogDirection};
pub use fs::{DirEntry, EntryKind, Listing, Permissions};
pub use ops::{Measurement, PermissionBits};
pub use registry::{Connected, Sessions};
pub use session::Session;
pub use system::CoreInfo;
pub use transfer::{ConflictPolicy, JobState, ResumeMarker, TransferJob, TransferSide};
