//! Bounded fmt/data-first view of an already qualified integer PCM resource.
use crate::label_sounds::{PcmResource, PcmWave, WaveError};

#[derive(Debug, thiserror::Error)]
pub enum DecoderWaveProblem {
    #[error("canonical PCM copy exceeds byte or platform layout budget")]
    Budget,
    #[error("canonical PCM allocation failed")]
    Allocation,
    #[error("source PCM: {0}")]
    Wave(#[from] WaveError),
    #[error("source PCM metadata differs from its qualified resource")]
    Metadata,
    #[error("PCM fmt chunk length {0} is outside the installed decoder profile")]
    FormatLength(usize),
}

#[derive(Debug, thiserror::Error)]
#[error("PCM resource {source_context}: {problem}")]
pub struct DecoderWaveError {
    pub source_context: String,
    #[source]
    pub problem: DecoderWaveProblem,
}

/// Original receipt stays attached. Only chunk positions change;
/// PCM, fmt/ancillary payloads and padding bytes are retained verbatim.
/// This is a decoder view, not a rewrite of opaque ancillary offset semantics.
#[derive(Debug)]
pub struct DecoderWave<'a> {
    source: &'a PcmResource,
    bytes: Vec<u8>,
}
fn checked_copy_len(len: usize, limit: usize) -> Result<usize, DecoderWaveProblem> {
    std::alloc::Layout::array::<u8>(limit).map_err(|_| DecoderWaveProblem::Budget)?;
    if len > limit {
        return Err(DecoderWaveProblem::Budget);
    }
    std::alloc::Layout::array::<u8>(len).map_err(|_| DecoderWaveProblem::Budget)?;
    Ok(len)
}
impl<'a> DecoderWave<'a> {
    /// Limits the one additional RIFF byte buffer. The source resource is not
    /// copied or replaced. Input framing and profile are checked before reserve.
    pub fn read(source: &'a PcmResource, max_bytes: usize) -> Result<Self, DecoderWaveError> {
        Self::read_inner(source, max_bytes).map_err(|problem| DecoderWaveError {
            source_context: source.source_label().into(),
            problem,
        })
    }
    fn read_inner(source: &'a PcmResource, max_bytes: usize) -> Result<Self, DecoderWaveProblem> {
        let input = source.bytes();
        let len = checked_copy_len(input.len(), max_bytes)?;
        let parsed = PcmWave::inspect(input)?;
        if &parsed != source.pcm() {
            return Err(DecoderWaveProblem::Metadata);
        }
        let mut at = 12usize;
        let payload = parsed.data_range();
        let data = payload.start - 8..payload.end + payload.len() % 2;
        // inspect() has proved every chunk header/body/padding span, and exactly
        // one fmt chunk. This second pass allocates no chunk index or payload.
        let format = loop {
            let body = at + 8;
            let size = u32::from_le_bytes(input[at + 4..body].try_into().unwrap()) as usize;
            let next = body + size + size % 2;
            if &input[at..at + 4] == b"fmt " {
                // Installed hound PCM framing contract, not asset content.
                if !matches!(size, 16 | 18 | 40) {
                    return Err(DecoderWaveProblem::FormatLength(size));
                }
                break at..next;
            }
            at = next;
        };
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(len)
            .map_err(|_| DecoderWaveProblem::Allocation)?;
        bytes.extend_from_slice(&input[..12]);
        bytes.extend_from_slice(&input[format.clone()]);
        bytes.extend_from_slice(&input[data.clone()]);
        let mut at = 12usize;
        while at < len {
            let body = at + 8;
            let size = u32::from_le_bytes(input[at + 4..body].try_into().unwrap()) as usize;
            let next = body + size + size % 2;
            if at != format.start && at != data.start {
                bytes.extend_from_slice(&input[at..next]);
            }
            at = next;
        }
        Ok(Self { source, bytes })
    }
    pub fn source(&self) -> &'a PcmResource {
        self.source
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_sounds::{Assets, Budget, SoundInput};
    use crate::scripts::{AssetIdentity, ScriptAsset};

    fn chunk(tag: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut bytes = tag.to_vec();
        bytes.extend(u32::try_from(body.len()).unwrap().to_le_bytes());
        bytes.extend(body);
        if !body.len().is_multiple_of(2) {
            bytes.push(0xb7);
        }
        bytes
    }
    fn fixture(channels: u16, bits: u16, format_len: usize, frames: usize) -> (Vec<u8>, Vec<u8>) {
        let align = channels * (bits / 8);
        let rate = 7000u32;
        let mut format = Vec::new();
        format.extend(1u16.to_le_bytes());
        format.extend(channels.to_le_bytes());
        format.extend(rate.to_le_bytes());
        format.extend((rate * u32::from(align)).to_le_bytes());
        format.extend(align.to_le_bytes());
        format.extend(bits.to_le_bytes());
        format.resize(format_len, 0x63);
        let payload: Vec<u8> = (0..usize::from(align) * frames)
            .map(|i| u8::try_from((i * 31 + 5) % 256).unwrap())
            .collect();
        let format = chunk(b"fmt ", &format);
        let data = chunk(b"data", &payload);
        let ancillary = chunk(b"JUNK", &[0x26, 0x48, 0x71]);
        let riff = |chunks: &[&[u8]]| {
            let mut bytes = b"RIFF".to_vec();
            bytes.extend(
                u32::try_from(4 + chunks.iter().map(|c| c.len()).sum::<usize>())
                    .unwrap()
                    .to_le_bytes(),
            );
            bytes.extend(b"WAVE");
            for c in chunks {
                bytes.extend_from_slice(c);
            }
            bytes
        };
        (
            riff(&[&ancillary, &data, &format]),
            riff(&[&format, &data, &ancillary]),
        )
    }
    fn assets(bytes: &[u8]) -> Result<Assets, crate::file_sounds::Error> {
        Assets::read(
            &[SoundInput {
                member: "synthetic.wav",
                asset: ScriptAsset {
                    label: "synthetic:PCM",
                    bytes,
                },
                origin: AssetIdentity {
                    source: "synthetic:archive".into(),
                    edition: "synthetic".into(),
                    version: "test".into(),
                },
            }],
            Budget {
                resources: 1,
                resource_bytes: bytes.len(),
                total_bytes: bytes.len(),
                metadata_bytes: 256,
            },
        )
    }

    #[test]
    fn resource_samples_chunks_and_source_receipt_survive_reordering() {
        for channels in [1, 2] {
            for bits in [8, 16] {
                for format_len in [16, 18, 40] {
                    for frames in [0, 3] {
                        let (input, expected) = fixture(channels, bits, format_len, frames);
                        let assets = assets(&input).unwrap();
                        let source = &assets.resources()[0];
                        let digest = source.sha256();
                        let converted = DecoderWave::read(source, input.len()).unwrap();
                        assert_eq!(converted.bytes(), expected);
                        assert!(std::ptr::eq(converted.source(), source));
                        assert_eq!(converted.source().sha256(), digest);
                        assert_eq!(source.bytes(), input);
                        assert_eq!(source.member(), "synthetic.wav");
                        assert_eq!(source.source_label(), "synthetic:PCM");
                        assert_eq!(source.origin().source, "synthetic:archive");
                        let pcm = PcmWave::inspect(converted.bytes()).unwrap();
                        assert_eq!(
                            (
                                pcm.channels,
                                pcm.bits_per_sample,
                                pcm.sample_rate,
                                pcm.block_align,
                                pcm.frames()
                            ),
                            (channels, bits, 7000, channels * (bits / 8), frames)
                        );
                        assert_eq!(
                            &source.bytes()[source.pcm().data_range()],
                            &converted.bytes()[pcm.data_range()]
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn malformed_resources_and_preflight_fail_without_replacing_source() {
        let (input, _) = fixture(1, 8, 16, 3);
        assert!(assets(&input[..input.len() - 1]).is_err());
        let resources = assets(&input).unwrap();
        let source = &resources.resources()[0];
        let error = DecoderWave::read(source, input.len() - 1).unwrap_err();
        assert!(matches!(error.problem, DecoderWaveProblem::Budget));
        assert_eq!(error.source_context, "synthetic:PCM");
        assert_eq!(source.bytes(), input);
        let (input, _) = fixture(1, 16, 20, 3);
        let resources = assets(&input).unwrap();
        let source = &resources.resources()[0];
        assert!(matches!(
            DecoderWave::read(source, input.len()),
            Err(DecoderWaveError {
                problem: DecoderWaveProblem::FormatLength(20),
                ..
            })
        ));
        assert_eq!(source.bytes(), input);
    }

    #[test]
    fn checked_lengths_include_platform_limits_without_large_allocations() {
        assert_eq!(checked_copy_len(27, 27).unwrap(), 27);
        assert!(matches!(
            checked_copy_len(28, 27),
            Err(DecoderWaveProblem::Budget)
        ));
        assert!(matches!(
            checked_copy_len(1, usize::MAX),
            Err(DecoderWaveProblem::Budget)
        ));
        assert!(matches!(
            checked_copy_len(usize::MAX, 1),
            Err(DecoderWaveProblem::Budget)
        ));
        assert_eq!(checked_copy_len(0, 0).unwrap(), 0);
    }
}
