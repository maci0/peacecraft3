use std::{
    io::{self, Read, Write},
    time::{Duration, Instant},
};
use tungstenite::{
    Error, HandshakeError, WebSocket,
    handshake::{
        MidHandshake,
        server::{NoCallback, ServerHandshake},
    },
    protocol::WebSocketConfig,
};

// Project admission-work limits, unrelated to game content or native player limits.
const CALLS_PER_POLL: usize = 8;
const BYTES_PER_POLL: usize = 4096;
const TIMEOUT: Duration = Duration::from_secs(5);

pub(super) struct Io<S> {
    stream: S,
    budget: Option<(usize, usize)>,
}
impl<S> Io<S> {
    fn allowance(&mut self, requested: usize) -> io::Result<usize> {
        if let Some((calls, bytes)) = &mut self.budget {
            if *calls == 0 || (requested != 0 && *bytes == 0) {
                return Err(io::ErrorKind::WouldBlock.into());
            }
            *calls -= 1;
            Ok(requested.min(*bytes))
        } else {
            Ok(requested)
        }
    }
    fn used(&mut self, count: usize) {
        if let Some((_, bytes)) = &mut self.budget {
            *bytes -= count;
        }
    }
}
impl<S: Read> Read for Io<S> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        let allowed = self.allowance(buffer.len())?;
        let count = self.stream.read(&mut buffer[..allowed])?;
        self.used(count);
        Ok(count)
    }
}
impl<S: Write> Write for Io<S> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        let allowed = self.allowance(buffer.len())?;
        let count = self.stream.write(&buffer[..allowed])?;
        self.used(count);
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.allowance(0)?;
        self.stream.flush()
    }
}

pub(super) struct Pending<S: Read + Write> {
    handshake: Option<MidHandshake<ServerHandshake<Io<S>, NoCallback>>>,
    deadline: Instant,
}
impl<S: Read + Write> Pending<S> {
    /// The caller must configure the underlying stream as nonblocking first.
    pub(super) fn new(stream: S, config: WebSocketConfig, now: Instant) -> Self {
        Self {
            handshake: Some(ServerHandshake::start(
                Io {
                    stream,
                    budget: Some((0, 0)),
                },
                NoCallback,
                Some(config),
            )),
            deadline: now + TIMEOUT,
        }
    }

    pub(super) fn poll(&mut self, now: Instant) -> Result<Option<WebSocket<Io<S>>>, Error> {
        let mut handshake = self.handshake.take().ok_or(Error::AlreadyClosed)?;
        if now >= self.deadline {
            return Err(io::Error::from(io::ErrorKind::TimedOut).into());
        }
        handshake.get_mut().get_mut().budget = Some((CALLS_PER_POLL, BYTES_PER_POLL));
        match handshake.handshake() {
            Ok(mut socket) => {
                socket.get_mut().budget = None;
                Ok(Some(socket))
            }
            Err(HandshakeError::Interrupted(next)) => {
                self.handshake = Some(next);
                Ok(None)
            }
            Err(HandshakeError::Failure(error)) => Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, collections::VecDeque, rc::Rc};

    const REQUEST: &[u8] = b"GET / HTTP/1.1\r\nHost: example.test\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n";

    struct State {
        input: VecDeque<u8>,
        output: Vec<u8>,
        read_chunk: usize,
        write_chunk: usize,
        calls: usize,
        bytes: usize,
    }
    #[derive(Clone)]
    struct Stream(Rc<RefCell<State>>);
    impl Read for Stream {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            let mut state = self.0.borrow_mut();
            state.calls += 1;
            if state.input.is_empty() {
                return Err(io::ErrorKind::WouldBlock.into());
            }
            let count = out.len().min(state.read_chunk).min(state.input.len());
            for byte in &mut out[..count] {
                *byte = state.input.pop_front().unwrap();
            }
            state.bytes += count;
            Ok(count)
        }
    }
    impl Write for Stream {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            let mut state = self.0.borrow_mut();
            state.calls += 1;
            let count = bytes.len().min(state.write_chunk);
            state.output.extend_from_slice(&bytes[..count]);
            state.bytes += count;
            Ok(count)
        }
        fn flush(&mut self) -> io::Result<()> {
            self.0.borrow_mut().calls += 1;
            Ok(())
        }
    }
    fn fixture(bytes: &[u8], read_chunk: usize, write_chunk: usize) -> (Pending<Stream>, Stream) {
        let stream = Stream(Rc::new(RefCell::new(State {
            input: bytes.iter().copied().collect(),
            output: Vec::new(),
            read_chunk,
            write_chunk,
            calls: 0,
            bytes: 0,
        })));
        (
            Pending::new(stream.clone(), WebSocketConfig::default(), Instant::now()),
            stream,
        )
    }
    fn poll(
        pending: &mut Pending<Stream>,
        stream: &Stream,
    ) -> Result<Option<WebSocket<Io<Stream>>>, Error> {
        {
            let mut state = stream.0.borrow_mut();
            state.calls = 0;
            state.bytes = 0;
        }
        let result = pending.poll(Instant::now());
        let state = stream.0.borrow();
        assert!(
            state.calls <= CALLS_PER_POLL,
            "unbounded handshake syscalls: {}",
            state.calls
        );
        assert!(
            state.bytes <= BYTES_PER_POLL,
            "unbounded handshake bytes: {}",
            state.bytes
        );
        result
    }

    #[test]
    fn drip_upgrade_yields_each_poll_while_authority_keeps_advancing() {
        let mut input = b"GET / HTTP/1.1\r\nX: ".to_vec();
        input.extend(std::iter::repeat_n(b'a', BYTES_PER_POLL * 4));
        let (mut pending, stream) = fixture(&input, 64, usize::MAX);
        let mut session = pcx_net::session::Session::new(pcx_sim::World::empty(), 1);
        for tick in 1..=4 {
            assert!(poll(&mut pending, &stream).unwrap().is_none());
            session.advance();
            assert_eq!(session.world().tick, tick);
        }
        assert!(stream.0.borrow().output.is_empty());
    }

    #[test]
    fn continuous_input_cannot_exceed_the_byte_allowance() {
        let mut input = b"GET / HTTP/1.1\r\nX: ".to_vec();
        input.extend(std::iter::repeat_n(b'a', BYTES_PER_POLL * 4));
        let (mut pending, stream) = fixture(&input, usize::MAX, usize::MAX);
        assert!(poll(&mut pending, &stream).unwrap().is_none());
        assert_eq!(stream.0.borrow().bytes, BYTES_PER_POLL);
    }

    #[test]
    fn partial_reads_and_response_writes_resume_without_restarting() {
        let (mut pending, stream) = fixture(REQUEST, 16, 16);
        let mut polls = 0;
        loop {
            polls += 1;
            assert!(polls < 64);
            if poll(&mut pending, &stream).unwrap().is_some() {
                break;
            }
        }
        assert!(polls > 1);
        let state = stream.0.borrow();
        let response = std::str::from_utf8(&state.output).unwrap();
        assert!(response.starts_with("HTTP/1.1 101"));
        assert_eq!(response.matches("HTTP/1.1").count(), 1);
        assert!(state.input.is_empty());
    }

    #[test]
    fn completed_io_accepts_messages_larger_than_the_handshake_allowance() {
        let text = vec![b'x'; BYTES_PER_POLL + 1];
        let mut input = Vec::new();
        input.extend_from_slice(&[0x81, 0xfe]);
        input.extend_from_slice(&u16::try_from(text.len()).unwrap().to_be_bytes());
        let mask = [1, 2, 3, 4];
        input.extend_from_slice(&mask);
        input.extend(
            text.iter()
                .enumerate()
                .map(|(i, byte)| byte ^ mask[i % mask.len()]),
        );
        let (mut pending, stream) = fixture(REQUEST, usize::MAX, usize::MAX);
        let mut socket = loop {
            if let Some(socket) = poll(&mut pending, &stream).unwrap() {
                break socket;
            }
        };
        stream.0.borrow_mut().input.extend(input);
        let message = socket.read().unwrap();
        assert_eq!(message.into_text().unwrap().as_bytes(), text);
        assert!(matches!(
            pending.poll(Instant::now()),
            Err(Error::AlreadyClosed)
        ));
    }

    #[test]
    fn bytes_after_the_http_request_keep_the_existing_protocol_refusal() {
        let mut input = REQUEST.to_vec();
        input.push(0x81);
        let (mut pending, stream) = fixture(&input, usize::MAX, usize::MAX);
        assert!(matches!(
            poll(&mut pending, &stream),
            Err(Error::Protocol(
                tungstenite::error::ProtocolError::JunkAfterRequest
            ))
        ));
    }

    #[test]
    fn expired_admission_is_terminal_without_io_or_elapsed_time_gates() {
        let (mut pending, stream) = fixture(REQUEST, usize::MAX, usize::MAX);
        let result = pending.poll(pending.deadline);
        assert!(matches!(result, Err(Error::Io(error)) if error.kind() == io::ErrorKind::TimedOut));
        assert_eq!(stream.0.borrow().calls, 0);
        assert!(matches!(
            pending.poll(Instant::now()),
            Err(Error::AlreadyClosed)
        ));
    }

    #[test]
    fn invalid_upgrade_is_terminal_without_repeating_peer_io() {
        let (mut pending, stream) = fixture(b"not an HTTP upgrade\r\n\r\n", usize::MAX, usize::MAX);
        assert!(poll(&mut pending, &stream).is_err());
        let calls = stream.0.borrow().calls;
        assert!(matches!(
            pending.poll(Instant::now()),
            Err(Error::AlreadyClosed)
        ));
        assert_eq!(stream.0.borrow().calls, calls);
    }
}
