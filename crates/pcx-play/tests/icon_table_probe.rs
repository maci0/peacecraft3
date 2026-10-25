//! Build-only probe for the selected-unit attack/armor icon tables.
//!
//! It packs a synthetic map with the existing terrain and `config`/`main`
//! harness, resolves that script against the selected `common.j` and
//! `Blizzard.j`, and round-trips every packed member. It does not launch a
//! client. Root owns the isolated muted original run.
//!
//! Source rows, identical in `War3.mpq` and `War3x.mpq` `Units\UnitBalance.slk`
//! and `Units\UnitWeapons.slk` (locals have no `UnitBalance.slk`):
//!
//! - `hfoo` Footman, unchanged: `weapTp1=normal`, `atkType1=normal`,
//!   `def=2`, `defType=large`, `upgrades=Rhar,Rhme,Rhde,Rhpm,Rguv`
//! - `hfx0` is a map override whose original id is `hfoo` and whose only
//!   modification is `upgr` = `_`. Weapons and defense are not rewritten.
//!
//! `nshp` is unarmed (`weapTp1=_`) and is not a row. Different stock rawcodes
//! are not treated as an upgrades-list experiment. `GetPlayerNeutralPassive`
//! returns an integer in the selected `common.j` (line 162), so the owner is
//! `Player(GetPlayerNeutralPassive())`.

use pcx_map::info::{CONTROLLER_HUMAN, RACE_HUMAN};
use pcx_map::{Doodads, Map, MapInfo, Pathing, Units};
use pcx_objects::{ObjectFile, Value};
use pcx_play::scripts::{AssetIdentity, MapScripts, ScriptAsset};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

/// Measured world units per terrain tile (`pcx_tera::TILE_XY`).
const TILE_XY: f32 = 128.0;
const TILES: u32 = 8;
/// Game-time seconds before the first marker. Not a host-clock capture time.
const FIRST_MARKER: f32 = 45.0;
const HOLD: f32 = 20.0;

const SCRIPT: &str = r#"globals
    unit FootmanPlayer = null
    unit FootmanNeutral = null
    unit BarePlayer = null
    unit BareNeutral = null
    multiboard Board = null
endglobals

function Marker takes string row, unit which returns nothing
    call MultiboardSetTitleText(Board, row)
    call MultiboardDisplay(Board, true)
    call DisplayTimedTextToPlayer(Player(0), 0, 0, 20.00, row)
    call ClearSelection()
    call SelectUnit(which, true)
    call SetCameraPosition(GetUnitX(which), GetUnitY(which))
endfunction

function ShowFootmanPlayer takes nothing returns nothing
    call Marker("ICONROW hfoo player", FootmanPlayer)
endfunction

function ShowFootmanNeutral takes nothing returns nothing
    call Marker("ICONROW hfoo neutral", FootmanNeutral)
endfunction

function ShowBarePlayer takes nothing returns nothing
    call Marker("ICONROW hfx0 player", BarePlayer)
endfunction

function ShowBareNeutral takes nothing returns nothing
    call Marker("ICONROW hfx0 neutral", BareNeutral)
endfunction

function config takes nothing returns nothing
    call SetPlayers(1)
    call SetPlayerSlotAvailable(Player(0), MAP_CONTROL_USER)
    call FogEnable(false)
    call FogMaskEnable(false)
    call SetCameraBounds(0, 0, 0, 1024, 1024, 1024, 1024, 0)
endfunction

function main takes nothing returns nothing
    local integer neutral = GetPlayerNeutralPassive()
    local timer clock
    set Board = CreateMultiboard()
    call MultiboardMinimize(Board, false)
    set FootmanPlayer = CreateUnit(Player(0), 'hfoo', 448, 512, 270)
    set FootmanNeutral = CreateUnit(Player(neutral), 'hfoo', 576, 512, 270)
    set BarePlayer = CreateUnit(Player(0), 'hfx0', 512, 448, 270)
    set BareNeutral = CreateUnit(Player(neutral), 'hfx0', 512, 576, 270)
    set clock = CreateTimer()
    call TimerStart(clock, 45.00, false, function ShowFootmanPlayer)
    set clock = CreateTimer()
    call TimerStart(clock, 65.00, false, function ShowFootmanNeutral)
    set clock = CreateTimer()
    call TimerStart(clock, 85.00, false, function ShowBarePlayer)
    set clock = CreateTimer()
    call TimerStart(clock, 105.00, false, function ShowBareNeutral)
endfunction
"#;

fn sha256(bytes: &[u8]) -> String {
    hex(Sha256::digest(bytes))
}

fn hex(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend(value.to_le_bytes());
}

fn push_f32(bytes: &mut Vec<u8>, value: f32) {
    bytes.extend(value.to_le_bytes());
}

fn push_str(bytes: &mut Vec<u8>, value: &str) {
    bytes.extend(value.as_bytes());
    bytes.push(0);
}

fn info_bytes(min: [f32; 2], max: [f32; 2], start: [f32; 2]) -> Vec<u8> {
    let mut bytes = Vec::new();
    push_u32(&mut bytes, 18);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 0);
    for _ in 0..4 {
        push_str(&mut bytes, "");
    }
    for value in [
        min[0], min[1], max[0], max[1], min[0], min[1], max[0], max[1],
    ] {
        push_f32(&mut bytes, value);
    }
    for _ in 0..4 {
        push_u32(&mut bytes, 0);
    }
    push_u32(&mut bytes, TILES);
    push_u32(&mut bytes, TILES);
    push_u32(&mut bytes, 0);
    bytes.push(0);
    push_u32(&mut bytes, 0);
    for _ in 0..3 {
        push_str(&mut bytes, "");
    }
    push_u32(&mut bytes, 0);
    for _ in 0..3 {
        push_str(&mut bytes, "");
    }
    push_u32(&mut bytes, 1);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, CONTROLLER_HUMAN);
    push_u32(&mut bytes, RACE_HUMAN);
    push_u32(&mut bytes, 0);
    push_str(&mut bytes, "Player 1");
    push_f32(&mut bytes, start[0]);
    push_f32(&mut bytes, start[1]);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 0);
    for _ in 0..4 {
        push_u32(&mut bytes, 0);
    }
    let info = MapInfo::parse(&bytes).expect("synthetic map info");
    assert_eq!(info.camera.first, [min[0], min[1], max[0], max[1]]);
    assert_eq!(info.camera.second, [min[0], min[1], max[0], max[1]]);
    assert_eq!(info.bounds, [TILES, TILES]);
    let [player] = info.players() else {
        panic!("one human slot");
    };
    assert_eq!(player.number(), 0);
    assert_eq!(player.control(), CONTROLLER_HUMAN);
    assert_eq!(player.race_id(), RACE_HUMAN);
    assert_eq!([player.start_x, player.start_y], start);
    info.canonical_bytes()
}

fn overlay_bytes() -> Vec<u8> {
    let mut bytes = Vec::new();
    push_u32(&mut bytes, 1);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 1);
    bytes.extend(*b"hfoo");
    bytes.extend(*b"hfx0");
    push_u32(&mut bytes, 1);
    bytes.extend(*b"upgr");
    push_u32(&mut bytes, 3);
    bytes.extend(b"_\0");
    push_u32(&mut bytes, 0);
    bytes
}

fn terrain_bytes(width: u32, height: u32) -> Vec<u8> {
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

fn pathing_bytes(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = b"MP3W".to_vec();
    for word in [0u32, width * 4, height * 4] {
        bytes.extend(word.to_le_bytes());
    }
    bytes.resize(bytes.len() + (width * height * 16) as usize, 0);
    bytes
}

fn units_bytes() -> Vec<u8> {
    Units {
        version: 8,
        subversion: 9,
        instances: Vec::new(),
    }
    .canonical_bytes()
}

fn doodad_bytes() -> Vec<u8> {
    let mut bytes = b"W3do".to_vec();
    for word in [7u32, 9, 0, 0, 0] {
        bytes.extend(word.to_le_bytes());
    }
    Doodads::from_bytes(bytes.clone()).expect("empty doodads");
    bytes
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
    for data in files.iter().map(|(_, data)| *data) {
        let file_pos = payload_base_offset + payloads.len();
        let file_size = data.len() as u32;
        let flags: u32 = 0x8000_0000 | 0x0100_0000;
        block_entries.push((file_pos as u32, file_size, file_size, flags));
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
        block_table_raw.extend_from_slice(&flags.to_le_bytes());
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

#[test]
#[ignore = "build-only; reads PCX_ICON_PROBE_COMMON and PCX_ICON_PROBE_BLIZZARD; does not launch a client"]
fn build_icon_table_probe_and_roundtrip_its_members() {
    let common_path = std::env::var("PCX_ICON_PROBE_COMMON").expect("PCX_ICON_PROBE_COMMON");
    let blizzard_path = std::env::var("PCX_ICON_PROBE_BLIZZARD").expect("PCX_ICON_PROBE_BLIZZARD");
    let common = std::fs::read(&common_path).unwrap();
    let blizzard = std::fs::read(&blizzard_path).unwrap();
    let script = SCRIPT.as_bytes();
    let extent = TILES as f32 * TILE_XY;
    let min = [0.0, 0.0];
    let max = [extent, extent];
    let places = [
        [448.0, 512.0],
        [576.0, 512.0],
        [512.0, 448.0],
        [512.0, 576.0],
    ];
    for place in places {
        assert!(place[0] > min[0] && place[0] < max[0] && place[1] > min[1] && place[1] < max[1]);
        assert!(SCRIPT.contains(&format!("{}, {}, 270", place[0], place[1])));
    }
    let terrain = terrain_bytes(TILES, TILES);
    let decoded = pcx_map::Terrain::from_bytes(terrain.clone()).unwrap();
    assert_eq!(decoded.origin(), (0.0, 0.0));
    assert_eq!(decoded.width(), TILES);
    let info = info_bytes(min, max, [512.0, 512.0]);
    let pathing = pathing_bytes(TILES, TILES);
    let units = units_bytes();
    let doodads = doodad_bytes();
    let shadow = vec![0u8; 16 * TILES as usize * TILES as usize];
    let overlay = overlay_bytes();
    let objects = ObjectFile::parse("war3map.w3u", &overlay).unwrap();
    assert!(objects.base().is_empty());
    let [custom] = objects.custom() else {
        panic!("one custom unit");
    };
    assert_eq!(&custom.original, b"hfoo");
    assert_eq!(&custom.modified, b"hfx0");
    let [modification] = custom.mods.as_slice() else {
        panic!("only the upgrades field is overridden");
    };
    assert_eq!(&modification.field, b"upgr");
    assert_eq!(modification.value, Value::Str(b"_".to_vec()));
    Pathing::parse(&pathing).unwrap();
    let files = [
        ("war3map.w3e", terrain.as_slice()),
        ("war3map.w3i", info.as_slice()),
        ("war3map.wpm", pathing.as_slice()),
        ("war3mapUnits.doo", units.as_slice()),
        ("war3map.doo", doodads.as_slice()),
        ("war3map.shd", shadow.as_slice()),
        ("war3map.w3u", overlay.as_slice()),
        ("war3map.j", script),
    ];
    let packed = build_synthetic_map("IconTableProbe", &files);
    let map = Map::parse(&packed).unwrap();
    for (name, bytes) in files {
        assert_eq!(map.extract(name).unwrap(), bytes, "{name}");
    }
    let scripts = MapScripts::compile(
        AssetIdentity {
            edition: "legacy".into(),
            version: "1.29.2.9231".into(),
            source: "War3x.mpq Scripts\\common.j + Scripts\\Blizzard.j".into(),
        },
        ScriptAsset {
            label: "Scripts\\common.j",
            bytes: &common,
        },
        ScriptAsset {
            label: "Scripts\\Blizzard.j",
            bytes: &blizzard,
        },
        ScriptAsset {
            label: "war3map.j",
            bytes: script,
        },
        None,
    )
    .unwrap();
    assert!(scripts.program().function("config").is_some());
    assert!(scripts.program().function("main").is_some());
    let calls = scripts
        .program()
        .natives_called()
        .map(|(name, _)| name)
        .collect::<Vec<_>>();
    for native in [
        "SetPlayers",
        "GetPlayerNeutralPassive",
        "Player",
        "CreateUnit",
        "CreateTimer",
        "TimerStart",
        "ClearSelection",
        "SelectUnit",
        "FogEnable",
        "FogMaskEnable",
        "SetCameraBounds",
        "SetCameraPosition",
        "DisplayTimedTextToPlayer",
        "CreateMultiboard",
        "MultiboardSetTitleText",
    ] {
        assert!(calls.contains(&native), "{native} {calls:?}");
    }
    let out =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.scratch/unit-stat-panel/icon-probe");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("icon-probe-2.w3m"), &packed).unwrap();
    std::fs::write(out.join("war3map-2.j"), script).unwrap();
    let review = format!(
        "{{\n  \"revision\": 2,\n  \"launch\": \"root only; fully headless and muted; no desktop and no audio\",\n  \"capture\": \"ignore host clock; a row is visible only while its multiboard title and text are the marker\",\n  \"first_marker_game_seconds\": {FIRST_MARKER},\n  \"hold_game_seconds\": {HOLD},\n  \"playable\": {{\"min\": [0, 0], \"max\": [{extent}, {extent}], \"tile_xy\": {TILE_XY}, \"tiles\": {TILES}}},\n  \"map_sha256\": \"{}\",\n  \"script_sha256\": \"{}\",\n  \"common_sha256\": \"{}\",\n  \"blizzard_sha256\": \"{}\",\n  \"common_member\": \"War3x.mpq Scripts\\\\common.j\",\n  \"blizzard_member\": \"War3x.mpq Scripts\\\\Blizzard.j\",\n  \"prelude_bytes_stored\": false,\n  \"rows\": [\n    {{\"marker\": \"ICONROW hfoo player\", \"game_seconds\": 45, \"rawcode\": \"hfoo\", \"owner\": \"Player(0)\", \"upgrades\": \"Rhar,Rhme,Rhde,Rhpm,Rguv\"}},\n    {{\"marker\": \"ICONROW hfoo neutral\", \"game_seconds\": 65, \"rawcode\": \"hfoo\", \"owner\": \"Player(GetPlayerNeutralPassive())\", \"upgrades\": \"Rhar,Rhme,Rhde,Rhpm,Rguv\"}},\n    {{\"marker\": \"ICONROW hfx0 player\", \"game_seconds\": 85, \"rawcode\": \"hfx0\", \"base\": \"hfoo\", \"owner\": \"Player(0)\", \"upgrades\": \"_\"}},\n    {{\"marker\": \"ICONROW hfx0 neutral\", \"game_seconds\": 105, \"rawcode\": \"hfx0\", \"base\": \"hfoo\", \"owner\": \"Player(GetPlayerNeutralPassive())\", \"upgrades\": \"_\"}}\n  ]\n}}\n",
        sha256(&packed),
        sha256(script),
        sha256(&common),
        sha256(&blizzard)
    );
    std::fs::write(out.join("REVIEW-2.json"), review).unwrap();
}
