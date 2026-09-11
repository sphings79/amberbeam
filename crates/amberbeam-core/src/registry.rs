//! Every open session, and the one door to them.
//!
//! This is what both shells talk to: the desktop through Tauri's channel, the
//! container build of M7 over HTTP and WebSocket. Neither knows anything about
//! sessions beyond what is offered here, which is the only reason the second
//! shell is a matter of a different transport rather than a second program.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::endpoint::{EndpointId, Protocol};
use crate::error::{Error, Result};
use crate::events::{Events, LogDirection};
use crate::fs::Listing;
use crate::local::LocalSession;
use crate::ops::Measurement;
use crate::session::Session;
use crate::sftp::{ConnectParams, SftpSession};

/// The identifier the local file system always has. The panes address it like
/// any other endpoint — in the container build it is the container's own disk,
/// not the user's.
pub const LOCAL: &str = "local";

/// What a freshly opened session reports back.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Connected {
    pub endpoint: EndpointId,
    pub protocol: Protocol,
    /// Directory the pane should open, canonical.
    pub home: String,
}

/// All open sessions.
///
/// Each session carries its own lock, and the map is only held long enough to
/// find one. Two panes on two servers therefore list at the same time, while
/// two panes on the *same* connection take turns — which they must, because a
/// session is a single channel and interleaved requests on it are nonsense.
#[derive(Debug)]
pub struct Sessions {
    open: Mutex<HashMap<EndpointId, Arc<Mutex<Session>>>>,
    events: Events,
}

impl Sessions {
    /// Starts with the local file system already open — there is nothing to
    /// connect to.
    pub fn new(events: Events) -> Self {
        let mut open = HashMap::new();
        open.insert(
            EndpointId::new(LOCAL),
            Arc::new(Mutex::new(Session::Local(LocalSession::new()))),
        );
        Self {
            open: Mutex::new(open),
            events,
        }
    }

    pub fn events(&self) -> &Events {
        &self.events
    }

    /// Opens an SFTP connection under `endpoint`, replacing whatever was there.
    pub async fn connect_sftp(
        &self,
        endpoint: &EndpointId,
        params: &ConnectParams,
    ) -> Result<Connected> {
        // Reconnecting a pane that already holds a session closes the old one
        // first, or the count of open connections only ever grows.
        self.disconnect(endpoint).await;

        let session = SftpSession::connect(params, endpoint, &self.events).await?;
        let home = session.home().await?;

        self.open.lock().await.insert(
            endpoint.clone(),
            Arc::new(Mutex::new(Session::Sftp(Box::new(session)))),
        );

        Ok(Connected {
            endpoint: endpoint.clone(),
            protocol: Protocol::Sftp,
            home,
        })
    }

    /// Reports the local session the way a connection would, so a pane can
    /// treat both the same.
    pub async fn local(&self) -> Result<Connected> {
        let endpoint = EndpointId::new(LOCAL);
        let session = self.find(&endpoint).await?;
        let home = session.lock().await.home().await?;
        Ok(Connected {
            endpoint,
            protocol: Protocol::Local,
            home,
        })
    }

    pub async fn list_dir(&self, endpoint: &EndpointId, path: &str) -> Result<Listing> {
        let session = self.find(endpoint).await?;
        let listing = session.lock().await.list_dir(path).await?;
        self.events.emit(crate::events::Event::Listed {
            endpoint: endpoint.clone(),
            path: listing.path.clone(),
        });
        Ok(listing)
    }

    pub async fn parent(&self, endpoint: &EndpointId, path: &str) -> Result<Option<String>> {
        let session = self.find(endpoint).await?;
        let parent = session.lock().await.parent(path);
        Ok(parent)
    }

    pub async fn join(&self, endpoint: &EndpointId, directory: &str, name: &str) -> Result<String> {
        let session = self.find(endpoint).await?;
        let joined = session.lock().await.join(directory, name);
        Ok(joined)
    }

    pub async fn create_dir(&self, endpoint: &EndpointId, path: &str) -> Result<()> {
        let session = self.find(endpoint).await?;
        let result = session.lock().await.create_dir(path).await;
        result
    }

    pub async fn create_file(&self, endpoint: &EndpointId, path: &str) -> Result<()> {
        let session = self.find(endpoint).await?;
        let result = session.lock().await.create_file(path).await;
        result
    }

    pub async fn rename(&self, endpoint: &EndpointId, from: &str, to: &str) -> Result<()> {
        let session = self.find(endpoint).await?;
        let result = session.lock().await.rename(from, to).await;
        result
    }

    pub async fn measure(&self, endpoint: &EndpointId, path: &str) -> Result<Measurement> {
        let session = self.find(endpoint).await?;
        let result = session.lock().await.measure(path).await;
        result
    }

    pub async fn remove(&self, endpoint: &EndpointId, path: &str) -> Result<()> {
        let session = self.find(endpoint).await?;
        let result = session.lock().await.remove(path).await;
        result
    }

    pub async fn set_permissions(
        &self,
        endpoint: &EndpointId,
        path: &str,
        mode: u32,
        recursive: bool,
    ) -> Result<()> {
        let session = self.find(endpoint).await?;
        let result = session
            .lock()
            .await
            .set_permissions(path, mode, recursive)
            .await;
        result
    }

    /// Closes a session. The local one stays: there is nothing to close, and a
    /// pane pointing at it must not end up pointing at nothing.
    pub async fn disconnect(&self, endpoint: &EndpointId) {
        if endpoint.as_str() == LOCAL {
            return;
        }
        let session = self.open.lock().await.remove(endpoint);
        if let Some(session) = session {
            session.lock().await.disconnect().await;
            self.events
                .log(endpoint, LogDirection::Note, "disconnected");
        }
    }

    pub async fn is_connected(&self, endpoint: &EndpointId) -> bool {
        self.open.lock().await.contains_key(endpoint)
    }

    /// Finds a session without holding the map while it is used.
    async fn find(&self, endpoint: &EndpointId) -> Result<Arc<Mutex<Session>>> {
        self.open
            .lock()
            .await
            .get(endpoint)
            .cloned()
            .ok_or(Error::NotConnected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_local_file_system_is_open_from_the_start() {
        let sessions = Sessions::new(Events::new());
        let local = sessions.local().await.expect("local session");
        assert_eq!(local.protocol, Protocol::Local);
        assert!(local.home.starts_with('/') || local.home.contains(':'));
        assert!(sessions.is_connected(&EndpointId::new(LOCAL)).await);
    }

    #[tokio::test]
    async fn a_pane_pointing_nowhere_is_told_so() {
        let sessions = Sessions::new(Events::new());
        let error = sessions
            .list_dir(&EndpointId::new("server-a"), "/")
            .await
            .expect_err("nothing is connected there");
        assert!(matches!(error, Error::NotConnected));
    }

    #[tokio::test]
    async fn the_local_session_cannot_be_closed_away() {
        let sessions = Sessions::new(Events::new());
        let local = EndpointId::new(LOCAL);
        sessions.disconnect(&local).await;
        assert!(sessions.is_connected(&local).await);
    }
}
