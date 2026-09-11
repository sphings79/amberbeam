//! Reading and writing a file, from a given offset.
//!
//! The transfer engine works on these and never asks which side it is holding.
//! That is the first seam of the concept paper in its most concrete form: a
//! copy is a reader and a writer, and where either of them lives is of no
//! interest to the loop that moves the bytes.
//!
//! Both are opened at an offset, because resuming a transfer that broke at
//! 3.2 GB is the point of the exercise — see [`crate::transfer::ResumeMarker`]
//! for what has to be true before that is safe.

use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

/// An FTP data connection, carrying one transfer.
///
/// Unlike the other two it is not a file handle but a second socket, and it has
/// to be closed properly: the server's verdict on the transfer arrives on the
/// control connection afterwards, and a transfer nobody asked about is a
/// transfer nobody knows succeeded.
pub type FtpTransfer = suppaftp::tokio::TransferStream<suppaftp::tokio::AsyncRustlsStream>;

/// A file being read, local or remote.
pub enum Reader {
    Local(tokio::fs::File),
    Sftp(Box<russh_sftp::client::fs::File>),
    Ftp(Box<FtpTransfer>),
}

/// A file being written, local or remote.
pub enum Writer {
    Local(tokio::fs::File),
    Sftp(Box<russh_sftp::client::fs::File>),
    Ftp(Box<FtpTransfer>),
}

impl AsyncRead for Reader {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            Reader::Local(file) => Pin::new(file).poll_read(cx, buf),
            Reader::Sftp(file) => Pin::new(file.as_mut()).poll_read(cx, buf),
            Reader::Ftp(transfer) => Pin::new(transfer.as_mut()).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for Writer {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        data: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        match self.get_mut() {
            Writer::Local(file) => Pin::new(file).poll_write(cx, data),
            Writer::Sftp(file) => Pin::new(file.as_mut()).poll_write(cx, data),
            Writer::Ftp(transfer) => Pin::new(transfer.as_mut()).poll_write(cx, data),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            Writer::Local(file) => Pin::new(file).poll_flush(cx),
            Writer::Sftp(file) => Pin::new(file.as_mut()).poll_flush(cx),
            Writer::Ftp(transfer) => Pin::new(transfer.as_mut()).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            Writer::Local(file) => Pin::new(file).poll_shutdown(cx),
            Writer::Sftp(file) => Pin::new(file.as_mut()).poll_shutdown(cx),
            Writer::Ftp(transfer) => Pin::new(transfer.as_mut()).poll_shutdown(cx),
        }
    }
}

/// How much is moved per read. Large enough that the round trips do not
/// dominate on a fast link, small enough that progress stays lively and a
/// cancelled transfer stops quickly.
pub const CHUNK: usize = 64 * 1024;
