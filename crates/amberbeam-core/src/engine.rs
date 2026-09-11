//! Moving the bytes.
//!
//! One loop, two endpoints, no idea which of them is local. Everything that
//! makes a transfer safe rather than merely fast lives here:
//!
//! * A download is written to a `.ampart` file and renamed only when it is
//!   whole. A half file must never look finished in a file manager.
//! * An upload writes to its final name, because the detour over a temporary
//!   one needs rename rights that plenty of accounts do not have.
//! * **Continuing is refused unless the source is unchanged.** Appending to a
//!   file whose source has moved on produces a mixture of two versions that
//!   looks complete, and that is the worst thing a transfer program can do,
//!   because it stays silent.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::error::{Error, Result};
use crate::stream::CHUNK;
use crate::transfer::ResumeMarker;

/// The suffix a download wears until it is complete.
pub const PARTIAL_SUFFIX: &str = ".ampart";

/// What a running transfer reports, and how it is stopped.
#[derive(Debug, Default)]
pub struct Progress {
    /// Bytes written so far, including anything a resume started from.
    done: AtomicU64,
    /// Size of the source, when it is known.
    total: AtomicU64,
    cancelled: AtomicBool,
}

impl Progress {
    pub fn new(total: Option<u64>, already: u64) -> Arc<Self> {
        Arc::new(Self {
            done: AtomicU64::new(already),
            total: AtomicU64::new(total.unwrap_or(0)),
            cancelled: AtomicBool::new(false),
        })
    }

    pub fn set_total(&self, total: u64) {
        self.total.store(total, Ordering::Relaxed);
    }

    pub fn set_done(&self, done: u64) {
        self.done.store(done, Ordering::Relaxed);
    }

    pub fn done(&self) -> u64 {
        self.done.load(Ordering::Relaxed)
    }

    pub fn total(&self) -> u64 {
        self.total.load(Ordering::Relaxed)
    }

    /// Asks a running transfer to stop at the next chunk.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}

/// What the caller has to know about the source before continuing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResumeVerdict {
    /// The source is as it was; appending is safe.
    Safe,
    /// The source has changed. Ask before doing anything.
    Changed,
    /// Nothing was written before; there is nothing to continue.
    Fresh,
}

/// Decides whether a broken transfer may simply carry on.
///
/// The check is deliberately strict: without both a size and a timestamp on
/// both sides, the answer is "changed". A silent wrong answer here produces a
/// file that is corrupt and looks fine.
pub fn may_resume(
    marker: Option<&ResumeMarker>,
    size: u64,
    modified: Option<i64>,
) -> ResumeVerdict {
    let Some(marker) = marker else {
        return ResumeVerdict::Fresh;
    };
    if marker.offset == 0 {
        return ResumeVerdict::Fresh;
    }
    if marker.matches_source(size, modified) {
        ResumeVerdict::Safe
    } else {
        ResumeVerdict::Changed
    }
}

/// Copies from a reader to a writer, counting as it goes.
///
/// Returns the number of bytes moved in this run. Cancellation is not a
/// failure: the caller keeps the offset and can pick it up later.
pub async fn copy<R, W>(reader: &mut R, writer: &mut W, progress: &Progress) -> Result<u64>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut buffer = vec![0_u8; CHUNK];
    let mut moved = 0_u64;

    loop {
        if progress.is_cancelled() {
            break;
        }
        let read = reader.read(&mut buffer).await.map_err(Error::other)?;
        if read == 0 {
            break;
        }
        writer
            .write_all(&buffer[..read])
            .await
            .map_err(Error::other)?;

        moved += read as u64;
        progress.done.fetch_add(read as u64, Ordering::Relaxed);
    }

    // Flushed before anyone is told the transfer finished, or a rename could
    // put the final name on a file that is still missing its last chunk.
    writer.flush().await.map_err(Error::other)?;
    Ok(moved)
}

/// The name a download carries while it is incomplete.
pub fn partial_name(name: &str) -> String {
    format!("{name}{PARTIAL_SUFFIX}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marker(offset: u64, size: u64, modified: Option<i64>) -> ResumeMarker {
        ResumeMarker {
            offset,
            source_size: size,
            source_modified: modified,
        }
    }

    #[test]
    fn nothing_written_means_nothing_to_continue() {
        assert_eq!(may_resume(None, 100, Some(5)), ResumeVerdict::Fresh);
        assert_eq!(
            may_resume(Some(&marker(0, 100, Some(5))), 100, Some(5)),
            ResumeVerdict::Fresh
        );
    }

    #[test]
    fn an_unchanged_source_may_be_continued() {
        assert_eq!(
            may_resume(Some(&marker(50, 100, Some(5))), 100, Some(5)),
            ResumeVerdict::Safe
        );
    }

    #[test]
    fn a_source_that_grew_is_not_continued_behind_the_users_back() {
        assert_eq!(
            may_resume(Some(&marker(50, 100, Some(5))), 140, Some(5)),
            ResumeVerdict::Changed
        );
    }

    #[test]
    fn a_source_with_the_same_size_but_a_new_timestamp_is_not_continued() {
        // The same size says nothing: an edit that replaces one word for
        // another of equal length is exactly the case that would go unnoticed.
        assert_eq!(
            may_resume(Some(&marker(50, 100, Some(5))), 100, Some(9)),
            ResumeVerdict::Changed
        );
    }

    #[test]
    fn a_server_without_timestamps_never_gets_the_benefit_of_the_doubt() {
        assert_eq!(
            may_resume(Some(&marker(50, 100, None)), 100, None),
            ResumeVerdict::Changed
        );
    }

    #[tokio::test]
    async fn copying_moves_everything_and_counts_it() {
        let source = b"0123456789".repeat(10_000);
        let mut reader = std::io::Cursor::new(source.clone());
        let mut target: Vec<u8> = Vec::new();
        let progress = Progress::new(Some(source.len() as u64), 0);

        let moved = copy(&mut reader, &mut target, &progress).await.unwrap();

        assert_eq!(moved, source.len() as u64);
        assert_eq!(target, source);
        assert_eq!(progress.done(), source.len() as u64);
    }

    #[tokio::test]
    async fn a_cancelled_copy_stops_and_keeps_what_it_managed() {
        let source = vec![7_u8; CHUNK * 8];
        let mut reader = std::io::Cursor::new(source);
        let mut target: Vec<u8> = Vec::new();
        let progress = Progress::new(Some((CHUNK * 8) as u64), 0);

        // Cancelled before the first chunk: nothing moves, and that is not an
        // error — the offset simply stays where it was.
        progress.cancel();
        let moved = copy(&mut reader, &mut target, &progress).await.unwrap();

        assert_eq!(moved, 0);
        assert!(target.is_empty());
        assert!(progress.is_cancelled());
    }

    #[tokio::test]
    async fn a_resumed_copy_counts_from_where_it_left_off() {
        let rest = vec![1_u8; 400];
        let mut reader = std::io::Cursor::new(rest);
        let mut target: Vec<u8> = Vec::new();
        let progress = Progress::new(Some(1000), 600);

        copy(&mut reader, &mut target, &progress).await.unwrap();

        assert_eq!(progress.done(), 1000, "progress counts the whole file");
        assert_eq!(target.len(), 400, "only the rest was moved");
    }

    #[test]
    fn a_partial_download_is_not_mistaken_for_a_finished_one() {
        assert_eq!(partial_name("holiday.mov"), "holiday.mov.ampart");
    }
}
