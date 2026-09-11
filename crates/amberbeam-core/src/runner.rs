//! Working through the queue.
//!
//! The queue itself is a list of values; this is what makes it move. Kept in
//! the core rather than in a shell, so the desktop window and the container
//! service of M7 drive the same machinery instead of each growing its own.
//!
//! What it has to get right:
//!
//! * Never start more at one endpoint than that endpoint allows, and count
//!   both ends of a transfer.
//! * A failure is retried a few times with growing pauses, and then the job is
//!   **paused, not discarded** — the offset is worth more than the attempt.
//! * A server that refuses another channel is not a fault: ask for fewer.
//! * Every change is written to disk, because the whole point of the queue is
//!   that it is still there in the morning.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::endpoint::EndpointId;
use crate::engine::Progress;
use crate::error::Error;
use crate::events::{Event, Events, JobProgress, LogDirection};
use crate::queue::{self, Queue, QueuedJob, Totals};
use crate::registry::{Sessions, TransferRun};
use crate::transfer::{ConflictPolicy, JobState, ResumeMarker};

/// How often the window is told how far things have come.
const PROGRESS_EVERY: std::time::Duration = std::time::Duration::from_millis(300);

/// The pauses between attempts: one second, then two, four, eight, sixteen.
fn backoff(attempt: u8) -> std::time::Duration {
    std::time::Duration::from_secs(1_u64 << attempt.min(4))
}

struct Live {
    progress: Arc<Progress>,
    /// Where the job stood when this run started, for working out a rate.
    started_at: std::time::Instant,
    started_from: u64,
}

/// Drives the queue.
pub struct Runner {
    sessions: Arc<Sessions>,
    events: Events,
    queue: Mutex<Queue>,
    running: Mutex<HashMap<String, Live>>,
    path: PathBuf,
    /// Rung whenever something changed that might let a job start.
    ///
    /// A single scheduling loop waits on this rather than each change starting
    /// jobs itself: a job finishing would then call the scheduler, which starts
    /// a job, which finishes and calls the scheduler — a recursion with no end
    /// that the compiler cannot even describe.
    wake: tokio::sync::Notify,
}

impl Runner {
    pub fn new(sessions: Arc<Sessions>, events: Events, path: PathBuf) -> Arc<Self> {
        let queue = queue::load(&path);
        Arc::new(Self {
            sessions,
            events,
            queue: Mutex::new(queue),
            running: Mutex::new(HashMap::new()),
            path,
            wake: tokio::sync::Notify::new(),
        })
    }

    /// Starts the two background loops.
    ///
    /// Apart from `new` on purpose: spawning needs a runtime, and a shell
    /// builds its runtime after it builds its state. Calling this from outside
    /// one panics with a message about a missing reactor that says nothing
    /// about the real mistake.
    pub fn start(self: &Arc<Self>) {
        Arc::clone(self).start_reporting();
        Arc::clone(self).start_scheduling();
        // Whatever was left over from the last run is waiting to be picked up.
        self.wake.notify_one();
    }

    /// A copy of the queue, for the window to draw.
    pub async fn snapshot(&self) -> Queue {
        self.queue.lock().await.clone()
    }

    pub async fn totals(&self) -> Totals {
        self.queue.lock().await.totals()
    }

    pub async fn add(self: &Arc<Self>, job: QueuedJob) {
        self.queue.lock().await.add(job);
        self.after_change().await;
    }

    /// Puts what was selected into the queue, folders and all.
    ///
    /// Directories are resolved now rather than on the way, so the window can
    /// say how many files are coming and the structure is worked out once.
    pub async fn enqueue(
        self: &Arc<Self>,
        request: &EnqueueRequest,
    ) -> crate::error::Result<usize> {
        let mut jobs = Vec::new();
        let added = now_seconds();

        for name in &request.names {
            let found = self
                .sessions
                .expand(
                    &request.source_endpoint,
                    &request.source_directory,
                    name,
                    &request.target_directory,
                )
                .await?;

            for item in found {
                let target_path = join_relative(
                    &self.sessions,
                    &request.target_endpoint,
                    &request.target_directory,
                    &item.relative,
                )
                .await?;

                // A folder that holds nothing still has to appear at the other
                // end, and there is nothing to transfer for it.
                if item.size.is_none() {
                    let _ = self
                        .sessions
                        .ensure_dir(&request.target_endpoint, &target_path)
                        .await;
                    continue;
                }

                jobs.push(QueuedJob {
                    id: format!("{}-{}", added, jobs.len()),
                    source_endpoint: request.source_endpoint.clone(),
                    source_path: item.source_path,
                    target_endpoint: request.target_endpoint.clone(),
                    target_path,
                    name: item.relative.clone(),
                    state: JobState::Queued,
                    conflict_policy: request.conflict_policy,
                    total_bytes: item.size,
                    done_bytes: 0,
                    resume: None,
                    keep_modified: request.keep_modified,
                    keep_permissions: request.keep_permissions,
                    use_temporary_name: request.use_temporary_name,
                    attempts: 0,
                    retries: request.retries,
                    failure: None,
                    added,
                });
            }
        }

        let count = jobs.len();
        self.add_all(jobs).await;
        Ok(count)
    }

    pub async fn add_all(self: &Arc<Self>, jobs: Vec<QueuedJob>) {
        {
            let mut queue = self.queue.lock().await;
            for job in jobs {
                queue.add(job);
            }
        }
        self.after_change().await;
    }

    /// Stops a running job and takes it out of the list.
    pub async fn remove(self: &Arc<Self>, id: &str) {
        if let Some(live) = self.running.lock().await.get(id) {
            live.progress.cancel();
        }
        self.queue.lock().await.remove(id);
        self.after_change().await;
    }

    pub async fn clear_finished(self: &Arc<Self>) {
        self.queue.lock().await.clear_finished();
        self.after_change().await;
    }

    pub async fn set_paused(self: &Arc<Self>, paused: bool) {
        self.queue.lock().await.paused = paused;
        self.after_change().await;
    }

    /// Holds one job. A running one is stopped where it is and keeps its
    /// offset, so continuing costs nothing.
    pub async fn hold(self: &Arc<Self>, id: &str) {
        if let Some(live) = self.running.lock().await.get(id) {
            live.progress.cancel();
        }
        if let Some(job) = self.queue.lock().await.get_mut(id) {
            job.state = JobState::Paused;
        }
        self.after_change().await;
    }

    /// Puts a held or failed job back in line.
    pub async fn resume(self: &Arc<Self>, id: &str) {
        if let Some(job) = self.queue.lock().await.get_mut(id) {
            job.state = JobState::Queued;
            job.attempts = 0;
            job.failure = None;
        }
        self.after_change().await;
    }

    pub async fn move_by(self: &Arc<Self>, id: &str, delta: isize) {
        self.queue.lock().await.move_by(id, delta);
        self.after_change().await;
    }

    pub async fn move_to(self: &Arc<Self>, id: &str, index: usize) {
        self.queue.lock().await.move_to(id, index);
        self.after_change().await;
    }

    /// Answers a job that was waiting on a conflict.
    pub async fn decide(self: &Arc<Self>, id: &str, policy: ConflictPolicy, for_all: bool) {
        {
            let mut queue = self.queue.lock().await;
            if for_all {
                // "For all remaining" means exactly that: everything still
                // waiting, not everything in the list.
                for job in queue.jobs.iter_mut() {
                    if job.state == JobState::Asking || job.state == JobState::Queued {
                        job.conflict_policy = policy;
                    }
                }
            }
            if let Some(job) = queue.get_mut(id) {
                job.conflict_policy = policy;
                job.state = JobState::Queued;
            }
        }
        self.after_change().await;
    }

    async fn after_change(self: &Arc<Self>) {
        self.persist().await;
        self.events.emit(Event::Queue);
        self.wake.notify_one();
    }

    /// One loop, waiting to be told that something might now fit.
    fn start_scheduling(self: Arc<Self>) {
        tokio::spawn(async move {
            loop {
                self.wake.notified().await;
                Arc::clone(&self).start_what_fits().await;
            }
        });
    }

    async fn persist(&self) {
        let queue = self.queue.lock().await.clone();
        // A queue that cannot be written is not worth stopping the transfer
        // for; the window shows the truth either way.
        let _ = queue::save(&self.path, &queue);
    }

    /// Starts everything the limits allow, in the order the user set.
    async fn start_what_fits(self: Arc<Self>) {
        loop {
            let limits = self.limits().await;
            let busy = self.busy().await;

            let next = {
                let queue = self.queue.lock().await;
                queue.next_startable(&limits, &busy).cloned()
            };
            let Some(job) = next else { break };

            {
                let mut queue = self.queue.lock().await;
                let Some(entry) = queue.get_mut(&job.id) else {
                    break;
                };
                entry.state = JobState::Running;
            }

            let progress = Progress::new(job.total_bytes, job.done_bytes);
            self.running.lock().await.insert(
                job.id.clone(),
                Live {
                    progress: Arc::clone(&progress),
                    started_at: std::time::Instant::now(),
                    started_from: job.done_bytes,
                },
            );

            let runner = Arc::clone(&self);
            tokio::spawn(async move {
                runner.run_one(job, progress).await;
            });
        }
    }

    /// What each endpoint currently allows.
    async fn limits(&self) -> HashMap<EndpointId, u32> {
        let mut limits = HashMap::new();
        for endpoint in self.sessions.open_endpoints().await {
            if let Ok(session) = self.sessions.find(&endpoint).await {
                limits.insert(endpoint, session.concurrency().await);
            }
        }
        limits
    }

    /// How many channels each endpoint is using right now.
    async fn busy(&self) -> HashMap<EndpointId, u32> {
        let running = self.running.lock().await;
        let queue = self.queue.lock().await;
        let mut busy: HashMap<EndpointId, u32> = HashMap::new();
        for id in running.keys() {
            if let Some(job) = queue.get(id) {
                *busy.entry(job.source_endpoint.clone()).or_default() += 1;
                *busy.entry(job.target_endpoint.clone()).or_default() += 1;
            }
        }
        busy
    }

    /// Works out what to do about a file that is already at the target.
    ///
    /// Returns the job as it should now run, or `None` when nothing should
    /// happen — because the user has to answer first, or because the answer was
    /// to leave it alone.
    async fn resolve_conflict(&self, job: &QueuedJob) -> ConflictOutcome {
        let Ok(target) = self.sessions.find(&job.target_endpoint).await else {
            return ConflictOutcome::Run(Box::new(job.clone()));
        };
        let Ok((existing_size, existing_modified)) = target.stat(&job.target_path).await else {
            // Nothing there, nothing to decide.
            return ConflictOutcome::Run(Box::new(job.clone()));
        };

        match job.conflict_policy {
            ConflictPolicy::Ask => ConflictOutcome::Ask,
            ConflictPolicy::Overwrite => ConflictOutcome::Run(Box::new(job.clone())),
            ConflictPolicy::Skip => ConflictOutcome::Skip,
            ConflictPolicy::OverwriteIfNewer => {
                let source_modified = match self.sessions.find(&job.source_endpoint).await {
                    Ok(session) => session
                        .stat(&job.source_path)
                        .await
                        .ok()
                        .and_then(|(_, when)| when),
                    Err(_) => None,
                };
                match (source_modified, existing_modified) {
                    // Without both times there is no "newer", and guessing
                    // would overwrite something for no reason.
                    (Some(source), Some(target)) if source > target => {
                        ConflictOutcome::Run(Box::new(job.clone()))
                    }
                    _ => ConflictOutcome::Skip,
                }
            }
            ConflictPolicy::Rename => {
                let free = self.free_name(job).await;
                ConflictOutcome::Run(Box::new(QueuedJob {
                    target_path: free,
                    ..job.clone()
                }))
            }
            ConflictPolicy::Resume => {
                // The user asked for this one, so it happens — but the source
                // was never shown to be unchanged, and the log says so rather
                // than leaving it to be discovered in a corrupt file.
                self.events.log(
                    &job.target_endpoint,
                    LogDirection::Note,
                    format!(
                        "{}: continuing from {existing_size} bytes on request, source not verified",
                        job.name
                    ),
                );
                let source = self.sessions.find(&job.source_endpoint).await.ok();
                let (size, modified) = match &source {
                    Some(session) => session
                        .stat(&job.source_path)
                        .await
                        .unwrap_or((existing_size, None)),
                    None => (existing_size, None),
                };
                ConflictOutcome::Run(Box::new(QueuedJob {
                    resume: Some(ResumeMarker {
                        offset: existing_size,
                        source_size: size,
                        source_modified: modified,
                    }),
                    done_bytes: existing_size,
                    ..job.clone()
                }))
            }
        }
    }

    /// A target name nothing is using yet: `name-1`, `name-2`, and so on.
    async fn free_name(&self, job: &QueuedJob) -> String {
        let Ok(target) = self.sessions.find(&job.target_endpoint).await else {
            return job.target_path.clone();
        };
        let (stem, extension) = match job.target_path.rsplit_once('.') {
            // A dot in the directory rather than the name is not an extension.
            Some((stem, extension)) if !extension.contains('/') && !stem.ends_with('/') => {
                (stem.to_string(), format!(".{extension}"))
            }
            _ => (job.target_path.clone(), String::new()),
        };
        for index in 1..1000 {
            let candidate = format!("{stem}-{index}{extension}");
            if target.stat(&candidate).await.is_err() {
                return candidate;
            }
        }
        job.target_path.clone()
    }

    async fn run_one(self: Arc<Self>, job: QueuedJob, progress: Arc<Progress>) {
        // The directory has to be there before the file is. Doing it here
        // rather than at enqueue time keeps a queue that waited overnight
        // working even if somebody tidied up in the meantime.
        if let Ok(session) = self.sessions.find(&job.target_endpoint).await {
            if let Some(parent) = session.parent(&job.target_path) {
                let _ = self
                    .sessions
                    .ensure_dir(&job.target_endpoint, &parent)
                    .await;
            }
        }

        let job = match self.resolve_conflict(&job).await {
            ConflictOutcome::Run(job) => *job,
            ConflictOutcome::Ask => {
                self.running.lock().await.remove(&job.id);
                if let Some(entry) = self.queue.lock().await.get_mut(&job.id) {
                    entry.state = JobState::Asking;
                }
                self.after_change().await;
                return;
            }
            ConflictOutcome::Skip => {
                self.running.lock().await.remove(&job.id);
                if let Some(entry) = self.queue.lock().await.get_mut(&job.id) {
                    entry.state = JobState::Skipped;
                }
                self.after_change().await;
                return;
            }
        };

        let run = TransferRun {
            source_endpoint: job.source_endpoint.clone(),
            source_path: job.source_path.clone(),
            target_endpoint: job.target_endpoint.clone(),
            target_path: job.target_path.clone(),
            resume: job.resume.clone(),
            keep_modified: job.keep_modified,
            keep_permissions: job.keep_permissions,
            source_permissions: None,
            use_temporary_name: job.use_temporary_name,
        };

        let outcome = self.sessions.transfer(&run, &progress).await;
        self.running.lock().await.remove(&job.id);

        match outcome {
            Ok(done) if done.complete => {
                let mut queue = self.queue.lock().await;
                if let Some(entry) = queue.get_mut(&job.id) {
                    entry.state = JobState::Done;
                    entry.done_bytes = progress.done();
                    entry.total_bytes = Some(progress.done());
                    entry.resume = None;
                    entry.failure = None;
                }
            }
            Ok(stopped) => {
                // Cancelled, which means held or removed. The offset is kept so
                // continuing costs nothing.
                let mut queue = self.queue.lock().await;
                if let Some(entry) = queue.get_mut(&job.id) {
                    entry.done_bytes = progress.done();
                    entry.resume = stopped.resume;
                    if entry.state == JobState::Running {
                        entry.state = JobState::Paused;
                    }
                }
            }
            Err(error) => self.failed(&job, &progress, error).await,
        }

        self.after_change().await;
    }

    async fn failed(&self, job: &QueuedJob, progress: &Progress, error: Error) {
        let retries = job_retries(job);
        let mut queue = self.queue.lock().await;
        let Some(entry) = queue.get_mut(&job.id) else {
            return;
        };

        entry.done_bytes = progress.done();
        entry.resume = Some(ResumeMarker {
            offset: progress.done(),
            source_size: entry.total_bytes.unwrap_or(progress.done()),
            source_modified: entry
                .resume
                .as_ref()
                .and_then(|marker| marker.source_modified),
        });
        entry.failure = Some(error.clone());
        entry.attempts = entry.attempts.saturating_add(1);

        // A source that moved on is not something a retry can fix; the user has
        // to say whether to start over or append anyway.
        let hopeless = matches!(
            error,
            Error::SourceChanged
                | Error::AuthenticationFailed { .. }
                | Error::HostKeyChanged { .. }
        );

        if hopeless || entry.attempts >= retries {
            entry.state = JobState::Paused;
            self.events.log(
                &entry.target_endpoint,
                LogDirection::Note,
                format!("{}: paused after {} attempts", entry.name, entry.attempts),
            );
        } else {
            entry.state = JobState::Queued;
            let wait = backoff(entry.attempts);
            self.events.log(
                &entry.target_endpoint,
                LogDirection::Note,
                format!(
                    "{}: attempt {} failed, trying again in {}s",
                    entry.name,
                    entry.attempts,
                    wait.as_secs()
                ),
            );
            drop(queue);
            tokio::time::sleep(wait).await;
        }
    }

    /// Reports progress on a timer for as long as anything is moving.
    fn start_reporting(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(PROGRESS_EVERY);
            loop {
                ticker.tick().await;
                let running = self.running.lock().await;
                if running.is_empty() {
                    continue;
                }
                let jobs = running
                    .iter()
                    .map(|(id, live)| {
                        let done = live.progress.done();
                        let seconds = live.started_at.elapsed().as_secs_f64();
                        // A rate needs a moment of history; before that it says
                        // nothing rather than something wrong.
                        let rate = (seconds > 0.5)
                            .then(|| {
                                ((done.saturating_sub(live.started_from)) as f64 / seconds) as u64
                            })
                            .filter(|rate| *rate > 0);
                        JobProgress {
                            id: id.clone(),
                            done_bytes: done,
                            total_bytes: (live.progress.total() > 0).then(|| live.progress.total()),
                            rate,
                        }
                    })
                    .collect();
                drop(running);
                self.events.emit(Event::Progress { jobs });
            }
        });
    }
}

/// What to do about a file that is already at the target.
enum ConflictOutcome {
    /// Boxed because a job is far larger than the other two answers, and the
    /// enum would otherwise carry that weight on every decision.
    Run(Box<QueuedJob>),
    Ask,
    Skip,
}

/// What the window asks for when something is dragged or the button is used.
#[derive(Debug, Clone)]
pub struct EnqueueRequest {
    pub source_endpoint: EndpointId,
    pub source_directory: String,
    pub names: Vec<String>,
    pub target_endpoint: EndpointId,
    pub target_directory: String,
    pub conflict_policy: ConflictPolicy,
    pub keep_modified: bool,
    pub keep_permissions: bool,
    pub use_temporary_name: bool,
    pub retries: Option<u8>,
}

fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs() as i64)
        .unwrap_or_default()
}

/// Joins a relative path onto a directory, in the target's own notation.
async fn join_relative(
    sessions: &Sessions,
    endpoint: &EndpointId,
    directory: &str,
    relative: &str,
) -> crate::error::Result<String> {
    let mut path = directory.to_string();
    for part in relative.split('/').filter(|part| !part.is_empty()) {
        path = sessions.join(endpoint, &path, part).await?;
    }
    Ok(path)
}

/// How many attempts this job gets. Carried on the job so a queue written by an
/// older version still knows.
fn job_retries(job: &QueuedJob) -> u8 {
    job.retries.unwrap_or(5).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pauses_between_attempts_grow_and_then_stop_growing() {
        assert_eq!(backoff(0).as_secs(), 1);
        assert_eq!(backoff(1).as_secs(), 2);
        assert_eq!(backoff(2).as_secs(), 4);
        assert_eq!(backoff(3).as_secs(), 8);
        assert_eq!(backoff(4).as_secs(), 16);
        // Beyond that it stays put: a job is paused long before an hour.
        assert_eq!(backoff(9).as_secs(), 16);
    }
}
