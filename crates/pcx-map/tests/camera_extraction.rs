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

fn map_fixture(files: &[(&str, &[u8])]) -> Vec<u8> {
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

use pcx_map::cameras::{
    CAMERAS_NAME, CameraError, CameraLimits, CameraLoadCause, CameraLoadError, CameraProfile,
};
use pcx_mpq::checked::{ReadBudget, ReadError};
const LABEL: &str = "synthetic/source/explicit-profile";
fn camera(profile: CameraProfile) -> Vec<u8> {
    let mut raw = (-17i32).to_le_bytes().to_vec();
    raw.extend(2u32.to_le_bytes());
    for name in [&b"A\xff"[..], &b"B"[..]] {
        for i in 0..10u32 {
            raw.extend((0x7fc1_2345 + i).to_le_bytes());
        }
        raw.extend(name);
        raw.push(0);
        if profile == CameraProfile::LocalAngles {
            raw.extend([0xfe; 12]);
        }
    }
    raw
}
fn framing() -> CameraLimits {
    CameraLimits {
        max_bytes: 512,
        max_records: 2,
        max_name_bytes: 2,
    }
}
fn read_budget(bytes: &[u8]) -> ReadBudget {
    ReadBudget {
        max_archive_bytes: bytes.len() - MPQ,
        max_archive_table_bytes: (word(bytes, MPQ + 0x18) as usize
            + word(bytes, MPQ + 0x1c) as usize)
            * 16,
        max_raw_packed_bytes: 512,
        max_decoded_bytes: 512,
    }
}
fn load(
    bytes: &[u8],
    profile: CameraProfile,
    limits: CameraLimits,
    budget: ReadBudget,
) -> Result<pcx_map::cameras::MapCameras, CameraLoadError> {
    let map = pcx_map::Map::parse(bytes).unwrap();
    map.cameras(LABEL.into(), profile, limits, budget)
}
fn extraction(error: CameraLoadError, profile: CameraProfile) -> ReadError {
    assert_eq!(error.source_label, LABEL);
    assert_eq!(error.member, CAMERAS_NAME);
    assert_eq!(error.profile, profile);
    assert!(error.to_string().contains(LABEL));
    assert!(error.to_string().contains(CAMERAS_NAME));
    let CameraLoadCause::Extraction(cause) = error.cause else {
        panic!("expected extraction refusal")
    };
    assert_eq!(cause.member, CAMERAS_NAME);
    cause.source
}
pub fn composed_cases() -> u32 {
    let mut groups = 0;
    for profile in [CameraProfile::Base, CameraProfile::LocalAngles] {
        let raw = camera(profile);
        let bytes = map_fixture(&[(CAMERAS_NAME, &raw)]);
        let cameras = load(&bytes, profile, framing(), read_budget(&bytes)).unwrap();
        assert_eq!(cameras.source_label(), LABEL);
        let view = cameras.view();
        assert_eq!(view.raw(), raw);
        assert_eq!(view.version(), -17);
        assert_eq!(view.profile(), profile);
        assert_eq!(view.record_count(), 2);
        let records: Vec<_> = view.records().collect();
        assert_eq!(records[0].span.start, 8);
        assert_eq!(records[1].span.start, records[0].span.end);
        assert_eq!(records[1].span.end, raw.len());
        assert_eq!(records[0].name, b"A\xff");
        assert_eq!(records[0].name_span, 48..50);
        assert_eq!(records[0].words[..4], 0x7fc1_2345u32.to_le_bytes());
        assert_eq!(
            records[0].local_angles,
            if profile == CameraProfile::Base {
                None
            } else {
                Some(&[0xfe; 12][..])
            }
        );
    }
    groups += 1;
    let profile = CameraProfile::Base;
    let raw = camera(profile);
    let bytes = map_fixture(&[(CAMERAS_NAME, &raw)]);
    let budget = read_budget(&bytes);
    // Each caller ceiling wins independently, before decoder allocation.
    for (limits, read, cap) in [
        (
            CameraLimits {
                max_bytes: raw.len() - 1,
                ..framing()
            },
            budget,
            raw.len() - 1,
        ),
        (
            framing(),
            ReadBudget {
                max_decoded_bytes: raw.len() - 2,
                ..budget
            },
            raw.len() - 2,
        ),
    ] {
        let error = extraction(load(&bytes, profile, limits, read).unwrap_err(), profile);
        assert!(
            matches!(error,ReadError::BudgetExceeded{map,member:CAMERAS_NAME,declared_bytes,max_bytes} if map==LABEL&&declared_bytes==raw.len()&&max_bytes==cap)
        );
    }
    groups += 1;
    let packed = extraction(
        load(
            &bytes,
            profile,
            framing(),
            ReadBudget {
                max_raw_packed_bytes: raw.len() - 1,
                ..budget
            },
        )
        .unwrap_err(),
        profile,
    );
    assert!(matches!(
        packed,
        ReadError::BudgetExceeded {
            member: CAMERAS_NAME,
            ..
        }
    ));
    groups += 1;
    let error = extraction(
        load(
            &bytes,
            profile,
            framing(),
            ReadBudget {
                max_archive_bytes: budget.max_archive_bytes - 1,
                ..budget
            },
        )
        .unwrap_err(),
        profile,
    );
    assert!(matches!(
        error,
        ReadError::BudgetExceeded {
            member: "embedded archive",
            ..
        }
    ));
    let error = extraction(
        load(
            &bytes,
            profile,
            framing(),
            ReadBudget {
                max_archive_table_bytes: budget.max_archive_table_bytes - 1,
                ..budget
            },
        )
        .unwrap_err(),
        profile,
    );
    assert!(matches!(
        error,
        ReadError::ArchiveTableBudgetExceeded { .. }
    ));
    groups += 1;
    let missing = map_fixture(&[]);
    let error = load(&missing, profile, framing(), read_budget(&missing)).unwrap_err();
    assert_eq!(error.source_label, LABEL);
    assert_eq!(error.profile, profile);
    assert!(matches!(error.cause, CameraLoadCause::MissingMember));
    groups += 1;
    for malformed in [&[][..], &raw[..3], &raw[..raw.len() - 1]] {
        let bytes = map_fixture(&[(CAMERAS_NAME, malformed)]);
        let error = load(&bytes, profile, framing(), read_budget(&bytes)).unwrap_err();
        assert_eq!(error.source_label, LABEL);
        assert_eq!(error.member, CAMERAS_NAME);
        assert_eq!(error.profile, profile);
        assert!(matches!(error.cause, CameraLoadCause::Framing(_)));
    }
    groups += 1;
    let mut bad = bytes.clone();
    table_mut(&mut bad, true, |table| {
        for row in table.as_chunks_mut::<16>().0 {
            if word(row, 12) != u32::MAX {
                row[12..16].copy_from_slice(&99u32.to_le_bytes());
            }
        }
    });
    let error = extraction(load(&bad, profile, framing(), budget).unwrap_err(), profile);
    assert!(matches!(
        error,
        ReadError::ExtractionFailed {
            member: CAMERAS_NAME,
            source: pcx_mpq::Error::BadBlockIndex { index: 99, .. },
            ..
        }
    ));
    let mut bad = bytes.clone();
    let end = (bytes.len() - MPQ) as u32;
    table_mut(&mut bad, false, |table| {
        table[..4].copy_from_slice(&end.to_le_bytes())
    });
    assert!(matches!(
        extraction(load(&bad, profile, framing(), budget).unwrap_err(), profile),
        ReadError::ExtractionFailed {
            member: CAMERAS_NAME,
            ..
        }
    ));
    groups += 1;
    let mut dup = map_fixture(&[(CAMERAS_NAME, &raw), (CAMERAS_NAME, &raw)]);
    let duplicate_budget = read_budget(&dup);
    assert!(matches!(
        extraction(
            load(&dup, profile, framing(), duplicate_budget).unwrap_err(),
            profile
        ),
        ReadError::AmbiguousMember { count: 2, .. }
    ));
    table_mut(&mut dup, true, |table| {
        for row in table.as_chunks_mut::<16>().0 {
            if word(row, 12) != u32::MAX {
                row[12..16].copy_from_slice(&0u32.to_le_bytes());
            }
        }
    });
    assert_eq!(
        load(&dup, profile, framing(), duplicate_budget)
            .unwrap()
            .view()
            .raw(),
        raw
    );
    groups += 1;
    let mut wrong = bytes.clone();
    wrong[MPQ + 0x10..MPQ + 0x14].copy_from_slice(&0u32.to_le_bytes());
    assert!(matches!(
        extraction(
            load(&wrong, profile, framing(), budget).unwrap_err(),
            profile
        ),
        ReadError::MpqOpen { .. }
    ));
    let error = load(&bytes, CameraProfile::LocalAngles, framing(), budget).unwrap_err();
    assert_eq!(error.profile, CameraProfile::LocalAngles);
    assert!(matches!(
        error.cause,
        CameraLoadCause::Framing(CameraError::CountFraming { .. })
    ));
    groups += 1;
    let empty = [0u8; 8];
    let bytes = map_fixture(&[(CAMERAS_NAME, &empty)]);
    let cameras = load(
        &bytes,
        CameraProfile::LocalAngles,
        framing(),
        read_budget(&bytes),
    )
    .unwrap();
    assert_eq!(cameras.view().record_count(), 0);
    assert_eq!(cameras.view().raw(), empty);
    groups += 1;
    groups
}
#[test]
fn actual_camera_extraction_preserves_raw_profiles_and_refuses_each_boundary() {
    assert_eq!(composed_cases(), 10);
}
