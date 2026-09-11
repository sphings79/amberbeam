//! The transfer queue.
//!
//! What it has to survive is a restart: somebody breaks off in the evening and
//! finds the same list in the morning, with each job knowing where it stopped.
//! So the queue is a list of plain values written to disk on every change, not
//! a set of running tasks.
//!
//! Running is a separate matter, handled by whoever drives the queue: it asks
//! for the next job that may start, and reports what happened. That split is
//! what lets the desktop shell and the container service of M7 drive the same
//! queue.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::endpoint::EndpointId;
use crate::error::{Error, Result};
use crate::transfer::{ConflictPolicy, JobState, ResumeMarker};

/// One file waiting to move, or moving.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueuedJob {
    pub id: String,
    pub source_endpoint: EndpointId,
    pub source_path: String,
    pub target_endpoint: EndpointId,
    pub target_path: String,
    /// What the row shows. Kept apart from the path so the window does not have
    /// to take a path apart to draw a line.
    pub name: String,
    pub state: JobState,
    pub conflict_policy: ConflictPolicy,
    pub total_bytes: Option<u64>,
    pub done_bytes: u64,
    pub resume: Option<ResumeMarker>,
    pub keep_modified: bool,
    pub keep_permissions: bool,
    pub use_temporary_name: bool,
    /// How many attempts have already failed.
    pub attempts: u8,
    /// Attempts this job gets before it is paused. Carried on the job so a
    /// queue written earlier still knows what it agreed to.
    #[serde(default)]
    pub retries: Option<u8>,
    /// Why it last failed, for the row to explain itself.
    pub failure: Option<Error>,
    /// Seconds since the epoch, for stable ordering of equals.
    pub added: i64,
}

impl QueuedJob {
    /// Whether both sides sit on the same connection. Only for labelling: the
    /// engine does not branch on it.
    pub fn is_within_one_endpoint(&self) -> bool {
        self.source_endpoint == self.target_endpoint
    }

    /// Whether neither side is the local disk — a copy between two servers,
    /// which travels through AmberBeam rather than directly between them.
    pub fn is_between_two_endpoints(&self, local: &EndpointId) -> bool {
        &self.source_endpoint != local && &self.target_endpoint != local
    }
}

/// The queue as a whole.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Queue {
    pub jobs: Vec<QueuedJob>,
    /// While true, nothing new starts. Running jobs are left to finish.
    pub paused: bool,
}

impl Queue {
    pub fn add(&mut self, job: QueuedJob) {
        self.jobs.push(job);
    }

    pub fn get(&self, id: &str) -> Option<&QueuedJob> {
        self.jobs.iter().find(|job| job.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut QueuedJob> {
        self.jobs.iter_mut().find(|job| job.id == id)
    }

    /// Removes a job. A running one has to be stopped first; that is the
    /// caller's business, since only it holds the handle to stop it.
    pub fn remove(&mut self, id: &str) {
        self.jobs.retain(|job| job.id != id);
    }

    /// Clears out everything that will not run again.
    pub fn clear_finished(&mut self) {
        self.jobs.retain(|job| job.state != JobState::Done);
    }

    /// Moves a job one place up or down, which is what the buttons do.
    pub fn move_by(&mut self, id: &str, delta: isize) {
        let Some(from) = self.jobs.iter().position(|job| job.id == id) else {
            return;
        };
        let to = (from as isize + delta).clamp(0, self.jobs.len() as isize - 1) as usize;
        if from == to {
            return;
        }
        let job = self.jobs.remove(from);
        self.jobs.insert(to, job);
    }

    /// Moves a job to an exact place, which is what dragging does.
    pub fn move_to(&mut self, id: &str, index: usize) {
        let Some(from) = self.jobs.iter().position(|job| job.id == id) else {
            return;
        };
        let to = index.min(self.jobs.len().saturating_sub(1));
        if from == to {
            return;
        }
        let job = self.jobs.remove(from);
        self.jobs.insert(to, job);
    }

    /// The next job that may start, given what is already running.
    ///
    /// Order is the user's: the list is walked from the top. A job is skipped
    /// when its endpoint is already as busy as it is allowed to be, rather than
    /// blocking everything behind it — a slow server must not hold up a queue
    /// that also has work for a fast one.
    pub fn next_startable(
        &self,
        limits: &HashMap<EndpointId, u32>,
        running: &HashMap<EndpointId, u32>,
    ) -> Option<&QueuedJob> {
        if self.paused {
            return None;
        }
        self.jobs.iter().find(|job| {
            if job.state != JobState::Queued {
                return false;
            }
            // Both sides count: a transfer occupies a channel at each end, and
            // for a copy within one connection that is the same connection
            // twice.
            let mut needed: HashMap<&EndpointId, u32> = HashMap::new();
            *needed.entry(&job.source_endpoint).or_default() += 1;
            *needed.entry(&job.target_endpoint).or_default() += 1;

            needed.into_iter().all(|(endpoint, wanted)| {
                let limit = limits.get(endpoint).copied().unwrap_or(1);
                let busy = running.get(endpoint).copied().unwrap_or(0);
                busy + wanted <= limit
            })
        })
    }

    /// Everything still to do, for the progress shown over the whole queue.
    pub fn totals(&self) -> Totals {
        let mut totals = Totals::default();
        for job in &self.jobs {
            match job.state {
                JobState::Done => totals.done_jobs += 1,
                JobState::Failed => totals.failed_jobs += 1,
                JobState::Running => totals.running_jobs += 1,
                JobState::Queued => totals.waiting_jobs += 1,
                JobState::Paused => totals.paused_jobs += 1,
                // A job waiting for an answer is not working and not finished;
                // counted apart so the window can say why nothing moves.
                JobState::Asking => totals.asking_jobs += 1,
            }
            totals.done_bytes += job.done_bytes;
            totals.total_bytes += job.total_bytes.unwrap_or(job.done_bytes);
        }
        totals
    }
}

/// What the queue adds up to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Totals {
    pub waiting_jobs: u32,
    pub asking_jobs: u32,
    pub running_jobs: u32,
    pub paused_jobs: u32,
    pub done_jobs: u32,
    pub failed_jobs: u32,
    pub done_bytes: u64,
    pub total_bytes: u64,
}

/// Reads and writes the queue beside the rest of the configuration.
pub fn load(path: &std::path::Path) -> Queue {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .map(|queue: Queue| {
            // Anything that claimed to be running when the program stopped was
            // not: nothing survives a restart mid-flight. It goes back to
            // waiting, with its offset intact.
            let mut queue = queue;
            for job in &mut queue.jobs {
                if job.state == JobState::Running {
                    job.state = JobState::Queued;
                }
            }
            queue
        })
        .unwrap_or_default()
}

pub fn save(path: &std::path::Path, queue: &Queue) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(Error::from)?;
    }
    let text = serde_json::to_string_pretty(queue).map_err(Error::other)?;
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, text).map_err(Error::from)?;
    std::fs::rename(&temporary, path).map_err(Error::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(id: &str, source: &str, target: &str) -> QueuedJob {
        QueuedJob {
            id: id.into(),
            source_endpoint: EndpointId::new(source),
            source_path: format!("/from/{id}"),
            target_endpoint: EndpointId::new(target),
            target_path: format!("/to/{id}"),
            name: id.into(),
            state: JobState::Queued,
            conflict_policy: ConflictPolicy::Ask,
            total_bytes: Some(1000),
            done_bytes: 0,
            resume: None,
            keep_modified: true,
            keep_permissions: false,
            use_temporary_name: true,
            attempts: 0,
            retries: Some(5),
            failure: None,
            added: 0,
        }
    }

    fn limits(pairs: &[(&str, u32)]) -> HashMap<EndpointId, u32> {
        pairs
            .iter()
            .map(|(id, limit)| (EndpointId::new(*id), *limit))
            .collect()
    }

    fn running(pairs: &[(&str, u32)]) -> HashMap<EndpointId, u32> {
        limits(pairs)
    }

    #[test]
    fn jobs_start_in_the_order_they_were_put_in() {
        let mut queue = Queue::default();
        queue.add(job("one", "local", "server"));
        queue.add(job("two", "local", "server"));

        let next = queue
            .next_startable(&limits(&[("local", 8), ("server", 8)]), &running(&[]))
            .expect("something to start");
        assert_eq!(next.id, "one");
    }

    #[test]
    fn a_busy_server_is_skipped_rather_than_blocking_the_rest() {
        let mut queue = Queue::default();
        queue.add(job("slow", "local", "slow-server"));
        queue.add(job("fast", "local", "fast-server"));

        // The slow server is already at its limit; the job behind it belongs to
        // another server and has no reason to wait.
        let next = queue
            .next_startable(
                &limits(&[("local", 8), ("slow-server", 1), ("fast-server", 4)]),
                &running(&[("slow-server", 1), ("local", 1)]),
            )
            .expect("something to start");
        assert_eq!(next.id, "fast");
    }

    #[test]
    fn a_copy_within_one_connection_counts_against_it_twice() {
        let mut queue = Queue::default();
        queue.add(job("inside", "server", "server"));

        // One channel free is not enough: the job needs one at each end, and
        // both ends are the same connection.
        assert!(queue
            .next_startable(&limits(&[("server", 2)]), &running(&[("server", 1)]))
            .is_none());
        assert!(queue
            .next_startable(&limits(&[("server", 2)]), &running(&[]))
            .is_some());
    }

    #[test]
    fn a_paused_queue_starts_nothing() {
        let mut queue = Queue::default();
        queue.add(job("one", "local", "server"));
        queue.paused = true;
        assert!(queue
            .next_startable(&limits(&[("local", 8), ("server", 8)]), &running(&[]))
            .is_none());
    }

    #[test]
    fn only_waiting_jobs_are_started() {
        let mut queue = Queue::default();
        let mut done = job("done", "local", "server");
        done.state = JobState::Done;
        let mut held = job("held", "local", "server");
        held.state = JobState::Paused;
        queue.add(done);
        queue.add(held);
        assert!(queue
            .next_startable(&limits(&[("local", 8), ("server", 8)]), &running(&[]))
            .is_none());
    }

    #[test]
    fn the_order_can_be_changed_by_one_place_or_to_a_place() {
        let mut queue = Queue::default();
        for id in ["a", "b", "c"] {
            queue.add(job(id, "local", "server"));
        }
        queue.move_by("c", -1);
        assert_eq!(ids(&queue), ["a", "c", "b"]);

        queue.move_to("b", 0);
        assert_eq!(ids(&queue), ["b", "a", "c"]);

        // Past the ends is not an error; it simply stops there.
        queue.move_by("b", -5);
        assert_eq!(ids(&queue), ["b", "a", "c"]);
        queue.move_by("b", 99);
        assert_eq!(ids(&queue), ["a", "c", "b"]);
    }

    fn ids(queue: &Queue) -> Vec<&str> {
        queue.jobs.iter().map(|job| job.id.as_str()).collect()
    }

    #[test]
    fn a_queue_survives_a_restart_with_its_offsets() {
        let path = std::env::temp_dir().join("amberbeam-queue-test.json");
        let _ = std::fs::remove_file(&path);

        let mut queue = Queue::default();
        let mut moving = job("half", "server", "local");
        moving.state = JobState::Running;
        moving.done_bytes = 400;
        moving.resume = Some(ResumeMarker {
            offset: 400,
            source_size: 1000,
            source_modified: Some(17),
        });
        queue.add(moving);
        save(&path, &queue).expect("save");

        let back = load(&path);
        let job = back.get("half").expect("the job");
        // Nothing is running after a restart, but nothing is lost either.
        assert_eq!(job.state, JobState::Queued);
        assert_eq!(job.done_bytes, 400);
        assert_eq!(job.resume.as_ref().map(|marker| marker.offset), Some(400));

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn a_queue_file_full_of_nonsense_does_not_stop_the_program() {
        let path = std::env::temp_dir().join("amberbeam-queue-broken.json");
        std::fs::write(&path, "{not json").unwrap();
        assert!(load(&path).jobs.is_empty());
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn the_totals_count_what_the_window_shows() {
        let mut queue = Queue::default();
        let mut done = job("done", "local", "server");
        done.state = JobState::Done;
        done.done_bytes = 1000;
        let mut moving = job("moving", "local", "server");
        moving.state = JobState::Running;
        moving.done_bytes = 250;
        queue.add(done);
        queue.add(moving);
        queue.add(job("waiting", "local", "server"));

        let totals = queue.totals();
        assert_eq!(totals.done_jobs, 1);
        assert_eq!(totals.running_jobs, 1);
        assert_eq!(totals.waiting_jobs, 1);
        assert_eq!(totals.done_bytes, 1250);
        assert_eq!(totals.total_bytes, 3000);
    }

    #[test]
    fn a_copy_between_two_servers_is_recognised_as_such() {
        let local = EndpointId::new("local");
        let between = job("x", "server-a", "server-b");
        assert!(between.is_between_two_endpoints(&local));
        assert!(!between.is_within_one_endpoint());

        let upload = job("y", "local", "server-a");
        assert!(!upload.is_between_two_endpoints(&local));
    }
}
