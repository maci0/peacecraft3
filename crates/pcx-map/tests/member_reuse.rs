//! One parsed archive serves every later extraction from the same `Map`.
use pcx_map::Map;
use pcx_mpq::checked::{CheckedArchive, ReadBudget, ReadError};
use std::sync::Arc;
const MPQ: usize = 0x200;
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

const MEMBER: &str = "war3map.w3c";
const SECOND: &str = "war3map.w3d";
const PAYLOAD: [u8; 8] = [7; 8];
const OTHER: [u8; 5] = [9; 5];

fn budget(raw: &[u8]) -> ReadBudget {
    ReadBudget {
        max_archive_bytes: raw.len() - MPQ,
        max_archive_table_bytes: 512,
        max_raw_packed_bytes: 64,
        max_decoded_bytes: 64,
    }
}

#[test]
fn checked_extractions_share_one_set_of_tables_per_label_and_budget() {
    let raw = fixture(&[(MEMBER, &PAYLOAD), (SECOND, &OTHER)]);
    let map = Map::parse(&raw).unwrap();
    let budget = budget(&raw);
    let first = map.checked_archive("a", budget).unwrap();
    let second = map.checked_archive("a", budget).unwrap();
    assert!(Arc::ptr_eq(first.tables(), second.tables()));
    assert_eq!(
        map.extract_checked("a", MEMBER, budget).unwrap().unwrap(),
        PAYLOAD
    );
    assert_eq!(
        map.extract_checked("a", SECOND, budget).unwrap().unwrap(),
        OTHER
    );
    assert_eq!(map.extract_checked("a", "absent", budget).unwrap(), None);

    let relabelled = map.checked_archive("b", budget).unwrap();
    assert!(!Arc::ptr_eq(first.tables(), relabelled.tables()));
    assert_eq!(relabelled.source(), "b");
    let narrower = ReadBudget {
        max_decoded_bytes: 4,
        ..budget
    };
    let error = map.extract_checked("b", MEMBER, narrower).unwrap_err();
    assert_eq!(error.member, MEMBER);
    assert!(matches!(
        error.source,
        ReadError::BudgetExceeded { ref map, .. } if map == "b"
    ));
    // The budget is part of the key: the earlier wider one still passes.
    assert_eq!(
        map.extract_checked("b", MEMBER, budget).unwrap().unwrap(),
        PAYLOAD
    );
}

#[test]
fn tables_from_another_archive_are_refused() {
    let raw = fixture(&[(MEMBER, &PAYLOAD)]);
    let map = Map::parse(&raw).unwrap();
    let tables = Arc::clone(map.checked_archive("a", budget(&raw)).unwrap().tables());
    let shorter = &map.archive()[..map.archive().len() - 1];
    assert!(CheckedArchive::from_tables(shorter, "a", Arc::clone(&tables)).is_none());
    assert!(CheckedArchive::from_tables(map.archive(), "a", tables).is_some());
}

#[test]
fn a_failed_parse_is_not_kept_and_the_error_repeats() {
    let raw = fixture(&[(MEMBER, &PAYLOAD)]);
    let map = Map::parse(&raw).unwrap();
    let tight = ReadBudget {
        max_archive_table_bytes: 1,
        ..budget(&raw)
    };
    for _ in 0..2 {
        let error = map.extract_checked("a", MEMBER, tight).unwrap_err();
        assert!(matches!(
            error.source,
            ReadError::ArchiveTableBudgetExceeded { .. }
        ));
    }
    assert_eq!(
        map.extract_checked("a", MEMBER, budget(&raw))
            .unwrap()
            .unwrap(),
        PAYLOAD
    );
}

#[test]
fn unchecked_extraction_repeats_and_clones_start_fresh() {
    let raw = fixture(&[(MEMBER, &PAYLOAD), (SECOND, &OTHER)]);
    let map = Map::parse(&raw).unwrap();
    for _ in 0..3 {
        assert_eq!(map.extract(MEMBER).unwrap(), PAYLOAD);
        assert_eq!(map.extract(SECOND).unwrap(), OTHER);
        assert!(map.extract("absent").is_err());
    }
    let copy = map.clone();
    assert_eq!(copy, map);
    assert_eq!(copy.extract(SECOND).unwrap(), OTHER);
}
