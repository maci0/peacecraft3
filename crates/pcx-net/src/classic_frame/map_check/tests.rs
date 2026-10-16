use super::*;
use crate::classic_frame::{Decoder, FrameLimits, decode_one};
const MPQ: usize = 0x200;

fn selected(bytes: &[u8]) -> SelectedMap<'_> {
    let source = |bytes| SelectedBytes {
        edition: "synthetic",
        version: "1",
        source: "selected-fixture",
        bytes,
    };
    SelectedMap {
        protocol_path: b"Maps\\fixture.w3m",
        map: source(bytes),
        common: source(b"A"),
        blizzard: source(b"B"),
    }
}

fn budget(bytes: usize) -> MapCheckBudget {
    MapCheckBudget {
        archive: ReadBudget {
            max_archive_bytes: bytes,
            max_archive_table_bytes: 4096,
            max_raw_packed_bytes: 4096,
            max_decoded_bytes: 4096,
        },
        max_hashed_bytes: bytes + 4096,
    }
}

// Independent bitwise CRC reference for synthetic fixture expectations.
fn crc(bytes: &[u8]) -> u32 {
    let mut value = u32::MAX;
    for byte in bytes {
        value ^= u32::from(*byte);
        for _ in 0..8 {
            value = (value >> 1) ^ (0xEDB8_8320 & 0u32.wrapping_sub(value & 1));
        }
    }
    !value
}

fn packet(bytes: &[u8]) -> Vec<u8> {
    let mut packet = vec![W3GS_FAMILY, MAPCHECK_ID, 0, 0, 1, 0, 0, 0];
    packet.extend(b"Maps\\fixture.w3m\0");
    packet.extend(u32::try_from(bytes.len()).unwrap().to_le_bytes());
    packet.extend(crc(bytes).to_le_bytes());
    // Pinned algorithm applied independently to A, B, separator, C.
    packet.extend(0xFC4D_C740u32.to_le_bytes());
    packet.extend([
        0x5a, 0x73, 0xb2, 0x53, 0x2c, 0xae, 0x50, 0xf1, 0x5e, 0x75, 0xb9, 0x8c, 0x13, 0x96, 0xbc,
        0xf1, 0x99, 0x7b, 0x48, 0x3b,
    ]);
    let len = u16::try_from(packet.len()).unwrap();
    packet[2..4].copy_from_slice(&len.to_le_bytes());
    packet
}

#[test]
fn map_size_reply_uses_verified_file_size_and_preserves_prefix() {
    let map = fixture(&[("war3map.j", b"C")]);
    assert!(map.len() > usize::from(u8::MAX));
    let wire = packet(&map);
    let frame = decode_one(&wire, wire.len()).unwrap();
    let matched = frame
        .bind_selected_map(selected(&map), budget(map.len()))
        .unwrap();
    let prefix = [0x93, 0, 0xF1, 0x80];
    let reply = matched.map_size_reply(prefix);
    let reply_frame = decode_one(&reply, reply.len()).unwrap();
    assert_eq!(reply_frame.family(), W3GS_FAMILY);
    assert_eq!(reply_frame.id(), 66);
    assert_eq!(reply_frame.bytes().len(), 13);
    assert_eq!(&reply_frame.body()[..4], &prefix);
    assert_eq!(reply_frame.body()[4], 1);
    assert_eq!(
        u32::from_le_bytes(reply_frame.body()[5..].try_into().unwrap()),
        u32::try_from(map.len()).unwrap()
    );
    let mut decoder = Decoder::new(FrameLimits {
        max_frame_bytes: reply.len(),
        max_frames: 1,
    })
    .unwrap();
    let mut frames = Vec::new();
    for byte in reply.as_chunks::<1>().0 {
        frames.extend(decoder.push(byte).unwrap());
    }
    decoder.finish().unwrap();
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].bytes(), reply);
    assert!(
        frame
            .bind_selected_map(selected(&[]), budget(map.len()))
            .map(|matched| matched.map_size_reply(prefix))
            .is_err()
    );
}

#[test]
fn mismatched_or_absent_local_map_refuses_before_ready() {
    assert_eq!(crc(b"abc"), 0x3524_41c2);
    let map = fixture(&[("war3map.j", b"C")]);
    let wire = packet(&map);
    let frame = decode_one(&wire, wire.len()).unwrap();
    let selection = selected(&map);
    let matched = frame
        .bind_selected_map(selection, budget(map.len()))
        .unwrap();
    assert_eq!(matched.fingerprints().xoro, 0xFC4D_C740);
    assert_eq!(matched.map_preludes(), [false, false]);
    assert!(std::ptr::eq(matched.map().bytes.as_ptr(), map.as_ptr()));
    assert_eq!(matched.protocol_path(), selection.protocol_path);

    assert!(matches!(
        frame.bind_selected_map(selected(b""), budget(map.len())),
        Err(MapCheckError::Selection(_))
    ));
    let not_archive = b"synthetic-map-file";
    let invalid = packet(not_archive);
    assert!(matches!(
        decode_one(&invalid, invalid.len())
            .unwrap()
            .bind_selected_map(selected(not_archive), budget(map.len())),
        Err(MapCheckError::Map { .. })
    ));
    for (offset, field) in [
        (wire.len() - 32, "size"),
        (wire.len() - 28, "CRC-32"),
        (wire.len() - 24, "XORO"),
        (wire.len() - 1, "SHA-1"),
    ] {
        let mut bad = wire.clone();
        bad[offset] ^= 1;
        assert!(
            matches!(decode_one(&bad, bad.len()).unwrap().bind_selected_map(selection, budget(map.len())), Err(MapCheckError::Mismatch(actual)) if actual == field)
        );
    }
    let mut wrong_path = selection;
    wrong_path.protocol_path = b"other";
    assert!(matches!(
        frame.bind_selected_map(wrong_path, budget(map.len())),
        Err(MapCheckError::PathMismatch)
    ));
    let mut wrong_identity = selection;
    wrong_identity.map.version = "";
    assert!(matches!(
        frame.bind_selected_map(wrong_identity, budget(map.len())),
        Err(MapCheckError::Selection(_))
    ));
    let mut wrong_prelude = selection;
    wrong_prelude.common.edition = "another";
    assert!(matches!(
        frame.bind_selected_map(wrong_prelude, budget(map.len())),
        Err(MapCheckError::Selection(_))
    ));
    let mut missing_prelude = selection;
    missing_prelude.blizzard.bytes = b"";
    assert!(matches!(
        frame.bind_selected_map(missing_prelude, budget(map.len())),
        Err(MapCheckError::Selection(_))
    ));

    let mut exact = budget(map.len());
    exact.max_hashed_bytes = map.len() + 3;
    frame.bind_selected_map(selection, exact).unwrap();
    exact.max_hashed_bytes -= 1;
    assert!(matches!(
        frame.bind_selected_map(selection, exact),
        Err(MapCheckError::HashBudget)
    ));
    let mut low_tables = budget(map.len());
    low_tables.archive.max_archive_table_bytes = 0;
    assert!(matches!(
        frame.bind_selected_map(selection, low_tables),
        Err(MapCheckError::Archive(_))
    ));
    let mut decoder = Decoder::new(FrameLimits {
        max_frame_bytes: wire.len(),
        max_frames: 1,
    })
    .unwrap();
    let mut frames = decoder.push(&wire[..7]).unwrap();
    frames.extend(decoder.push(&wire[7..]).unwrap());
    decoder.finish().unwrap();
    assert_eq!(frames.len(), 1);
    assert_eq!(
        frames[0]
            .bind_selected_map(selection, budget(map.len()))
            .unwrap()
            .fingerprints(),
        matched.fingerprints()
    );
}

#[test]
fn map_preludes_and_script_priority_use_selected_archive_bytes() {
    let map = fixture(&[
        ("Scripts\\common.j", b"A"),
        ("Scripts\\blizzard.j", b"B"),
        ("war3map.j", b"C"),
        ("scripts\\war3map.j", b"must not be mixed"),
    ]);
    let wire = packet(&map);
    let mut selection = selected(&map);
    selection.common.bytes = b"wrong external bytes";
    selection.common.edition = "unused different edition";
    selection.blizzard.bytes = b"";
    let result = decode_one(&wire, wire.len())
        .unwrap()
        .bind_selected_map(selection, budget(map.len()))
        .unwrap();
    assert_eq!(result.map_preludes(), [true, true]);
    let alternate = fixture(&[("Scripts\\common.j", b""), ("scripts\\war3map.j", b"C")]);
    let wire = packet(&alternate);
    decode_one(&wire, wire.len())
        .unwrap()
        .bind_selected_map(selected(&alternate), budget(alternate.len()))
        .unwrap();
    let missing = fixture(&[("war3map.j", b"")]);
    let wire = packet(&missing);
    assert!(matches!(
        decode_one(&wire, wire.len())
            .unwrap()
            .bind_selected_map(selected(&missing), budget(missing.len())),
        Err(MapCheckError::Selection("map has no nonempty script"))
    ));
}

#[test]
fn malformed_mapcheck_refuses_before_archive_work() {
    let map = fixture(&[("war3map.j", b"C")]);
    let wire = packet(&map);
    for len in 4..wire.len() {
        let mut truncated = wire[..len].to_vec();
        truncated[2..4].copy_from_slice(&(len as u16).to_le_bytes());
        assert!(matches!(
            decode_one(&truncated, truncated.len())
                .unwrap()
                .bind_selected_map(selected(&map), budget(map.len())),
            Err(MapCheckError::Packet(_))
        ));
    }
    for (at, value) in [(0, super::super::BNCS_FAMILY), (1, 4)] {
        let mut bad = wire.clone();
        bad[at] = value;
        assert!(matches!(
            decode_one(&bad, bad.len())
                .unwrap()
                .bind_selected_map(selected(&map), budget(map.len())),
            Err(MapCheckError::Packet(_))
        ));
    }
    assert!(matches!(
        decode_one(&wire, wire.len() - 1),
        Err(super::super::FrameError::Oversize { .. })
    ));
    assert_eq!(xor_rotate(&[1, 2, 3, 4, 5]), 0x00C0_8069);
}

#[test]
fn every_map_member_contributes_in_pinned_order() {
    let map = fixture(&[
        ("war3map.j", b"C"),
        ("war3map.w3e", b"D"),
        ("war3map.wpm", b"E"),
        ("war3map.doo", b"F"),
        ("war3map.w3u", b"G"),
        ("war3map.w3b", b"H"),
        ("war3map.w3d", b"I"),
        ("war3map.w3a", b"J"),
        ("war3map.w3q", b"K"),
    ]);
    let mut wire = packet(&map);
    let end = wire.len();
    wire[end - 24..end - 20].copy_from_slice(&0x2C00_5B05u32.to_le_bytes());
    wire[end - 20..].copy_from_slice(&[
        0x47, 0x19, 0x83, 0xa6, 0x93, 0xbf, 0xd6, 0x51, 0xd5, 0x96, 0xbc, 0xdf, 0x22, 0xed, 0x56,
        0xf1, 0xe6, 0x15, 0x9e, 0x2d,
    ]);
    wire[4..8].copy_from_slice(&[7, 8, 9, 10]);
    let matched = decode_one(&wire, wire.len())
        .unwrap()
        .bind_selected_map(selected(&map), budget(map.len()))
        .unwrap();
    assert_eq!(matched.fingerprints().xoro, 0x2C00_5B05);
    assert_eq!(matched.prefix(), [7, 8, 9, 10]);
}

fn encrypt(bytes: &mut [u8], mut key: u32) {
    let mut seed = 0xEEEE_EEEEu32;
    for word in bytes.as_chunks_mut::<4>().0 {
        seed = seed.wrapping_add(pcx_mpq::crypt::table()[0x400 + (key & 0xff) as usize]);
        let plain = u32::from_le_bytes(*word);
        *word = (plain ^ key.wrapping_add(seed)).to_le_bytes();
        key = ((!key) << 21).wrapping_add(0x1111_1111) | (key >> 11);
        seed = plain
            .wrapping_add(seed)
            .wrapping_add(seed << 5)
            .wrapping_add(3);
    }
}

fn fixture(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut archive = vec![0; 0x40];
    let mut blocks = Vec::new();
    let mut hashes = vec![[u32::MAX, u32::MAX, 0, u32::MAX]; 16];
    for (index, (name, bytes)) in files.iter().enumerate() {
        blocks.push([
            archive.len() as u32,
            bytes.len() as u32,
            bytes.len() as u32,
            pcx_mpq::flags::EXISTING | pcx_mpq::flags::SINGLE_UNIT,
        ]);
        archive.extend_from_slice(bytes);
        let mut slot = pcx_mpq::hash::start_slot(name, hashes.len());
        while hashes[slot][3] != u32::MAX {
            slot = (slot + 1) % hashes.len();
        }
        let (name1, name2) = pcx_mpq::hash::hash_string_both(name);
        hashes[slot] = [name1, name2, 0, index as u32];
    }
    let hash_pos = archive.len() as u32;
    let mut hash_bytes: Vec<u8> = hashes
        .iter()
        .flat_map(|row| row.iter().flat_map(|v| v.to_le_bytes()))
        .collect();
    encrypt(&mut hash_bytes, pcx_mpq::hash::hash_table_key());
    archive.extend(hash_bytes);
    let block_pos = archive.len() as u32;
    let mut block_bytes: Vec<u8> = blocks
        .iter()
        .flat_map(|row| row.iter().flat_map(|v| v.to_le_bytes()))
        .collect();
    encrypt(&mut block_bytes, pcx_mpq::hash::block_table_key());
    archive.extend(block_bytes);
    archive[..4].copy_from_slice(b"MPQ\x1a");
    for (at, value) in [
        (4, 0x20),
        (8, archive.len() as u32),
        (0x10, hash_pos),
        (0x14, block_pos),
        (0x18, hashes.len() as u32),
        (0x1c, blocks.len() as u32),
    ] {
        archive[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    archive[0x0e] = 3;
    let mut map = b"HM3W\0\0\0\0Synthetic\0".to_vec();
    map.extend_from_slice(&1u32.to_le_bytes());
    map.extend_from_slice(&1u32.to_le_bytes());
    map.resize(MPQ, 0);
    map.extend(archive);
    map
}
