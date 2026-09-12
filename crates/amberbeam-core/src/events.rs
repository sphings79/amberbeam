//! What the core tells the window while it works.
//!
//! Listings are questions with answers; the server log is not. It arrives on
//! its own schedule and has to reach the window as it happens, which is why the
//! bridge grew a second half — an event stream, carried over Tauri's channel on
//! the desktop and over a WebSocket in the container build of M7.
//!
//! Events carry no finished sentences, for the same reason errors do not: the
//! language file writes them. Raw protocol lines are the exception, because a
//! server log that translated the server would be worthless.

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::endpoint::EndpointId;
use crate::error::Error;

/// Who said a line in the server log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LogDirection {
    /// AmberBeam asked.
    Sent,
    /// The server answered.
    Received,
    /// AmberBeam's own remark about what it is doing.
    Note,
}

/// Where a connection stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum ConnectionState {
    Connecting,
    Connected {
        /// What the server said about itself, for the log.
        banner: Option<String>,
    },
    Disconnected,
    Failed {
        error: Error,
    },
}

/// Everything the core pushes towards the window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "kebab-case")]
pub enum Event {
    /// One line for the server log.
    Log {
        endpoint: EndpointId,
        direction: LogDirection,
        /// Verbatim, untranslated.
        text: String,
    },
    /// A connection changed state.
    Connection {
        endpoint: EndpointId,
        #[serde(flatten)]
        state: ConnectionState,
    },
    /// A directory was read again, so the pane showing it should follow.
    Listed { endpoint: EndpointId, path: String },
    /// How far the running transfers have come.
    ///
    /// Sent on a timer rather than per chunk: a transfer moves a thousand
    /// chunks a second and the window redraws sixty times.
    Progress { jobs: Vec<JobProgress> },
    /// A server refused another channel and the connection is now asking for
    /// fewer. Worth saying out loud: a queue that suddenly runs three at a time
    /// instead of eight otherwise looks broken.
    ConcurrencyLowered { endpoint: EndpointId, allowed: u32 },
    /// The queue changed in a way the window cannot infer from progress alone:
    /// a job finished, failed, was added or needs an answer.
    Queue,
    /// How far a comparison has got.
    ///
    /// A recursive walk of two trees is many listings and no visible sign of
    /// life. Sent as it goes, so a window can show what it is doing rather
    /// than appearing to have stopped.
    Comparing { directories: usize, rows: usize },
    /// A watched directory changed and something went up because of it.
    Watched { id: String, sent: usize },
    /// Something happened to a file that is open for editing.
    ///
    /// Its own event because nobody is looking: the file is open in another
    /// program, and a write-back that silently failed there would be found out
    /// the next time somebody wondered why the site still looks the same.
    Edited {
        id: String,
        name: String,
        #[serde(flatten)]
        what: Edited,
    },
}

/// What happened to a file that is open for editing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "what", rename_all = "kebab-case")]
pub enum Edited {
    /// Saved somewhere else and sent up.
    Pushed,
    /// Saved, but the server's copy is no longer the one that was taken. The
    /// window has to ask; nothing was written.
    Changed { path: String },
    /// Saved, and the write-back failed for some other reason.
    Failed { error: Error },
}

/// One running transfer, as the queue reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobProgress {
    pub id: String,
    pub done_bytes: u64,
    pub total_bytes: Option<u64>,
    /// Bytes per second over the last stretch, once there is enough to say.
    pub rate: Option<u64>,
}

/// A listener on the event stream.
pub type Listener = broadcast::Receiver<Event>;

/// Why receiving failed. Re-exported so a shell can tell "fell behind" from
/// "the core is gone" without taking a dependency on the channel underneath.
pub use broadcast::error::RecvError;

/// How many events are held for a listener that is briefly behind.
///
/// A listing of fifty thousand entries produces a handful of log lines, not
/// thousands, so this is generous. A listener that still falls behind loses the
/// oldest lines and is told so by the channel — losing log lines is acceptable,
/// blocking the core to keep them is not.
const BACKLOG: usize = 512;

/// The core's end of the event stream. Cheap to clone, hands out listeners.
#[derive(Debug, Clone)]
pub struct Events {
    sender: broadcast::Sender<Event>,
}

impl Events {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(BACKLOG);
        Self { sender }
    }

    /// Sends an event. Succeeds even when nobody is listening — the core does
    /// not care whether a window is open.
    pub fn emit(&self, event: Event) {
        let _ = self.sender.send(event);
    }

    /// Convenience for the most frequent event by far.
    pub fn log(&self, endpoint: &EndpointId, direction: LogDirection, text: impl Into<String>) {
        self.emit(Event::Log {
            endpoint: endpoint.clone(),
            direction,
            text: text.into(),
        });
    }

    pub fn connection(&self, endpoint: &EndpointId, state: ConnectionState) {
        self.emit(Event::Connection {
            endpoint: endpoint.clone(),
            state,
        });
    }

    /// A new listener. It receives what is emitted from now on.
    pub fn subscribe(&self) -> Listener {
        self.sender.subscribe()
    }
}

impl Default for Events {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_listener_hears_what_is_emitted_after_it_arrived() {
        let events = Events::new();
        let mut listener = events.subscribe();
        let endpoint = EndpointId::new("server-a");

        events.log(&endpoint, LogDirection::Sent, "MLSD /var/www");
        let heard = listener.recv().await.expect("one event");

        assert_eq!(
            heard,
            Event::Log {
                endpoint,
                direction: LogDirection::Sent,
                text: "MLSD /var/www".into(),
            }
        );
    }

    #[tokio::test]
    async fn emitting_without_listeners_is_not_a_failure() {
        let events = Events::new();
        events.log(&EndpointId::new("a"), LogDirection::Note, "nobody is here");
    }

    #[test]
    fn a_connection_event_is_flat_enough_to_read_in_the_frontend() {
        let event = Event::Connection {
            endpoint: EndpointId::new("server-a"),
            state: ConnectionState::Connected {
                banner: Some("SSH-2.0-OpenSSH_9.6".into()),
            },
        };
        let text = serde_json::to_string(&event).expect("serialise");
        assert!(text.contains("\"event\":\"connection\""));
        assert!(text.contains("\"state\":\"connected\""));
        assert!(text.contains("OpenSSH_9.6"));
    }
}
