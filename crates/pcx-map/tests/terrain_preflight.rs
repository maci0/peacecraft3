//! Synthetic preflight checks; no original asset bytes or native admission policy.
use pcx_map::{Error, Terrain};

fn header(primary: &[[u8; 4]], secondary: &[[u8; 4]], x: u32, y: u32) -> Vec<u8> {
    let mut bytes = pcx_map::TERRAIN_MAGIC.to_vec();
    bytes.extend(pcx_map::TERRAIN_VERSION.to_le_bytes());
    bytes.push(203);
    bytes.extend(0x1357_9bdfu32.to_le_bytes());
    bytes.extend((primary.len() as u32).to_le_bytes());
    for slot in primary {
        bytes.extend(slot);
    }
    bytes.extend((secondary.len() as u32).to_le_bytes());
    for slot in secondary {
        bytes.extend(slot);
    }
    bytes.extend(x.to_le_bytes());
    bytes.extend(y.to_le_bytes());
    bytes.extend((-0.0f32).to_bits().to_le_bytes());
    bytes.extend(0x7fc1_2345u32.to_le_bytes());
    bytes
}
fn fixture() -> Vec<u8> {
    let mut bytes = header(&[*b"SYN0", *b"SYN1"], &[*b"SYC0"], 3, 2);
    for index in 0..6u16 {
        bytes.extend((0xc123u16 + index).to_le_bytes());
        bytes.extend((0xb456u16 + index).to_le_bytes());
        bytes.extend([0xa7, 0x58, 0xe9]);
    }
    bytes
}

#[test]
fn complete_frame_preserves_raw_header_lists_origin_bits_and_corner_words() {
    let bytes = fixture();
    for terrain in [
        Terrain::from_bytes(bytes.clone()).unwrap(),
        pcx_map::parse_terrain(&bytes).unwrap(),
    ] {
        assert_eq!(terrain.raw(), bytes);
        assert_eq!(terrain.version(), pcx_map::TERRAIN_VERSION);
        assert_eq!(terrain.default_slot(), 203);
        assert_eq!(terrain.slot_flag_word(), 0x1357_9bdf);
        assert_eq!(terrain.slots(), &[*b"SYN0", *b"SYN1"]);
        assert_eq!(terrain.secondary_slots(), &[*b"SYC0"]);
        assert_eq!((terrain.width(), terrain.height()), (2, 1));
        assert_eq!(terrain.origin().0.to_bits(), (-0.0f32).to_bits());
        assert_eq!(terrain.origin().1.to_bits(), 0x7fc1_2345);
        assert_eq!(terrain.corner_count(), 6);
        let first = terrain.corner(0, 0).unwrap();
        assert_eq!(first.height_index, 0xc123 & 0x3fff);
        assert_eq!(first.position_word, 0xb456);
        assert_eq!(
            (
                first.slot_nibble,
                first.corner_flags,
                first.byte5,
                first.byte6
            ),
            (7, 10, 0x58, 0xe9)
        );
    }
}

#[test]
fn every_proper_prefix_is_an_error_without_unwinding() {
    let bytes = fixture();
    for end in 0..bytes.len() {
        assert!(
            Terrain::from_bytes(bytes[..end].to_vec()).is_err(),
            "owned prefix {end}"
        );
        assert!(
            pcx_map::parse_terrain(&bytes[..end]).is_err(),
            "borrowed prefix {end}"
        );
    }
}

#[test]
fn truncated_origin_reports_full_required_span_after_valid_slot_tables() {
    let mut bytes = header(&[*b"SYN0"], &[*b"SYC0"], 2, 2);
    let required = bytes.len();
    bytes.pop();
    assert!(
        matches!(Terrain::from_bytes(bytes), Err(Error::TerrainTruncated { have, need }) if have == required - 1 && need == required)
    );
}

#[test]
fn forged_primary_and_secondary_counts_refuse_before_traversal() {
    let mut bytes = pcx_map::TERRAIN_MAGIC.to_vec();
    bytes.extend(pcx_map::TERRAIN_VERSION.to_le_bytes());
    bytes.push(0);
    bytes.extend(0u32.to_le_bytes());
    bytes.extend(65u32.to_le_bytes());
    assert!(
        matches!(Terrain::from_bytes(bytes.clone()), Err(Error::TerrainTruncated { have, need }) if have == bytes.len() && need == bytes.len() + 65 * std::mem::size_of::<[u8; 4]>())
    );
    bytes.truncate(bytes.len() - std::mem::size_of::<u32>());
    bytes.extend(0u32.to_le_bytes());
    bytes.extend(65u32.to_le_bytes());
    assert!(
        matches!(Terrain::from_bytes(bytes.clone()), Err(Error::TerrainTruncated { have, need }) if have == bytes.len() && need == bytes.len() + 65 * std::mem::size_of::<[u8; 4]>())
    );
}

#[test]
fn zero_corner_words_refuse_without_unsigned_tile_underflow() {
    for (x, y) in [(0, 2), (2, 0), (0, 0)] {
        let bytes = header(&[], &[], x, y);
        assert!(
            matches!(Terrain::from_bytes(bytes), Err(Error::TerrainCornerDimensions { corners_x, corners_y, .. }) if corners_x == x && corners_y == y)
        );
    }
    // One corner per axis is still a zero-tile terrain, as the old decoder allowed.
    let mut bytes = header(&[], &[], 1, 1);
    bytes.extend([0; pcx_map::CORNER_RECORD]);
    let terrain = Terrain::from_bytes(bytes).unwrap();
    assert_eq!((terrain.width(), terrain.height()), (0, 0));
}

#[test]
fn enormous_products_return_overflow_without_needing_a_large_body() {
    let bytes = header(&[], &[], u32::MAX, u32::MAX);
    let error = Terrain::from_bytes(bytes).unwrap_err();
    #[cfg(target_pointer_width = "32")]
    assert!(matches!(
        error,
        Error::TerrainOverflow {
            what: "corner count",
            ..
        }
    ));
    #[cfg(target_pointer_width = "64")]
    assert!(matches!(
        error,
        Error::TerrainOverflow {
            what: "corner bytes",
            ..
        }
    ));
}

#[test]
fn extra_body_bytes_keep_existing_typed_size_error() {
    let mut bytes = fixture();
    let need = bytes.len();
    bytes.push(0);
    assert!(
        matches!(Terrain::from_bytes(bytes), Err(Error::TerrainSize { width: 2, height: 1, need: expected, .. }) if expected == need)
    );
}

#[test]
fn short_magic_errors_report_the_supplied_length() {
    for size in 0..4 {
        let bytes = vec![0; size];
        assert!(
            matches!(Terrain::from_bytes(bytes), Err(Error::TerrainTruncated { have, need: 4 }) if have == size)
        );
    }
}

#[test]
fn real_map_terrain_consumer_preserves_valid_data_and_propagates_preflight_failure() {
    let bytes = fixture();
    let map_bytes = build_synthetic_map("SyntheticTerrain", &[("war3map.w3e", &bytes)]);
    let map = pcx_map::Map::parse(&map_bytes).unwrap();
    let terrain = map.terrain().unwrap();
    assert_eq!(terrain.raw(), bytes);
    assert_eq!(terrain.slots(), &[*b"SYN0", *b"SYN1"]);
    let mut malformed = header(&[*b"SYN0"], &[], 2, 2);
    malformed.pop();
    let map_bytes = build_synthetic_map("SyntheticTerrain", &[("war3map.w3e", &malformed)]);
    let map = pcx_map::Map::parse(&map_bytes).unwrap();
    assert!(matches!(map.terrain(), Err(Error::TerrainTruncated { .. })));
}

#[test]
fn missing_terrain_keeps_the_named_extraction_failure() {
    let map_bytes = build_synthetic_map("MissingTerrain", &[("other.txt", b"synthetic")]);
    let map = pcx_map::Map::parse(&map_bytes).unwrap();
    assert!(matches!(
        map.terrain(),
        Err(Error::Extraction(pcx_mpq::Error::NotFound(path))) if path == "war3map.w3e"
    ));
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
