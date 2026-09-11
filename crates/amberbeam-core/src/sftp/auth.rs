//! How AmberBeam proves who it is.
//!
//! Everything a current server offers: password, a key file with or without a
//! passphrase, a key held by the agent, and keyboard-interactive for servers
//! that turned plain passwords off.
//!
//! PuTTY's `.ppk` works too, in both versions, and needed no work at all —
//! `russh` recognises the header and reads the file. This comment used to say
//! the opposite, that it needed its own parser and would arrive later. It was
//! wrong, and it was wrong for a year of commits because nobody tried. See
//! `tests/ppk.rs`, which tries, against files PuTTY itself wrote.

use std::sync::Arc;

use russh::client::{Handle, Handler, KeyboardInteractiveAuthResponse};
use russh::keys::agent::client::AgentClient;
use russh::keys::{load_secret_key, PrivateKeyWithHashAlg};

use crate::endpoint::EndpointId;
use crate::error::{Error, Result};
use crate::events::{Events, LogDirection};

/// What the user offers the server.
#[derive(Clone)]
pub enum AuthMethod {
    /// A password. Also used to answer keyboard-interactive prompts, which is
    /// what servers with `PasswordAuthentication no` fall back to.
    Password { password: String },
    /// A private key file. `passphrase` is `None` for an unencrypted key.
    KeyFile {
        path: String,
        passphrase: Option<String>,
    },
    /// Whatever the running ssh-agent holds — including keys in the Secure
    /// Enclave and on hardware tokens, which never leave the device.
    Agent,
}

impl std::fmt::Debug for AuthMethod {
    /// Hand written so a stray `{:?}` cannot put a password in a log file.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthMethod::Password { .. } => f.write_str("Password"),
            AuthMethod::KeyFile { path, passphrase } => f
                .debug_struct("KeyFile")
                .field("path", path)
                .field("encrypted", &passphrase.is_some())
                .finish(),
            AuthMethod::Agent => f.write_str("Agent"),
        }
    }
}

impl AuthMethod {
    /// The translation key naming this method in the window.
    pub const fn label_key(&self) -> &'static str {
        match self {
            AuthMethod::Password { .. } => "auth.password",
            AuthMethod::KeyFile { .. } => "auth.key-file",
            AuthMethod::Agent => "auth.agent",
        }
    }
}

/// Runs the chosen method against an open connection.
pub async fn authenticate<H: Handler>(
    handle: &mut Handle<H>,
    user: &str,
    method: &AuthMethod,
    events: &Events,
    endpoint: &EndpointId,
) -> Result<()> {
    match method {
        AuthMethod::Password { password } => {
            password_or_prompts(handle, user, password, events, endpoint).await
        }
        AuthMethod::KeyFile { path, passphrase } => {
            key_file(handle, user, path, passphrase.as_deref(), events, endpoint).await
        }
        AuthMethod::Agent => agent(handle, user, events, endpoint).await,
    }
}

async fn password_or_prompts<H: Handler>(
    handle: &mut Handle<H>,
    user: &str,
    password: &str,
    events: &Events,
    endpoint: &EndpointId,
) -> Result<()> {
    events.log(endpoint, LogDirection::Sent, "userauth password");
    let attempt = handle
        .authenticate_password(user, password)
        .await
        .map_err(Error::other)?;
    if attempt.success() {
        return Ok(());
    }

    // Servers with `PasswordAuthentication no` but `KbdInteractiveAuthentication
    // yes` refuse the line above and then ask the same question through
    // keyboard-interactive. Answering it with the same password is what every
    // other client does, and without it those servers look broken.
    events.log(
        endpoint,
        LogDirection::Note,
        "password refused, trying keyboard-interactive",
    );
    keyboard_interactive(handle, user, password, events, endpoint).await
}

async fn keyboard_interactive<H: Handler>(
    handle: &mut Handle<H>,
    user: &str,
    password: &str,
    events: &Events,
    endpoint: &EndpointId,
) -> Result<()> {
    let mut response = handle
        .authenticate_keyboard_interactive_start(user, None)
        .await
        .map_err(Error::other)?;

    loop {
        match response {
            KeyboardInteractiveAuthResponse::Success => return Ok(()),
            KeyboardInteractiveAuthResponse::Failure { .. } => {
                return Err(Error::AuthenticationFailed {
                    user: user.to_string(),
                })
            }
            KeyboardInteractiveAuthResponse::InfoRequest { prompts, .. } => {
                // Every prompt gets the password. A server asking two different
                // questions — a one time code on top of the password — needs the
                // window to ask the user, which is a dialog of its own and does
                // not exist yet.
                events.log(
                    endpoint,
                    LogDirection::Received,
                    format!("keyboard-interactive: {} prompt(s)", prompts.len()),
                );
                let answers = prompts.iter().map(|_| password.to_string()).collect();
                response = handle
                    .authenticate_keyboard_interactive_respond(answers)
                    .await
                    .map_err(Error::other)?;
            }
        }
    }
}

async fn key_file<H: Handler>(
    handle: &mut Handle<H>,
    user: &str,
    path: &str,
    passphrase: Option<&str>,
    events: &Events,
    endpoint: &EndpointId,
) -> Result<()> {
    let key = load_secret_key(path, passphrase).map_err(|source| {
        // Telling "wrong passphrase" apart from "not a key at all" is the
        // difference between the user typing again and the user picking another
        // file.
        let text = source.to_string().to_lowercase();
        if text.contains("passphrase") || text.contains("decrypt") || text.contains("crypt") {
            Error::KeyPassphrase {
                path: path.to_string(),
            }
        } else {
            Error::KeyFile {
                path: path.to_string(),
            }
        }
    })?;

    // RSA keys exist in three signature flavours and old servers know only the
    // oldest. Asking the server first avoids offering one it will reject.
    let hash_alg = handle
        .best_supported_rsa_hash()
        .await
        .ok()
        .flatten()
        .flatten();

    events.log(
        endpoint,
        LogDirection::Sent,
        format!("userauth publickey ({})", key.algorithm()),
    );
    let attempt = handle
        .authenticate_publickey(user, PrivateKeyWithHashAlg::new(Arc::new(key), hash_alg))
        .await
        .map_err(Error::other)?;

    if attempt.success() {
        Ok(())
    } else {
        Err(Error::AuthenticationFailed {
            user: user.to_string(),
        })
    }
}

/// Offers whatever the running agent holds.
///
/// How one reaches the agent differs by system, which is the only thing that
/// differs: a Unix socket named by `SSH_AUTH_SOCK`, or a named pipe on
/// Windows. Both end up handing the same keys to the same call.
async fn agent<H: Handler>(
    handle: &mut Handle<H>,
    user: &str,
    events: &Events,
    endpoint: &EndpointId,
) -> Result<()> {
    #[cfg(unix)]
    let mut agent = AgentClient::connect_env().await.map_err(|_| Error::Agent)?;

    #[cfg(windows)]
    let mut agent = {
        // OpenSSH for Windows listens on this pipe. Pageant, PuTTY's agent, is
        // a different mechanism and belongs with the rest of the PuTTY support
        // in M4.
        const OPENSSH_PIPE: &str = r"\\.\pipe\openssh-ssh-agent";
        AgentClient::connect_named_pipe(OPENSSH_PIPE)
            .await
            .map_err(|_| Error::Agent)?
    };

    let identities = agent.request_identities().await.map_err(|_| Error::Agent)?;
    if identities.is_empty() {
        return Err(Error::Agent);
    }

    let hash_alg = handle
        .best_supported_rsa_hash()
        .await
        .ok()
        .flatten()
        .flatten();

    // The agent may hold a dozen keys and the server accepts one of them. Which
    // one is not knowable in advance, so they are offered in turn — exactly what
    // the ssh command line does.
    for identity in identities {
        let russh::keys::agent::AgentIdentity::PublicKey { key, comment } = identity else {
            continue;
        };
        events.log(
            endpoint,
            LogDirection::Sent,
            format!("userauth publickey from agent ({comment})"),
        );
        let attempt = handle
            .authenticate_publickey_with(user, key, hash_alg, &mut agent)
            .await
            .map_err(Error::other)?;
        if attempt.success() {
            return Ok(());
        }
    }

    Err(Error::AuthenticationFailed {
        user: user.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_password_never_reaches_a_log_line() {
        let method = AuthMethod::Password {
            password: "hunter2".into(),
        };
        assert_eq!(format!("{method:?}"), "Password");
    }

    #[test]
    fn a_key_file_shows_its_path_but_not_its_passphrase() {
        let method = AuthMethod::KeyFile {
            path: "/home/dennis/.ssh/id_ed25519".into(),
            passphrase: Some("hunter2".into()),
        };
        let shown = format!("{method:?}");
        assert!(shown.contains("id_ed25519"));
        assert!(shown.contains("encrypted: true"));
        assert!(!shown.contains("hunter2"));
    }

    #[test]
    fn every_method_names_a_translation_key() {
        for method in [
            AuthMethod::Password {
                password: String::new(),
            },
            AuthMethod::KeyFile {
                path: String::new(),
                passphrase: None,
            },
            AuthMethod::Agent,
        ] {
            assert!(method.label_key().starts_with("auth."));
        }
    }
}
