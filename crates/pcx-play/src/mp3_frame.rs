//! Bounded MPEG audio decode through the already-locked Symphonia 0.6.1 `mp3` feature.
//!
//! The caller keeps the encoded bytes. This decoder hashes that slice, then takes an owned
//! working copy only because `MediaSource` is `'static`. Channel count, sample rate, frame
//! count, and gapless delay/padding come from the decoder. Nothing is resampled and the bytes
//! are not wrapped in a RIFF header.

use sha2::{Digest, Sha256};
use std::io::Cursor;
use symphonia::core::audio::sample::SampleBytes;
use symphonia::core::audio::{Audio, AudioBuffer, GenericAudioBufferRef};
use symphonia::core::codecs::audio::{AudioCodecParameters, AudioDecoder, AudioDecoderOptions};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, Track, TrackType};
use symphonia::core::io::{MediaSource, MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;

/// Smallest `MediaSourceStream` ring Symphonia 0.6.1 accepts (`buffer_len > 32 KiB`, power of two).
pub const DECODER_RING_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub max_encoded_bytes: usize,
    /// Owned working copy plus the decoder read-ahead ring.
    pub max_input_allocation_bytes: usize,
    /// `next_packet`, reset, and `decode` calls, after a mandatory probe step.
    pub max_decoder_steps: u32,
    pub max_packets: u32,
    pub max_frames: u64,
    pub max_samples: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleKind {
    U8,
    U16,
    U24,
    U32,
    S8,
    S16,
    S24,
    S32,
    F32,
    F64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameReport {
    pub encoded_len: usize,
    pub encoded_sha256: [u8; 32],
    pub channels: u16,
    pub sample_rate: u32,
    pub sample_kind: SampleKind,
    pub packets: u32,
    pub decoded_frames: u64,
    pub decoded_samples: u64,
    pub gapless_delay: Option<u32>,
    pub gapless_padding: Option<u32>,
    /// `Track::num_frames`, possibly estimated by the demuxer. Not a duration or completeness check.
    pub container_frames: Option<u64>,
    /// Little-endian native sample bits in decoder plane order. Not a RIFF payload.
    pub decoded_sha256: [u8; 32],
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum DecodeError {
    #[error("encoded byte limit: {len} bytes exceeds {max}")]
    EncodedByteLimit { len: usize, max: usize },
    #[error("input allocation limit: {needed} bytes exceeds {max}")]
    AllocationLimit { needed: usize, max: usize },
    #[error("decoder work limit {max} reached")]
    WorkLimit { max: u32 },
    #[error("packet limit {max} reached")]
    PacketLimit { max: u32 },
    #[error("frame limit: {frames} exceeds {max}")]
    FrameLimit { frames: u64, max: u64 },
    #[error("sample limit: {samples} exceeds {max}")]
    SampleLimit { samples: u64, max: u64 },
    #[error("malformed MPEG audio at {stage}")]
    Malformed { stage: &'static str },
    #[error("unsupported MPEG audio profile at {stage}")]
    Unsupported { stage: &'static str },
    #[error("decoder supplied no sample rate")]
    MissingSampleRate,
    #[error("decoder supplied no channels")]
    MissingChannels,
    #[error("decoded signal changed inside one stream")]
    InconsistentSignal,
    #[error("decoder channel count {count} does not fit the report")]
    UnsupportedChannels { count: usize },
}

#[cfg_attr(
    feature = "profile-cpu",
    tracing::instrument(skip_all, target = "pcx_cpu", name = "audio.mp3_decode")
)]
pub fn decode(encoded: &[u8], limits: &Limits) -> Result<FrameReport, DecodeError> {
    if encoded.len() > limits.max_encoded_bytes {
        return Err(DecodeError::EncodedByteLimit {
            len: encoded.len(),
            max: limits.max_encoded_bytes,
        });
    }
    if limits.max_decoder_steps == 0 {
        return Err(DecodeError::WorkLimit {
            max: limits.max_decoder_steps,
        });
    }
    if limits.max_packets == 0 {
        return Err(DecodeError::PacketLimit {
            max: limits.max_packets,
        });
    }
    if limits.max_frames == 0 {
        return Err(DecodeError::FrameLimit {
            frames: 0,
            max: limits.max_frames,
        });
    }
    if limits.max_samples == 0 {
        return Err(DecodeError::SampleLimit {
            samples: 0,
            max: limits.max_samples,
        });
    }
    let encoded_sha256 = sha256(encoded);
    let needed =
        encoded
            .len()
            .checked_add(DECODER_RING_BYTES)
            .ok_or(DecodeError::AllocationLimit {
                needed: usize::MAX,
                max: limits.max_input_allocation_bytes,
            })?;
    if needed > limits.max_input_allocation_bytes {
        return Err(DecodeError::AllocationLimit {
            needed,
            max: limits.max_input_allocation_bytes,
        });
    }
    std::alloc::Layout::array::<u8>(encoded.len()).map_err(|_| DecodeError::AllocationLimit {
        needed,
        max: limits.max_input_allocation_bytes,
    })?;
    std::alloc::Layout::array::<u8>(DECODER_RING_BYTES).map_err(|_| {
        DecodeError::AllocationLimit {
            needed,
            max: limits.max_input_allocation_bytes,
        }
    })?;
    let mut owned = Vec::new();
    owned
        .try_reserve_exact(encoded.len())
        .map_err(|_| DecodeError::AllocationLimit {
            needed,
            max: limits.max_input_allocation_bytes,
        })?;
    owned.extend_from_slice(encoded);

    let stream = MediaSourceStream::new(
        Box::new(Cursor::new(owned)) as Box<dyn MediaSource>,
        MediaSourceStreamOptions {
            buffer_len: DECODER_RING_BYTES,
        },
    );
    let mut steps = 1u32;
    let mut format = symphonia::default::get_probe()
        .probe(
            &Hint::new(),
            stream,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|_| DecodeError::Malformed { stage: "probe" })?;
    let track_id = format
        .default_track(TrackType::Audio)
        .ok_or(DecodeError::Malformed { stage: "no track" })?
        .id;
    let mut decoder = open_decoder(format.as_ref(), track_id)?;
    let mut packets = 0u32;
    let mut decoded_frames = 0u64;
    let mut decoded_samples = 0u64;
    let mut decoded_hash = Sha256::new();
    let mut signal: Option<Signal> = None;

    loop {
        charge(&mut steps, limits.max_decoder_steps)?;
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(SymError::ResetRequired) => {
                charge(&mut steps, limits.max_decoder_steps)?;
                decoder = open_decoder(format.as_ref(), track_id)?;
                continue;
            }
            Err(_) => return Err(DecodeError::Malformed { stage: "packet" }),
        };
        if packet.track_id != track_id {
            continue;
        }
        if packets >= limits.max_packets {
            return Err(DecodeError::PacketLimit {
                max: limits.max_packets,
            });
        }
        charge(&mut steps, limits.max_decoder_steps)?;
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(_) => return Err(DecodeError::Malformed { stage: "decode" }),
        };
        let spec = decoded.spec();
        let channels = spec.channels().count();
        if channels == 0 {
            return Err(DecodeError::MissingChannels);
        }
        let channels = u16::try_from(channels)
            .map_err(|_| DecodeError::UnsupportedChannels { count: channels })?;
        if spec.rate() == 0 {
            return Err(DecodeError::MissingSampleRate);
        }
        if let Some(seen) = signal {
            if seen.channels != channels || seen.sample_rate != spec.rate() {
                return Err(DecodeError::InconsistentSignal);
            }
        } else {
            let params = track_params(format.as_ref(), track_id)?;
            if let Some(header_rate) = params.sample_rate
                && header_rate != spec.rate()
            {
                return Err(DecodeError::InconsistentSignal);
            }
            if let Some(header_channels) = &params.channels
                && header_channels.count() != usize::from(channels)
            {
                return Err(DecodeError::InconsistentSignal);
            }
        }
        let add_frames = u64::try_from(decoded.frames()).map_err(|_| DecodeError::FrameLimit {
            frames: u64::MAX,
            max: limits.max_frames,
        })?;
        let next_frames =
            decoded_frames
                .checked_add(add_frames)
                .ok_or(DecodeError::FrameLimit {
                    frames: u64::MAX,
                    max: limits.max_frames,
                })?;
        if next_frames > limits.max_frames {
            return Err(DecodeError::FrameLimit {
                frames: next_frames,
                max: limits.max_frames,
            });
        }
        let add_samples =
            add_frames
                .checked_mul(u64::from(channels))
                .ok_or(DecodeError::SampleLimit {
                    samples: u64::MAX,
                    max: limits.max_samples,
                })?;
        let next_samples =
            decoded_samples
                .checked_add(add_samples)
                .ok_or(DecodeError::SampleLimit {
                    samples: u64::MAX,
                    max: limits.max_samples,
                })?;
        if next_samples > limits.max_samples {
            return Err(DecodeError::SampleLimit {
                samples: next_samples,
                max: limits.max_samples,
            });
        }
        let kind = hash_native(&decoded, &mut decoded_hash);
        if let Some(seen) = signal
            && seen.kind != kind
        {
            return Err(DecodeError::InconsistentSignal);
        }
        signal = Some(Signal {
            channels,
            sample_rate: spec.rate(),
            kind,
        });
        packets += 1;
        decoded_frames = next_frames;
        decoded_samples = next_samples;
    }

    let Some(signal) = signal else {
        return Err(DecodeError::Malformed { stage: "no frames" });
    };
    if packets == 0 || decoded_frames == 0 {
        return Err(DecodeError::Malformed { stage: "no frames" });
    }
    let track = track(format.as_ref(), track_id)?;
    Ok(FrameReport {
        encoded_len: encoded.len(),
        encoded_sha256,
        channels: signal.channels,
        sample_rate: signal.sample_rate,
        sample_kind: signal.kind,
        packets,
        decoded_frames,
        decoded_samples,
        gapless_delay: track.delay,
        gapless_padding: track.padding,
        container_frames: track.num_frames,
        decoded_sha256: decoded_hash.finalize().into(),
    })
}

#[derive(Clone, Copy)]
struct Signal {
    channels: u16,
    sample_rate: u32,
    kind: SampleKind,
}

fn charge(steps: &mut u32, max: u32) -> Result<(), DecodeError> {
    if *steps >= max {
        return Err(DecodeError::WorkLimit { max });
    }
    *steps += 1;
    Ok(())
}

fn open_decoder(
    format: &dyn FormatReader,
    track_id: u32,
) -> Result<Box<dyn AudioDecoder>, DecodeError> {
    let params = track_params(format, track_id)?;
    // Preserve untrimmed frames; delay and padding remain separate source metadata.
    symphonia::default::get_codecs()
        .make_audio_decoder(params, &AudioDecoderOptions::default().gapless(false))
        .map_err(|error| match error {
            SymError::Unsupported(_) => DecodeError::Unsupported { stage: "codec" },
            _ => DecodeError::Malformed { stage: "codec" },
        })
}

fn track(format: &dyn FormatReader, track_id: u32) -> Result<&Track, DecodeError> {
    format
        .tracks()
        .iter()
        .find(|track| track.id == track_id)
        .ok_or(DecodeError::Malformed { stage: "no track" })
}

fn track_params(
    format: &dyn FormatReader,
    track_id: u32,
) -> Result<&AudioCodecParameters, DecodeError> {
    track(format, track_id)?
        .codec_params
        .as_ref()
        .and_then(|params| params.audio())
        .ok_or(DecodeError::Malformed {
            stage: "no audio codec",
        })
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn hash_native(decoded: &GenericAudioBufferRef<'_>, hash: &mut Sha256) -> SampleKind {
    match decoded {
        GenericAudioBufferRef::U8(buf) => hash_le(buf, hash, SampleKind::U8),
        GenericAudioBufferRef::U16(buf) => hash_le(buf, hash, SampleKind::U16),
        GenericAudioBufferRef::U24(buf) => hash_le(buf, hash, SampleKind::U24),
        GenericAudioBufferRef::U32(buf) => hash_le(buf, hash, SampleKind::U32),
        GenericAudioBufferRef::S8(buf) => hash_le(buf, hash, SampleKind::S8),
        GenericAudioBufferRef::S16(buf) => hash_le(buf, hash, SampleKind::S16),
        GenericAudioBufferRef::S24(buf) => hash_le(buf, hash, SampleKind::S24),
        GenericAudioBufferRef::S32(buf) => hash_le(buf, hash, SampleKind::S32),
        GenericAudioBufferRef::F32(buf) => hash_le(buf, hash, SampleKind::F32),
        GenericAudioBufferRef::F64(buf) => hash_le(buf, hash, SampleKind::F64),
    }
}

fn hash_le<S: SampleBytes>(buf: &AudioBuffer<S>, hash: &mut Sha256, kind: SampleKind) -> SampleKind
where
    S::RawType: AsRef<[u8]>,
{
    for plane in buf.iter_planes() {
        for sample in plane {
            hash.update(sample.to_le_sample_bytes().as_ref());
        }
    }
    kind
}

#[cfg(test)]
mod tests {
    use super::{DECODER_RING_BYTES, DecodeError, Limits, SampleKind, decode};
    use sha2::Digest;

    const MONO_XING: &[u8] = include_bytes!("../tests/fixtures/synthetic-mp3/mono-xing.mp3");
    const MONO_NOXING: &[u8] = include_bytes!("../tests/fixtures/synthetic-mp3/mono-noxing.mp3");
    const STEREO_XING: &[u8] = include_bytes!("../tests/fixtures/synthetic-mp3/stereo-xing.mp3");

    fn open_limits(len: usize) -> Limits {
        Limits {
            max_encoded_bytes: len,
            max_input_allocation_bytes: len + DECODER_RING_BYTES,
            max_decoder_steps: 512,
            max_packets: 128,
            max_frames: 200_000,
            max_samples: 400_000,
        }
    }

    #[test]
    fn synthetic_streams_keep_their_bytes_and_report_decoder_fields() {
        for (bytes, channels) in [(MONO_XING, 1u16), (MONO_NOXING, 1), (STEREO_XING, 2)] {
            let owned = bytes.to_vec();
            let before = owned.clone();
            let report = decode(&owned, &open_limits(owned.len())).unwrap();
            eprintln!("PCX_SYNTHETIC_MP3 {report:?}");
            assert_eq!(owned, before);
            assert_eq!(report.encoded_len, bytes.len());
            assert_eq!(
                report.encoded_sha256.as_slice(),
                sha2::Sha256::digest(bytes).as_slice()
            );
            assert_eq!(report.channels, channels);
            assert_eq!(report.sample_rate, 44_100);
            assert_eq!(report.sample_kind, SampleKind::F32);
            assert!(report.packets > 1, "packets {}", report.packets);
            assert!(report.decoded_frames > 0);
            assert_eq!(
                report.decoded_samples,
                report.decoded_frames * u64::from(report.channels)
            );
            let again = decode(bytes, &open_limits(bytes.len())).unwrap();
            assert_eq!(report, again);
        }
    }

    #[test]
    fn gapless_fields_are_the_demuxer_values_for_tagged_and_untagged_encodes() {
        let tagged = decode(MONO_XING, &open_limits(MONO_XING.len())).unwrap();
        let plain = decode(MONO_NOXING, &open_limits(MONO_NOXING.len())).unwrap();
        assert_ne!(
            (tagged.gapless_delay, tagged.gapless_padding),
            (plain.gapless_delay, plain.gapless_padding)
        );
        assert!(tagged.container_frames.is_some() || tagged.gapless_delay.is_some());
        assert_ne!(tagged.decoded_frames, 0);
        assert_ne!(plain.decoded_frames, 0);
    }

    #[test]
    fn malformed_truncated_and_riff_inputs_are_refused() {
        let limits = open_limits(64);
        assert_eq!(
            decode(b"", &limits).unwrap_err(),
            DecodeError::Malformed { stage: "probe" }
        );
        assert_eq!(
            decode(b"ID3", &limits).unwrap_err(),
            DecodeError::Malformed { stage: "probe" }
        );
        assert_eq!(
            decode(b"not mpeg audio", &limits).unwrap_err(),
            DecodeError::Malformed { stage: "probe" }
        );
        assert_eq!(
            decode(b"RIFF\0\0\0\0WAVE", &limits).unwrap_err(),
            DecodeError::Malformed { stage: "probe" }
        );
        assert!(matches!(
            decode(&MONO_XING[..16], &open_limits(16)).unwrap_err(),
            DecodeError::Malformed { .. }
        ));
        let full = decode(MONO_XING, &open_limits(MONO_XING.len())).unwrap();
        let half = &MONO_XING[..MONO_XING.len() / 2];
        let partial = decode(half, &open_limits(half.len())).unwrap();
        assert_eq!(partial.encoded_len, half.len());
        assert_ne!(partial.encoded_sha256, full.encoded_sha256);
        assert!(partial.decoded_frames < full.decoded_frames);
        assert_ne!(
            Some(partial.decoded_frames),
            partial.container_frames,
            "decoded frame count must stay the packets actually decoded"
        );
    }

    #[test]
    fn limits_refuse_before_a_successful_report() {
        let bytes = MONO_XING;
        let full = decode(bytes, &open_limits(bytes.len())).unwrap();
        let mut limits = open_limits(bytes.len());
        limits.max_encoded_bytes = bytes.len() - 1;
        assert_eq!(
            decode(bytes, &limits).unwrap_err(),
            DecodeError::EncodedByteLimit {
                len: bytes.len(),
                max: bytes.len() - 1
            }
        );
        limits = open_limits(bytes.len());
        limits.max_input_allocation_bytes = DECODER_RING_BYTES;
        assert!(matches!(
            decode(bytes, &limits).unwrap_err(),
            DecodeError::AllocationLimit { .. }
        ));
        limits = open_limits(4);
        limits.max_decoder_steps = 0;
        assert_eq!(
            decode(b"abcd", &limits).unwrap_err(),
            DecodeError::WorkLimit { max: 0 }
        );
        limits = open_limits(bytes.len());
        limits.max_decoder_steps = 1;
        assert_eq!(
            decode(bytes, &limits).unwrap_err(),
            DecodeError::WorkLimit { max: 1 }
        );
        limits = open_limits(bytes.len());
        limits.max_packets = 0;
        assert_eq!(
            decode(bytes, &limits).unwrap_err(),
            DecodeError::PacketLimit { max: 0 }
        );
        limits = open_limits(bytes.len());
        limits.max_packets = full.packets - 1;
        assert_eq!(
            decode(bytes, &limits).unwrap_err(),
            DecodeError::PacketLimit {
                max: full.packets - 1
            }
        );
        limits = open_limits(bytes.len());
        limits.max_frames = 0;
        assert_eq!(
            decode(bytes, &limits).unwrap_err(),
            DecodeError::FrameLimit { frames: 0, max: 0 }
        );
        limits = open_limits(bytes.len());
        limits.max_frames = full.decoded_frames - 1;
        assert!(matches!(
            decode(bytes, &limits).unwrap_err(),
            DecodeError::FrameLimit { .. }
        ));
        limits = open_limits(bytes.len());
        limits.max_samples = 0;
        assert_eq!(
            decode(bytes, &limits).unwrap_err(),
            DecodeError::SampleLimit { samples: 0, max: 0 }
        );
        limits = open_limits(bytes.len());
        limits.max_samples = full.decoded_samples - 1;
        assert!(matches!(
            decode(bytes, &limits).unwrap_err(),
            DecodeError::SampleLimit { .. }
        ));
    }
}
