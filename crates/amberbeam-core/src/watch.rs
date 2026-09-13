//! Watching a directory on this machine and sending up what changes.
//!
//! One direction only, and deliberately so. The local side is watched by the
//! operating system, which tells us the moment a file is written; the far side
//! cannot be watched at all, because neither FTP nor SFTP has any way to say
//! "something changed" — the only way to notice would be to list the tree over
//! and over, which is a standing load on somebody else's server rather than a
//! background service.
//!
//! What arrives from the system is not one event per save. An editor writing a
//! file produces a create, a write and a rename, and a copy of a folder
//! produces thousands in a burst. So events are collected and acted on once
//! the noise stops.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode, Watcher as _};
use serde::Serialize;
use tokio::sync::{mpsc, Mutex};

use crate::compare::excluded;
use crate::endpoint::EndpointId;
use crate::error::{Error, Result};
use crate::events::{Event, Events};
use crate::runner::{EnqueueRequest, Runner};
use crate::transfer::ConflictPolicy;

/// How long the noise has to stop before anything is sent.
///
/// An editor saving a file writes it, renames it and touches it again inside a
/// few milliseconds. Acting on the first of those uploads half a file; waiting
/// a moment and acting once uploads the file.
const SETTLE: Duration = Duration::from_millis(400);

/// And the longest a steady stream of changes may hold everything back.
///
/// Something writing continuously — a build, a log — would otherwise keep
/// resetting the wait and never send anything at all.
const AT_LATEST: Duration = Duration::from_secs(3);

/// How a watch is to remove something on the far side.
///
/// Handed in rather than done here, and that is the point: what "delete" means
/// depends on whether the server entry names a wastebasket, and this crate
/// knows nothing about server entries. A watch that deleted for itself would
/// be a second answer to a question that already has one — and the wrong one
/// exactly where it matters most, because nobody is looking while a watch
/// runs.
pub type Remover =
    Arc<dyn Fn(String, String) -> Pin<Box<dyn Future<Output = bool> + Send>> + Send + Sync>;

/// One directory being watched.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Watch {
    pub id: String,
    /// The endpoint the watched directory is on. Always the local one: it is
    /// the only side anything can watch.
    pub endpoint: String,
    pub root: String,
    pub target_endpoint: String,
    pub target_root: String,
    /// Where the files are going, as the window shows it.
    pub target_title: Option<String>,
    /// How many files have been sent since this started.
    pub sent: usize,
    pub started: i64,
}

/// The directories being watched, and the machinery keeping them watched.
pub struct Watches {
    open: Mutex<HashMap<String, Held>>,
}

struct Held {
    watch: Watch,
    /// Counted by the task rather than here, because the task is the only
    /// thing that knows when something went up — and it cannot reach back into
    /// this register without holding what holds it.
    sent: Arc<AtomicUsize>,
    /// Dropping this stops the operating system telling us anything. It is
    /// kept for no other reason.
    _watcher: RecommendedWatcher,
    stop: mpsc::UnboundedSender<()>,
}

impl std::fmt::Debug for Watches {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Watches")
    }
}

impl Default for Watches {
    fn default() -> Self {
        Self::new()
    }
}

impl Watches {
    pub fn new() -> Self {
        Self {
            open: Mutex::new(HashMap::new()),
        }
    }

    pub async fn list(&self) -> Vec<Watch> {
        let mut found: Vec<Watch> = self
            .open
            .lock()
            .await
            .values()
            .map(|held| Watch {
                sent: held.sent.load(Ordering::Relaxed),
                ..held.watch.clone()
            })
            .collect();
        found.sort_by_key(|watch| watch.started);
        found
    }

    /// Starts watching a directory on this machine.
    ///
    /// Refuses a second watch of the same directory: two watchers on one tree
    /// would send every change twice, and the second upload would arrive while
    /// the first was still going.
    #[allow(clippy::too_many_arguments)]
    pub async fn start(
        &self,
        queue: &Arc<Runner>,
        remove: Remover,
        events: Events,
        root: String,
        target_endpoint: String,
        target_root: String,
        target_title: Option<String>,
        excludes: Vec<String>,
        // Whether a file that goes here goes there too. Read once, when the
        // watch starts: a switch changing under a running watch would leave
        // nobody able to say what it had done.
        delete_along: bool,
    ) -> Result<Watch> {
        if let Some(already) = self
            .open
            .lock()
            .await
            .values()
            .find(|h| h.watch.root == root)
        {
            return Ok(already.watch.clone());
        }
        let folder = PathBuf::from(&root);
        if !folder.is_dir() {
            return Err(Error::Path {
                path: root,
                reason: crate::error::PathProblem::NotADirectory,
            });
        }

        let (changes, mut incoming) = mpsc::unbounded_channel::<PathBuf>();
        let mut watcher =
            notify::recommended_watcher(move |answer: notify::Result<notify::Event>| {
                let Ok(event) = answer else {
                    return;
                };
                // Only what left something behind. A read, a permission change or
                // a file being looked at is not a reason to upload anything.
                let worth_it = matches!(
                    event.kind,
                    notify::EventKind::Create(_) | notify::EventKind::Modify(_)
                ) || (delete_along
                    && matches!(event.kind, notify::EventKind::Remove(_)));
                if !worth_it {
                    return;
                }
                for path in event.paths {
                    // The receiver going away is the watch being stopped, which is
                    // not a failure worth reporting from inside the operating
                    // system's own thread.
                    let _ = changes.send(path);
                }
            })
            .map_err(|why| Error::other(format!("this directory cannot be watched: {why}")))?;
        watcher
            .watch(&folder, RecursiveMode::Recursive)
            .map_err(|why| Error::other(format!("this directory cannot be watched: {why}")))?;

        let id = new_id();
        let watch = Watch {
            id: id.clone(),
            endpoint: crate::registry::LOCAL.to_string(),
            root: root.clone(),
            target_endpoint: target_endpoint.clone(),
            target_root: target_root.clone(),
            target_title,
            sent: 0,
            started: now_seconds(),
        };

        let (stop, mut stopped) = mpsc::unbounded_channel::<()>();
        let counted = Arc::new(AtomicUsize::new(0));
        let counting = Arc::clone(&counted);
        let queue = Arc::clone(queue);
        let mine = id.clone();
        let root_for_task = root.clone();
        tokio::spawn(async move {
            let mut waiting: HashSet<PathBuf> = HashSet::new();
            let mut oldest: Option<std::time::Instant> = None;

            loop {
                let wait = if waiting.is_empty() {
                    AT_LATEST
                } else {
                    SETTLE
                };
                tokio::select! {
                    _ = stopped.recv() => return,
                    path = incoming.recv() => match path {
                        Some(path) => {
                            waiting.insert(path);
                            oldest.get_or_insert_with(std::time::Instant::now);
                            // Held back until it goes quiet, unless it has
                            // been noisy for long enough that quiet is not
                            // coming.
                            if oldest.is_some_and(|since| since.elapsed() < AT_LATEST) {
                                continue;
                            }
                        }
                        // The watcher is gone, and so is any reason to sit here.
                        None => return,
                    },
                    _ = tokio::time::sleep(wait) => {}
                }

                if waiting.is_empty() {
                    continue;
                }
                let batch: Vec<PathBuf> = waiting.drain().collect();
                oldest = None;
                let (sent, refused) = send_up(
                    &queue,
                    &remove,
                    &root_for_task,
                    &target_endpoint,
                    &target_root,
                    &excludes,
                    delete_along,
                    batch,
                )
                .await;
                if sent > 0 || refused > 0 {
                    counting.fetch_add(sent, Ordering::Relaxed);
                    events.emit(Event::Watched {
                        id: mine.clone(),
                        sent,
                        refused,
                    });
                }
            }
        });

        let watch_out = watch.clone();
        self.open.lock().await.insert(
            id,
            Held {
                watch,
                sent: counted,
                _watcher: watcher,
                stop,
            },
        );
        Ok(watch_out)
    }

    /// Stops one watch. Nothing already in the queue is taken back: it was
    /// asked for before the watch ended.
    pub async fn stop(&self, id: &str) -> Option<Watch> {
        let held = self.open.lock().await.remove(id)?;
        let _ = held.stop.send(());
        Some(Watch {
            sent: held.sent.load(Ordering::Relaxed),
            ..held.watch
        })
    }

    pub async fn stop_all(&self) -> Vec<Watch> {
        let all: Vec<Held> = self
            .open
            .lock()
            .await
            .drain()
            .map(|(_, held)| held)
            .collect();
        all.iter().for_each(|held| {
            let _ = held.stop.send(());
        });
        all.into_iter()
            .map(|held| Watch {
                sent: held.sent.load(Ordering::Relaxed),
                ..held.watch
            })
            .collect()
    }
}

/// Puts what changed into the queue, and says how many that was.
#[allow(clippy::too_many_arguments)]
async fn send_up(
    queue: &Arc<Runner>,
    remove: &Remover,
    root: &str,
    target_endpoint: &str,
    target_root: &str,
    excludes: &[String],
    delete_along: bool,
    batch: Vec<PathBuf>,
) -> (usize, usize) {
    let mut sent = 0;
    let mut refused = 0;
    for path in batch {
        let Some(relative) = below(root, &path) else {
            continue;
        };
        let gone = !path.exists();
        // A directory has nothing to send: the files inside it arrive as their
        // own events, and each of those makes the directory on the way. A
        // directory that has *gone* is a different matter.
        if !gone && !path.is_file() {
            continue;
        }
        // Every part of the way down, not only the file: a change inside
        // `node_modules` is excluded by the rule that names the directory.
        if relative.split('/').any(|part| excluded(part, excludes)) {
            continue;
        }

        let (Some(directory), Some(name)) = (path.parent(), path.file_name()) else {
            continue;
        };
        let into = match relative.rsplit_once('/') {
            Some((below, _)) => format!("{}/{below}", target_root.trim_end_matches('/')),
            None => target_root.to_string(),
        };

        if gone {
            // Only when this watch was started with deletions carried across,
            // and never through the queue even then: the queue moves a file
            // from one place to another, and taking one away is not that.
            if !delete_along {
                continue;
            }
            let there = format!("{}/{}", into.trim_end_matches('/'), name.to_string_lossy());
            if remove(target_endpoint.to_string(), there).await {
                sent += 1;
            } else {
                // Plenty of FTP servers refuse to rename at all, and a
                // wastebasket is a rename. Said rather than swallowed: the
                // file is still up there, and somebody who asked for
                // deletions to be carried across should not have to find that
                // out from the server.
                refused += 1;
            }
            continue;
        }

        let request = EnqueueRequest {
            source_endpoint: EndpointId::new(crate::registry::LOCAL),
            source_directory: directory.to_string_lossy().into_owned(),
            names: vec![name.to_string_lossy().into_owned()],
            target_endpoint: EndpointId::new(target_endpoint.to_string()),
            target_directory: into,
            // Nobody is looking. A question here would stop the queue and wait
            // for an answer that is not coming, and the local file is the one
            // this watch exists to carry across.
            conflict_policy: ConflictPolicy::Overwrite,
            keep_modified: true,
            keep_permissions: false,
            use_temporary_name: true,
            retries: None,
            held: false,
        };
        if queue.enqueue(&request).await.is_ok() {
            sent += 1;
        } else {
            refused += 1;
        }
    }
    (sent, refused)
}

/// Where a path sits below the watched directory, with forward slashes.
fn below(root: &str, path: &Path) -> Option<String> {
    let rest = path.strip_prefix(Path::new(root)).ok()?;
    let text = rest.to_string_lossy().replace('\\', "/");
    (!text.is_empty()).then_some(text)
}

fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs() as i64)
        .unwrap_or_default()
}

fn new_id() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_millis())
        .unwrap_or_default();
    format!("{now:x}-{:x}", COUNTER.fetch_add(1, Ordering::Relaxed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_is_placed_below_the_directory_being_watched() {
        let root = "/Users/someone/site";
        assert_eq!(
            below(root, Path::new("/Users/someone/site/index.php")).as_deref(),
            Some("index.php")
        );
        assert_eq!(
            below(root, Path::new("/Users/someone/site/bilder/foto.jpg")).as_deref(),
            Some("bilder/foto.jpg")
        );
        // The directory itself is not below itself, and something outside it
        // is not below it either.
        assert_eq!(below(root, Path::new("/Users/someone/site")), None);
        assert_eq!(below(root, Path::new("/Users/someone/other/x")), None);
    }

    #[test]
    fn an_excluded_directory_excludes_what_is_inside_it() {
        let patterns = vec!["node_modules".to_string(), "*.log".to_string()];
        let hidden = |relative: &str| relative.split('/').any(|part| excluded(part, &patterns));

        assert!(hidden("node_modules/left-pad/index.js"));
        assert!(hidden("logs/error.log"));
        assert!(!hidden("src/index.php"));
        assert!(!hidden("node_modules.txt"));
    }
}
