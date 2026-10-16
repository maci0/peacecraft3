//! BNCS (`0xFF`) and W3GS (`0xF7`) header framing.
//!
//! The four-byte header is the framing contract. BNCS length is the
//! `t_bnet_header.size` field in pinned PvPGN `src/common/bnet_protocol.h`: a
//! little-endian count of the whole packet, including the header, and the
//! first stored type byte is `0xFF`. W3GS uses the same width. Pinned GHost++
//! `ghost/gameprotocol.h` commit `6ba90f426e96f80fe013a468d1e07151a510adbe`
//! defines `W3GS_HEADER_CONSTANT` as 247, and `AssignLength` writes the
//! packet's byte count into indices 2 and 3. Bodies stay opaque except
//! [`Frame::reject_join`] and [`auth_info`], which read join refusals and
//! source-profile-qualified authentication challenges.
//! A decoded reason is always a refusal. It is not admission and it is not
//! simulation state.
//!
//! Resource limits are caller-supplied. A declared length above that limit is
//! refused before any buffer is grown to the declared size. A decoding error
//! poisons the stream decoder; later `push` and `finish` calls repeat that error.

#![deny(missing_docs)]

use std::fmt;

/// BNCS marker. Little-endian storage of a PvPGN `bn_short` type such as `0x50ff`.
pub const BNCS_FAMILY: u8 = 0xFF;

/// GHost++ `W3GS_HEADER_CONSTANT`.
pub const W3GS_FAMILY: u8 = 247;

/// GHost++ `W3GS_REJECTJOIN` at commit `6ba90f426e96f80fe013a468d1e07151a510adbe`.
pub const W3GS_REJECTJOIN: u8 = 5;

/// GHost++ `REJECTJOIN_FULL` from that same `gameprotocol.h`.
pub const REJECTJOIN_FULL: u32 = 9;

/// GHost++ `REJECTJOIN_STARTED` from that same `gameprotocol.h`.
pub const REJECTJOIN_STARTED: u32 = 10;

/// GHost++ `REJECTJOIN_WRONGPASSWORD` from that same `gameprotocol.h`.
pub const REJECTJOIN_WRONGPASSWORD: u32 = 27;

/// Marker, identifier, and little-endian length.
pub const HEADER_LEN: usize = 4;

/// Caller-supplied ceilings. These are not protocol defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameLimits {
    /// Largest accepted packet, including its four-byte header.
    pub max_frame_bytes: usize,
    /// Largest number of frames one decoder will emit.
    pub max_frames: usize,
}

/// One accepted frame. The stored bytes include the four-byte header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedFrame {
    bytes: Vec<u8>,
}

impl OwnedFrame {
    fn from_accepted(bytes: Vec<u8>) -> Self {
        debug_assert!(bytes.len() >= HEADER_LEN);
        Self { bytes }
    }

    /// Header family byte.
    pub fn family(&self) -> u8 {
        self.bytes[0]
    }

    /// Header identifier byte.
    pub fn id(&self) -> u8 {
        self.bytes[1]
    }

    /// Header and body.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Bytes after the four-byte header.
    pub fn body(&self) -> &[u8] {
        &self.bytes[HEADER_LEN..]
    }

    /// Parse a challenge against the caller-selected product and versioncheck profile.
    pub fn server_auth_info(
        &self,
        variant: auth_info::AuthReplyVariant,
        profile: auth_info::CheckRevisionProfile<'_>,
        limits: auth_info::AuthInfoLimits,
    ) -> Result<auth_info::ServerAuthInfo<'_>, auth_info::AuthInfoError> {
        auth_info::parse(
            self.family(),
            self.id(),
            self.body(),
            variant,
            profile,
            limits,
        )
    }

    /// Read a W3GS reject-join reason from this frame.
    ///
    /// [`Ok`] is a refusal. This method does not admit a player.
    pub fn reject_join(&self) -> Result<RejectJoin, RejectJoinError> {
        classify_reject_join(self.family(), self.id(), self.body())
    }
}

/// A frame borrowed from the caller's buffer. Constructing one does not allocate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame<'a> {
    bytes: &'a [u8],
}

impl<'a> Frame<'a> {
    fn from_accepted(bytes: &'a [u8]) -> Self {
        debug_assert!(bytes.len() >= HEADER_LEN);
        Self { bytes }
    }

    /// Header family byte.
    pub fn family(self) -> u8 {
        self.bytes[0]
    }

    /// Header identifier byte.
    pub fn id(self) -> u8 {
        self.bytes[1]
    }

    /// Header and body.
    pub fn bytes(self) -> &'a [u8] {
        self.bytes
    }

    /// Bytes after the four-byte header.
    pub fn body(self) -> &'a [u8] {
        &self.bytes[HEADER_LEN..]
    }

    /// Parse a challenge against the caller-selected product and versioncheck profile.
    pub fn server_auth_info(
        self,
        variant: auth_info::AuthReplyVariant,
        profile: auth_info::CheckRevisionProfile<'_>,
        limits: auth_info::AuthInfoLimits,
    ) -> Result<auth_info::ServerAuthInfo<'a>, auth_info::AuthInfoError> {
        auth_info::parse(
            self.family(),
            self.id(),
            self.body(),
            variant,
            profile,
            limits,
        )
    }

    /// Read a W3GS reject-join reason from this frame.
    ///
    /// [`Ok`] is a refusal. This method does not admit a player.
    pub fn reject_join(self) -> Result<RejectJoin, RejectJoinError> {
        classify_reject_join(self.family(), self.id(), self.body())
    }
}

/// Framing failure. After a [`Decoder`] returns one of these, that same value
/// is sticky.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameError {
    /// `max_frame_bytes` is below 4 or `max_frames` is zero.
    Limit,
    /// `finish` saw a partial header or body.
    Incomplete {
        /// Bytes retained before the stream was poisoned.
        have: usize,
    },
    /// The length field is shorter than the header.
    LengthTooSmall {
        /// Value of the little-endian length field.
        declared: usize,
    },
    /// The length field exceeds the caller limit or `u16`.
    Oversize {
        /// Value that was refused.
        declared: usize,
        /// Ceiling that was in force.
        limit: usize,
    },
    /// First header byte is neither [`BNCS_FAMILY`] nor [`W3GS_FAMILY`].
    UnknownFamily(u8),
    /// [`decode_one`] input continues past the declared packet.
    Trailing {
        /// Bytes after the declared packet.
        remaining: usize,
    },
    /// Another frame arrived after [`FrameLimits::max_frames`].
    FrameCount {
        /// The configured ceiling.
        limit: usize,
    },
    /// `try_reserve` failed before copying untrusted bytes.
    Allocation,
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Limit => write!(f, "frame limits must allow a 4-byte header and one frame"),
            Self::Incomplete { have } => {
                write!(f, "frame is incomplete after {have} buffered bytes")
            }
            Self::LengthTooSmall { declared } => {
                write!(
                    f,
                    "declared length {declared} is shorter than the 4-byte header"
                )
            }
            Self::Oversize { declared, limit } => {
                write!(
                    f,
                    "declared length {declared} exceeds the {limit}-byte frame limit"
                )
            }
            Self::UnknownFamily(family) => write!(f, "unknown header family {family}"),
            Self::Trailing { remaining } => {
                write!(f, "trailing bytes after a well-formed frame: {remaining}")
            }
            Self::FrameCount { limit } => write!(f, "frame count exceeds the limit of {limit}"),
            Self::Allocation => write!(f, "frame buffer allocation failed"),
        }
    }
}

impl std::error::Error for FrameError {}

/// A decoded W3GS reject-join reason.
///
/// Every variant refuses the join. GHost++ `SEND_W3GS_REJECTJOIN` appends one
/// `uint32_t` through `UTIL_AppendByteArray(packet, reason, false)`. At commit
/// `6ba90f426e96f80fe013a468d1e07151a510adbe`, `false` keeps the low byte first
/// from `UTIL_CreateByteArray`, so the reason is little-endian.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectJoin {
    /// `REJECTJOIN_FULL`.
    Full,
    /// `REJECTJOIN_STARTED`.
    Started,
    /// `REJECTJOIN_WRONGPASSWORD`.
    WrongPassword,
    /// A reason other than the three named constants, retained raw.
    Unknown(u32),
}

/// Why a frame is not a W3GS reject-join refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectJoinError {
    /// Header family is not [`W3GS_FAMILY`].
    WrongFamily(u8),
    /// Header id is not [`W3GS_REJECTJOIN`].
    WrongId(u8),
    /// Body is not the four-byte reason.
    BodyLength {
        /// Body length that was refused.
        len: usize,
    },
}

impl fmt::Display for RejectJoinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongFamily(family) => write!(f, "reject-join family {family} is not W3GS"),
            Self::WrongId(id) => write!(f, "reject-join id {id} is not W3GS_REJECTJOIN"),
            Self::BodyLength { len } => {
                write!(f, "reject-join body is {len} bytes, not the 4-byte reason")
            }
        }
    }
}

impl std::error::Error for RejectJoinError {}

fn classify_reject_join(family: u8, id: u8, body: &[u8]) -> Result<RejectJoin, RejectJoinError> {
    if family != W3GS_FAMILY {
        return Err(RejectJoinError::WrongFamily(family));
    }
    if id != W3GS_REJECTJOIN {
        return Err(RejectJoinError::WrongId(id));
    }
    let Ok(bytes) = <[u8; 4]>::try_from(body) else {
        return Err(RejectJoinError::BodyLength { len: body.len() });
    };
    Ok(match u32::from_le_bytes(bytes) {
        REJECTJOIN_FULL => RejectJoin::Full,
        REJECTJOIN_STARTED => RejectJoin::Started,
        REJECTJOIN_WRONGPASSWORD => RejectJoin::WrongPassword,
        raw => RejectJoin::Unknown(raw),
    })
}

/// Streaming decoder.
///
/// The buffer is never resized to an untrusted declared length. Any error
/// clears the buffer and becomes terminal: later [`Decoder::push`] and
/// [`Decoder::finish`] calls return that same error.
#[derive(Debug)]
pub struct Decoder {
    buf: Vec<u8>,
    limits: FrameLimits,
    emitted: usize,
    poison: Option<FrameError>,
}

impl Decoder {
    /// Empty decoder. Fails before allocating when the limits cannot hold one header.
    pub fn new(limits: FrameLimits) -> Result<Self, FrameError> {
        if limits.max_frame_bytes < HEADER_LEN || limits.max_frames == 0 {
            return Err(FrameError::Limit);
        }
        Ok(Self {
            buf: Vec::new(),
            limits,
            emitted: 0,
            poison: None,
        })
    }

    /// Bytes waiting for a complete frame. Zero after the decoder is poisoned.
    pub fn buffered_len(&self) -> usize {
        self.buf.len()
    }

    /// Current allocation capacity of the stream buffer.
    pub fn capacity(&self) -> usize {
        self.buf.capacity()
    }

    /// Accept `chunk` and return only frames completed by this call.
    ///
    /// A later malformed header or frame-count overflow in the same chunk
    /// fails the whole call. Frames parsed earlier in that chunk are not
    /// returned, and the decoder will not resume.
    pub fn push(&mut self, chunk: &[u8]) -> Result<Vec<OwnedFrame>, FrameError> {
        if let Some(error) = self.poison.clone() {
            return Err(error);
        }
        match self.push_inner(chunk) {
            Ok(frames) => Ok(frames),
            Err(error) => Err(self.fail(error)),
        }
    }

    /// End of the byte stream.
    ///
    /// An empty healthy buffer is success. A partial buffer, or any earlier
    /// error, is a sticky failure.
    pub fn finish(&mut self) -> Result<(), FrameError> {
        if let Some(error) = self.poison.clone() {
            return Err(error);
        }
        if self.buf.is_empty() {
            Ok(())
        } else {
            let have = self.buf.len();
            Err(self.fail(FrameError::Incomplete { have }))
        }
    }

    fn fail(&mut self, error: FrameError) -> FrameError {
        self.buf.clear();
        self.poison.get_or_insert(error).clone()
    }

    fn push_inner(&mut self, mut chunk: &[u8]) -> Result<Vec<OwnedFrame>, FrameError> {
        let mut out = Vec::new();
        loop {
            if self.buf.len() >= HEADER_LEN {
                let declared = self.validated_length()?;
                if self.buf.len() >= declared {
                    if self.emitted >= self.limits.max_frames {
                        return Err(FrameError::FrameCount {
                            limit: self.limits.max_frames,
                        });
                    }
                    out.try_reserve(1).map_err(|_| FrameError::Allocation)?;
                    out.push(OwnedFrame::from_accepted(std::mem::take(&mut self.buf)));
                    self.emitted += 1;
                    continue;
                }
            }
            if chunk.is_empty() {
                break;
            }
            if self.emitted >= self.limits.max_frames {
                return Err(FrameError::FrameCount {
                    limit: self.limits.max_frames,
                });
            }
            let need = if self.buf.len() < HEADER_LEN {
                HEADER_LEN - self.buf.len()
            } else {
                let declared = self.validated_length()?;
                declared - self.buf.len()
            };
            let room = self.limits.max_frame_bytes.saturating_sub(self.buf.len());
            if room == 0 {
                let declared = u16::from_le_bytes([self.buf[2], self.buf[3]]) as usize;
                return Err(FrameError::Oversize {
                    declared,
                    limit: self.limits.max_frame_bytes,
                });
            }
            let take = need.min(chunk.len()).min(room);
            if take == 0 {
                break;
            }
            self.buf
                .try_reserve(take)
                .map_err(|_| FrameError::Allocation)?;
            self.buf.extend_from_slice(&chunk[..take]);
            chunk = &chunk[take..];
            if self.buf.len() >= HEADER_LEN {
                self.validated_length()?;
            }
        }
        Ok(out)
    }

    fn validated_length(&self) -> Result<usize, FrameError> {
        validate_header(&self.buf[..HEADER_LEN], self.limits.max_frame_bytes)
    }
}

/// Decode one frame that occupies the whole slice. No body allocation.
pub fn decode_one(input: &[u8], max_frame_bytes: usize) -> Result<Frame<'_>, FrameError> {
    if max_frame_bytes < HEADER_LEN {
        return Err(FrameError::Limit);
    }
    if input.len() < HEADER_LEN {
        return Err(FrameError::Incomplete { have: input.len() });
    }
    let declared = validate_header(&input[..HEADER_LEN], max_frame_bytes)?;
    if input.len() < declared {
        return Err(FrameError::Incomplete { have: input.len() });
    }
    if input.len() != declared {
        return Err(FrameError::Trailing {
            remaining: input.len() - declared,
        });
    }
    Ok(Frame::from_accepted(input))
}

/// Encode a frame from caller-owned body bytes.
///
/// The output buffer is reserved exactly once for the checked size.
pub fn encode(family: u8, id: u8, body: &[u8]) -> Result<Vec<u8>, FrameError> {
    if family != BNCS_FAMILY && family != W3GS_FAMILY {
        return Err(FrameError::UnknownFamily(family));
    }
    let total = HEADER_LEN
        .checked_add(body.len())
        .ok_or(FrameError::Oversize {
            declared: usize::MAX,
            limit: u16::MAX as usize,
        })?;
    let len = u16::try_from(total).map_err(|_| FrameError::Oversize {
        declared: total,
        limit: u16::MAX as usize,
    })?;
    let mut out = Vec::new();
    out.try_reserve_exact(total)
        .map_err(|_| FrameError::Allocation)?;
    out.push(family);
    out.push(id);
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(body);
    Ok(out)
}

fn validate_header(header: &[u8], max_frame_bytes: usize) -> Result<usize, FrameError> {
    let family = header[0];
    if family != BNCS_FAMILY && family != W3GS_FAMILY {
        return Err(FrameError::UnknownFamily(family));
    }
    let declared = u16::from_le_bytes([header[2], header[3]]) as usize;
    if declared < HEADER_LEN {
        return Err(FrameError::LengthTooSmall { declared });
    }
    if declared > max_frame_bytes {
        return Err(FrameError::Oversize {
            declared,
            limit: max_frame_bytes,
        });
    }
    Ok(declared)
}

#[cfg(not(target_arch = "wasm32"))]
pub mod tcp;

pub mod auth_info;

#[cfg(test)]
mod tests {
    use super::*;

    fn limits(max_frame_bytes: usize, max_frames: usize) -> FrameLimits {
        FrameLimits {
            max_frame_bytes,
            max_frames,
        }
    }

    #[test]
    fn length_below_header_is_refused_before_growth() {
        let mut decoder = Decoder::new(limits(64, 2)).unwrap();
        let error = decoder.push(&[BNCS_FAMILY, 0x50, 0x03, 0x00]).unwrap_err();
        assert_eq!(error, FrameError::LengthTooSmall { declared: 3 });
        assert_eq!(decoder.buffered_len(), 0);
        assert!(decoder.capacity() < 64);
        assert_eq!(decoder.finish().unwrap_err(), error);
        assert_eq!(decoder.push(&[]).unwrap_err(), error);
    }

    #[test]
    fn oversize_declaration_does_not_allocate_that_length() {
        let mut decoder = Decoder::new(limits(32, 2)).unwrap();
        let declared = 4_096usize;
        let header = [
            BNCS_FAMILY,
            0x50,
            (declared as u16).to_le_bytes()[0],
            (declared as u16).to_le_bytes()[1],
        ];
        let error = decoder.push(&header).unwrap_err();
        assert_eq!(
            error,
            FrameError::Oversize {
                declared,
                limit: 32
            }
        );
        assert_eq!(decoder.buffered_len(), 0);
        assert!(decoder.capacity() < declared);
        assert_eq!(decoder.finish().unwrap_err(), error);
    }

    #[test]
    fn slice_oversize_returns_before_a_frame_borrow() {
        let input = [W3GS_FAMILY, 1, 0x00, 0x10];
        let error = decode_one(&input, 32).unwrap_err();
        assert_eq!(
            error,
            FrameError::Oversize {
                declared: 0x1000,
                limit: 32
            }
        );
    }

    #[test]
    fn unknown_family_trailing_and_count_are_refused() {
        let error = decode_one(&[0x01, 0x00, 0x04, 0x00], 32).unwrap_err();
        assert_eq!(error, FrameError::UnknownFamily(0x01));

        let mut padded = encode(W3GS_FAMILY, 1, &[]).unwrap();
        padded.push(0);
        let error = decode_one(&padded, 32).unwrap_err();
        assert_eq!(error, FrameError::Trailing { remaining: 1 });

        let mut decoder = Decoder::new(limits(16, 1)).unwrap();
        let one = encode(W3GS_FAMILY, 1, &[]).unwrap();
        let got = decoder.push(&one).unwrap();
        assert_eq!(got[0].id(), 1);
        let error = decoder.push(&one).unwrap_err();
        assert_eq!(error, FrameError::FrameCount { limit: 1 });
        assert_eq!(decoder.finish().unwrap_err(), error);
    }

    #[test]
    fn fragmented_and_coalesced_w3gs_headers() {
        let frame = encode(W3GS_FAMILY, 1, &[]).unwrap();
        assert_eq!(frame, [W3GS_FAMILY, 1, 4, 0]);
        let mut decoder = Decoder::new(limits(16, 2)).unwrap();
        for (index, byte) in frame.iter().enumerate() {
            let got = decoder.push(&[*byte]).unwrap();
            if index + 1 == frame.len() {
                assert_eq!(got.len(), 1);
                assert_eq!(got[0].id(), 1);
                assert_eq!(got[0].family(), W3GS_FAMILY);
                assert_eq!(got[0].bytes(), frame.as_slice());
                assert!(got[0].body().is_empty());
            } else {
                assert!(got.is_empty());
            }
        }
        decoder.finish().unwrap();

        let mut coalesced = frame.clone();
        coalesced.extend_from_slice(&frame);
        let mut decoder = Decoder::new(limits(16, 2)).unwrap();
        let got = decoder.push(&coalesced).unwrap();
        assert_eq!(got.len(), 2);
        assert!(got.iter().all(|frame| frame.family() == W3GS_FAMILY));
        decoder.finish().unwrap();
    }

    #[test]
    fn partial_body_stays_incomplete_until_finish() {
        let mut decoder = Decoder::new(limits(16, 1)).unwrap();
        let got = decoder
            .push(&[BNCS_FAMILY, 0x25, 0x06, 0x00, 0xAA])
            .unwrap();
        assert!(got.is_empty());
        let error = decoder.finish().unwrap_err();
        assert_eq!(error, FrameError::Incomplete { have: 5 });
        assert_eq!(decoder.buffered_len(), 0);
        assert_eq!(decoder.finish().unwrap_err(), error);
        assert_eq!(decoder.push(&[0xBB]).unwrap_err(), error);
    }

    #[test]
    fn coalesced_malformed_tail_poisons_without_finish_success() {
        let good = encode(W3GS_FAMILY, 1, &[]).unwrap();
        let mut bytes = good.clone();
        bytes.extend_from_slice(&[0x01, 0x00, 0x04, 0x00]);
        let mut decoder = Decoder::new(limits(32, 2)).unwrap();
        let error = decoder.push(&bytes).unwrap_err();
        assert_eq!(error, FrameError::UnknownFamily(0x01));
        assert_eq!(decoder.buffered_len(), 0);
        assert_eq!(decoder.finish().unwrap_err(), error);
        assert_eq!(decoder.push(&good).unwrap_err(), error);
    }

    #[test]
    fn fragmented_length_error_poisons() {
        let header = [BNCS_FAMILY, 0x50, 0x03, 0x00];
        let mut decoder = Decoder::new(limits(32, 2)).unwrap();
        assert!(decoder.push(&header[..3]).unwrap().is_empty());
        let error = decoder.push(&header[3..]).unwrap_err();
        assert_eq!(error, FrameError::LengthTooSmall { declared: 3 });
        assert_eq!(decoder.finish().unwrap_err(), error);
        assert_eq!(decoder.push(&header).unwrap_err(), error);
    }

    #[test]
    fn coalesced_count_overflow_poisons() {
        let one = encode(BNCS_FAMILY, 0x25, &[1, 2, 3, 4]).unwrap();
        let mut both = one.clone();
        both.extend_from_slice(&one);
        let mut decoder = Decoder::new(limits(32, 1)).unwrap();
        let error = decoder.push(&both).unwrap_err();
        assert_eq!(error, FrameError::FrameCount { limit: 1 });
        assert_eq!(decoder.buffered_len(), 0);
        assert_eq!(decoder.finish().unwrap_err(), error);
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn frames_from_decoded_caller(bytes: &[u8], max_frames: usize) -> Vec<OwnedFrame> {
        use std::io::{self, Read, Write};
        use std::time::{Duration, Instant};

        struct Reply {
            data: Vec<u8>,
            pos: usize,
        }

        impl Read for Reply {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                let rest = &self.data[self.pos..];
                let count = rest.len().min(buf.len());
                buf[..count].copy_from_slice(&rest[..count]);
                self.pos += count;
                Ok(count)
            }
        }

        impl Write for Reply {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                Ok(buf.len())
            }

            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let mut stream = Reply {
            data: bytes.to_vec(),
            pos: 0,
        };
        let policy = super::tcp::CallPolicy {
            deadline: Instant::now() + Duration::from_secs(2),
            connect_timeout: Duration::from_secs(1),
            max_write_bytes: 1,
            max_read_bytes: bytes.len().max(1),
            frames: limits(64, max_frames),
        };
        super::tcp::exchange_io(&mut stream, b"", &policy, |_stream, timeout| {
            assert!(!timeout.is_zero());
            Ok(())
        })
        .unwrap_or_else(|error| panic!("exchange_io failed: {error}"))
    }

    #[cfg(target_arch = "wasm32")]
    fn frames_from_decoded_caller(bytes: &[u8], max_frames: usize) -> Vec<OwnedFrame> {
        let mut decoder = Decoder::new(limits(64, max_frames)).unwrap();
        let frames = decoder.push(bytes).unwrap();
        decoder.finish().unwrap();
        frames
    }

    #[test]
    fn decoded_exchange_reject_join_refuses_named_and_unknown_reasons() {
        let named = [
            (REJECTJOIN_FULL, RejectJoin::Full),
            (REJECTJOIN_STARTED, RejectJoin::Started),
            (REJECTJOIN_WRONGPASSWORD, RejectJoin::WrongPassword),
        ];
        let mut packets = Vec::new();
        for (reason, _) in named {
            packets.push(encode(W3GS_FAMILY, W3GS_REJECTJOIN, &reason.to_le_bytes()).unwrap());
        }
        packets.push(encode(W3GS_FAMILY, W3GS_REJECTJOIN, &1u32.to_le_bytes()).unwrap());
        packets.push(encode(W3GS_FAMILY, W3GS_REJECTJOIN, &0u32.to_le_bytes()).unwrap());
        packets.push(encode(BNCS_FAMILY, W3GS_REJECTJOIN, &REJECTJOIN_FULL.to_le_bytes()).unwrap());
        // Pinned `W3GS_SLOTINFOJOIN` is 4. That frame is not admission here.
        packets.push(encode(W3GS_FAMILY, 4, &REJECTJOIN_FULL.to_le_bytes()).unwrap());
        packets.push(encode(W3GS_FAMILY, W3GS_REJECTJOIN, &[9, 0, 0]).unwrap());
        packets.push(encode(W3GS_FAMILY, W3GS_REJECTJOIN, &[9, 0, 0, 0, 1]).unwrap());

        let mut stream = Vec::new();
        for packet in &packets {
            stream.extend_from_slice(packet);
        }
        let frames = frames_from_decoded_caller(&stream, packets.len());
        assert_eq!(frames.len(), packets.len());

        for (frame, (_, expected)) in frames.iter().zip(named) {
            assert_eq!(frame.reject_join(), Ok(expected));
        }
        assert_eq!(frames[3].reject_join(), Ok(RejectJoin::Unknown(1)));
        assert_eq!(frames[4].reject_join(), Ok(RejectJoin::Unknown(0)));
        assert_eq!(
            frames[5].reject_join(),
            Err(RejectJoinError::WrongFamily(BNCS_FAMILY))
        );
        assert_eq!(frames[6].reject_join(), Err(RejectJoinError::WrongId(4)));
        assert_eq!(
            frames[7].reject_join(),
            Err(RejectJoinError::BodyLength { len: 3 })
        );
        assert_eq!(
            frames[8].reject_join(),
            Err(RejectJoinError::BodyLength { len: 5 })
        );
    }

    #[test]
    fn encode_rejects_a_length_that_does_not_fit_u16() {
        let body = vec![0u8; usize::from(u16::MAX)];
        let error = encode(BNCS_FAMILY, 1, &body).unwrap_err();
        assert_eq!(
            error,
            FrameError::Oversize {
                declared: usize::from(u16::MAX) + HEADER_LEN,
                limit: usize::from(u16::MAX),
            }
        );
    }
}
