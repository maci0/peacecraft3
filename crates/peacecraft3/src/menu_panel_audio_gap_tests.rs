fn origin() -> pcx_play::scripts::AssetIdentity {
    pcx_play::scripts::AssetIdentity {
        edition: "synthetic".into(),
        version: "synthetic".into(),
        source: "synthetic selected source".into(),
    }
}

fn request(path: &str, frame: u32) -> SoundDependency {
    SoundDependency {
        event: "SNDXTEST".into(),
        frame,
        path: path.into(),
        volume: 0.37,
    }
}

fn unsupported(path: &str, mask: u8) -> PanelError {
    PanelError::NativeRead {
        path: path.into(),
        archive: "synthetic-selected.mpq".into(),
        source: pcx_mpq::Error::Payload(pcx_mpq::payload::PayloadError::UnsupportedCompression {
            mask,
        }),
    }
}

fn wave() -> Vec<u8> {
    let mut bytes = b"RIFF".to_vec();
    bytes.extend(40u32.to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16u32.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(8000u32.to_le_bytes());
    bytes.extend(8000u32.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(8u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(4u32.to_le_bytes());
    bytes.extend([31, 73, 127, 211]);
    bytes
}

#[test]
fn only_exact_known_unsupported_codecs_are_gaps_not_arbitrary_decode_errors() {
    for mask in 0..=u8::MAX {
        assert_eq!(
            unavailable_codec(&unsupported("Local/gap.wav", mask)).map(|v| v.0),
            matches!(mask, 0x01 | 0x41 | 0x81).then_some(mask)
        );
    }
    for error in [
        PanelError::Missing("Local/missing.wav".into()),
        PanelError::Decode {
            path: "Local/bad.wav".into(),
            reason: "unsupported compression mask 0x41".into(),
        },
        PanelError::NativeRead {
            path: "Local/bad.wav".into(),
            archive: "synthetic.mpq".into(),
            source: pcx_mpq::Error::Unsupported("unsupported compression mask 0x41"),
        },
        PanelError::NativeRead {
            path: "Local/corrupt.wav".into(),
            archive: "synthetic.mpq".into(),
            source: pcx_mpq::Error::Payload(pcx_mpq::payload::PayloadError::Huffman(
                pcx_mpq::huffman::HuffmanError::Corrupt,
            )),
        },
    ] {
        assert!(unavailable_codec(&error).is_none());
    }
}

#[test]
fn mixed_audio_retains_each_playable_occurrence_and_deduplicates_qualified_gap_requests() {
    let mut reads = Vec::new();
    let admitted = loaded_sounds_with(
        vec![
            request("Local/play.wav", 11),
            request("Local/gap.wav", 13),
            request("local\\GAP.wav", 17),
            request("Local/play.wav", 19),
        ],
        origin(),
        |path, limit| {
            assert_eq!(limit, MAX_SOUND_BYTES);
            reads.push(path.to_string());
            if path == "Local/play.wav" {
                Ok(wave())
            } else {
                Err(unsupported(path, 0x41))
            }
        },
    )
    .unwrap();
    assert_eq!(reads, ["Local/play.wav", "Local/gap.wav"]);
    assert_eq!(
        admitted.sounds.iter().map(|s| s.frame).collect::<Vec<_>>(),
        [11, 19]
    );
    assert!(Arc::ptr_eq(
        &admitted.sounds[0].bytes,
        &admitted.sounds[1].bytes
    ));
    assert!(Arc::ptr_eq(
        &admitted.sounds[0].source,
        &admitted.sounds[1].source
    ));
    assert!(
        admitted
            .sounds
            .iter()
            .all(|s| s.volume == 0.37 && s.bytes.starts_with(b"RIFF"))
    );
    assert_eq!(admitted.unavailable.len(), 1);
    let gap = &admitted.unavailable[0];
    assert_eq!(gap.path, "Local/gap.wav");
    assert_eq!(gap.mask, 0x41);
    assert!(gap.context.contains("synthetic-selected.mpq"));
    assert_eq!(
        gap.requests.iter().map(|r| r.frame).collect::<Vec<_>>(),
        [13, 17]
    );
    assert_eq!(gap.requests[1].path, "local\\GAP.wav");
    assert!(
        gap.requests
            .iter()
            .all(|r| r.volume == 0.37 && r.event == "SNDXTEST")
    );
}

#[test]
fn later_invalid_required_audio_refuses_the_whole_admission_after_a_codec_gap() {
    for failure in ["missing", "pcm", "unknown-mask"] {
        let result = loaded_sounds_with(
            vec![
                request("gap.wav", 3),
                request("play.wav", 7),
                request(failure, 9),
            ],
            origin(),
            |path, _| match path {
                "gap.wav" => Err(unsupported(path, 0x81)),
                "play.wav" => Ok(wave()),
                "missing" => Err(PanelError::Missing(path.into())),
                "pcm" => Ok(b"malformed waveform".to_vec()),
                "unknown-mask" => Err(unsupported(path, 0x03)),
                _ => unreachable!(),
            },
        );
        assert!(
            result.is_err(),
            "later {failure} cannot be hidden by a prior gap"
        );
    }
}

#[test]
fn metadata_and_native_member_limits_precede_reading_or_decoding() {
    for (packed, decoded) in [(17, 1), (1, 17), (u32::MAX, 1), (1, u32::MAX)] {
        assert!(checked_member_size("synthetic", packed, decoded, 16).is_err());
    }
    checked_member_size("synthetic", 16, 16, 16).unwrap();
    for mut bad in [
        request("bad.wav", 3),
        request("bad.wav", 5),
        request("bad.wav", 7),
    ] {
        bad.volume = match bad.frame {
            3 => f32::NAN,
            5 => -0.1,
            _ => 1.1,
        };
        assert!(
            loaded_sounds_with(vec![request("gap.wav", 1), bad], origin(), |_, _| {
                panic!("all request metadata must be validated before the first read")
            })
            .is_err()
        );
    }
    let huge = "é".repeat(MAX_SOUND_METADATA_BYTES);
    assert!(loaded_sounds_with(vec![request(&huge, 1)], origin(), |_, _| unreachable!()).is_err());
    let context = unavailable_codec(&PanelError::NativeRead {
        path: "gap.wav".into(),
        archive: huge,
        source: pcx_mpq::Error::Payload(pcx_mpq::payload::PayloadError::UnsupportedCompression {
            mask: 1,
        }),
    })
    .unwrap()
    .1;
    assert!(context.len() <= MAX_SOUND_METADATA_BYTES + "selected archive ".len());
}

#[test]
fn browser_adapter_uses_the_shared_typed_single_source_contract() {
    use peacecraft3::browser_read::{ArchiveOrigin, ReadError};
    let origin = ArchiveOrigin {
        ordinal: 0,
        name: "synthetic-selected.mpq".into(),
    };
    for sources in [1usize, 2] {
        let error = PanelError::BrowserRead {
            path: "gap.wav".into(),
            source: Box::new(ReadError::BoundedExtraction {
                requested_path: "gap.wav".into(),
                key: "gap.wav".into(),
                revision: 11,
                origin: origin.clone(),
                matching_archives: (0..sources)
                    .map(|ordinal| {
                        if ordinal == 0 {
                            origin.clone()
                        } else {
                            ArchiveOrigin {
                                ordinal,
                                name: format!("synthetic-{ordinal}.mpq"),
                            }
                        }
                    })
                    .collect(),
                source: Box::new(pcx_mpq::ExtractionError {
                    archive: "synthetic-selected.mpq".into(),
                    member: "gap.wav".into(),
                    cause: Box::new(pcx_mpq::ExtractionCause::Mpq(pcx_mpq::Error::Payload(
                        pcx_mpq::payload::PayloadError::UnsupportedCompression { mask: 0x81 },
                    ))),
                }),
            }),
        };
        let result = unavailable_codec(&error);
        if sources == 1 {
            let (mask, context) = result.unwrap();
            assert_eq!(mask, 0x81);
            assert!(context.contains("revision 11") && context.contains("synthetic-selected.mpq"));
        } else {
            assert!(
                result.is_none(),
                "unread second source cannot be certified equal"
            );
        }
    }
    let legacy = PanelError::BrowserRead {
        path: "gap.wav".into(),
        source: Box::new(ReadError::Extraction {
            key: "gap.wav".into(),
            origin,
            source: Box::new(pcx_mpq::Error::Payload(
                pcx_mpq::payload::PayloadError::UnsupportedCompression { mask: 0x81 },
            )),
        }),
    };
    assert!(unavailable_codec(&legacy).is_none());
}
