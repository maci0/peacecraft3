//! Native byte-stream caller for an already framed BNCS or W3GS stream.
//!
//! This module is not built for `wasm32`. Deadlines and byte ceilings are
//! caller supplied. Each write and read rearms its socket timeout to the time
//! still left before the absolute deadline. The caller chooses the
//! [`std::net::SocketAddr`]; this module does not impose a loopback restriction.

use std::io::{self, ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

use super::{Decoder, FrameError, FrameLimits, OwnedFrame};

/// Bounds for one call. None of these values are protocol constants.
#[derive(Debug, Clone, Copy)]
pub struct CallPolicy {
    /// Absolute monotonic deadline for connect, write, and read.
    pub deadline: Instant,
    /// Upper bound on the connect wait. Zero, or a deadline that is not still
    /// in the future, fails before a socket is opened.
    pub connect_timeout: Duration,
    /// Largest request that will be written.
    pub max_write_bytes: usize,
    /// Largest total number of bytes that will be read.
    pub max_read_bytes: usize,
    /// Decoder ceilings for the reply.
    pub frames: FrameLimits,
}

/// Failure of [`exchange`] or its byte-stream I/O.
#[derive(Debug)]
pub enum CallError {
    /// The deadline elapsed, the connect budget was zero, or a wait timed out.
    Deadline,
    /// The request is larger than [`CallPolicy::max_write_bytes`].
    WriteTooLarge {
        /// Request length.
        len: usize,
        /// Configured ceiling.
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

/// Write `request` to an already connected stream and read complete frames.
///
/// `arm` runs before every write and read with the time remaining until
/// `policy.deadline`. A TCP caller uses it to set the socket timeout. A stream
/// that blocks past that duration can still overrun the deadline; [`exchange`]
/// arms a real socket. `Interrupted` is retried until the deadline elapses.
/// `policy.connect_timeout` is not used here.
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
    let mut decoder = Decoder::new(policy.frames)?;
    write_all_bounded(stream, request, || remaining(policy.deadline), &mut arm)?;
    let mut frames = Vec::new();
    let mut read_total = 0usize;
    let mut scratch = [0u8; 512];
    while frames.len() < policy.frames.max_frames {
        if read_total >= policy.max_read_bytes {
            return Err(CallError::ReadTooLarge {
                read: read_total,
                limit: policy.max_read_bytes,
            });
        }
        let want = scratch.len().min(policy.max_read_bytes - read_total);
        let count = read_some(stream, &mut scratch[..want], policy.deadline, &mut arm)?;
        if count == 0 {
            break;
        }
        read_total += count;
        let decoded = decoder.push(&scratch[..count])?;
        frames
            .try_reserve(decoded.len())
            .map_err(|_| CallError::Frame(FrameError::Allocation))?;
        frames.extend(decoded);
    }
    decoder.finish()?;
    Ok(frames)
}

/// Connect to `addr`, write `request`, and read complete frames.
///
/// A zero `connect_timeout`, or a `deadline` that is not still in the future,
/// returns [`CallError::Deadline`] and does not open a socket.
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
    let _ = Decoder::new(policy.frames)?;
    if policy.connect_timeout.is_zero() {
        return Err(CallError::Deadline);
    }
    let connect_for = remaining(policy.deadline)?.min(policy.connect_timeout);
    if connect_for.is_zero() {
        return Err(CallError::Deadline);
    }
    let mut stream = TcpStream::connect_timeout(&addr, connect_for).map_err(map_wait)?;
    exchange_io(&mut stream, request, policy, arm_tcp)
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::{Duration, Instant};

    use super::super::{BNCS_FAMILY, FrameError, FrameLimits, encode};
    use super::{CallError, CallPolicy, exchange, exchange_io, write_all_bounded};

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
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
}
