//! AmberBeam as something a program can drive, over stdio.
//!
//! Registered in a client roughly like this:
//!
//! ```json
//! {
//!   "mcpServers": {
//!     "amberbeam": { "command": "/path/to/amberbeam-mcp" }
//!   }
//! }
//! ```
//!
//! On a Mac the desktop program is the same thing with `--mcp` after it, and
//! that is the one to point at: the keychain grants access per program, so the
//! binary that stored a password is the one that can use it again without
//! asking.
//!
//! This binary exists for the two places that cannot do that — a container,
//! and Windows, where a windowed program has no standard input to read.

use std::sync::Arc;

use amberbeam_commands::Service;
use amberbeam_core::config::Config;
use amberbeam_core::editing::Edits;
use amberbeam_core::events::Events;
use amberbeam_core::registry::Sessions;
use amberbeam_core::runner::Runner;
use amberbeam_core::secrets::{FileStore, MemoryStore, SecretStore, SystemStore};
use amberbeam_core::watch::Watches;
use amberbeam_mcp::Journal;

#[tokio::main]
async fn main() {
    let config = match std::env::var("AMBERBEAM_CONFIG") {
        Ok(path) => Config::at(path),
        Err(_) => match Config::default_location() {
            Ok(config) => config,
            Err(why) => {
                eprintln!("there is nowhere to keep the configuration: {why}");
                std::process::exit(1);
            }
        },
    };

    // The same two stores the other shells choose between, chosen the same
    // way: a passphrase means a sealed file, and no passphrase means whatever
    // this system keeps secrets in.
    let secrets: Box<dyn SecretStore> = match passphrase() {
        Some(passphrase) => {
            match FileStore::open(config.root().join("secrets.sealed"), &passphrase) {
                Ok(store) => Box::new(store),
                Err(why) => {
                    eprintln!("the stored passwords could not be opened: {why}");
                    std::process::exit(1);
                }
            }
        }
        None => Box::new(SystemStore::default()),
    };

    let events = Events::new();
    let sessions = Arc::new(Sessions::new(events.clone()));
    let service = Arc::new(Service {
        sessions: Arc::clone(&sessions),
        queue: Runner::new(
            Arc::clone(&sessions),
            events.clone(),
            config.root().join("mcp-queue.json"),
        ),
        edits: Edits::beneath_temp(),
        events: events.clone(),
        watches: Watches::new(),
        secrets,
        session: MemoryStore::default(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        config,
    });

    let journal = Journal::beside(service.config.root());
    if let Err(why) = amberbeam_mcp::serve(service, journal).await {
        eprintln!("the connection ended badly: {why}");
        std::process::exit(1);
    }
}

/// The passphrase for the sealed store, from the environment or from a file.
///
/// The file wins, as it does for the service: somebody who mounted a secret
/// meant it.
fn passphrase() -> Option<String> {
    if let Ok(path) = std::env::var("AMBERBEAM_SECRET_PASSPHRASE_FILE") {
        if let Ok(text) = std::fs::read_to_string(path) {
            let text = text.trim().to_string();
            if !text.is_empty() {
                return Some(text);
            }
        }
    }
    std::env::var("AMBERBEAM_SECRET_PASSPHRASE")
        .ok()
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}
