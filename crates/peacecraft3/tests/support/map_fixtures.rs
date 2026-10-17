//! Synthetic fixture helpers reused from existing world/script import tests.
use pcx_map::{MapInfo, Pathing, RandomUnit, UnitPlacement, Units};
use pcx_objects::Fourcc;
use pcx_objects::editor::ObjectKind;
use pcx_sim::world::{
    Definition, FieldValue, ObjectCatalog, ObjectField, ResolutionProblem, Source,
};
use std::collections::BTreeMap;
pub(crate) fn source() -> Source {
    Source {
        install: "synthetic".into(),
        edition: "synthetic schema".into(),
        map: "scenario".into(),
    }
}
pub(crate) fn info() -> MapInfo {
    // Empty version-18 sections, each authored explicitly in this fixture.
    let mut bytes = Vec::new();
    for word in [18u32, 0, 0] {
        bytes.extend(word.to_le_bytes());
    }
    bytes.extend([0; 4]); // Four NUL strings.
    bytes.extend([0; 32 + 16 + 8 + 4]); // Camera, bounds and flags.
    bytes.push(0); // Tileset byte.
    bytes.extend(0u32.to_le_bytes()); // Loading word.
    bytes.extend([0; 3]);
    bytes.extend(0u32.to_le_bytes()); // Prologue word.
    bytes.extend([0; 3]);
    bytes.extend([0; 20]); // Five empty trailing counted sections.
    MapInfo::parse(&bytes).expect("synthetic map info")
}
pub(crate) fn pathing() -> Pathing {
    Pathing {
        version: 0,
        width: 2,
        height: 1,
        cells: vec![0x12, 0x84],
    }
}
pub(crate) fn placement(rawcode: Fourcc) -> UnitPlacement {
    UnitPlacement {
        type_id: rawcode,
        variation: 7,
        position: [1.25, -2.5, 3.75],
        angle: 0.125,
        scale: [1.5, 2.5, 3.5],
        flags: 9,
        control_word: u16::from_le_bytes((-2i16).to_le_bytes()),
        state_word: 0x1234,
        life: -1,
        mana: -1,
        item_table: None,
        item_sets: vec![],
        post_drop_word: 0,
        post_drop_float: -1.0,
        progression_word: 1,
        progression_words: None,
        inventory: vec![],
        abilities: vec![],
        random: RandomUnit::None,
        tail_words: [u32::MAX; 2],
        creation_number: 42,
    }
}
pub(crate) struct Catalog(
    pub(crate) BTreeMap<Fourcc, Definition>,
    pub(crate) std::cell::Cell<usize>,
);
impl ObjectCatalog for Catalog {
    fn resolve(&self, rawcode: Fourcc) -> Result<Definition, ResolutionProblem> {
        self.1.set(self.1.get() + 1);
        self.0.get(&rawcode).cloned().ok_or_else(|| {
            ResolutionProblem::MissingRawcode(String::from_utf8_lossy(&rawcode).into_owned())
        })
    }
}
pub(crate) fn definition(rawcode: Fourcc, kind: ObjectKind) -> Definition {
    Definition {
        rawcode,
        base_rawcode: String::from_utf8_lossy(&rawcode).into_owned(),
        kind,
        fields: BTreeMap::from([(
            *b"STAT",
            ObjectField {
                metadata: None,
                value: Ok(FieldValue::Cell("137".into())),
            },
        )]),
        modifications: vec![],
    }
}
pub(crate) fn mark_unused_hash_slot(map: &mut [u8]) {
    let mpq = &mut map[0x200..];
    let hash_pos = u32::from_le_bytes(mpq[0x10..0x14].try_into().unwrap()) as usize;
    let entries = u32::from_le_bytes(mpq[0x18..0x1C].try_into().unwrap()) as usize;
    let mut table = mpq[hash_pos..hash_pos + entries * 16].to_vec();
    pcx_mpq::crypt::decrypt(
        &mut table,
        pcx_mpq::hash::hash_table_key(),
        &pcx_mpq::crypt::table(),
    );
    let spare = table
        .chunks_exact(16)
        .position(|entry| entry[12..16] == [0xFF, 0xFF, 0xFF, 0xFF])
        .expect("synthetic hash table has an unused slot");
    let row = &mut table[spare * 16..spare * 16 + 16];
    row[0..4].copy_from_slice(&0x0102_0304u32.to_le_bytes());
    row[4..8].copy_from_slice(&0x0506_0708u32.to_le_bytes());
    encrypt_mpq_block(&mut table, pcx_mpq::hash::hash_table_key());
    mpq[hash_pos..hash_pos + table.len()].copy_from_slice(&table);
}
pub(crate) fn encrypt_mpq_block(data: &mut [u8], key: u32) {
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

pub(crate) fn build_synthetic_map(map_name: &str, files: &[(&str, &[u8])]) -> Vec<u8> {
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

pub(crate) fn catalog() -> Catalog {
    Catalog(
        BTreeMap::from([(*b"UNIT", definition(*b"UNIT", ObjectKind::Unit))]),
        std::cell::Cell::new(0),
    )
}
pub(crate) fn terrain_bytes(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = pcx_map::TERRAIN_MAGIC.to_vec();
    bytes.extend(pcx_map::TERRAIN_VERSION.to_le_bytes());
    bytes.push(0);
    for word in [0, 0, 0, width + 1, height + 1] {
        bytes.extend(word.to_le_bytes());
    }
    bytes.extend([0; 8]);
    bytes.resize(
        bytes.len() + ((width + 1) * (height + 1)) as usize * pcx_map::CORNER_RECORD,
        0,
    );
    bytes
}
pub(crate) fn pathing_bytes(width: u32, height: u32) -> Vec<u8> {
    let authored = pathing();
    let mut bytes = b"MP3W".to_vec();
    for word in [authored.version, width, height] {
        bytes.extend(word.to_le_bytes());
    }
    bytes.extend((0..width * height).map(|i| authored.cells[i as usize % authored.cells.len()]));
    bytes
}
pub(crate) fn units() -> Units {
    Units {
        version: 8,
        subversion: 9,
        instances: vec![placement(*b"UNIT"), placement(*b"MISS")],
    }
}
pub(crate) fn map_bytes(terrain: Option<&[u8]>, pathing: &[u8]) -> Vec<u8> {
    let info = info().canonical_bytes();
    let units = units().canonical_bytes();
    let mut files = vec![
        ("war3map.w3i", info.as_slice()),
        ("war3map.wpm", pathing),
        ("war3mapUnits.doo", units.as_slice()),
    ];
    if let Some(terrain) = terrain {
        files.push(("war3map.w3e", terrain));
    }
    build_synthetic_map("SyntheticWorldGrid", &files)
}

pub(crate) fn alter_block(bytes: &mut [u8], member: &str, change: impl FnOnce(&mut [u8])) {
    let map = pcx_map::Map::parse(bytes).unwrap();
    let offset = map.header.archive_offset;
    let header = pcx_mpq::Header::parse(&mut std::io::Cursor::new(map.archive())).unwrap();
    let archive =
        pcx_mpq::MpqArchive::from_bytes(map.archive().to_vec(), "synthetic metadata mutation")
            .unwrap();
    let (a, b) = pcx_mpq::hash::hash_string_both(member);
    let entry = archive
        .hash_entries()
        .iter()
        .find(|e| e.name1 == a && e.name2 == b)
        .unwrap();
    let index = entry.block_index as usize;
    let start = offset + header.block_table_pos as usize;
    let end = start + header.block_table_bytes() as usize;
    let key = pcx_mpq::hash::block_table_key();
    let table = pcx_mpq::crypt::table();
    pcx_mpq::crypt::decrypt(&mut bytes[start..end], key, table);
    change(&mut bytes[start + index * 16..start + (index + 1) * 16]);
    encrypt_mpq_block(&mut bytes[start..end], key);
}
