use pcx_map::cameras::{CameraError, CameraFile, CameraLimits, CameraLoadCause, CameraProfile};
fn limits() -> CameraLimits {
    CameraLimits {
        max_bytes: 512,
        max_records: 3,
        max_name_bytes: 16,
    }
}
fn fixture(profile: CameraProfile) -> Vec<u8> {
    let mut raw = (-17i32).to_le_bytes().to_vec();
    raw.extend(2u32.to_le_bytes());
    for name in [&b"A\xff"[..], &b"B"[..]] {
        for i in 0..10u32 {
            raw.extend((0x7fc1_2345 + i).to_le_bytes());
        }
        raw.extend(name);
        raw.push(0);
        if profile == CameraProfile::LocalAngles {
            raw.extend([0xff; 12]);
        }
    }
    raw
}
#[test]
fn both_profiles_preserve_raw_signed_header_words_names_offsets_and_optional_suffix() {
    for profile in [CameraProfile::Base, CameraProfile::LocalAngles] {
        let raw = fixture(profile);
        let file = CameraFile::parse(&raw, profile, limits()).unwrap();
        assert_eq!(file.raw(), raw);
        assert_eq!(file.version(), -17);
        assert_eq!(file.record_count(), 2);
        let records: Vec<_> = file.records().collect();
        assert_eq!(records[0].span.start, 8);
        assert_eq!(records[0].name_span, 48..50);
        assert_eq!(records[0].name, b"A\xff");
        assert_eq!(records[0].words[..4], 0x7fc1_2345u32.to_le_bytes());
        assert_eq!(records[1].span.start, records[0].span.end);
        assert_eq!(records[1].span.end, raw.len());
        assert_eq!(
            records[0].local_angles,
            if profile == CameraProfile::Base {
                None
            } else {
                Some(&[0xff; 12][..])
            }
        );
    }
}
#[test]
fn every_truncated_prefix_and_extra_byte_are_rejected_for_both_profiles() {
    for profile in [CameraProfile::Base, CameraProfile::LocalAngles] {
        let raw = fixture(profile);
        for end in 0..raw.len() {
            assert!(
                CameraFile::parse(&raw[..end], profile, limits()).is_err(),
                "prefix {end}"
            );
        }
        let mut trailing = raw;
        trailing.push(0);
        assert!(matches!(
            CameraFile::parse(&trailing, profile, limits()),
            Err(CameraError::TrailingBytes { .. })
        ));
    }
}
#[test]
fn header_version_is_not_an_extended_profile_selector() {
    let extended = fixture(CameraProfile::LocalAngles);
    let base = fixture(CameraProfile::Base);
    assert!(CameraFile::parse(&extended, CameraProfile::Base, limits()).is_err());
    assert!(CameraFile::parse(&base, CameraProfile::LocalAngles, limits()).is_err());
    for version in [0, 1, i32::MIN, i32::MAX] {
        let mut raw = base.clone();
        raw[..4].copy_from_slice(&version.to_le_bytes());
        assert_eq!(
            CameraFile::parse(&raw, CameraProfile::Base, limits())
                .unwrap()
                .version(),
            version
        );
    }
}
#[test]
fn caller_byte_record_and_name_limits_have_independent_exact_boundaries() {
    let raw = fixture(CameraProfile::Base);
    for small in [
        CameraLimits {
            max_bytes: raw.len() - 1,
            ..limits()
        },
        CameraLimits {
            max_records: 1,
            ..limits()
        },
        CameraLimits {
            max_name_bytes: 1,
            ..limits()
        },
    ] {
        assert!(matches!(
            CameraFile::parse(&raw, CameraProfile::Base, small),
            Err(CameraError::Limit { .. })
        ));
    }
    assert!(
        CameraFile::parse(
            &raw,
            CameraProfile::Base,
            CameraLimits {
                max_bytes: raw.len(),
                max_records: 2,
                max_name_bytes: 2
            }
        )
        .is_ok()
    );
}
#[test]
fn forged_count_and_unterminated_name_are_not_silently_accepted() {
    let mut raw = fixture(CameraProfile::Base);
    raw[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        CameraFile::parse(
            &raw,
            CameraProfile::Base,
            CameraLimits {
                max_records: u32::MAX,
                ..limits()
            }
        ),
        Err(CameraError::CountFraming { at: 8, .. })
    ));
    let mut raw = 0i32.to_le_bytes().to_vec();
    raw.extend(1u32.to_le_bytes());
    raw.extend([0xff; 41]);
    assert!(matches!(
        CameraFile::parse(&raw, CameraProfile::Base, limits()),
        Err(CameraError::UnterminatedName { at: 48 })
    ));
}
#[test]
fn empty_container_and_empty_name_are_valid_without_defaults() {
    let mut raw = 0i32.to_le_bytes().to_vec();
    raw.extend(0u32.to_le_bytes());
    assert_eq!(
        CameraFile::parse(&raw, CameraProfile::LocalAngles, limits())
            .unwrap()
            .records()
            .count(),
        0
    );
    raw[4..8].copy_from_slice(&1u32.to_le_bytes());
    raw.extend([0; 41]);
    let file = CameraFile::parse(
        &raw,
        CameraProfile::Base,
        CameraLimits {
            max_name_bytes: 0,
            ..limits()
        },
    )
    .unwrap();
    assert!(file.records().next().unwrap().name.is_empty());
}
#[test]
fn actual_map_accessor_reports_missing_malformed_and_valid_member_with_provenance() {
    let raw = fixture(CameraProfile::Base);
    for member in [None, Some(&raw[..3]), Some(&raw[..])] {
        let files: Vec<_> = member.map(|b| ("war3map.w3c", b)).into_iter().collect();
        let bytes = build_synthetic_map("CameraSynthetic", &files);
        let map = pcx_map::Map::parse(&bytes).unwrap();
        match map.cameras(
            "synthetic/source/build-profile".into(),
            CameraProfile::Base,
            limits(),
            pcx_mpq::checked::ReadBudget {
                max_archive_bytes: bytes.len(),
                max_archive_table_bytes: bytes.len(),
                max_raw_packed_bytes: 512,
                max_decoded_bytes: 512,
            },
        ) {
            Ok(cameras) => {
                assert_eq!(cameras.source_label(), "synthetic/source/build-profile");
                assert_eq!(cameras.view().raw(), raw);
                assert_eq!(cameras.view().records().count(), 2);
            }
            Err(error) => {
                assert_eq!(error.source_label, "synthetic/source/build-profile");
                assert_eq!(error.member, "war3map.w3c");
                match (member, error.cause) {
                    (None, CameraLoadCause::MissingMember) => {}
                    (Some(_), CameraLoadCause::Framing(_)) => {}
                    _ => panic!("wrong error boundary"),
                }
            }
        }
    }
}

fn encrypt_mpq_block(data: &mut [u8], key: u32) {
    let table = pcx_mpq::crypt::table();
    let mut seed1 = key;
    let mut seed2 = 0xEEEE_EEEEu32;
    for chunk in data.as_chunks_mut::<4>().0 {
        seed2 = seed2.wrapping_add(table[0x400 + (seed1 & 0xFF) as usize]);
        let plaintext = u32::from_le_bytes(*chunk);
        let ciphertext = plaintext ^ seed1.wrapping_add(seed2);
        chunk.copy_from_slice(&ciphertext.to_le_bytes());
        seed1 = ((!seed1) << 0x15).wrapping_add(0x1111_1111) | (seed1 >> 0x0B);
        seed2 = plaintext
            .wrapping_add(seed2)
            .wrapping_add(seed2 << 5)
            .wrapping_add(3);
    }
}

fn build_synthetic_map(map_name: &str, files: &[(&str, &[u8])]) -> Vec<u8> {
    const HASH_ENTRIES: usize = 16;
    let mut payloads = Vec::new();
    let mut block_entries = Vec::new();
    let payload_base_offset = 0x40usize;

    for (_name, data) in files {
        let file_pos = payload_base_offset + payloads.len();
        let file_size = data.len() as u32;
        let compressed_size = file_size;
        let flags: u32 = 0x8000_0000 | 0x0100_0000; // EXISTING | SINGLE_UNIT

        block_entries.push((file_pos as u32, compressed_size, file_size, flags));
        payloads.extend_from_slice(data);
        while payloads.len() % 4 != 0 {
            payloads.push(0);
        }
    }

    let hash_table_pos = (payload_base_offset + payloads.len()) as u32;
    let mut hash_entries = vec![
        pcx_mpq::HashEntry {
            name1: 0xFFFF_FFFF,
            name2: 0xFFFF_FFFF,
            locale: 0,
            platform: 0,
            block_index: 0xFFFF_FFFF,
        };
        HASH_ENTRIES
    ];

    for (i, (name, _)) in files.iter().enumerate() {
        let (name1, name2) = pcx_mpq::hash::hash_string_both(name);
        let mut slot = pcx_mpq::hash::start_slot(name, HASH_ENTRIES);
        while hash_entries[slot].block_index != 0xFFFF_FFFF {
            slot = (slot + 1) % HASH_ENTRIES;
        }
        hash_entries[slot].name1 = name1;
        hash_entries[slot].name2 = name2;
        hash_entries[slot].block_index = i as u32;
    }

    let mut hash_table_raw = Vec::with_capacity(HASH_ENTRIES * 16);
    for entry in &hash_entries {
        hash_table_raw.extend_from_slice(&entry.name1.to_le_bytes());
        hash_table_raw.extend_from_slice(&entry.name2.to_le_bytes());
        hash_table_raw.extend_from_slice(&entry.locale.to_le_bytes());
        hash_table_raw.extend_from_slice(&entry.platform.to_le_bytes());
        hash_table_raw.extend_from_slice(&entry.block_index.to_le_bytes());
    }
    encrypt_mpq_block(&mut hash_table_raw, pcx_mpq::hash::hash_table_key());

    let block_table_pos = hash_table_pos + (HASH_ENTRIES as u32 * 16);
    let mut block_table_raw = Vec::with_capacity(block_entries.len() * 16);
    for (pos, comp, size, flags) in &block_entries {
        block_table_raw.extend_from_slice(&pos.to_le_bytes());
        block_table_raw.extend_from_slice(&comp.to_le_bytes());
        block_table_raw.extend_from_slice(&size.to_le_bytes());
        block_table_raw.extend_from_slice(&(*flags).to_le_bytes());
    }
    encrypt_mpq_block(&mut block_table_raw, pcx_mpq::hash::block_table_key());

    let archive_size = block_table_pos + (block_entries.len() as u32 * 16);

    let mut mpq = Vec::new();
    mpq.extend_from_slice(b"MPQ\x1a");
    mpq.extend_from_slice(&0x20u32.to_le_bytes());
    mpq.extend_from_slice(&archive_size.to_le_bytes());
    mpq.extend_from_slice(&0u16.to_le_bytes());
    mpq.push(3u8);
    mpq.push(0u8);
    mpq.extend_from_slice(&hash_table_pos.to_le_bytes());
    mpq.extend_from_slice(&block_table_pos.to_le_bytes());
    mpq.extend_from_slice(&(HASH_ENTRIES as u32).to_le_bytes());
    mpq.extend_from_slice(&(files.len() as u32).to_le_bytes());
    mpq.extend_from_slice(&0u32.to_le_bytes());
    mpq.extend_from_slice(&0u32.to_le_bytes());

    while mpq.len() < payload_base_offset {
        mpq.push(0);
    }
    mpq.extend_from_slice(&payloads);
    mpq.extend_from_slice(&hash_table_raw);
    mpq.extend_from_slice(&block_table_raw);

    assert_eq!(mpq.len(), archive_size as usize);

    let mut map = Vec::new();
    map.extend_from_slice(b"HM3W");
    map.extend_from_slice(&0u32.to_le_bytes());
    map.extend_from_slice(map_name.as_bytes());
    map.push(0);
    map.extend_from_slice(&1u32.to_le_bytes());
    map.extend_from_slice(&1u32.to_le_bytes());
    while map.len() < 0x200 {
        map.push(0);
    }
    map.extend_from_slice(&mpq);
    map
}
