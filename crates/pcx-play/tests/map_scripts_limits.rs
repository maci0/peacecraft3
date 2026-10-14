//! Synthetic archive fixtures exercise import refusal, not native execution.
use pcx_map::Map;
use pcx_play::map_scripts_import::{MapScriptBudget, MapScriptImportError, import_map_scripts};
use pcx_play::scripts::{AssetIdentity, ScriptAsset};

const COMMON: &[u8] = b"native SyntheticNative takes nothing returns nothing\n";
const BLIZZARD: &[u8] = b"function SyntheticPrelude takes nothing returns nothing\nendfunction\n";
const SCRIPT: &[u8] = b"function config takes nothing returns nothing\nendfunction\nfunction main takes nothing returns nothing\nendfunction\n";
const WTS: &[u8] = b"STRING 1\r\n{\r\nSynthetic body\r\n}\r\n";
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

fn import(
    bytes: &[u8],
    budget: MapScriptBudget,
) -> Result<pcx_play::scripts::MapScripts, MapScriptImportError> {
    let map = Map::parse(bytes).unwrap();
    import_map_scripts(
        &map,
        "synthetic-map",
        AssetIdentity {
            edition: "synthetic".into(),
            version: "fixture-v1".into(),
            source: "synthetic-local-bytes".into(),
        },
        ScriptAsset {
            label: "synthetic-common.j",
            bytes: COMMON,
        },
        ScriptAsset {
            label: "synthetic-Blizzard.j",
            bytes: BLIZZARD,
        },
        budget,
    )
}

#[test]
fn archive_and_table_limits_precede_malformed_table_lookup() {
    let mut bytes = fixture(&[("war3map.j", SCRIPT)]);
    table_mut(&mut bytes, true, |table| {
        for row in table.as_chunks_mut::<16>().0 {
            if word(row, 12) != u32::MAX {
                row[12..16].copy_from_slice(&99u32.to_le_bytes());
            }
        }
    });
    let archive_len = bytes.len() - MPQ;
    let budget = MapScriptBudget {
        max_archive_bytes: archive_len - 1,
        ..Default::default()
    };
    assert!(matches!(
        import(&bytes, budget),
        Err(MapScriptImportError::BudgetExceeded { .. })
    ));
    let budget = MapScriptBudget {
        max_archive_table_bytes: 1,
        ..Default::default()
    };
    assert!(matches!(
        import(&bytes, budget),
        Err(MapScriptImportError::ArchiveTableBudgetExceeded { .. })
    ));
}

#[test]
fn oversized_table_declaration_is_refused_before_opening_inconsistent_tables() {
    let mut bytes = fixture(&[("war3map.j", SCRIPT)]);
    bytes[MPQ + 0x18..MPQ + 0x1c].copy_from_slice(&65536u32.to_le_bytes());
    assert!(matches!(
        import(&bytes, Default::default()),
        Err(MapScriptImportError::ArchiveTableBudgetExceeded { .. })
    ));
}

#[test]
fn member_and_aggregate_limits_precede_bad_payload_compilation() {
    let bad_script = vec![b'!'; SCRIPT.len()];
    let bytes = fixture(&[("war3map.j", &bad_script), ("war3map.wts", b"bad WTS")]);
    for budget in [
        MapScriptBudget {
            max_raw_packed_bytes: SCRIPT.len() - 1,
            ..Default::default()
        },
        MapScriptBudget {
            max_decoded_bytes: SCRIPT.len() - 1,
            ..Default::default()
        },
        MapScriptBudget {
            max_script_bytes: SCRIPT.len() - 1,
            ..Default::default()
        },
        MapScriptBudget {
            max_wts_bytes: 1,
            ..Default::default()
        },
    ] {
        assert!(matches!(
            import(&bytes, budget),
            Err(MapScriptImportError::BudgetExceeded { .. })
        ));
    }
    let budget = MapScriptBudget {
        max_prelude_bytes: COMMON.len() + BLIZZARD.len() - 1,
        ..Default::default()
    };
    assert!(matches!(
        import(&bytes, budget),
        Err(MapScriptImportError::AggregatePreludeExceeded { .. })
    ));
    let input =
        bytes.len() - MPQ + bad_script.len() + b"bad WTS".len() + COMMON.len() + BLIZZARD.len();
    let budget = MapScriptBudget {
        max_aggregate_input_bytes: input - 1,
        ..Default::default()
    };
    assert!(matches!(
        import(&bytes, budget),
        Err(MapScriptImportError::AggregateInputExceeded { .. })
    ));
}

#[test]
fn aggregate_limit_counts_archive_clone_and_accepts_exact_boundary() {
    let bytes = fixture(&[("war3map.j", SCRIPT), ("war3map.wts", WTS)]);
    let input = bytes.len() - MPQ + SCRIPT.len() + WTS.len() + COMMON.len() + BLIZZARD.len();
    let budget = MapScriptBudget {
        max_aggregate_input_bytes: input - 1,
        ..Default::default()
    };
    assert!(matches!(
        import(&bytes, budget),
        Err(MapScriptImportError::AggregateInputExceeded { .. })
    ));
    let budget = MapScriptBudget {
        max_aggregate_input_bytes: input,
        ..Default::default()
    };
    assert!(import(&bytes, budget).is_ok());
}

#[test]
fn packed_and_decoded_declarations_are_independently_bounded() {
    for (offset, budget) in [
        (
            4,
            MapScriptBudget {
                max_raw_packed_bytes: 512,
                ..Default::default()
            },
        ),
        (
            8,
            MapScriptBudget {
                max_decoded_bytes: 512,
                ..Default::default()
            },
        ),
    ] {
        let mut bytes = fixture(&[("war3map.j", SCRIPT)]);
        table_mut(&mut bytes, false, |table| {
            table[offset..offset + 4].copy_from_slice(&1024u32.to_le_bytes())
        });
        assert!(matches!(
            import(&bytes, budget),
            Err(MapScriptImportError::BudgetExceeded { .. })
        ));
    }
}

#[test]
fn localized_or_distinct_block_candidates_are_refused_but_exact_duplicates_are_allowed() {
    for mode in ["platform", "block", "identical"] {
        let mut bytes = fixture(&[("war3map.j", SCRIPT), ("war3map.j", SCRIPT)]);
        table_mut(&mut bytes, true, |table| {
            let matching: Vec<usize> = table
                .as_chunks::<16>()
                .0
                .iter()
                .enumerate()
                .filter_map(|(i, row)| (word(row, 12) != u32::MAX).then_some(i))
                .collect();
            let first = matching[0] * 16;
            let second = matching[1] * 16;
            if mode != "block" {
                let block = word(table, first + 12);
                table[second + 12..second + 16].copy_from_slice(&block.to_le_bytes());
            }
            if mode == "platform" {
                table[second + 10..second + 12].copy_from_slice(&1u16.to_le_bytes());
            }
        });
        let result = import(&bytes, Default::default());
        if mode == "identical" {
            assert!(result.is_ok());
        } else {
            assert!(matches!(
                result,
                Err(MapScriptImportError::AmbiguousLocalizedScript { count: 2, .. })
            ));
        }
    }
}

#[test]
fn invalid_sector_sizes_and_versions_fail_without_panicking() {
    for shift in [23, 31, 32, 255] {
        let mut bytes = fixture(&[("war3map.j", SCRIPT)]);
        bytes[MPQ + 0x0e] = shift;
        assert!(import(&bytes, Default::default()).is_err());
    }
    let mut bytes = fixture(&[("war3map.j", SCRIPT)]);
    bytes[MPQ + 0x0c..MPQ + 0x0e].copy_from_slice(&1u16.to_le_bytes());
    assert!(matches!(
        import(&bytes, Default::default()),
        Err(MapScriptImportError::MpqOpen { .. })
    ));
}

#[test]
fn missing_wts_and_malformed_matching_wts_are_distinct() {
    assert!(import(&fixture(&[("war3map.j", SCRIPT)]), Default::default()).is_ok());
    let mut bytes = fixture(&[("war3map.j", SCRIPT), ("war3map.wts", WTS)]);
    let expected = pcx_mpq::hash::hash_string_both("war3map.wts");
    table_mut(&mut bytes, true, |table| {
        for row in table.as_chunks_mut::<16>().0 {
            if (word(row, 0), word(row, 4)) == expected {
                row[12..16].copy_from_slice(&99u32.to_le_bytes());
            }
        }
    });
    assert!(matches!(
        import(&bytes, Default::default()),
        Err(MapScriptImportError::MalformedWts { .. })
    ));
    for malformed in [
        b"STRING 1\r\n{\r\nunterminated".as_slice(),
        b"STRING 1\r\n{\r\na\r\n}\r\nSTRING 1\r\n{\r\nb\r\n}\r\n".as_slice(),
    ] {
        let bytes = fixture(&[("war3map.j", SCRIPT), ("war3map.wts", malformed)]);
        assert!(matches!(
            import(&bytes, Default::default()),
            Err(MapScriptImportError::MalformedWts { .. })
        ));
    }
}

#[test]
fn payload_extents_are_checked_for_both_members() {
    for index in [0, 1] {
        let mut bytes = fixture(&[("war3map.j", SCRIPT), ("war3map.wts", WTS)]);
        let archive_len = (bytes.len() - MPQ) as u32;
        table_mut(&mut bytes, false, |table| {
            let row = &mut table[index * 16..index * 16 + 16];
            row[..4].copy_from_slice(&archive_len.to_le_bytes());
        });
        let result = import(&bytes, Default::default());
        if index == 0 {
            assert!(matches!(
                result,
                Err(MapScriptImportError::ExtractionFailed { .. })
            ));
        } else {
            assert!(matches!(
                result,
                Err(MapScriptImportError::MalformedWts { .. })
            ));
        }
    }
}

#[test]
fn unused_non_utf8_wts_is_preserved_and_only_requested_body_is_refused() {
    let mut script = SCRIPT.to_vec();
    script.extend_from_slice(
        b"function message takes nothing returns string\nreturn \"TRIGSTR_1\"\nendfunction\n",
    );
    let bytes = fixture(&[
        ("war3map.j", &script),
        ("war3map.wts", b"STRING 1\r\n{\r\n\xff\r\n}\r\n"),
    ]);
    let scripts = import(&bytes, Default::default())
        .expect("raw WTS body must not become a whole-file UTF-8 refusal");
    scripts
        .probe()
        .expect("unused invalid text does not break synthetic config/main");
    let mut vm = scripts.vm(pcx_script::NullHost);
    let error = vm
        .call("message", &[])
        .expect_err("requested non-UTF-8 body has a contextual refusal");
    assert!(
        error.to_string().contains("WTS entry 1 is not UTF-8"),
        "{error}"
    );
}
