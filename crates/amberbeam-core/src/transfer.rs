//! Transfer jobs and the queue they live in.
//!
//! The types here exist from the first commit so that no later code can grow a
//! "download" or "upload" special case. A job connects two sides, and a side is
//! an endpoint plus a path. Which of them happens to be the local disk is of no
//! interest to this module.
//!
//! The behaviour behind these types lands in milestone M2; M0 fixes the shape.

use serde::{Deserialize, Serialize};

use crate::endpoint::EndpointId;

/// One side of a transfer: where on which endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferSide {
    pub endpoint: EndpointId,
    /// Absolute path on that endpoint, in the endpoint's own notation.
    pub path: String,
}

impl TransferSide {
    pub fn new(endpoint: EndpointId, path: impl Into<String>) -> Self {
        Self {
            endpoint,
            path: path.into(),
        }
    }
}

/// What the queue does with a job that already exists at the target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConflictPolicy {
    Ask,
    Overwrite,
    Skip,
    OverwriteIfNewer,
    Rename,
    Resume,
}

/// Where a job stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum JobState {
    Queued,
    Running,
    /// Held by the user, or by the core after the retry limit was reached. A
    /// job is never silently dropped.
    Paused,
    Done,
    Failed,
}

/// What a broken off transfer has to remember in order to be continued safely.
///
/// Continuing is only sound while the source is still the same file. If it has
/// changed in the meantime, appending produces a mixture of two versions that
/// looks like a complete file — the worst mistake a transfer program can make,
/// because it stays silent. Size and modification time are therefore kept with
/// the queue entry, survive a restart, and are compared before continuing; on a
/// mismatch the user is asked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResumeMarker {
    /// Bytes already written to the target.
    pub offset: u64,
    /// Size of the source when the transfer broke off.
    pub source_size: u64,
    /// Modification time of the source when the transfer broke off, as seconds
    /// since the Unix epoch. `None` when the endpoint reports no usable time.
    pub source_modified: Option<i64>,
}

impl ResumeMarker {
    /// Whether the source still matches what was seen at the break.
    ///
    /// A missing modification time on either side is not treated as a match:
    /// without it, size alone is too weak a guarantee to append silently.
    pub fn matches_source(&self, size: u64, modified: Option<i64>) -> bool {
        match (self.source_modified, modified) {
            (Some(then), Some(now)) => self.source_size == size && then == now,
            _ => false,
        }
    }
}

/// A single file on its way from one endpoint to another.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferJob {
    pub id: String,
    pub source: TransferSide,
    pub target: TransferSide,
    pub state: JobState,
    pub conflict_policy: ConflictPolicy,
    /// Size of the source in bytes, as far as the endpoint reports it.
    pub total_bytes: Option<u64>,
    /// Bytes transferred so far.
    pub done_bytes: u64,
    /// Present once the job has broken off at least one time.
    pub resume: Option<ResumeMarker>,
}

impl TransferJob {
    pub fn new(id: impl Into<String>, source: TransferSide, target: TransferSide) -> Self {
        Self {
            id: id.into(),
            source,
            target,
            state: JobState::Queued,
            conflict_policy: ConflictPolicy::Ask,
            total_bytes: None,
            done_bytes: 0,
            resume: None,
        }
    }

    /// Whether both sides sit on the same endpoint, as with copying inside one
    /// server. Nothing in the transfer engine branches on this; it exists so
    /// the user interface can label such a job.
    pub fn is_within_one_endpoint(&self) -> bool {
        self.source.endpoint == self.target.endpoint
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::endpoint::EndpointId;

    fn side(endpoint: &str, path: &str) -> TransferSide {
        TransferSide::new(EndpointId::new(endpoint), path)
    }

    #[test]
    fn a_job_between_two_servers_needs_no_local_side() {
        let job = TransferJob::new(
            "1",
            side("server-a", "/var/www/index.html"),
            side("server-b", "/srv/html/index.html"),
        );
        assert!(!job.is_within_one_endpoint());
        assert_eq!(job.state, JobState::Queued);
    }

    #[test]
    fn copying_inside_one_server_is_recognised() {
        let job = TransferJob::new("2", side("server-a", "/a.txt"), side("server-a", "/b.txt"));
        assert!(job.is_within_one_endpoint());
    }

    #[test]
    fn resuming_requires_size_and_time_to_match() {
        let marker = ResumeMarker {
            offset: 1024,
            source_size: 4096,
            source_modified: Some(1_700_000_000),
        };
        assert!(marker.matches_source(4096, Some(1_700_000_000)));
        assert!(!marker.matches_source(4097, Some(1_700_000_000)));
        assert!(!marker.matches_source(4096, Some(1_700_000_001)));
    }

    #[test]
    fn a_source_without_a_timestamp_never_counts_as_unchanged() {
        let marker = ResumeMarker {
            offset: 1024,
            source_size: 4096,
            source_modified: None,
        };
        assert!(!marker.matches_source(4096, None));
    }
}
