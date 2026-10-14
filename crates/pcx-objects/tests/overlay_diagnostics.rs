//! Synthetic caller/archive order and overlay diagnostic gate; no native priority policy.
use pcx_objects::editor::{Applied, Editor, ObjectKind, Overlay, Unresolved};
use pcx_objects::{ObjectFile, Value};
use std::path::PathBuf;

type RecordFixture = ([u8; 4], [u8; 4], Vec<u8>, u32);
fn object_bytes(base: &[RecordFixture], custom: &[RecordFixture]) -> Vec<u8> {
    let mut raw = 2u32.to_le_bytes().to_vec();
    for table in [base, custom] {
        raw.extend((table.len() as u32).to_le_bytes());
        for (original, modified, mods, count) in table {
            raw.extend(original);
            raw.extend(modified);
            raw.extend(count.to_le_bytes());
            raw.extend(mods);
        }
    }
    raw
}
fn editor_archive(map_members: &[(&str, &[u8])], test: &str) -> (Editor, pcx_mpq::MpqSet, PathBuf) {
    let base =
        b"ID;PWXL;N;E\r\nC;X1;Y1;K\"unitID\"\r\nC;X1;Y2;K\"BASE\"\r\nC;X1;Y3;K\"TAKN\"\r\nE\r\n";
    let map = build_synthetic_map("SyntheticOverlay", map_members);
    let outer = build_synthetic_map(
        "SyntheticInstall",
        &[
            ("(listfile)", b"Units\\UnitData.slk\nScenario.w3m\n"),
            ("Units\\UnitData.slk", base),
            ("Scenario.w3m", &map),
        ],
    );
    let dir = std::env::temp_dir().join(format!("pcx-overlay-{}-{test}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("War3.mpq"), &outer[0x200..]).unwrap();
    let mut set = pcx_mpq::MpqSet::open_wc3(&dir).unwrap();
    let editor = Editor::open_in(&mut set, &dir).unwrap();
    assert_eq!(editor.resolve("BASE"), Some(ObjectKind::Unit));
    (editor, set, dir)
}
#[test]
fn actual_map_consumer_refuses_missing_custom_original_in_both_member_families() {
    let unit = object_bytes(&[], &[(*b"MISS", *b"NEWU", vec![], 0)]);
    let item = object_bytes(&[], &[(*b"LOST", *b"NEWI", vec![], 0)]);
    // Listfile order is caller-authored and retained; this does not assert native precedence.
    let (editor, mut set, dir) = editor_archive(
        &[
            ("(listfile)", b"war3map.w3t\nwar3map.w3u\n"),
            ("war3map.w3u", &unit),
            ("war3map.w3t", &item),
        ],
        "missing",
    );
    let applied = Applied::apply_map(&editor, &mut set, "Scenario.w3m").unwrap();
    assert_eq!(applied.reports.len(), 2);
    for (report, name, rawcode) in [
        (&applied.reports[0], "war3map.w3t", *b"LOST"),
        (&applied.reports[1], "war3map.w3u", *b"MISS"),
    ] {
        assert_eq!(report.name, name);
        assert_eq!(report.map, "Scenario.w3m");
        assert_eq!(report.build, editor.build());
        assert!(!report.clean());
        assert!(!report.resolved_all());
        assert!(report.resolved.is_empty());
        assert_eq!(report.custom_records(), 1);
        assert_eq!(report.base_records(), 0);
        assert_eq!(
            report.unresolved,
            vec![Unresolved::UnknownOriginal {
                rawcode,
                table: 1,
                map: "Scenario.w3m".into()
            }]
        );
        assert!(
            report.unresolved[0]
                .to_string()
                .contains("custom record derives from")
        );
    }
    assert_eq!(applied.unresolved().count(), 2);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn custom_collision_keeps_precedence_even_when_original_is_missing() {
    let (editor, _set, dir) = editor_archive(&[], "collision");
    let raw = object_bytes(&[], &[(*b"MISS", *b"TAKN", vec![], 0)]);
    let file = ObjectFile::parse("war3map.w3u", &raw).unwrap();
    let report = Applied::default().push(&editor, "M", "war3map.w3u", &file);
    assert_eq!(
        report.unresolved,
        vec![Unresolved::ModifiedCollides {
            rawcode: *b"TAKN",
            kind: ObjectKind::Unit
        }]
    );
    assert!(report.resolved.is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn missing_base_table_original_retains_existing_refusal_and_display() {
    let (editor, _set, dir) = editor_archive(&[], "base-missing");
    let raw = object_bytes(&[(*b"MISS", [0; 4], vec![], 0)], &[]);
    let file = ObjectFile::parse("war3map.w3u", &raw).unwrap();
    let report = Applied::default().push(&editor, "MapLabel", "war3map.w3u", &file);
    assert_eq!(
        report.unresolved,
        vec![Unresolved::UnknownOriginal {
            rawcode: *b"MISS",
            table: 0,
            map: "MapLabel".into()
        }]
    );
    assert_eq!(report.base_records(), 1);
    assert_eq!(report.custom_records(), 0);
    assert!(
        report.unresolved[0]
            .to_string()
            .contains("base record modifies")
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn valid_custom_records_keep_raw_overrides_selector_pairs_and_field_refusals() {
    let (editor, _set, dir) = editor_archive(&[], "valid");
    let mut mods = Vec::new();
    for (field, wire, payload, end) in [
        (
            *b"I001",
            0u32,
            0xffff_fff1u32.to_le_bytes().to_vec(),
            0x1357_2468u32,
        ),
        (
            *b"F001",
            1,
            (-0.0f32).to_bits().to_le_bytes().to_vec(),
            0x2468_1357,
        ),
        (*b"S001", 3, b"raw\xff\0".to_vec(), 0x9753_8642),
    ] {
        mods.extend(field);
        mods.extend(wire.to_le_bytes());
        mods.extend(payload);
        mods.extend(end.to_le_bytes());
    }
    let raw = object_bytes(&[], &[(*b"BASE", *b"NEWU", mods, 3)]);
    let file = ObjectFile::parse("war3map.w3u", &raw).unwrap();
    let overlay = Overlay::of(&file);
    let report = Applied::default().push(&editor, "M", "war3map.w3u", &file);
    assert!(report.resolved_all());
    assert!(!report.clean());
    assert_eq!(report.resolved[0].kind, Some(ObjectKind::Unit));
    assert_eq!(report.resolved[0].table, 1);
    assert_eq!(report.mods(), 3);
    assert_eq!(report.refusals.len(), 3);
    let (base, stored) = overlay.new_object("NEWU").unwrap();
    assert_eq!(base, "BASE");
    assert_eq!(stored, file.custom()[0].mods);
    assert_eq!(stored[0].value, Value::Int(0xffff_fff1));
    assert_eq!(stored[0].end_word, 0x1357_2468);
    match stored[1].value {
        Value::Real(v) => assert_eq!(v.to_bits(), (-0.0f32).to_bits()),
        _ => panic!("wire changed"),
    }
    assert_eq!(stored[2].value, Value::Str(b"raw\xff".to_vec()));
    assert_eq!(file.raw(), raw);
    // Indexed wire variants remain separate, not flattened into scalar stat defaults.
    let mut indexed = Vec::new();
    for selector in [3u32, 7] {
        indexed.extend(b"DATA");
        for word in [0u32, selector, 11, selector + 5, 0x4321_1234] {
            indexed.extend(word.to_le_bytes());
        }
    }
    let raw = object_bytes(&[], &[(*b"BASE", *b"NEWQ", indexed, 2)]);
    let file = ObjectFile::parse("war3map.w3q", &raw).unwrap();
    let overlay = Overlay::of(&file);
    let report = Applied::default().push(&editor, "M", "war3map.w3q", &file);
    assert!(report.resolved_all());
    assert_eq!(report.refusals.len(), 4);
    assert_eq!(
        overlay.indexed_value("NEWQ", *b"DATA", [3, 11]),
        Some(&Value::Int(8))
    );
    assert_eq!(
        overlay.indexed_value("NEWQ", *b"DATA", [7, 11]),
        Some(&Value::Int(12))
    );
    assert_eq!(overlay.new_object("NEWQ").unwrap().1, file.custom()[0].mods);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn mixed_custom_records_count_valid_and_unresolved_without_replacing_valid_resolution() {
    let (editor, _set, dir) = editor_archive(&[], "mixed");
    let raw = object_bytes(
        &[],
        &[
            (*b"BASE", *b"GOOD", vec![], 0),
            (*b"MISS", *b"BAD0", vec![], 0),
        ],
    );
    let file = ObjectFile::parse("war3map.w3t", &raw).unwrap();
    let mut applied = Applied::default();
    let report = applied.push(&editor, "SourceMap", "war3map.w3t", &file);
    assert_eq!(report.custom_records(), 2);
    assert_eq!(report.resolved.len(), 1);
    assert_eq!(report.resolved[0].rawcode, "BASE");
    assert_eq!(report.unresolved.len(), 1);
    assert_eq!(applied.reports[0].name, "war3map.w3t");
    assert!(!report.resolved_all());
    let cleanraw = object_bytes(&[], &[(*b"BASE", *b"GOOD", vec![], 0)]);
    let cleanfile = ObjectFile::parse("war3map.w3t", &cleanraw).unwrap();
    let clean = applied.push(&editor, "AnotherMap", "war3map.w3t", &cleanfile);
    assert!(clean.clean());
    assert_eq!(applied.reports.len(), 2);
    std::fs::remove_dir_all(dir).unwrap();
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
        while !payloads.len().is_multiple_of(4) {
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
