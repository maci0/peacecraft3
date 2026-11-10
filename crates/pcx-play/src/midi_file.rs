//! Bounded Standard MIDI File (SMF) parsing: header, `MTrk` chunks, and typed events.
//!
//! Parsing only: no synthesis or playback. Events borrow from the input bytes. Chunks other
//! than `MTrk` are skipped, as the SMF specification requires. Every track must end with an
//! end-of-track meta event, and no bytes may follow it. Sysex and meta events cancel running
//! status, as the specification requires.

const HEADER_ID: [u8; 4] = *b"MThd";
const TRACK_ID: [u8; 4] = *b"MTrk";
const HEADER_MIN_LEN: usize = 6;
const CHUNK_HEADER_LEN: usize = 8;
const VLQ_MAX_BYTES: usize = 4;
/// Division high bit: the value is SMPTE frames per second and ticks per frame.
const SMPTE_DIVISION_FLAG: u16 = 0x8000;
const SMPTE_FRAME_RATES: [u8; 4] = [24, 25, 29, 30];

const NOTE_OFF: u8 = 0x80;
const NOTE_ON: u8 = 0x90;
const POLY_PRESSURE: u8 = 0xA0;
const CONTROL_CHANGE: u8 = 0xB0;
const PROGRAM_CHANGE: u8 = 0xC0;
const CHANNEL_PRESSURE: u8 = 0xD0;
const PITCH_BEND: u8 = 0xE0;
/// Last channel status byte: the low nibble is the channel.
const CHANNEL_STATUS_LAST: u8 = PITCH_BEND | 0x0F;
const SYSEX_START: u8 = 0xF0;
const SYSEX_ESCAPE: u8 = 0xF7;
const META_PREFIX: u8 = 0xFF;

const META_END_OF_TRACK: u8 = 0x2F;

#[derive(Debug, thiserror::Error)]
#[error("MIDI file at byte {at}: {reason}")]
pub struct MidiError {
    pub at: usize,
    pub reason: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    SingleTrack,
    Simultaneous,
    Sequential,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Division {
    TicksPerQuarter(u16),
    Smpte {
        frames_per_second: u8,
        ticks_per_frame: u8,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MidiFile<'a> {
    pub format: Format,
    pub division: Division,
    pub tracks: Vec<Track<'a>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track<'a> {
    pub events: Vec<TrackEvent<'a>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrackEvent<'a> {
    pub delta_ticks: u32,
    pub event: Event<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event<'a> {
    Channel {
        channel: u8,
        message: ChannelMessage,
    },
    Meta(Meta<'a>),
    /// `F0` payload, without the length.
    SysEx(&'a [u8]),
    /// `F7` payload, without the length.
    Escape(&'a [u8]),
}

/// A channel voice message. A `NoteOn` with velocity 0 stays a `NoteOn`; the file does not
/// normalize it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelMessage {
    NoteOff {
        key: u8,
        velocity: u8,
    },
    NoteOn {
        key: u8,
        velocity: u8,
    },
    PolyPressure {
        key: u8,
        pressure: u8,
    },
    ControlChange {
        controller: u8,
        value: u8,
    },
    ProgramChange {
        program: u8,
    },
    ChannelPressure {
        pressure: u8,
    },
    /// 14-bit value, LSB first in the file, combined here.
    PitchBend {
        value: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextKind {
    Text,
    Copyright,
    TrackName,
    InstrumentName,
    Lyric,
    Marker,
    CuePoint,
    ProgramName,
    DeviceName,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Meta<'a> {
    /// `None` when the file gives the zero-length form.
    SequenceNumber(Option<u16>),
    Text {
        kind: TextKind,
        bytes: &'a [u8],
    },
    ChannelPrefix(u8),
    Port(u8),
    EndOfTrack,
    /// Microseconds per quarter note.
    SetTempo {
        micros_per_quarter: u32,
    },
    /// Raw `hr mn se fr ff` bytes. The hours byte carries the frame-rate bits.
    SmpteOffset([u8; 5]),
    TimeSignature {
        numerator: u8,
        denominator_power: u8,
        clocks_per_click: u8,
        thirty_seconds_per_quarter: u8,
    },
    KeySignature {
        /// Negative counts flats, positive counts sharps.
        sharps_flats: i8,
        minor: bool,
    },
    SequencerSpecific(&'a [u8]),
    /// A meta type this parser does not name. The spec requires readers to skip it.
    Unknown {
        kind: u8,
        bytes: &'a [u8],
    },
}

impl<'a> MidiFile<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, MidiError> {
        let bad = |at, reason| MidiError { at, reason };
        let (tag, header, mut at) = read_chunk(bytes, 0)?;
        if tag != HEADER_ID || header.len() < HEADER_MIN_LEN {
            return Err(bad(0, "expected MThd header of at least six bytes"));
        }
        let format = match u16::from_be_bytes([header[0], header[1]]) {
            0 => Format::SingleTrack,
            1 => Format::Simultaneous,
            2 => Format::Sequential,
            _ => return Err(bad(CHUNK_HEADER_LEN, "unknown SMF format")),
        };
        let declared_tracks = u16::from_be_bytes([header[2], header[3]]);
        if format == Format::SingleTrack && declared_tracks != 1 {
            return Err(bad(CHUNK_HEADER_LEN + 2, "format 0 must declare one track"));
        }
        let division = parse_division(u16::from_be_bytes([header[4], header[5]]))
            .map_err(|reason| bad(CHUNK_HEADER_LEN + 4, reason))?;

        let mut tracks = Vec::new();
        while at < bytes.len() {
            let (tag, body, next) = read_chunk(bytes, at)?;
            if tag == TRACK_ID {
                tracks.push(parse_track(body, at + CHUNK_HEADER_LEN)?);
            }
            at = next;
        }
        if tracks.len() != usize::from(declared_tracks) {
            return Err(bad(
                at,
                "track chunk count differs from the header track count",
            ));
        }
        Ok(Self {
            format,
            division,
            tracks,
        })
    }
}

fn parse_division(raw: u16) -> Result<Division, &'static str> {
    if raw & SMPTE_DIVISION_FLAG == 0 {
        if raw == 0 {
            return Err("ticks per quarter note is zero");
        }
        return Ok(Division::TicksPerQuarter(raw));
    }
    // The high byte is the negated frame rate in two's complement.
    let frames_per_second = (raw >> 8) as u8 as i8;
    let frames_per_second = frames_per_second.unsigned_abs();
    let ticks_per_frame = (raw & 0xFF) as u8;
    if !SMPTE_FRAME_RATES.contains(&frames_per_second) {
        return Err("unsupported SMPTE frame rate");
    }
    if ticks_per_frame == 0 {
        return Err("ticks per frame is zero");
    }
    Ok(Division::Smpte {
        frames_per_second,
        ticks_per_frame,
    })
}

/// Reads the chunk at `at`: its tag, its body, and the offset just past it.
fn read_chunk(bytes: &[u8], at: usize) -> Result<([u8; 4], &[u8], usize), MidiError> {
    let bad = |reason| MidiError { at, reason };
    let rest = bytes.get(at..).unwrap_or_default();
    let header = rest
        .get(..CHUNK_HEADER_LEN)
        .ok_or_else(|| bad("truncated chunk header"))?;
    let tag: [u8; 4] = header[..4].try_into().unwrap();
    let len = u32::from_be_bytes(header[4..8].try_into().unwrap()) as usize;
    let body_end = CHUNK_HEADER_LEN
        .checked_add(len)
        .filter(|&n| n <= rest.len())
        .ok_or_else(|| bad("chunk overruns the file"))?;
    Ok((tag, &rest[CHUNK_HEADER_LEN..body_end], at + body_end))
}

fn parse_track(body: &[u8], body_at: usize) -> Result<Track<'_>, MidiError> {
    let err = |pos: usize, reason| MidiError {
        at: body_at + pos,
        reason,
    };
    let mut events = Vec::new();
    let mut at = 0usize;
    let mut running: Option<u8> = None;
    let mut ended = false;
    while at < body.len() {
        if ended {
            return Err(err(at, "bytes after end-of-track"));
        }
        let (delta_ticks, after_delta) = read_vlq(body, at).map_err(|reason| err(at, reason))?;
        let first = *body
            .get(after_delta)
            .ok_or_else(|| err(after_delta, "truncated event"))?;
        let (status, mut pos) = if first & 0x80 != 0 {
            (first, after_delta + 1)
        } else {
            let status =
                running.ok_or_else(|| err(after_delta, "data byte without running status"))?;
            (status, after_delta)
        };
        let event = match status {
            SYSEX_START | SYSEX_ESCAPE => {
                running = None;
                let (len, data_at) = read_length(body, pos).map_err(|reason| err(pos, reason))?;
                let data = take(body, data_at, len)
                    .ok_or_else(|| err(data_at, "sysex payload overruns the track"))?;
                pos = data_at + len;
                if status == SYSEX_START {
                    Event::SysEx(data)
                } else {
                    Event::Escape(data)
                }
            }
            META_PREFIX => {
                running = None;
                let kind = *body
                    .get(pos)
                    .ok_or_else(|| err(pos, "truncated meta event"))?;
                let (len, data_at) =
                    read_length(body, pos + 1).map_err(|reason| err(pos, reason))?;
                let data = take(body, data_at, len)
                    .ok_or_else(|| err(data_at, "meta payload overruns the track"))?;
                pos = data_at + len;
                if kind == META_END_OF_TRACK {
                    ended = true;
                }
                Event::Meta(parse_meta(kind, data).map_err(|reason| err(data_at, reason))?)
            }
            NOTE_OFF..=CHANNEL_STATUS_LAST => {
                running = Some(status);
                let data_len = match status & 0xF0 {
                    PROGRAM_CHANGE | CHANNEL_PRESSURE => 1,
                    _ => 2,
                };
                let data = take(body, pos, data_len)
                    .ok_or_else(|| err(pos, "truncated channel message"))?;
                if data.iter().any(|&b| b & 0x80 != 0) {
                    return Err(err(pos, "channel data byte has its high bit set"));
                }
                pos += data_len;
                Event::Channel {
                    channel: status & 0x0F,
                    message: channel_message(status & 0xF0, data),
                }
            }
            _ => return Err(err(at, "unsupported status byte")),
        };
        events.push(TrackEvent { delta_ticks, event });
        at = pos;
    }
    if !ended {
        return Err(err(body.len(), "track ends without end-of-track"));
    }
    Ok(Track { events })
}

/// Maps a status high nibble and its validated data bytes to a message.
fn channel_message(kind: u8, data: &[u8]) -> ChannelMessage {
    match kind {
        NOTE_OFF => ChannelMessage::NoteOff {
            key: data[0],
            velocity: data[1],
        },
        NOTE_ON => ChannelMessage::NoteOn {
            key: data[0],
            velocity: data[1],
        },
        POLY_PRESSURE => ChannelMessage::PolyPressure {
            key: data[0],
            pressure: data[1],
        },
        CONTROL_CHANGE => ChannelMessage::ControlChange {
            controller: data[0],
            value: data[1],
        },
        PROGRAM_CHANGE => ChannelMessage::ProgramChange { program: data[0] },
        CHANNEL_PRESSURE => ChannelMessage::ChannelPressure { pressure: data[0] },
        _ => ChannelMessage::PitchBend {
            value: u16::from(data[0]) | (u16::from(data[1]) << 7),
        },
    }
}

fn parse_meta(kind: u8, data: &[u8]) -> Result<Meta<'_>, &'static str> {
    Ok(match kind {
        0x00 => match data {
            [] => Meta::SequenceNumber(None),
            [hi, lo] => Meta::SequenceNumber(Some(u16::from_be_bytes([*hi, *lo]))),
            _ => return Err("sequence number is not zero or two bytes"),
        },
        0x01..=0x09 => Meta::Text {
            kind: text_kind(kind),
            bytes: data,
        },
        0x20 => {
            let [channel] = exact(data).ok_or("channel prefix is not one byte")?;
            Meta::ChannelPrefix(channel)
        }
        0x21 => {
            let [port] = exact(data).ok_or("port is not one byte")?;
            Meta::Port(port)
        }
        META_END_OF_TRACK => {
            if !data.is_empty() {
                return Err("end-of-track carries data");
            }
            Meta::EndOfTrack
        }
        0x51 => {
            let [a, b, c] = exact(data).ok_or("set tempo is not three bytes")?;
            Meta::SetTempo {
                micros_per_quarter: u32::from_be_bytes([0, a, b, c]),
            }
        }
        0x54 => Meta::SmpteOffset(exact(data).ok_or("SMPTE offset is not five bytes")?),
        0x58 => {
            let [
                numerator,
                denominator_power,
                clocks_per_click,
                thirty_seconds_per_quarter,
            ] = exact(data).ok_or("time signature is not four bytes")?;
            Meta::TimeSignature {
                numerator,
                denominator_power,
                clocks_per_click,
                thirty_seconds_per_quarter,
            }
        }
        0x59 => {
            let [sharps_flats, minor] = exact(data).ok_or("key signature is not two bytes")?;
            if minor > 1 {
                return Err("key signature mode is neither major nor minor");
            }
            Meta::KeySignature {
                sharps_flats: sharps_flats as i8,
                minor: minor == 1,
            }
        }
        0x7F => Meta::SequencerSpecific(data),
        _ => Meta::Unknown { kind, bytes: data },
    })
}

fn text_kind(kind: u8) -> TextKind {
    match kind {
        0x01 => TextKind::Text,
        0x02 => TextKind::Copyright,
        0x03 => TextKind::TrackName,
        0x04 => TextKind::InstrumentName,
        0x05 => TextKind::Lyric,
        0x06 => TextKind::Marker,
        0x07 => TextKind::CuePoint,
        0x08 => TextKind::ProgramName,
        _ => TextKind::DeviceName,
    }
}

fn exact<const N: usize>(data: &[u8]) -> Option<[u8; N]> {
    data.try_into().ok()
}

fn take(body: &[u8], at: usize, len: usize) -> Option<&[u8]> {
    body.get(at..at.checked_add(len)?)
}

/// Reads a length field (at most four VLQ bytes) as a `usize`.
fn read_length(body: &[u8], at: usize) -> Result<(usize, usize), &'static str> {
    let (len, next) = read_vlq(body, at)?;
    Ok((len as usize, next))
}

/// Reads a variable-length quantity of at most four bytes, returning the value and the
/// offset just past it.
fn read_vlq(body: &[u8], at: usize) -> Result<(u32, usize), &'static str> {
    let mut value = 0u32;
    for index in 0..VLQ_MAX_BYTES {
        let byte = *body
            .get(at + index)
            .ok_or("truncated variable-length quantity")?;
        value = (value << 7) | u32::from(byte & 0x7F);
        if byte & 0x80 == 0 {
            return Ok((value, at + index + 1));
        }
    }
    Err("variable-length quantity longer than four bytes")
}

#[cfg(test)]
mod tests {
    use super::*;

    const END_OF_TRACK: [u8; 4] = [0x00, 0xFF, 0x2F, 0x00];

    /// Builds an SMF from its parts. Lengths are big-endian, as the format requires.
    fn smf(
        format: u16,
        declared_tracks: u16,
        division: u16,
        chunks: &[(&[u8; 4], Vec<u8>)],
    ) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"MThd");
        out.extend_from_slice(&6u32.to_be_bytes());
        out.extend_from_slice(&format.to_be_bytes());
        out.extend_from_slice(&declared_tracks.to_be_bytes());
        out.extend_from_slice(&division.to_be_bytes());
        for (tag, body) in chunks {
            out.extend_from_slice(*tag);
            out.extend_from_slice(&(body.len() as u32).to_be_bytes());
            out.extend_from_slice(body);
        }
        out
    }

    /// A track body: `events` followed by a zero-delta end-of-track.
    fn track(events: &[u8]) -> Vec<u8> {
        let mut body = events.to_vec();
        body.extend_from_slice(&END_OF_TRACK);
        body
    }

    /// A format 0 file holding one track of `events`.
    fn one_track(events: &[u8]) -> Vec<u8> {
        smf(0, 1, 480, &[(b"MTrk", track(events))])
    }

    fn only_track_events(bytes: &[u8]) -> Vec<TrackEvent<'_>> {
        MidiFile::parse(bytes).unwrap().tracks.remove(0).events
    }

    fn reason(bytes: &[u8]) -> &'static str {
        MidiFile::parse(bytes).unwrap_err().reason
    }

    #[test]
    fn empty_and_truncated_headers_are_refused() {
        assert_eq!(reason(&[]), "truncated chunk header");
        assert_eq!(
            reason(b"MThd\x00\x00\x00\x06\x00\x00"),
            "chunk overruns the file"
        );
        assert_eq!(
            reason(b"MThd\x00\x00\x00\x05\x00\x00\x00\x01\x01\xE0"),
            "expected MThd header of at least six bytes"
        );
        assert_eq!(
            reason(b"RIFF\x00\x00\x00\x06\x00\x00\x00\x00\x00\x00"),
            "expected MThd header of at least six bytes"
        );
    }

    #[test]
    fn header_format_and_division_are_typed() {
        let bytes = smf(1, 2, 480, &[(b"MTrk", track(&[])), (b"MTrk", track(&[]))]);
        let file = MidiFile::parse(&bytes).unwrap();
        assert_eq!(file.format, Format::Simultaneous);
        assert_eq!(file.division, Division::TicksPerQuarter(480));
        assert_eq!(file.tracks.len(), 2);

        // 0xE7 is -25 frames per second, 0x19 is 25 ticks per frame.
        let bytes = smf(2, 1, 0xE719, &[(b"MTrk", track(&[]))]);
        let file = MidiFile::parse(&bytes).unwrap();
        assert_eq!(file.format, Format::Sequential);
        assert_eq!(
            file.division,
            Division::Smpte {
                frames_per_second: 25,
                ticks_per_frame: 0x19,
            }
        );
    }

    #[test]
    fn invalid_format_and_division_are_refused() {
        assert_eq!(
            reason(&smf(3, 1, 96, &[(b"MTrk", track(&[]))])),
            "unknown SMF format"
        );
        assert_eq!(
            reason(&smf(
                0,
                2,
                96,
                &[(b"MTrk", track(&[])), (b"MTrk", track(&[]))]
            )),
            "format 0 must declare one track"
        );
        assert_eq!(
            reason(&smf(0, 1, 0, &[(b"MTrk", track(&[]))])),
            "ticks per quarter note is zero"
        );
        // 0xE6 is -26, which is not an SMPTE frame rate.
        assert_eq!(
            reason(&smf(0, 1, 0xE619, &[(b"MTrk", track(&[]))])),
            "unsupported SMPTE frame rate"
        );
        assert_eq!(
            reason(&smf(0, 1, 0xE700, &[(b"MTrk", track(&[]))])),
            "ticks per frame is zero"
        );
    }

    #[test]
    fn track_count_must_match_header() {
        let missing = smf(1, 2, 96, &[(b"MTrk", track(&[]))]);
        assert_eq!(
            reason(&missing),
            "track chunk count differs from the header track count"
        );
        let extra = smf(1, 1, 96, &[(b"MTrk", track(&[])), (b"MTrk", track(&[]))]);
        assert_eq!(
            reason(&extra),
            "track chunk count differs from the header track count"
        );
        // Maximum declared count with no track data: refused without reading past the file.
        let declared_max = smf(1, u16::MAX, 96, &[(b"MTrk", track(&[]))]);
        assert_eq!(
            reason(&declared_max),
            "track chunk count differs from the header track count"
        );
    }

    #[test]
    fn many_tracks_parse() {
        let tracks: Vec<(&[u8; 4], Vec<u8>)> = (0..300).map(|_| (b"MTrk", track(&[]))).collect();
        let bytes = smf(1, 300, 96, &tracks);
        assert_eq!(MidiFile::parse(&bytes).unwrap().tracks.len(), 300);
    }

    #[test]
    fn unknown_chunks_are_skipped() {
        let bytes = smf(
            1,
            1,
            96,
            &[
                (b"XFIH", vec![1, 2, 3]),
                (b"MTrk", track(&[])),
                (b"junk", vec![]),
            ],
        );
        assert_eq!(MidiFile::parse(&bytes).unwrap().tracks.len(), 1);
    }

    #[test]
    fn running_status_applies_to_later_channel_messages() {
        let events = [
            0x00, 0x90, 60, 100, // note on, explicit status
            0x10, 62, 0, // running status: note on, velocity 0 stays a NoteOn
            0x20, 0xC3, 5, // program change on channel 3
            0x30, 0xE3, 0x00, 0x40, // pitch bend 0x2000 on channel 3, explicit status
        ];
        let bytes = one_track(&events);
        let parsed = only_track_events(&bytes);
        assert_eq!(parsed.len(), 5);
        assert_eq!(parsed[0].delta_ticks, 0);
        assert_eq!(
            parsed[0].event,
            Event::Channel {
                channel: 0,
                message: ChannelMessage::NoteOn {
                    key: 60,
                    velocity: 100
                },
            }
        );
        assert_eq!(parsed[1].delta_ticks, 0x10);
        assert_eq!(
            parsed[1].event,
            Event::Channel {
                channel: 0,
                message: ChannelMessage::NoteOn {
                    key: 62,
                    velocity: 0
                },
            }
        );
        assert_eq!(
            parsed[2].event,
            Event::Channel {
                channel: 3,
                message: ChannelMessage::ProgramChange { program: 5 },
            }
        );
        assert_eq!(
            parsed[3].event,
            Event::Channel {
                channel: 3,
                message: ChannelMessage::PitchBend { value: 0x2000 },
            }
        );
        assert_eq!(parsed[4].event, Event::Meta(Meta::EndOfTrack));
    }

    #[test]
    fn running_status_is_cancelled_by_meta_and_sysex() {
        let after_meta = [0x00, 0x90, 60, 100, 0x00, 0xFF, 0x01, 0x00, 0x00, 62, 0];
        assert_eq!(
            reason(&one_track(&after_meta)),
            "data byte without running status"
        );
        let after_sysex = [0x00, 0x90, 60, 100, 0x00, 0xF0, 0x01, 0xF7, 0x00, 62, 0];
        assert_eq!(
            reason(&one_track(&after_sysex)),
            "data byte without running status"
        );
    }

    #[test]
    fn data_byte_without_any_status_is_refused() {
        assert_eq!(
            reason(&one_track(&[0x00, 62, 0])),
            "data byte without running status"
        );
    }

    #[test]
    fn channel_data_with_high_bit_is_refused() {
        assert_eq!(
            reason(&one_track(&[0x00, 0x90, 0x80, 100])),
            "channel data byte has its high bit set"
        );
        assert_eq!(
            reason(&one_track(&[0x00, 0xC0, 0xFF])),
            "channel data byte has its high bit set"
        );
    }

    #[test]
    fn channel_message_truncated_by_end_of_track_is_refused() {
        assert_eq!(
            reason(&smf(0, 1, 96, &[(b"MTrk", vec![0x00, 0x90, 60])])),
            "truncated channel message"
        );
        assert_eq!(
            reason(&smf(0, 1, 96, &[(b"MTrk", vec![0x00, 0x90])])),
            "truncated channel message"
        );
    }

    #[test]
    fn variable_length_quantities_are_bounded() {
        // Largest legal delta: four bytes, 0x0FFFFFFF, followed by a text meta with no data.
        let max = [0xFF, 0xFF, 0xFF, 0x7F, 0xFF, 0x01, 0x00];
        let bytes = one_track(&max);
        let parsed = only_track_events(&bytes);
        assert_eq!(parsed[0].delta_ticks, 0x0FFF_FFFF);
        assert_eq!(
            parsed[0].event,
            Event::Meta(Meta::Text {
                kind: TextKind::Text,
                bytes: &[],
            })
        );

        let five = [0x80, 0x80, 0x80, 0x80, 0x00, 0xFF, 0x2F, 0x00];
        assert_eq!(
            reason(&one_track(&five)),
            "variable-length quantity longer than four bytes"
        );
        let truncated = [0x80, 0x80];
        assert_eq!(
            reason(&smf(0, 1, 96, &[(b"MTrk", truncated.to_vec())])),
            "truncated variable-length quantity"
        );
    }

    #[test]
    fn meta_events_are_typed_and_length_checked() {
        let events = [
            0x00, 0xFF, 0x51, 0x03, 0x07, 0xA1, 0x20, // set tempo 500000
            0x00, 0xFF, 0x58, 0x04, 4, 2, 24, 8, // 4/4
            0x00, 0xFF, 0x59, 0x02, 0xFE, 0x01, // D minor: 2 flats
            0x00, 0xFF, 0x03, 0x02, b'h', b'i', // track name
            0x00, 0xFF, 0x54, 0x05, 1, 2, 3, 4, 5, // SMPTE offset
            0x00, 0xFF, 0x7F, 0x01, 0x42, // sequencer specific
            0x00, 0xFF, 0x33, 0x01, 0x99, // unknown meta type 0x33
        ];
        let bytes = one_track(&events);
        let parsed = only_track_events(&bytes);
        assert_eq!(
            parsed[0].event,
            Event::Meta(Meta::SetTempo {
                micros_per_quarter: 500_000
            })
        );
        assert_eq!(
            parsed[1].event,
            Event::Meta(Meta::TimeSignature {
                numerator: 4,
                denominator_power: 2,
                clocks_per_click: 24,
                thirty_seconds_per_quarter: 8,
            })
        );
        assert_eq!(
            parsed[2].event,
            Event::Meta(Meta::KeySignature {
                sharps_flats: -2,
                minor: true,
            })
        );
        assert_eq!(
            parsed[3].event,
            Event::Meta(Meta::Text {
                kind: TextKind::TrackName,
                bytes: b"hi",
            })
        );
        assert_eq!(
            parsed[4].event,
            Event::Meta(Meta::SmpteOffset([1, 2, 3, 4, 5]))
        );
        assert_eq!(
            parsed[5].event,
            Event::Meta(Meta::SequencerSpecific(&[0x42]))
        );
        assert_eq!(
            parsed[6].event,
            Event::Meta(Meta::Unknown {
                kind: 0x33,
                bytes: &[0x99],
            })
        );

        let short_tempo = [0x00, 0xFF, 0x51, 0x02, 0x07, 0xA1];
        assert_eq!(
            reason(&one_track(&short_tempo)),
            "set tempo is not three bytes"
        );
        let eot_with_data = [0x00, 0xFF, 0x2F, 0x01, 0x00];
        assert_eq!(
            reason(&one_track(&eot_with_data)),
            "end-of-track carries data"
        );
        let bad_mode = [0x00, 0xFF, 0x59, 0x02, 0x00, 0x02];
        assert_eq!(
            reason(&one_track(&bad_mode)),
            "key signature mode is neither major nor minor"
        );
    }

    #[test]
    fn sequence_number_forms() {
        let events = [0x00, 0xFF, 0x00, 0x00, 0x00, 0xFF, 0x00, 0x02, 0x01, 0x02];
        let bytes = one_track(&events);
        let parsed = only_track_events(&bytes);
        assert_eq!(parsed[0].event, Event::Meta(Meta::SequenceNumber(None)));
        assert_eq!(
            parsed[1].event,
            Event::Meta(Meta::SequenceNumber(Some(0x0102)))
        );
        assert_eq!(
            reason(&one_track(&[0x00, 0xFF, 0x00, 0x01, 0x05])),
            "sequence number is not zero or two bytes"
        );
    }

    #[test]
    fn sysex_and_escape_payloads_are_borrowed() {
        let events = [
            0x00, 0xF0, 0x03, 0x41, 0x10, 0xF7, 0x00, 0xF7, 0x02, 0xF8, 0x00,
        ];
        let bytes = one_track(&events);
        let parsed = only_track_events(&bytes);
        assert_eq!(parsed[0].event, Event::SysEx(&[0x41, 0x10, 0xF7]));
        assert_eq!(parsed[1].event, Event::Escape(&[0xF8, 0x00]));
        assert_eq!(
            reason(&smf(0, 1, 96, &[(b"MTrk", vec![0x00, 0xF0, 0x05, 0x41])])),
            "sysex payload overruns the track"
        );
    }

    #[test]
    fn unsupported_status_is_refused() {
        assert_eq!(
            reason(&one_track(&[0x00, 0xF1, 0x00])),
            "unsupported status byte"
        );
        assert_eq!(reason(&one_track(&[0x00, 0xF8])), "unsupported status byte");
    }

    #[test]
    fn track_must_end_with_end_of_track_and_stop_there() {
        let no_eot = smf(0, 1, 96, &[(b"MTrk", vec![0x00, 0x90, 60, 100])]);
        assert_eq!(reason(&no_eot), "track ends without end-of-track");
        let empty = smf(0, 1, 96, &[(b"MTrk", vec![])]);
        assert_eq!(reason(&empty), "track ends without end-of-track");
        let trailing = smf(
            0,
            1,
            96,
            &[(b"MTrk", vec![0x00, 0xFF, 0x2F, 0x00, 0x00, 0x90, 60, 100])],
        );
        assert_eq!(reason(&trailing), "bytes after end-of-track");
    }

    #[test]
    fn every_proper_prefix_of_a_valid_file_is_refused() {
        let events = [
            0x00, 0x90, 60, 100, 0x60, 0x80, 60, 0, 0x00, 0xFF, 0x51, 0x03, 1, 2, 3,
        ];
        let bytes = smf(1, 1, 480, &[(b"MTrk", track(&events))]);
        assert!(MidiFile::parse(&bytes).is_ok());
        for end in 0..bytes.len() {
            assert!(
                MidiFile::parse(&bytes[..end]).is_err(),
                "prefix of {end} bytes parsed"
            );
        }
    }

    #[test]
    fn mutated_files_never_panic() {
        let events = [
            0x00, 0x90, 60, 100, 0x60, 0x80, 60, 0, 0x00, 0xF0, 0x02, 0x41, 0xF7, 0x00, 0xFF, 0x58,
            0x04, 4, 2, 24, 8, 0x00, 0xE0, 0x00, 0x40,
        ];
        let original = smf(1, 1, 480, &[(b"MTrk", track(&events))]);
        // xorshift64 with a fixed seed: deterministic byte mutations, no clock or host state.
        let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for _ in 0..4096 {
            let mut bytes = original.clone();
            let flips = 1 + (next() % 4) as usize;
            for _ in 0..flips {
                let index = (next() as usize) % bytes.len();
                bytes[index] = next() as u8;
            }
            let _ = MidiFile::parse(&bytes);
        }
    }
}
