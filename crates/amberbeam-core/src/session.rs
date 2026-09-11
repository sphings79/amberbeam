//! One open place, whatever protocol it speaks.
//!
//! An enumeration rather than a trait object: the set of protocols is closed
//! and short — local, SFTP, and FTP with FTPS in M3 — so dispatching by match
//! costs nothing, needs no boxing and no extra crate, and adding FTP is one
//! more variant instead of a new layer.
//!
//! What matters is what callers see: a session, with no notion of which side of
//! a transfer it is on. Nothing above this may ask "is this the local one".

use crate::endpoint::Protocol;
use crate::error::Result;
use crate::fs::{self, Listing};
use crate::ftp::{Encryption, FtpLease, FtpSession};
use crate::local::LocalSession;
use crate::ops::Measurement;
use crate::sftp::SftpSession;
use crate::stream::{Reader, Writer};

/// An open session on one endpoint.
#[derive(Debug)]
pub enum Session {
    Local(LocalSession),
    Sftp(Box<SftpSession>),
    Ftp(Box<FtpSession>),
}

/// What a transfer has to keep alive while it runs.
///
/// Over SFTP that is a channel inside the one connection; over FTP it is a
/// whole login, because that is what a second simultaneous transfer costs
/// there. Nothing above this needs to know which — only that it must not be
/// dropped until the transfer has been finished off.
pub enum Hold<'a> {
    Sftp(crate::sftp::Lease<'a>),
    Ftp(FtpLease<'a>),
}

impl Session {
    pub fn protocol(&self) -> Protocol {
        match self {
            Session::Local(_) => Protocol::Local,
            Session::Sftp(_) => Protocol::Sftp,
            Session::Ftp(session) => match session.encryption() {
                Encryption::None => Protocol::Ftp,
                Encryption::Explicit | Encryption::Implicit => Protocol::Ftps,
            },
        }
    }

    /// Where a pane starts when nothing else is known.
    pub async fn home(&self) -> Result<String> {
        match self {
            Session::Local(session) => session.home(),
            Session::Sftp(session) => session.home().await,
            Session::Ftp(session) => session.home().await,
        }
    }

    pub async fn list_dir(&self, path: &str) -> Result<Listing> {
        match self {
            Session::Local(session) => session.list_dir(path).await,
            Session::Sftp(session) => session.list_dir(path).await,
            Session::Ftp(session) => session.list_dir(path).await,
        }
    }

    /// The directory above, or `None` at the top.
    ///
    /// Paths are the endpoint's own: POSIX on a server whatever the client runs
    /// on, and the system's own notation locally. Mixing the two produces paths
    /// that look right and are not.
    pub fn parent(&self, path: &str) -> Option<String> {
        match self {
            Session::Local(session) => session.parent(path),
            Session::Sftp(_) | Session::Ftp(_) => fs::parent_remote(path),
        }
    }

    pub fn join(&self, directory: &str, name: &str) -> String {
        match self {
            Session::Local(session) => session.join(directory, name),
            Session::Sftp(_) | Session::Ftp(_) => fs::join_remote(directory, name),
        }
    }

    pub async fn create_dir(&self, path: &str) -> Result<()> {
        match self {
            Session::Local(session) => session.create_dir(path).await,
            Session::Sftp(session) => session.create_dir(path).await,
            Session::Ftp(session) => session.create_dir(path).await,
        }
    }

    pub async fn create_file(&self, path: &str) -> Result<()> {
        match self {
            Session::Local(session) => session.create_file(path).await,
            Session::Sftp(session) => session.create_file(path).await,
            Session::Ftp(session) => session.create_file(path).await,
        }
    }

    pub async fn rename(&self, from: &str, to: &str) -> Result<()> {
        match self {
            Session::Local(session) => session.rename(from, to).await,
            Session::Sftp(session) => session.rename(from, to).await,
            Session::Ftp(session) => session.rename(from, to).await,
        }
    }

    /// What a recursive delete would remove, so it can be shown before it is.
    pub async fn measure(&self, path: &str) -> Result<Measurement> {
        match self {
            Session::Local(session) => session.measure(path).await,
            Session::Sftp(session) => session.measure(path).await,
            Session::Ftp(session) => session.measure(path).await,
        }
    }

    pub async fn remove(&self, path: &str) -> Result<()> {
        match self {
            Session::Local(session) => session.remove(path).await,
            Session::Sftp(session) => session.remove(path).await,
            Session::Ftp(session) => session.remove(path).await,
        }
    }

    pub async fn set_permissions(&self, path: &str, mode: u32, recursive: bool) -> Result<()> {
        match self {
            Session::Local(session) => session.set_permissions(path, mode, recursive).await,
            Session::Sftp(session) => session.set_permissions(path, mode, recursive).await,
            Session::Ftp(session) => session.set_permissions(path, mode, recursive).await,
        }
    }

    /// Opens a file for reading at an offset. The lease, where there is one,
    /// holds the channel the transfer runs on and must be kept alive for as
    /// long as the reader is used.
    pub async fn open_read(&self, path: &str, offset: u64) -> Result<(Reader, Option<Hold<'_>>)> {
        match self {
            Session::Local(session) => Ok((session.open_read(path, offset).await?, None)),
            Session::Sftp(session) => {
                let (reader, lease) = session.open_read(path, offset).await?;
                Ok((reader, Some(Hold::Sftp(lease))))
            }
            Session::Ftp(session) => {
                let (reader, lease) = session.open_read(path, offset).await?;
                Ok((reader, Some(Hold::Ftp(lease))))
            }
        }
    }

    pub async fn open_write(&self, path: &str, offset: u64) -> Result<(Writer, Option<Hold<'_>>)> {
        match self {
            Session::Local(session) => Ok((session.open_write(path, offset).await?, None)),
            Session::Sftp(session) => {
                let (writer, lease) = session.open_write(path, offset).await?;
                Ok((writer, Some(Hold::Sftp(lease))))
            }
            Session::Ftp(session) => {
                let (writer, lease) = session.open_write(path, offset).await?;
                Ok((writer, Some(Hold::Ftp(lease))))
            }
        }
    }

    /// Ends a transfer and asks the endpoint whether it went through.
    ///
    /// A file handle needs nothing of the sort; an FTP data connection does,
    /// because the server's verdict arrives afterwards on the control
    /// connection and a transfer nobody asked about is a transfer nobody knows
    /// succeeded. Everything else here simply lets go.
    pub async fn finish_read(&self, reader: Reader, hold: Option<Hold<'_>>) -> Result<()> {
        match (self, reader, hold) {
            (Session::Ftp(session), Reader::Ftp(transfer), Some(Hold::Ftp(lease))) => {
                session.finish(*transfer, lease).await
            }
            _ => Ok(()),
        }
    }

    pub async fn finish_write(&self, writer: Writer, hold: Option<Hold<'_>>) -> Result<()> {
        match (self, writer, hold) {
            (Session::Ftp(session), Writer::Ftp(transfer), Some(Hold::Ftp(lease))) => {
                session.finish(*transfer, lease).await
            }
            _ => Ok(()),
        }
    }

    /// Puts a finished file under its final name, replacing whatever was there.
    pub async fn replace(&self, from: &str, to: &str) -> Result<()> {
        match self {
            Session::Local(session) => session.replace(from, to).await,
            Session::Sftp(session) => session.replace(from, to).await,
            Session::Ftp(session) => session.replace(from, to).await,
        }
    }

    /// Size and modification time, for deciding whether a resume is safe.
    pub async fn stat(&self, path: &str) -> Result<(u64, Option<i64>)> {
        match self {
            Session::Local(session) => session.stat(path).await,
            Session::Sftp(session) => session.stat(path).await,
            Session::Ftp(session) => session.stat(path).await,
        }
    }

    pub async fn set_modified(&self, path: &str, seconds: i64) -> Result<()> {
        match self {
            Session::Local(session) => session.set_modified(path, seconds).await,
            Session::Sftp(session) => session.set_modified(path, seconds).await,
            Session::Ftp(session) => session.set_modified(path, seconds).await,
        }
    }

    /// Whether a file arriving here is written under a temporary name first.
    ///
    /// The protocol still has the last word: over FTP the rename at the end
    /// needs a right many accounts lack, and a transfer that completes and then
    /// fails to rename leaves nothing usable behind.
    pub fn temporary_name(&self) -> bool {
        match self {
            // A local rename is free and atomic; there is no reason to write a
            // half file under its final name.
            Session::Local(_) => true,
            Session::Sftp(session) => session.temporary_name(),
            Session::Ftp(session) => session.temporary_name(),
        }
    }

    /// How many transfers this endpoint currently allows at once.
    pub async fn concurrency(&self) -> u32 {
        match self {
            // The local disk has no session limit worth speaking of; what
            // limits a local copy is the disk, and that needs no permit.
            Session::Local(_) => u32::from(Protocol::MAX_CONCURRENCY),
            Session::Sftp(session) => session.concurrency().await,
            Session::Ftp(session) => session.concurrency().await,
        }
    }

    pub async fn set_concurrency(&self, wanted: u32) {
        match self {
            Session::Local(_) => {}
            Session::Sftp(session) => session.set_concurrency(wanted).await,
            Session::Ftp(session) => session.set_concurrency(wanted).await,
        }
    }

    pub async fn disconnect(&self) {
        match self {
            Session::Local(_) => {}
            Session::Sftp(session) => session.disconnect().await,
            Session::Ftp(session) => session.disconnect().await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_local_session_walks_up_in_the_notation_of_this_system() {
        let session = Session::Local(LocalSession::new());
        assert_eq!(session.protocol(), Protocol::Local);
        // Not asserting a separator: the point is that it asks the platform
        // rather than assuming a slash.
        assert!(session.parent("/var/www/html").is_some());
        assert!(session
            .join("/var/www", "index.html")
            .ends_with("index.html"));
    }
}
