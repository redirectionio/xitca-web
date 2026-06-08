//! A synchronous IO bridge for adapting blocking TLS libraries (OpenSSL, native-tls)
//! to completion-based async IO traits.
//!
//! The bridge implements `std::io::Read + Write` using in-memory buffers.
//! TLS libraries read/write to the bridge synchronously, then the caller
//! drains/fills the buffers asynchronously via `AsyncBufRead`/`AsyncBufWrite`.

use std::io;

use xitca_io::{
    bytes::{Buf, BytesMut},
    io::{AsyncBufRead, AsyncBufWrite, BoundedBuf, BoundedBufMut},
};

/// A synchronous bridge that pairs in-memory buffers with an async IO handle.
///
/// TLS libraries see `Read + Write` backed by `read_buf` and `write_buf`.
/// The caller uses [`fill_read_buf`] and [`drain_write_buf`] to move data
/// between the buffers and the underlying async IO.
pub struct SyncBridge {
    /// Ciphertext read from the network, consumed by TLS `read`.
    /// Taken by the read path across await points.
    pub read_buf: Option<BytesMut>,
    /// Ciphertext produced by TLS `write`, drained to the network.
    /// Taken by the write path across await points.
    pub write_buf: Option<BytesMut>,
}

impl SyncBridge {
    pub(crate) fn new() -> Self {
        Self {
            read_buf: Some(BytesMut::new()),
            write_buf: Some(BytesMut::new()),
        }
    }

    /// Take the write buffer out for the caller to fill/drain across an await point.
    ///
    /// Returns `BrokenPipe` instead of panicking when the buffer is missing: this happens
    /// when a previous write future was dropped mid-await (e.g. cancelled by a `select!`
    /// or a timeout) before it could put the buffer back. The connection is unusable at
    /// that point and callers should treat this as a (broken pipe flavoured) IO error
    /// rather than crashing the task.
    pub(crate) fn take_write_buf(&mut self) -> io::Result<BytesMut> {
        self.write_buf.take().ok_or_else(broken_pipe)
    }

    /// Take the read buffer out for the caller to fill across an await point.
    ///
    /// See [`SyncBridge::take_write_buf`] for why this returns a `Result` rather than
    /// panicking: a cancelled read future leaves the buffer taken, and any later read
    /// must be reported as a (broken pipe flavoured) IO error instead of panicking.
    pub(crate) fn take_read_buf(&mut self) -> io::Result<BytesMut> {
        self.read_buf.take().ok_or_else(broken_pipe)
    }

    pub(crate) fn set_read_buf(&mut self, buf: BytesMut) {
        self.read_buf = Some(buf);
    }

    pub(crate) fn set_write_buf(&mut self, buf: BytesMut) {
        self.write_buf = Some(buf);
    }
}

impl io::Read for SyncBridge {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // `None` here means a previous async read was cancelled mid-await and never
        // returned the buffer. Report it as a broken pipe instead of panicking so the
        // TLS library surfaces it as a regular IO error.
        let read_buf = self.read_buf.as_mut().ok_or_else(broken_pipe)?;
        if read_buf.is_empty() {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        let len = buf.len().min(read_buf.len());
        buf[..len].copy_from_slice(&read_buf[..len]);
        read_buf.advance(len);
        Ok(len)
    }
}

impl io::Write for SyncBridge {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // see comment in `Read::read` above: `None` means a cancelled async write.
        self.write_buf.as_mut().ok_or_else(broken_pipe)?.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Read ciphertext from the network into `bridge.read_buf`.
pub(crate) async fn fill_read_buf(io: &impl AsyncBufRead, bridge: &mut SyncBridge) -> io::Result<()> {
    let mut buf = bridge.take_read_buf()?;
    let len = buf.len();
    buf.reserve(4096);

    let (res, buf) = io.read(buf.slice(len..)).await;
    bridge.set_read_buf(buf.into_inner());

    match res {
        Ok(0) => Err(io::ErrorKind::UnexpectedEof.into()),
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}

/// Drain all ciphertext from `bridge.write_buf` to the network.
pub(crate) async fn drain_write_buf(io: &impl AsyncBufWrite, bridge: &mut SyncBridge) -> io::Result<()> {
    let buf = bridge.take_write_buf()?;

    let (res, buf) = drain_write(io, buf).await;
    bridge.set_write_buf(buf);

    res
}

/// Drain a taken write buffer to the network.
/// Always returns the buffer (cleared on success) so the caller can put it back.
pub(crate) async fn drain_write(io: &impl AsyncBufWrite, buf: BytesMut) -> (io::Result<()>, BytesMut) {
    if buf.is_empty() {
        return (Ok(()), buf);
    }

    let (res, mut buf) = io.write_all(buf).await;
    buf.clear();

    (res, buf)
}

/// Get a mutable slice over the spare (uninitialized) capacity of a `BoundedBufMut`.
///
/// The returned slice is safe to write into freely. It is however unsafe to
/// read from, as the memory may be uninitialized.
///
/// # Safety
///
/// The caller must not read from the returned slice beyond what has been written.
pub(crate) unsafe fn spare_capacity_mut(buf: &mut impl BoundedBufMut) -> &mut [u8] {
    let init = buf.bytes_init();
    let total = buf.bytes_total();
    unsafe { core::slice::from_raw_parts_mut(buf.stable_mut_ptr().add(init), total - init) }
}

const POLL_TO_COMPLETE: &str = "previous call to future didn't polled to completion";

/// Error reported when a buffer is missing because a previous async read/write was
/// cancelled mid-await (e.g. dropped by a `select!` or a timeout) and never returned
/// it. The connection is unusable at that point; `BrokenPipe` lets callers (and the
/// wrapped TLS library) treat it as a regular IO error instead of panicking.
fn broken_pipe() -> io::Error {
    io::Error::new(io::ErrorKind::BrokenPipe, POLL_TO_COMPLETE)
}
