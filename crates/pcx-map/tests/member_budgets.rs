use pcx_map::Map;
use pcx_mpq::checked::{ReadBudget, ReadError};
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

fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
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

fn table_mut(bytes: &mut [u8], hash: bool, mutate: impl FnOnce(&mut [u8])) {
    let (pos_field, count_field, key) = if hash {
        (0x10, 0x18, pcx_mpq::hash::hash_table_key())
    } else {
        (0x14, 0x1c, pcx_mpq::hash::block_table_key())
    };
    let at = MPQ + word(bytes, MPQ + pos_field) as usize;
    let len = word(bytes, MPQ + count_field) as usize * 16;
    let table = &mut bytes[at..at + len];
    pcx_mpq::crypt::decrypt(table, key, pcx_mpq::crypt::table());
    mutate(table);
    encrypt(table, key);
}

const MEMBER: &str = "war3map.w3c";
fn policy(bytes: &[u8]) -> ReadBudget {
    ReadBudget {
        max_archive_bytes: bytes.len() - MPQ,
        max_archive_table_bytes: 272,
        max_raw_packed_bytes: 64,
        max_decoded_bytes: 64,
    }
}
fn checked(bytes: &[u8], budget: ReadBudget) -> Result<Option<Vec<u8>>, ReadError> {
    let map = Map::parse(bytes).unwrap();
    map.extract_checked("synthetic/source", MEMBER, budget)
        .map_err(|error| {
            assert_eq!(error.member, MEMBER);
            error.source
        })
}
#[test]
fn checked_members_preserve_bytes_and_refuse_budget_span_and_candidate_failures() {
    let payload = [0xa5; 64];
    let raw = fixture(&[(MEMBER, &payload)]);
    let budget = policy(&raw);
    assert_eq!(checked(&raw, budget).unwrap().unwrap(), payload);
    assert!(matches!(
        checked(
            &raw,
            ReadBudget {
                max_archive_bytes: budget.max_archive_bytes - 1,
                ..budget
            }
        ),
        Err(ReadError::BudgetExceeded {
            member: "embedded archive",
            ..
        })
    ));
    assert!(matches!(
        checked(
            &raw,
            ReadBudget {
                max_archive_table_bytes: 271,
                ..budget
            }
        ),
        Err(ReadError::ArchiveTableBudgetExceeded { .. })
    ));
    assert!(matches!(
        checked(
            &raw,
            ReadBudget {
                max_decoded_bytes: 63,
                ..budget
            }
        ),
        Err(ReadError::BudgetExceeded { member: MEMBER, .. })
    ));
    assert!(matches!(
        checked(
            &raw,
            ReadBudget {
                max_raw_packed_bytes: 63,
                ..budget
            }
        ),
        Err(ReadError::BudgetExceeded { member: MEMBER, .. })
    ));
    let mut bad = raw.clone();
    bad[MPQ + 0x10..MPQ + 0x14].copy_from_slice(&0u32.to_le_bytes());
    assert!(matches!(
        checked(&bad, budget),
        Err(ReadError::MpqOpen { .. })
    ));
    let mut bad = raw.clone();
    let end = (raw.len() - MPQ) as u32;
    table_mut(&mut bad, false, |t| {
        t[..4].copy_from_slice(&end.to_le_bytes())
    });
    assert!(matches!(
        checked(&bad, budget),
        Err(ReadError::ExtractionFailed { member: MEMBER, .. })
    ));
    let mut dup = fixture(&[(MEMBER, &payload), (MEMBER, &payload)]);
    let dup_budget = ReadBudget {
        max_archive_bytes: dup.len() - MPQ,
        max_archive_table_bytes: 288,
        ..budget
    };
    assert!(matches!(
        checked(&dup, dup_budget),
        Err(ReadError::AmbiguousMember { count: 2, .. })
    ));
    table_mut(&mut dup, true, |t| {
        for row in t.as_chunks_mut::<16>().0 {
            if word(row, 12) != u32::MAX {
                row[12..16].copy_from_slice(&0u32.to_le_bytes());
            }
        }
    });
    assert_eq!(checked(&dup, dup_budget).unwrap().unwrap(), payload);
    let missing = fixture(&[]);
    assert!(
        checked(
            &missing,
            ReadBudget {
                max_archive_bytes: missing.len() - MPQ,
                ..budget
            }
        )
        .unwrap()
        .is_none()
    );
    let mut bad = raw.clone();
    table_mut(&mut bad, true, |t| {
        for row in t.as_chunks_mut::<16>().0 {
            if word(row, 12) != u32::MAX {
                row[12..16].copy_from_slice(&99u32.to_le_bytes());
            }
        }
    });
    let Err(ReadError::ExtractionFailed {
        map,
        member,
        source,
    }) = checked(&bad, budget)
    else {
        panic!("malformed matching member collapsed into missing")
    };
    assert_eq!(map, "synthetic/source");
    assert_eq!(member, MEMBER);
    assert!(matches!(
        source,
        pcx_mpq::Error::BadBlockIndex { index: 99, .. }
    ));
}
