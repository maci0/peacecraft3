#![cfg(not(target_arch = "wasm32"))]

use bevy::audio::{AudioSource, Decodable};
use pcx_play::file_sounds::{Assets, Budget, SoundInput};
use pcx_play::label_sounds::{PcmResource, PcmWave};
use pcx_play::pcm_decoder::{DecoderWave, DecoderWaveError, DecoderWaveProblem};
use pcx_play::scripts::{AssetIdentity, ScriptAsset};

fn chunk(tag: &[u8; 4], body: &[u8], pad: u8) -> Vec<u8> {
    let mut bytes = tag.to_vec();
    bytes.extend(u32::try_from(body.len()).unwrap().to_le_bytes());
    bytes.extend(body);
    if !body.len().is_multiple_of(2) {
        bytes.push(pad);
    }
    bytes
}
fn riff(chunks: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = b"RIFF".to_vec();
    bytes.extend(
        u32::try_from(4 + chunks.iter().map(Vec::len).sum::<usize>())
            .unwrap()
            .to_le_bytes(),
    );
    bytes.extend(b"WAVE");
    for c in chunks {
        bytes.extend(c);
    }
    bytes
}
fn fixture(channels: u16, bits: u16, frames: usize, fmt_len: usize, scrambled: bool) -> Vec<u8> {
    let rate = 11000u32; // Synthetic data, not an original resource's settings.
    let align = channels * (bits / 8);
    let mut format = Vec::new();
    format.extend(1u16.to_le_bytes());
    format.extend(channels.to_le_bytes());
    format.extend(rate.to_le_bytes());
    format.extend((rate * u32::from(align)).to_le_bytes());
    format.extend(align.to_le_bytes());
    format.extend(bits.to_le_bytes());
    format.extend((16..fmt_len).map(|i| u8::try_from((i * 29 + 7) % 256).unwrap()));
    let samples: Vec<u8> = (0..usize::from(align) * frames)
        .map(|i| u8::try_from((i * 71 + 3) % 256).unwrap())
        .collect();
    let f = chunk(b"fmt ", &format, 0x4a);
    let d = chunk(b"data", &samples, 0x9b);
    let a = chunk(b"JUNK", &[0x31, 0x72, 0x24], 0xd3);
    let z = chunk(b"ZZZZ", &[0x95, 0x13], 0);
    if scrambled {
        riff(&[a, d, z, f])
    } else {
        riff(&[f, d, a, z])
    }
}
fn assets(bytes: &[u8]) -> Result<Assets, pcx_play::file_sounds::Error> {
    Assets::read(
        &[SoundInput {
            member: "synthetic.wav",
            asset: ScriptAsset {
                label: "synthetic:source",
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
fn samples(bytes: &[u8]) -> Vec<u32> {
    AudioSource {
        bytes: bytes.into(),
    }
    .decoder()
    .map(f32::to_bits)
    .collect()
}
fn unchanged_resource(source: &PcmResource, input: &[u8]) {
    assert_eq!(source.bytes(), input);
    assert_eq!(source.member(), "synthetic.wav");
    assert_eq!(source.source_label(), "synthetic:source");
    assert_eq!(source.origin().source, "synthetic:archive");
}

#[test]
fn all_profiles_preserve_pcm_chunks_receipt_and_decoder_samples() {
    for channels in [1, 2] {
        for bits in [8, 16] {
            for fmt_len in [16, 18, 40] {
                for frames in [1, 3, 8] {
                    let input = fixture(channels, bits, frames, fmt_len, true);
                    let control = fixture(channels, bits, frames, fmt_len, false);
                    let resources = assets(&input).unwrap();
                    let source = &resources.resources()[0];
                    let original_digest = source.sha256();
                    let converted = DecoderWave::read(source, input.len()).unwrap();
                    assert!(std::ptr::eq(converted.source(), source));
                    assert_eq!(converted.source().sha256(), original_digest);
                    assert_eq!(converted.bytes(), control);
                    let pcm = PcmWave::inspect(converted.bytes()).unwrap();
                    assert_eq!(
                        (
                            pcm.channels,
                            pcm.bits_per_sample,
                            pcm.sample_rate,
                            pcm.block_align,
                            pcm.frames()
                        ),
                        (channels, bits, 11000, channels * (bits / 8), frames)
                    );
                    assert_eq!(
                        &source.bytes()[source.pcm().data_range()],
                        &converted.bytes()[pcm.data_range()]
                    );
                    let observed = samples(converted.bytes());
                    assert_eq!(observed.len(), frames * usize::from(channels));
                    assert_eq!(observed, samples(&control));
                    unchanged_resource(source, &input);
                    assert_eq!(converted.into_bytes(), control);
                }
            }
        }
    }
}

#[test]
fn already_canonical_is_byte_identical_and_budget_errors_keep_source() {
    let input = fixture(1, 8, 3, 16, false);
    let resources = assets(&input).unwrap();
    let source = &resources.resources()[0];
    let failure = DecoderWave::read(source, input.len() - 1).unwrap_err();
    assert!(matches!(failure.problem, DecoderWaveProblem::Budget));
    assert_eq!(failure.source_context, "synthetic:source");
    assert!(matches!(
        DecoderWave::read(source, usize::MAX),
        Err(DecoderWaveError {
            problem: DecoderWaveProblem::Budget,
            ..
        })
    ));
    assert_eq!(
        DecoderWave::read(source, input.len()).unwrap().bytes(),
        input
    );
    unchanged_resource(source, &input);
}

#[test]
fn empty_pcm_is_retained_and_decodes_to_no_samples() {
    for channels in [1, 2] {
        for bits in [8, 16] {
            let input = fixture(channels, bits, 0, 16, true);
            let resources = assets(&input).unwrap();
            let source = &resources.resources()[0];
            let converted = DecoderWave::read(source, input.len()).unwrap();
            assert_eq!(source.pcm().frames(), 0);
            assert!(samples(converted.bytes()).is_empty());
            unchanged_resource(source, &input);
        }
    }
}

#[test]
fn pcm_cbsize_and_extension_bytes_are_preserved_without_interpretation() {
    for fmt_len in [18, 40] {
        let input = fixture(2, 16, 3, fmt_len, true);
        let expected = fixture(2, 16, 3, fmt_len, false);
        assert_ne!(&expected[36..38], &[0, 0]);
        let resources = assets(&input).unwrap();
        let source = &resources.resources()[0];
        let converted = DecoderWave::read(source, input.len()).unwrap();
        assert_eq!(
            &converted.bytes()[20..20 + fmt_len],
            &expected[20..20 + fmt_len]
        );
        assert_eq!(samples(converted.bytes()), samples(&expected));
        unchanged_resource(source, &input);
    }
}

#[test]
fn malformed_resources_and_remaining_unsupported_fmt_sizes_refuse() {
    let input = fixture(1, 16, 3, 16, true);
    for len in 0..input.len() {
        assert!(assets(&input[..len]).is_err());
    }
    let input = fixture(1, 16, 3, 20, true);
    let resources = assets(&input).unwrap();
    let source = &resources.resources()[0];
    assert!(matches!(
        DecoderWave::read(source, input.len()),
        Err(DecoderWaveError {
            problem: DecoderWaveProblem::FormatLength(20),
            ..
        })
    ));
    unchanged_resource(source, &input);
}

#[test]
#[should_panic(expected = "UnrecognizedFormat")]
fn original_data_before_fmt_regression_reproduces_without_conversion() {
    let control = fixture(1, 8, 3, 16, false);
    let data = PcmWave::inspect(&control).unwrap().data_range();
    let input = riff(&[
        control[data.start - 8..data.end + data.len() % 2].to_vec(),
        control[12..36].to_vec(),
    ]);
    assert!(PcmWave::inspect(&input).is_ok());
    let _ = samples(&input);
}

#[test]
#[should_panic(expected = "UnrecognizedFormat")]
fn odd_ancillary_before_data_regression_reproduces_without_conversion() {
    let control = fixture(1, 8, 3, 16, false);
    let data = PcmWave::inspect(&control).unwrap().data_range();
    let input = riff(&[
        control[12..36].to_vec(),
        chunk(b"JUNK", &[1, 2, 3], 0x66),
        control[data.start - 8..data.end + data.len() % 2].to_vec(),
    ]);
    assert!(PcmWave::inspect(&input).is_ok());
    let _ = samples(&input);
}
