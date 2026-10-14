//! Integration tests for bounded map scripts import pipeline.
//!
//! Validates actual extraction path, malformed MPQ, missing script member,
//! WTS extraction and localization, identity and digest preservation, and explicit
//! budget enforcement before decoded allocation.
//! Gated actual Legacy Human01 compilation and exact existing NullHost first failure
//! prove genuine sources without claiming chapter success.

use pcx_map::Map;

use pcx_play::map_scripts_import::{MapScriptBudget, MapScriptImportError, import_map_scripts};
use pcx_play::scripts::{AssetIdentity, ScriptAsset};

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

fn test_identity() -> AssetIdentity {
    AssetIdentity {
        edition: "test-edition".to_string(),
        version: "fixture-version".to_string(),
        source: "synthetic-archive".to_string(),
    }
}

const SAMPLE_COMMON: &[u8] = b"native SyntheticNative takes nothing returns nothing\n";
const SAMPLE_BLIZZARD: &[u8] =
    b"function BJInitGlobals takes nothing returns nothing\nendfunction\n";
const SAMPLE_WAR3MAP_J: &[u8] = b"function config takes nothing returns nothing\nendfunction\nfunction main takes nothing returns nothing\ncall BJInitGlobals()\nendfunction\n";

#[test]
fn synthetic_valid_map_extracts_and_compiles() {
    let map_bytes = build_synthetic_map("TestMap", &[("war3map.j", SAMPLE_WAR3MAP_J)]);
    let map = Map::parse(&map_bytes).expect("parse map container");

    let common = ScriptAsset {
        label: "common.j",
        bytes: SAMPLE_COMMON,
    };
    let blizzard = ScriptAsset {
        label: "Blizzard.j",
        bytes: SAMPLE_BLIZZARD,
    };

    let scripts = import_map_scripts(
        &map,
        "maps/test.w3m",
        test_identity(),
        common,
        blizzard,
        MapScriptBudget::DEFAULT,
    )
    .expect("import scripts");

    assert_eq!(scripts.identity.edition, "test-edition");
    assert_eq!(scripts.identity.version, "fixture-version");
    assert_eq!(scripts.identity.source, "synthetic-archive");
    assert_eq!(scripts.digests.len(), 3);
    assert!(scripts.program().function("main").is_some());
    assert!(scripts.program().function("config").is_some());
    assert!(scripts.program().function("BJInitGlobals").is_some());
}

#[test]
fn synthetic_valid_map_with_wts() {
    const WTS_DATA: &[u8] = b"STRING 1\r\n{\r\nMap Title\r\n}\r\n";
    let map_bytes = build_synthetic_map(
        "TestMapWts",
        &[("war3map.j", SAMPLE_WAR3MAP_J), ("war3map.wts", WTS_DATA)],
    );
    let map = Map::parse(&map_bytes).expect("parse map container");

    let common = ScriptAsset {
        label: "common.j",
        bytes: SAMPLE_COMMON,
    };
    let blizzard = ScriptAsset {
        label: "Blizzard.j",
        bytes: SAMPLE_BLIZZARD,
    };

    let scripts = import_map_scripts(
        &map,
        "maps/test_wts.w3m",
        test_identity(),
        common,
        blizzard,
        MapScriptBudget::DEFAULT,
    )
    .expect("import scripts");

    assert_eq!(scripts.digests.len(), 4);
    assert_eq!(scripts.digests[3].label, "maps/test_wts.w3m:war3map.wts");
}

#[test]
fn synthetic_missing_war3map_j() {
    let map_bytes = build_synthetic_map("NoScript", &[("dummy.txt", b"hello world")]);
    let map = Map::parse(&map_bytes).expect("parse map container");

    let common = ScriptAsset {
        label: "common.j",
        bytes: SAMPLE_COMMON,
    };
    let blizzard = ScriptAsset {
        label: "Blizzard.j",
        bytes: SAMPLE_BLIZZARD,
    };

    match import_map_scripts(
        &map,
        "maps/noscript.w3m",
        test_identity(),
        common,
        blizzard,
        MapScriptBudget::DEFAULT,
    ) {
        Err(MapScriptImportError::MissingScriptMember { map, member }) => {
            assert_eq!(map, "maps/noscript.w3m");
            assert_eq!(member, "war3map.j");
        }
        Ok(_) => panic!("expected MissingScriptMember, got Ok"),
        Err(err) => panic!("expected MissingScriptMember, got {err}"),
    }
}

#[test]
fn synthetic_script_budget_exceeded_before_allocation() {
    let large_script = vec![b' '; 2048];
    let map_bytes = build_synthetic_map("LargeScript", &[("war3map.j", &large_script)]);
    let map = Map::parse(&map_bytes).expect("parse map container");

    let common = ScriptAsset {
        label: "common.j",
        bytes: SAMPLE_COMMON,
    };
    let blizzard = ScriptAsset {
        label: "Blizzard.j",
        bytes: SAMPLE_BLIZZARD,
    };

    let budget = MapScriptBudget {
        max_script_bytes: 1024,
        max_wts_bytes: 1024,
        max_archive_bytes: MapScriptBudget::DEFAULT.max_archive_bytes,
        max_archive_table_bytes: MapScriptBudget::DEFAULT.max_archive_table_bytes,
        max_raw_packed_bytes: MapScriptBudget::DEFAULT.max_raw_packed_bytes,
        max_decoded_bytes: MapScriptBudget::DEFAULT.max_decoded_bytes,
        max_prelude_bytes: MapScriptBudget::DEFAULT.max_prelude_bytes,
        max_aggregate_input_bytes: MapScriptBudget::DEFAULT.max_aggregate_input_bytes,
    };

    match import_map_scripts(
        &map,
        "maps/largescript.w3m",
        test_identity(),
        common,
        blizzard,
        budget,
    ) {
        Err(MapScriptImportError::BudgetExceeded {
            map,
            member,
            declared_bytes,
            max_bytes,
        }) => {
            assert_eq!(map, "maps/largescript.w3m");
            assert_eq!(member, "war3map.j");
            assert_eq!(declared_bytes, 2048);
            assert_eq!(max_bytes, 1024);
        }
        Ok(_) => panic!("expected BudgetExceeded, got Ok"),
        Err(err) => panic!("expected BudgetExceeded, got {err}"),
    }
}

#[test]
fn synthetic_wts_budget_exceeded() {
    let large_wts = vec![b' '; 2048];
    let map_bytes = build_synthetic_map(
        "LargeWts",
        &[("war3map.j", SAMPLE_WAR3MAP_J), ("war3map.wts", &large_wts)],
    );
    let map = Map::parse(&map_bytes).expect("parse map container");

    let common = ScriptAsset {
        label: "common.j",
        bytes: SAMPLE_COMMON,
    };
    let blizzard = ScriptAsset {
        label: "Blizzard.j",
        bytes: SAMPLE_BLIZZARD,
    };

    let budget = MapScriptBudget {
        max_script_bytes: 4096,
        max_wts_bytes: 1024,
        max_archive_bytes: MapScriptBudget::DEFAULT.max_archive_bytes,
        max_archive_table_bytes: MapScriptBudget::DEFAULT.max_archive_table_bytes,
        max_raw_packed_bytes: MapScriptBudget::DEFAULT.max_raw_packed_bytes,
        max_decoded_bytes: MapScriptBudget::DEFAULT.max_decoded_bytes,
        max_prelude_bytes: MapScriptBudget::DEFAULT.max_prelude_bytes,
        max_aggregate_input_bytes: MapScriptBudget::DEFAULT.max_aggregate_input_bytes,
    };

    match import_map_scripts(
        &map,
        "maps/largewts.w3m",
        test_identity(),
        common,
        blizzard,
        budget,
    ) {
        Err(MapScriptImportError::BudgetExceeded {
            map,
            member,
            declared_bytes,
            max_bytes,
        }) => {
            assert_eq!(map, "maps/largewts.w3m");
            assert_eq!(member, "war3map.wts");
            assert_eq!(declared_bytes, 2048);
            assert_eq!(max_bytes, 1024);
        }
        Ok(_) => panic!("expected BudgetExceeded, got Ok"),
        Err(err) => panic!("expected BudgetExceeded, got {err}"),
    }
}

#[test]
fn synthetic_invalid_identity_rejected() {
    let map_bytes = build_synthetic_map("TestMap", &[("war3map.j", SAMPLE_WAR3MAP_J)]);
    let map = Map::parse(&map_bytes).expect("parse map container");

    let common = ScriptAsset {
        label: "common.j",
        bytes: SAMPLE_COMMON,
    };
    let blizzard = ScriptAsset {
        label: "Blizzard.j",
        bytes: SAMPLE_BLIZZARD,
    };

    let empty_edition = AssetIdentity {
        edition: "".to_string(),
        version: "1.29".to_string(),
        source: "src".to_string(),
    };
    assert!(matches!(
        import_map_scripts(
            &map,
            "map",
            empty_edition,
            common,
            blizzard,
            MapScriptBudget::DEFAULT
        ),
        Err(MapScriptImportError::InvalidIdentity { field: "edition" })
    ));
}

#[test]
fn synthetic_empty_prelude_rejected() {
    let map_bytes = build_synthetic_map("TestMap", &[("war3map.j", SAMPLE_WAR3MAP_J)]);
    let map = Map::parse(&map_bytes).expect("parse map container");

    let empty_common = ScriptAsset {
        label: "common.j",
        bytes: b"",
    };
    let blizzard = ScriptAsset {
        label: "Blizzard.j",
        bytes: SAMPLE_BLIZZARD,
    };

    assert!(matches!(
        import_map_scripts(
            &map,
            "map",
            test_identity(),
            empty_common,
            blizzard,
            MapScriptBudget::DEFAULT
        ),
        Err(MapScriptImportError::EmptyPrelude { .. })
    ));
}

#[test]
fn synthetic_malformed_mpq() {
    let mut map_bytes = build_synthetic_map("MalformedMpq", &[("war3map.j", SAMPLE_WAR3MAP_J)]);
    // Corrupt the hash table position in the MPQ header (offset 0x200 + 0x10) to out-of-bounds
    map_bytes[0x200 + 0x10..0x200 + 0x14].copy_from_slice(&0x00FF_FFFFu32.to_le_bytes());
    let map = Map::parse(&map_bytes).expect("parse map container");

    let common = ScriptAsset {
        label: "common.j",
        bytes: SAMPLE_COMMON,
    };
    let blizzard = ScriptAsset {
        label: "Blizzard.j",
        bytes: SAMPLE_BLIZZARD,
    };

    match import_map_scripts(
        &map,
        "maps/malformed.w3m",
        test_identity(),
        common,
        blizzard,
        MapScriptBudget::DEFAULT,
    ) {
        Err(MapScriptImportError::MpqOpen { map, .. }) => {
            assert_eq!(map, "maps/malformed.w3m");
        }
        Ok(_) => panic!("expected MpqOpen error, got Ok"),
        Err(err) => panic!("expected MpqOpen error, got {err}"),
    }
}

#[test]
#[ignore = "requires explicit local original map/prelude paths and caller-verified asset identity"]
fn gated_original_map_compiles_and_reports_nullhost_boundary() {
    let required = |name: &str| {
        std::env::var(name).unwrap_or_else(|_| panic!("{name} is required for this original gate"))
    };
    let common_path = required("PEACECRAFT3_MAP_IMPORT_COMMON");
    let blizzard_path = required("PEACECRAFT3_MAP_IMPORT_BLIZZARD");
    let map_path = required("PEACECRAFT3_MAP_IMPORT_MAP");
    let common_bytes = std::fs::read(&common_path).expect("read explicit common.j source");
    let blizzard_bytes = std::fs::read(&blizzard_path).expect("read explicit Blizzard.j source");
    let map_bytes = std::fs::read(&map_path).expect("read explicit original map source");
    let map = Map::parse(&map_bytes).expect("original map framing");
    let identity = AssetIdentity {
        edition: required("PEACECRAFT3_MAP_IMPORT_EDITION"),
        version: required("PEACECRAFT3_MAP_IMPORT_VERSION"),
        source: required("PEACECRAFT3_MAP_IMPORT_SOURCE"),
    };
    let scripts = import_map_scripts(
        &map,
        &map_path,
        identity,
        ScriptAsset {
            label: &common_path,
            bytes: &common_bytes,
        },
        ScriptAsset {
            label: &blizzard_path,
            bytes: &blizzard_bytes,
        },
        MapScriptBudget::DEFAULT,
    )
    .expect("original map and supplied preludes compile");
    assert!(scripts.program().function("main").is_some());
    assert!(scripts.program().function("config").is_some());
    let error = scripts
        .probe()
        .expect_err("unimplemented native execution must not become chapter success");
    assert!(
        error.contains("native `") && error.contains("not implemented"),
        "{error}"
    );
    eprintln!(
        "Original map import: identity={:?} digests={:?}; first execution boundary={error}",
        scripts.identity, scripts.digests
    );
}

#[test]
fn ambiguous_localized_script_members_are_refused() {
    // Two different `war3map.j` records with the same hash name but different
    // locale words must be refused; silent first-match selection is never
    // permitted per the ambiguity refusal policy.
    let a = b"function main takes nothing returns nothing\nendfunction\n";
    let b_bytes = b"function different takes nothing returns nothing\nendfunction\nfunction main takes nothing returns nothing\nendfunction\n";
    let map_bytes = build_synthetic_map_with_locale(
        "SyntheticAmbig",
        &[("war3map.j", a, 1), ("war3map.j", b_bytes, 2)],
    );
    let map = Map::parse(&map_bytes).expect("synthetic ambiguous map framing");
    let common = ScriptAsset {
        label: "synthetic-common.j",
        bytes: b"native Foo takes nothing returns nothing\n",
    };
    let blizzard = ScriptAsset {
        label: "synthetic-blizzard.j",
        bytes: b"function BJInitGlobals takes nothing returns nothing\nendfunction\n",
    };
    let result = import_map_scripts(
        &map,
        "synthetic-ambig",
        AssetIdentity {
            edition: "synthetic".into(),
            version: "synthetic".into(),
            source: "synthetic".into(),
        },
        common,
        blizzard,
        MapScriptBudget::DEFAULT,
    );
    assert!(
        result.is_err(),
        "two different locale candidates for war3map.j require explicit source selection or ambiguity refusal"
    );
}

fn build_synthetic_map_with_locale(map_name: &str, files: &[(&str, &[u8], u16)]) -> Vec<u8> {
    // Same synthetic MPQ builder as `build_synthetic_map` but accepts locale.
    const HASH_ENTRIES: usize = 16;
    let mut payloads = Vec::new();
    let mut block_entries = Vec::new();
    let payload_base_offset = 0x40usize;
    for (_name, data, _locale) in files {
        let file_pos = payload_base_offset + payloads.len();
        let file_size = data.len() as u32;
        let compressed_size = file_size;
        let flags: u32 = 0x8000_0000 | 0x0100_0000;
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
            block_index: 0xFFFF_FFFF
        };
        HASH_ENTRIES
    ];
    for (i, (name, _, locale)) in files.iter().enumerate() {
        let (name1, name2) = pcx_mpq::hash::hash_string_both(name);
        let mut slot = pcx_mpq::hash::start_slot(name, HASH_ENTRIES);
        while hash_entries[slot].block_index != 0xFFFF_FFFF {
            slot = (slot + 1) % HASH_ENTRIES;
        }
        hash_entries[slot].name1 = name1;
        hash_entries[slot].name2 = name2;
        hash_entries[slot].locale = *locale;
        hash_entries[slot].platform = 0;
        hash_entries[slot].block_index = i as u32;
    }
    // Note: the budget-check phase only sees the first entry's locale when
    // `find_script_blocks_ambiguity_checked` walks. The synthetic test
    // creates two hash entries with the same (name1, name2) but different
    // locale values; `find_script_blocks_ambiguity_checked` collects both,
    // detects `distinct.len() > 1`, and refuses.
    let mut hash_table_raw = Vec::with_capacity(HASH_ENTRIES * 16);
    for entry in &hash_entries {
        hash_table_raw.extend_from_slice(&entry.name1.to_le_bytes());
        hash_table_raw.extend_from_slice(&entry.name2.to_le_bytes());
        hash_table_raw.extend_from_slice(&entry.locale.to_le_bytes());
        hash_table_raw.extend_from_slice(&entry.platform.to_le_bytes());
        hash_table_raw.extend_from_slice(&entry.block_index.to_le_bytes());
    }
    let key = pcx_mpq::hash::hash_table_key();
    let mut seed1 = key;
    let mut seed2 = 0xEEEE_EEEEu32;
    for chunk in hash_table_raw.as_chunks_mut::<4>().0 {
        seed2 = seed2.wrapping_add(pcx_mpq::crypt::table()[0x400 + (seed1 & 0xFF) as usize]);
        let plaintext = u32::from_le_bytes(*chunk);
        let ciphertext = plaintext ^ seed1.wrapping_add(seed2);
        chunk.copy_from_slice(&ciphertext.to_le_bytes());
        seed1 = ((!seed1) << 0x15).wrapping_add(0x1111_1111) | (seed1 >> 0x0B);
        seed2 = plaintext
            .wrapping_add(seed2)
            .wrapping_add(seed2 << 5)
            .wrapping_add(3);
    }
    let block_table_pos = hash_table_pos + (HASH_ENTRIES as u32 * 16);
    let mut block_table_raw = Vec::with_capacity(block_entries.len() * 16);
    for (pos, comp, size, flags) in &block_entries {
        block_table_raw.extend_from_slice(&pos.to_le_bytes());
        block_table_raw.extend_from_slice(&comp.to_le_bytes());
        block_table_raw.extend_from_slice(&size.to_le_bytes());
        block_table_raw.extend_from_slice(&(*flags).to_le_bytes());
    }
    let block_key = pcx_mpq::hash::block_table_key();
    let mut seed1_b = block_key;
    let mut seed2_b = 0xEEEE_EEEEu32;
    for chunk in block_table_raw.as_chunks_mut::<4>().0 {
        seed2_b = seed2_b.wrapping_add(pcx_mpq::crypt::table()[0x400 + (seed1_b & 0xFF) as usize]);
        let plaintext = u32::from_le_bytes(*chunk);
        let ciphertext = plaintext ^ seed1_b.wrapping_add(seed2_b);
        chunk.copy_from_slice(&ciphertext.to_le_bytes());
        seed1_b = ((!seed1_b) << 0x15).wrapping_add(0x1111_1111) | (seed1_b >> 0x0B);
        seed2_b = plaintext
            .wrapping_add(seed2_b)
            .wrapping_add(seed2_b << 5)
            .wrapping_add(3);
    }
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
