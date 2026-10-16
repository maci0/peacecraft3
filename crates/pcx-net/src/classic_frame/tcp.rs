//! Native byte-stream caller for an already framed BNCS or W3GS stream.
//!
//! This module is not built for `wasm32`. Deadlines and byte ceilings are
//! caller supplied. Each write and read rearms its socket timeout to the time
//! still left before the absolute deadline. The caller chooses the
//! [`std::net::SocketAddr`]; this module does not impose a loopback restriction.

use std::io::{self, ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use super::{Decoder, FrameError, FrameLimits, OwnedFrame};

/// Bounds for one exchange or an entire [`Connection`]. No protocol defaults.
#[derive(Debug, Clone, Copy)]
pub struct CallPolicy {
    /// Absolute monotonic deadline for connect, write, and read.
    pub deadline: Instant,
    /// Upper bound on the connect wait. Zero, or a deadline that is not still
    /// in the future, fails before a socket is opened.
    pub connect_timeout: Duration,
    /// Largest total number of bytes that will be written.
    pub max_write_bytes: usize,
    /// Largest total number of bytes that will be read.
    pub max_read_bytes: usize,
    /// Decoder ceilings for the reply.
    pub frames: FrameLimits,
}

/// Failure of [`exchange`], [`Connection`] or their byte-stream I/O.
#[derive(Debug)]
pub enum CallError {
    /// The deadline elapsed, the connect budget was zero, or a wait timed out.
    Deadline,
    /// An explicit controller cancellation, distinct from peer EOF or timeout.
    Cancelled,
    /// The request exceeds the remaining cumulative write allowance.
    WriteTooLarge {
        /// Request length.
        len: usize,
        /// Remaining byte allowance (the full ceiling for a one-shot exchange).
        limit: usize,
    },
    /// The reply reached [`CallPolicy::max_read_bytes`] before the requested frames.
    ReadTooLarge {
        /// Bytes read so far.
        read: usize,
        /// Configured ceiling.
        limit: usize,
    },
    /// The frame decoder rejected the stream. The decoder is poisoned.
    Frame(FrameError),
    /// An I/O error other than a timeout or interruption.
    Io(io::Error),
}

impl std::fmt::Display for CallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Deadline => write!(f, "absolute I/O deadline elapsed"),
            Self::Cancelled => write!(f, "classic connection cancelled by controller"),
            Self::WriteTooLarge { len, limit } => {
                write!(f, "request is {len} bytes and the write limit is {limit}")
            }
            Self::ReadTooLarge { read, limit } => {
                write!(f, "read {read} bytes, past the {limit}-byte limit")
            }
            Self::Frame(error) => write!(f, "{error}"),
            Self::Io(error) => write!(f, "io: {error}"),
        }
    }
}

impl std::error::Error for CallError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Frame(error) => Some(error),
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<FrameError> for CallError {
    fn from(error: FrameError) -> Self {
        Self::Frame(error)
    }
}

fn remaining(deadline: Instant) -> Result<Duration, CallError> {
    match deadline.checked_duration_since(Instant::now()) {
        Some(left) if !left.is_zero() => Ok(left),
        _ => Err(CallError::Deadline),
    }
}

fn map_wait(error: io::Error) -> CallError {
    if error.kind() == ErrorKind::TimedOut || error.kind() == ErrorKind::WouldBlock {
        CallError::Deadline
    } else {
        CallError::Io(error)
    }
}

fn write_all_bounded<W: Write>(
    writer: &mut W,
    data: &[u8],
    mut time_left: impl FnMut() -> Result<Duration, CallError>,
    mut arm: impl FnMut(&mut W, Duration) -> Result<(), CallError>,
) -> Result<(), CallError> {
    let mut written = 0;
    while written < data.len() {
        arm(writer, time_left()?)?;
        match writer.write(&data[written..]) {
            Ok(0) => {
                return Err(CallError::Io(io::Error::new(
                    ErrorKind::WriteZero,
                    "write returned no bytes",
                )));
            }
            Ok(count) => written += count,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) => return Err(map_wait(error)),
        }
    }
    Ok(())
}

fn read_some<R: Read>(
    reader: &mut R,
    buf: &mut [u8],
    deadline: Instant,
    mut arm: impl FnMut(&mut R, Duration) -> Result<(), CallError>,
) -> Result<usize, CallError> {
    loop {
        let left = remaining(deadline)?;
        arm(reader, left)?;
        match reader.read(buf) {
            Ok(count) => return Ok(count),
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) => return Err(map_wait(error)),
        }
    }
}

fn arm_tcp(stream: &mut TcpStream, timeout: Duration) -> Result<(), CallError> {
    stream
        .set_read_timeout(Some(timeout))
        .map_err(CallError::Io)?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(CallError::Io)?;
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StreamState {
    Open,
    Eof,
    Failed,
}

struct FramedIo<S> {
    stream: S,
    decoder: Decoder,
    pending: std::vec::IntoIter<OwnedFrame>,
    policy: CallPolicy,
    read: usize,
    written: usize,
    state: StreamState,
}

fn failed_connection() -> CallError {
    CallError::Io(io::Error::new(
        ErrorKind::ConnectionAborted,
        "classic connection is unusable after a previous failure",
    ))
}

impl<S: Read + Write> FramedIo<S> {
    fn new(stream: S, policy: CallPolicy, decoder: Decoder) -> Self {
        Self {
            stream,
            decoder,
            pending: Vec::new().into_iter(),
            policy,
            read: 0,
            written: 0,
            state: StreamState::Open,
        }
    }

    fn send(
        &mut self,
        bytes: &[u8],
        arm: impl FnMut(&mut S, Duration) -> Result<(), CallError>,
    ) -> Result<(), CallError> {
        match self.state {
            StreamState::Failed => return Err(failed_connection()),
            StreamState::Eof => {
                return Err(CallError::Io(io::Error::new(
                    ErrorKind::NotConnected,
                    "classic connection reached EOF",
                )));
            }
            StreamState::Open => {}
        }
        let result = (|| {
            let available = self.policy.max_write_bytes - self.written;
            if bytes.len() > available {
                return Err(CallError::WriteTooLarge {
                    len: bytes.len(),
                    limit: available,
                });
            }
            remaining(self.policy.deadline)?;
            self.written += bytes.len();
            write_all_bounded(
                &mut self.stream,
                bytes,
                || remaining(self.policy.deadline),
                arm,
            )
        })();
        if result.is_err() {
            self.state = StreamState::Failed;
        }
        result
    }

    fn receive(
        &mut self,
        mut arm: impl FnMut(&mut S, Duration) -> Result<(), CallError>,
    ) -> Result<Option<OwnedFrame>, CallError> {
        match self.state {
            StreamState::Failed => return Err(failed_connection()),
            StreamState::Eof => return Ok(None),
            StreamState::Open => {}
        }
        let result = (|| {
            remaining(self.policy.deadline)?;
            if let Some(frame) = self.pending.next() {
                return Ok(Some(frame));
            }
            let mut scratch = [0u8; 512];
            loop {
                if self.read >= self.policy.max_read_bytes {
                    return Err(CallError::ReadTooLarge {
                        read: self.read,
                        limit: self.policy.max_read_bytes,
                    });
                }
                let want = scratch.len().min(self.policy.max_read_bytes - self.read);
                let count = read_some(
                    &mut self.stream,
                    &mut scratch[..want],
                    self.policy.deadline,
                    &mut arm,
                )?;
                if count == 0 {
                    self.decoder.finish()?;
                    self.state = StreamState::Eof;
                    return Ok(None);
                }
                self.read += count;
                self.pending = self.decoder.push(&scratch[..count])?.into_iter();
                if let Some(frame) = self.pending.next() {
                    return Ok(Some(frame));
                }
            }
        })();
        if result.is_err() {
            self.state = StreamState::Failed;
        }
        result
    }
}

/// Native connection retaining its frame decoder across caller responses.
///
/// The supplied deadline and byte/frame ceilings apply to the entire connection;
/// no call resets or extends them. I/O blocks with the remaining socket timeout.
/// EOF validates the final frame, shuts down the socket and returns `None`.
/// Any error is terminal; later operations refuse. Drop releases the socket.
/// Protocol admission, map validation and application worker placement are caller duties.
pub struct Connection {
    io: FramedIo<TcpStream>,
    cancelled: Arc<AtomicBool>,
}

/// Controller for one connection. It exposes shutdown, not socket reads/writes.
///
/// Dropping this handle does not cancel. Cancellation aborts transport; it is
/// not a graceful W3GS leave and cannot undo bytes already written or results
/// already delivered. Application publication still needs its request identity.
pub struct CancellationHandle {
    stream: TcpStream,
    cancelled: Arc<AtomicBool>,
}

impl CancellationHandle {
    /// Mark cancellation and wake an outstanding socket operation.
    ///
    /// Repeated shutdown of an already disconnected socket succeeds. Other
    /// shutdown errors are reported; the cancellation flag remains set.
    pub fn cancel(&self) -> io::Result<()> {
        self.cancelled.store(true, Ordering::Release);
        match self.stream.shutdown(std::net::Shutdown::Both) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::NotConnected => Ok(()),
            Err(error) => Err(error),
        }
    }
}

impl Connection {
    /// Validate limits and deadline before opening one TCP connection, without retrying.
    pub fn connect(addr: SocketAddr, policy: CallPolicy) -> Result<Self, CallError> {
        let decoder = Decoder::new(policy.frames)?;
        if policy.connect_timeout.is_zero() {
            return Err(CallError::Deadline);
        }
        let connect_for = remaining(policy.deadline)?.min(policy.connect_timeout);
        let stream = TcpStream::connect_timeout(&addr, connect_for).map_err(map_wait)?;
        Ok(Self {
            io: FramedIo::new(stream, policy, decoder),
            cancelled: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Create a controller for this established connection, without exposing I/O.
    /// The clone refers only to this socket, including after a replacement connects.
    pub fn cancellation_handle(&self) -> io::Result<CancellationHandle> {
        if self.io.state != StreamState::Open || self.cancelled.load(Ordering::Acquire) {
            return Err(io::Error::new(
                ErrorKind::NotConnected,
                "classic connection is no longer open",
            ));
        }
        Ok(CancellationHandle {
            stream: self.io.stream.try_clone()?,
            cancelled: Arc::clone(&self.cancelled),
        })
    }

    fn check_cancellation(&mut self) -> Result<(), CallError> {
        if !self.cancelled.load(Ordering::Acquire) {
            return Ok(());
        }
        let was_open = self.io.state == StreamState::Open;
        self.io.state = StreamState::Failed;
        if was_open {
            let _ = self.io.stream.shutdown(std::net::Shutdown::Both);
        }
        Err(CallError::Cancelled)
    }

    fn complete_operation<T>(
        &mut self,
        was_open: bool,
        result: Result<T, CallError>,
    ) -> Result<T, CallError> {
        let cancelled = self.cancelled.load(Ordering::Acquire);
        if cancelled {
            self.io.state = StreamState::Failed;
        }
        if was_open && self.io.state != StreamState::Open {
            let _ = self.io.stream.shutdown(std::net::Shutdown::Both);
        }
        if cancelled {
            Err(CallError::Cancelled)
        } else {
            result
        }
    }

    /// Actual local socket endpoint, without selecting a client listen port.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.io.stream.local_addr()
    }

    /// Write raw bytes within the cumulative write ceiling; no automatic resend.
    pub fn send(&mut self, bytes: &[u8]) -> Result<(), CallError> {
        self.check_cancellation()?;
        let was_open = self.io.state == StreamState::Open;
        let result = self.io.send(bytes, arm_tcp);
        self.complete_operation(was_open, result)
    }

    /// Return the next frame, preserving queued frames and any partial next frame.
    pub fn receive(&mut self) -> Result<Option<OwnedFrame>, CallError> {
        self.receive_with_arm(arm_tcp)
    }

    fn receive_with_arm(
        &mut self,
        arm: impl FnMut(&mut TcpStream, Duration) -> Result<(), CallError>,
    ) -> Result<Option<OwnedFrame>, CallError> {
        self.check_cancellation()?;
        let was_open = self.io.state == StreamState::Open;
        let result = self.io.receive(arm);
        self.complete_operation(was_open, result)
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        // A retained controller must not keep the peer connection alive.
        let _ = self.io.stream.shutdown(std::net::Shutdown::Both);
    }
}

fn collect_replies<S: Read + Write>(
    io: &mut FramedIo<S>,
    request: &[u8],
    mut arm: impl FnMut(&mut S, Duration) -> Result<(), CallError>,
) -> Result<Vec<OwnedFrame>, CallError> {
    io.send(request, &mut arm)?;
    let mut frames = Vec::new();
    while frames.len() < io.policy.frames.max_frames {
        let Some(frame) = io.receive(&mut arm)? else {
            break;
        };
        frames.try_reserve(1).map_err(|_| FrameError::Allocation)?;
        frames.push(frame);
    }
    io.decoder.finish()?;
    Ok(frames)
}

/// Write `request` to an already connected stream and read complete frames.
///
/// `arm` runs before every write and read with the remaining absolute deadline.
/// A blocking stream must honor that duration. `Interrupted` is retried without
/// resetting the deadline; `connect_timeout` is unused here.
#[cfg(test)]
pub(super) fn exchange_io<S: Read + Write>(
    stream: &mut S,
    request: &[u8],
    policy: &CallPolicy,
    mut arm: impl FnMut(&mut S, Duration) -> Result<(), CallError>,
) -> Result<Vec<OwnedFrame>, CallError> {
    if request.len() > policy.max_write_bytes {
        return Err(CallError::WriteTooLarge {
            len: request.len(),
            limit: policy.max_write_bytes,
        });
    }
    let decoder = Decoder::new(policy.frames)?;
    let mut io = FramedIo::new(stream, *policy, decoder);
    collect_replies(&mut io, request, |stream, timeout| {
        arm(&mut **stream, timeout)
    })
}

/// Connect to `addr`, write `request`, and read complete frames.
///
/// A zero `connect_timeout`, or a deadline that is not still in the future,
/// returns [`CallError::Deadline`] without opening a socket.
pub fn exchange(
    addr: SocketAddr,
    request: &[u8],
    policy: &CallPolicy,
) -> Result<Vec<OwnedFrame>, CallError> {
    if request.len() > policy.max_write_bytes {
        return Err(CallError::WriteTooLarge {
            len: request.len(),
            limit: policy.max_write_bytes,
        });
    }
    let mut connection = Connection::connect(addr, *policy)?;
    collect_replies(&mut connection.io, request, arm_tcp)
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::{Duration, Instant};

    use super::super::{BNCS_FAMILY, FrameError, FrameLimits, encode};
    use super::{
        CallError, CallPolicy, Connection, FramedIo, exchange, exchange_io, write_all_bounded,
    };

    fn policy(deadline: Instant) -> CallPolicy {
        CallPolicy {
            deadline,
            connect_timeout: Duration::from_secs(1),
            max_write_bytes: 64,
            max_read_bytes: 64,
            frames: FrameLimits {
                max_frame_bytes: 64,
                max_frames: 2,
            },
        }
    }

    struct Script {
        written: Vec<u8>,
        read_from: Vec<u8>,
        read_pos: usize,
        interrupt_write: bool,
        write_chunk: usize,
        timeout_after: Option<usize>,
        interrupt_read: bool,
    }

    impl Script {
        fn new(read_from: &[u8]) -> Self {
            Self {
                written: Vec::new(),
                read_from: read_from.to_vec(),
                read_pos: 0,
                interrupt_write: false,
                write_chunk: usize::MAX,
                timeout_after: None,
                interrupt_read: false,
            }
        }
    }

    impl Write for Script {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if self.interrupt_write {
                self.interrupt_write = false;
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Interrupted,
                    "write interrupted",
                ));
            }
            if self.timeout_after == Some(self.written.len()) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "write timed out",
                ));
            }
            if buf.is_empty() {
                return Ok(0);
            }
            let count = self.write_chunk.min(buf.len());
            self.written.extend_from_slice(&buf[..count]);
            Ok(count)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl Read for Script {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.interrupt_read {
                self.interrupt_read = false;
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Interrupted,
                    "read interrupted",
                ));
            }
            let rest = &self.read_from[self.read_pos..];
            if rest.is_empty() || buf.is_empty() {
                return Ok(0);
            }
            let count = rest.len().min(buf.len());
            buf[..count].copy_from_slice(&rest[..count]);
            self.read_pos += count;
            Ok(count)
        }
    }

    fn framed(script: Script, policy: CallPolicy) -> FramedIo<Script> {
        FramedIo::new(script, policy, super::Decoder::new(policy.frames).unwrap())
    }

    #[test]
    fn connection_write_count_cannot_wrap() {
        let mut limits = policy(Instant::now() + Duration::from_secs(2));
        limits.max_write_bytes = usize::MAX;
        let mut io = framed(Script::new(&[]), limits);
        io.written = usize::MAX - 1;
        assert!(io.send(b"xy", |_, _| Ok(())).is_err());
        assert!(io.stream.written.is_empty());
    }

    #[test]
    fn connection_retains_batch_queue_and_partial_header_between_responses() {
        let first = encode(BNCS_FAMILY, 0x25, &[]).unwrap();
        let second = encode(BNCS_FAMILY, 0x50, b"abc").unwrap();
        let third = encode(BNCS_FAMILY, 0x51, b"def").unwrap();
        let initial = [first, second, third[..3].to_vec()].concat();
        let mut limits = policy(Instant::now() + Duration::from_secs(2));
        limits.frames.max_frames = 3;
        let mut io = framed(Script::new(&initial), limits);
        assert_eq!(io.receive(|_, _| Ok(())).unwrap().unwrap().id(), 0x25);
        let read = io.stream.read_pos;
        assert_eq!(io.decoder.buffered_len(), 3);
        assert_eq!(io.receive(|_, _| Ok(())).unwrap().unwrap().id(), 0x50);
        assert_eq!(io.stream.read_pos, read);
        io.send(b"response", |_, _| Ok(())).unwrap();
        io.stream.read_from.extend_from_slice(&third[3..]);
        assert_eq!(io.receive(|_, _| Ok(())).unwrap().unwrap().body(), b"def");
        assert_eq!(io.stream.written, b"response");
        assert!(io.receive(|_, _| Ok(())).unwrap().is_none());
        assert!(
            io.receive(|_, _| panic!("EOF must not do I/O"))
                .unwrap()
                .is_none()
        );
        assert!(
            matches!(io.send(b"late", |_, _| panic!("EOF must not write")), Err(CallError::Io(e)) if e.kind() == std::io::ErrorKind::NotConnected)
        );
    }

    #[test]
    fn connection_does_not_reset_byte_or_frame_ceilings_between_calls() {
        let one = encode(BNCS_FAMILY, 0x25, &[]).unwrap();
        let mut limits = policy(Instant::now() + Duration::from_secs(2));
        limits.max_write_bytes = 3;
        let mut writer = framed(Script::new(&[]), limits);
        writer.send(b"ab", |_, _| Ok(())).unwrap();
        assert!(matches!(
            writer.send(b"cd", |_, _| Ok(())),
            Err(CallError::WriteTooLarge { .. })
        ));
        assert_eq!(writer.stream.written, b"ab");
        assert!(
            writer
                .send(b"x", |_, _| panic!("failed stream must not write"))
                .is_err()
        );
        limits.max_read_bytes = 8;
        limits.frames.max_frames = 3;
        let mut reader = framed(Script::new(&one), limits);
        reader.receive(|_, _| Ok(())).unwrap().unwrap();
        reader.stream.read_from.extend(&one);
        reader.receive(|_, _| Ok(())).unwrap().unwrap();
        reader.stream.read_from.extend(&one);
        assert!(matches!(
            reader.receive(|_, _| panic!("read limit must precede I/O")),
            Err(CallError::ReadTooLarge { read: 8, limit: 8 })
        ));
        limits.max_read_bytes = 64;
        limits.frames.max_frames = 2;
        let mut reader = framed(Script::new(&one), limits);
        reader.receive(|_, _| Ok(())).unwrap().unwrap();
        reader.stream.read_from.extend(&one);
        reader.receive(|_, _| Ok(())).unwrap().unwrap();
        reader.stream.read_from.extend(&one);
        assert!(matches!(
            reader.receive(|_, _| Ok(())),
            Err(CallError::Frame(FrameError::FrameCount { limit: 2 }))
        ));
        assert!(
            reader
                .receive(|_, _| panic!("frame failure is terminal"))
                .is_err()
        );
    }

    #[test]
    fn connection_failure_preserves_partial_write_and_rejects_partial_eof() {
        let limits = policy(Instant::now() + Duration::from_secs(2));
        let mut script = Script::new(&[]);
        script.write_chunk = 1;
        script.timeout_after = Some(1);
        let mut writer = framed(script, limits);
        assert!(matches!(
            writer.send(b"abc", |_, _| Ok(())),
            Err(CallError::Deadline)
        ));
        assert_eq!(writer.stream.written, b"a");
        writer.stream.timeout_after = None;
        assert!(
            writer
                .send(b"retry", |_, _| panic!("partial write must not resume"))
                .is_err()
        );
        assert_eq!(writer.stream.written, b"a");
        let one = encode(BNCS_FAMILY, 0x25, &[]).unwrap();
        let mut data = one.clone();
        data.extend_from_slice(&one[..2]);
        let mut reader = framed(Script::new(&data), limits);
        reader.receive(|_, _| Ok(())).unwrap().unwrap();
        assert!(matches!(
            reader.receive(|_, _| Ok(())),
            Err(CallError::Frame(FrameError::Incomplete { have: 2 }))
        ));
        assert!(
            reader
                .receive(|_, _| panic!("partial EOF must not resume"))
                .is_err()
        );
    }

    #[test]
    fn connection_deadline_applies_to_queued_frames_without_another_read() {
        let one = encode(BNCS_FAMILY, 0x25, &[]).unwrap();
        let data = [one.clone(), one].concat();
        let mut io = framed(
            Script::new(&data),
            policy(Instant::now() + Duration::from_secs(2)),
        );
        io.receive(|_, _| Ok(())).unwrap().unwrap();
        io.policy.deadline = Instant::now() - Duration::from_secs(1);
        assert!(matches!(
            io.receive(|_, _| panic!("expired queue must not read")),
            Err(CallError::Deadline)
        ));
        assert!(
            io.send(b"late", |_, _| panic!("deadline failure is terminal"))
                .is_err()
        );
    }

    #[test]
    fn connection_keeps_partial_frame_across_a_lobby_response() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let peer = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            socket
                .set_write_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut request = [0; 4];
            socket.read_exact(&mut request).unwrap();
            assert_eq!(&request, b"join");
            let first = encode(BNCS_FAMILY, 0x25, &[]).unwrap();
            let second = encode(BNCS_FAMILY, 0x50, b"xyz").unwrap();
            socket
                .write_all(&[first, second[..2].to_vec()].concat())
                .unwrap();
            let mut reply = [0; 5];
            socket.read_exact(&mut reply).unwrap();
            assert_eq!(&reply, b"ready");
            socket.write_all(&second[2..]).unwrap();
        });
        let mut connection =
            Connection::connect(address, policy(Instant::now() + Duration::from_secs(5))).unwrap();
        assert_eq!(connection.local_addr().unwrap().ip(), address.ip());
        connection.send(b"join").unwrap();
        assert_eq!(connection.receive().unwrap().unwrap().id(), 0x25);
        connection.send(b"ready").unwrap();
        let response = connection.receive().unwrap().unwrap();
        assert_eq!((response.id(), response.body()), (0x50, &b"xyz"[..]));
        assert!(connection.receive().unwrap().is_none());
        assert!(connection.receive().unwrap().is_none());
        peer.join().unwrap();
    }

    #[test]
    fn connection_closes_socket_after_frame_refusal_and_cannot_resume() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let peer = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            socket
                .set_write_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            socket.write_all(&[BNCS_FAMILY, 0x50, 0xFF, 0x7F]).unwrap();
            let mut after_refusal = Vec::new();
            socket.read_to_end(&mut after_refusal).unwrap();
            assert!(after_refusal.is_empty());
        });
        let mut connection =
            Connection::connect(address, policy(Instant::now() + Duration::from_secs(5))).unwrap();
        assert!(matches!(
            connection.receive(),
            Err(CallError::Frame(FrameError::Oversize {
                declared: 32767,
                limit: 64
            }))
        ));
        assert!(
            matches!(connection.send(b"retry"), Err(CallError::Io(e)) if e.kind() == std::io::ErrorKind::ConnectionAborted)
        );
        assert!(
            matches!(connection.receive(), Err(CallError::Io(e)) if e.kind() == std::io::ErrorKind::ConnectionAborted)
        );
        peer.join().unwrap();
    }

    #[test]
    fn cancellation_suppresses_queued_frames_and_further_writes() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut connection = Connection::connect(
            listener.local_addr().unwrap(),
            policy(Instant::now() + Duration::from_secs(5)),
        )
        .unwrap();
        let (mut peer, _) = listener.accept().unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        let cancel = connection.cancellation_handle().unwrap();
        let wire = encode(BNCS_FAMILY, 0x25, &[]).unwrap();
        connection.io.pending = super::Decoder::new(policy(Instant::now()).frames)
            .unwrap()
            .push(&wire)
            .unwrap()
            .into_iter();
        cancel.cancel().unwrap();
        cancel.cancel().unwrap();
        assert_eq!(
            connection.cancellation_handle().err().unwrap().kind(),
            std::io::ErrorKind::NotConnected
        );
        assert!(matches!(connection.receive(), Err(CallError::Cancelled)));
        assert!(matches!(
            connection.send(b"late"),
            Err(CallError::Cancelled)
        ));
        let mut after_cancel = Vec::new();
        peer.read_to_end(&mut after_cancel).unwrap();
        assert!(after_cancel.is_empty());
    }

    #[test]
    fn cancellation_wakes_an_in_progress_receive() {
        use std::sync::mpsc;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut connection = Connection::connect(
            listener.local_addr().unwrap(),
            policy(Instant::now() + Duration::from_secs(30)),
        )
        .unwrap();
        let (_peer, _) = listener.accept().unwrap();
        let cancel = connection.cancellation_handle().unwrap();
        let (armed_tx, armed_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let (result_tx, result_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let result = connection.receive_with_arm(|socket, timeout| {
                super::arm_tcp(socket, timeout)?;
                armed_tx.send(()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(3)).unwrap();
                Ok(())
            });
            result_tx.send(result).unwrap();
        });
        armed_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        cancel.cancel().unwrap();
        release_tx.send(()).unwrap();
        assert!(matches!(
            result_rx.recv_timeout(Duration::from_secs(3)).unwrap(),
            Err(CallError::Cancelled)
        ));
        worker.join().unwrap();
    }

    #[test]
    fn retained_controller_does_not_keep_dropped_connection_alive_or_cancel_replacement() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = listener.local_addr().unwrap();
        let original =
            Connection::connect(endpoint, policy(Instant::now() + Duration::from_secs(5))).unwrap();
        let (mut old_peer, _) = listener.accept().unwrap();
        old_peer
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let retired = original.cancellation_handle().unwrap();
        drop(original);
        assert_eq!(old_peer.read(&mut [0; 1]).unwrap(), 0);

        let mut replacement =
            Connection::connect(endpoint, policy(Instant::now() + Duration::from_secs(5))).unwrap();
        let (mut peer, _) = listener.accept().unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        retired.cancel().unwrap();
        replacement.send(b"new").unwrap();
        let mut request = [0; 3];
        peer.read_exact(&mut request).unwrap();
        assert_eq!(&request, b"new");
        let wire = encode(BNCS_FAMILY, 0x25, &[]).unwrap();
        peer.write_all(&wire).unwrap();
        assert_eq!(replacement.receive().unwrap().unwrap().bytes(), wire);
    }

    #[test]
    fn dropping_controller_preserves_connection_and_closed_connections_refuse_new_controllers() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut connection = Connection::connect(
            listener.local_addr().unwrap(),
            policy(Instant::now() + Duration::from_secs(5)),
        )
        .unwrap();
        let (mut peer, _) = listener.accept().unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        drop(connection.cancellation_handle().unwrap());
        connection.send(b"ok").unwrap();
        let mut request = [0; 2];
        peer.read_exact(&mut request).unwrap();
        assert_eq!(&request, b"ok");
        peer.shutdown(std::net::Shutdown::Both).unwrap();
        assert!(connection.receive().unwrap().is_none());
        assert_eq!(
            connection.cancellation_handle().err().unwrap().kind(),
            std::io::ErrorKind::NotConnected
        );
    }

    #[test]
    fn fragmented_writes_rearm_and_interrupted_is_retried() {
        let deadline = Instant::now() + Duration::from_millis(500);
        let request = b"abcdef";
        let mut script = Script::new(&[]);
        script.interrupt_write = true;
        script.write_chunk = 1;
        let mut arms = Vec::new();
        let mut policy = policy(deadline);
        policy.max_write_bytes = request.len();
        let frames = exchange_io(&mut script, request, &policy, |_stream, timeout| {
            assert!(!timeout.is_zero());
            arms.push(timeout);
            Ok(())
        })
        .unwrap();
        assert!(frames.is_empty());
        assert_eq!(script.written, request);
        assert!(arms.len() > request.len());
        for pair in arms.windows(2) {
            assert!(pair[1] <= pair[0]);
        }
    }

    #[test]
    fn write_timeout_and_expired_deadline_stop() {
        let mut script = Script::new(&[]);
        script.timeout_after = Some(1);
        script.write_chunk = 1;
        let error = exchange_io(
            &mut script,
            b"abcd",
            &policy(Instant::now() + Duration::from_secs(2)),
            |_stream, _timeout| Ok(()),
        )
        .unwrap_err();
        assert!(matches!(error, CallError::Deadline));
        assert_eq!(script.written, b"a");

        let past = Instant::now() - Duration::from_secs(1);
        let mut sink = Vec::new();
        let mut arms = 0usize;
        let error = write_all_bounded(
            &mut sink,
            b"zz",
            || super::remaining(past),
            |_stream, _timeout| {
                arms += 1;
                Ok(())
            },
        )
        .unwrap_err();
        assert!(matches!(error, CallError::Deadline));
        assert_eq!(arms, 0);
        assert!(sink.is_empty());
    }

    #[test]
    fn each_partial_write_uses_the_supplied_remaining_budget() {
        let budgets = [50, 9, 3].map(Duration::from_millis);
        let mut remaining = budgets.into_iter();
        let mut arms = Vec::new();
        let mut script = Script::new(&[]);
        script.write_chunk = 1;
        write_all_bounded(
            &mut script,
            b"xyz",
            || Ok(remaining.next().expect("one budget per write")),
            |_stream, timeout| {
                arms.push(timeout);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(arms, budgets);
        assert_eq!(script.written, b"xyz");
        assert!(remaining.next().is_none());
    }

    #[test]
    fn byte_and_frame_ceilings_and_read_interrupt() {
        let mut too_big = Script::new(&[]);
        let mut tight = policy(Instant::now() + Duration::from_secs(1));
        tight.max_write_bytes = 4;
        let error =
            exchange_io(&mut too_big, &[0; 8], &tight, |_stream, _timeout| Ok(())).unwrap_err();
        assert!(matches!(
            error,
            CallError::WriteTooLarge { len: 8, limit: 4 }
        ));
        assert!(too_big.written.is_empty());

        let mut policy = policy(Instant::now() + Duration::from_secs(1));
        policy.max_read_bytes = 16;
        policy.frames.max_frame_bytes = 128;
        policy.frames.max_frames = 1;
        let mut partial = [0u8; 40];
        partial[0] = BNCS_FAMILY;
        partial[1] = 0x50;
        partial[2] = 100;
        partial[3] = 0;
        let mut reader = Script::new(&partial);
        let error = exchange_io(&mut reader, &[], &policy, |_stream, _timeout| Ok(())).unwrap_err();
        assert!(matches!(
            error,
            CallError::ReadTooLarge {
                read: 16,
                limit: 16
            }
        ));

        let one = encode(BNCS_FAMILY, 0x25, &[]).unwrap();
        let mut two = one.clone();
        two.extend_from_slice(&one);
        let mut reader = Script::new(&two);
        reader.interrupt_read = true;
        policy.max_read_bytes = 64;
        policy.frames.max_frames = 1;
        let error = exchange_io(&mut reader, &[], &policy, |_stream, _timeout| Ok(())).unwrap_err();
        assert!(matches!(
            error,
            CallError::Frame(FrameError::FrameCount { limit: 1 })
        ));
    }

    #[test]
    fn zero_connect_timeout_and_expired_deadline_do_not_connect() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let addr = listener.local_addr().unwrap();
        let mut zero = policy(Instant::now() + Duration::from_secs(2));
        zero.connect_timeout = Duration::ZERO;
        let error = exchange(addr, b"hi", &zero).unwrap_err();
        assert!(matches!(error, CallError::Deadline));
        assert!(matches!(
            Connection::connect(addr, zero),
            Err(CallError::Deadline)
        ));

        for limits in [
            FrameLimits {
                max_frame_bytes: 3,
                max_frames: 1,
            },
            FrameLimits {
                max_frame_bytes: 64,
                max_frames: 0,
            },
        ] {
            let mut invalid = policy(Instant::now() + Duration::from_secs(2));
            invalid.frames = limits;
            assert!(matches!(
                exchange(addr, b"hi", &invalid),
                Err(CallError::Frame(FrameError::Limit))
            ));
            assert!(matches!(
                Connection::connect(addr, invalid),
                Err(CallError::Frame(FrameError::Limit))
            ));
            let mut script = Script::new(&[]);
            assert!(matches!(
                exchange_io(&mut script, b"hi", &invalid, |_, _| Ok(())),
                Err(CallError::Frame(FrameError::Limit))
            ));
            assert!(script.written.is_empty());
        }

        let past = Instant::now() - Duration::from_secs(1);
        let error = exchange(addr, b"hi", &policy(past)).unwrap_err();
        assert!(matches!(error, CallError::Deadline));
        assert!(matches!(
            Connection::connect(addr, policy(past)),
            Err(CallError::Deadline)
        ));
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
}
