use super::*;
use crate::pe_image::tests::{DATA, IMAGE_BASE, synthetic_image};
use crate::pe_image::{RDATA, TEXT};

const COLUMNS: [&str; 17] = [
    "EnvironmentType",
    "RoomType",
    "EffectVolume",
    "DecayTime",
    "Damping",
    "Size",
    "Diffusion",
    "Room",
    "RoomHF",
    "DecayHFRatio",
    "Reflections",
    "ReflectionsDelay",
    "Reverb",
    "ReverbDelay",
    "RoomRolloff",
    "AirAbsorption",
    "DisplayText",
];

/// SLK text with row 1 as the header; cells are the K-field bodies, so strings keep their quotes.
pub(crate) fn sheet(rows: &[[&str; 17]]) -> Vec<u8> {
    let mut text = format!("ID;PWXL;N;E\r\nB;X17;Y{};D0\r\n", rows.len());
    let header = COLUMNS.map(|name| format!("\"{name}\""));
    let all = std::iter::once(header).chain(rows.iter().map(|row| row.map(str::to_owned)));
    for (r, cells) in all.enumerate() {
        for (c, value) in cells.iter().enumerate() {
            let y = if c == 0 {
                format!(";Y{}", r + 1)
            } else {
                String::new()
            };
            text.push_str(&format!("C;X{}{y};K{value}\r\n", c + 1));
        }
    }
    text.push_str("E\r\n");
    text.into_bytes()
}

fn parse(bytes: &[u8]) -> Result<Vec<SoundEnvironment>, String> {
    let coordinates = pcx_slq::CoordinateLimits {
        max_row: 100,
        max_column: 32,
    };
    let table = Table::parse_bounded(bytes, coordinates).map_err(|e| e.to_string())?;
    from_table(&table)
}

const DEFAULT: [&str; 17] = [
    "\"Default\"",
    "\"MOUNTAINS\"",
    "1",
    "2.5",
    "1",
    "10",
    "1",
    "0",
    "0",
    "1",
    "-2250",
    "0.07",
    "-2500",
    "0.03",
    "0",
    "-2",
    "\"WESTRING_SNDENV_DEFAULT\"",
];

#[test]
fn every_row_keeps_its_authored_name_room_text_and_reverb_cells() {
    let mut dungeon = DEFAULT;
    dungeon[0] = "\"Dungeon\"";
    dungeon[3] = "3.2";
    let environments = parse(&sheet(&[DEFAULT, dungeon])).unwrap();
    assert_eq!(environments.len(), 2);
    assert_eq!(environments[0].name, "Default");
    assert_eq!(environments[0].room_type, "MOUNTAINS");
    assert_eq!(environments[0].display_text, "WESTRING_SNDENV_DEFAULT");
    assert_eq!(
        environments[0].reverb,
        Reverb {
            effect_volume: 1.0,
            decay_time: 2.5,
            damping: 1.0,
            size: 10.0,
            diffusion: 1.0,
            room: 0.0,
            room_hf: 0.0,
            decay_hf_ratio: 1.0,
            reflections: -2250.0,
            reflections_delay: 0.07,
            reverb: -2500.0,
            reverb_delay: 0.03,
            room_rolloff: 0.0,
            air_absorption: -2.0,
        }
    );
    assert_eq!(environments[1].name, "Dungeon");
    assert_eq!(environments[1].reverb.decay_time, 3.2);
}

#[test]
fn missing_columns_bad_numbers_duplicates_and_empty_tables_are_refused() {
    let mut bad_number = DEFAULT;
    bad_number[10] = "\"loud\"";
    assert!(
        parse(&sheet(&[bad_number]))
            .unwrap_err()
            .contains("Reflections")
    );

    let mut infinite = DEFAULT;
    infinite[2] = "inf";
    assert!(parse(&sheet(&[infinite])).unwrap_err().contains("finite"));

    assert!(
        parse(&sheet(&[DEFAULT, DEFAULT]))
            .unwrap_err()
            .contains("duplicate sound environment Default")
    );

    assert!(parse(&sheet(&[])).unwrap_err().contains("no data rows"));

    // Drop the RoomType header: the table no longer declares every column.
    let text = String::from_utf8(sheet(&[DEFAULT])).unwrap();
    let without_room = text.replace("K\"RoomType\"", "K\"Other\"");
    assert!(
        parse(without_room.as_bytes())
            .unwrap_err()
            .contains("RoomType")
    );
}

/// Explicit local original gate: `PEACECRAFT3_INSTALL` (or `PEACECRAFT3_LEGACY_DIR`)
/// names a Legacy install whose `War3.mpq` holds `UI\SoundInfo\EnvironmentSounds.slk`.
#[test]
#[ignore = "requires PEACECRAFT3_INSTALL or PEACECRAFT3_LEGACY_DIR pointing at a local Legacy install"]
fn shipped_environment_table_yields_its_authored_records() {
    let dir = std::env::var_os("PEACECRAFT3_INSTALL")
        .or_else(|| std::env::var_os("PEACECRAFT3_LEGACY_DIR"))
        .expect("set PEACECRAFT3_INSTALL or PEACECRAFT3_LEGACY_DIR");
    let mut set = pcx_mpq::MpqSet::open_wc3(dir).expect("open War3 archives");
    let bytes = set
        .extract("UI\\SoundInfo\\EnvironmentSounds.slk")
        .expect("extract environment table");
    let environments = from_table(&Table::parse(&bytes).expect("parse table")).unwrap();
    let names: Vec<&str> = environments.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["Default", "lake", "psychotic", "Dungeon"]);
    assert_eq!(environments[0].display_text, "WESTRING_SNDENV_DEFAULT");
    assert_eq!(environments[0].reverb.reflections, -2250.0);
}

/// Property table of the three names used by the derivation fixtures.
const FIXTURE_NAMES: [&str; 3] = ["EAX effect volume", "EAX decay time", "EAX room"];
const ROWS_VA: u32 = IMAGE_BASE + 0x2000;
const NAMES_VA: u32 = IMAGE_BASE + 0x3000;

/// Synthetic image: `.text` holds the operands that address the row table (`[edi*4+X]`,
/// stride 20 across `X`, `X+4`, ..., `X+16`) and the name table (`push [edx*4+Y]`);
/// `.rdata` holds rows and strings; `.data` holds the name pointers.
fn fixture_exe(names: &[&str], rows: &[[u8; ROW_LEN]]) -> Vec<u8> {
    let mut text = Vec::new();
    for step in [0u32, 4, 8, 12, 16] {
        text.extend_from_slice(&[0x8b, 0x04, 0xbd]);
        text.extend_from_slice(&(ROWS_VA + step).to_le_bytes());
    }
    text.extend_from_slice(&[0xff, 0x34, 0x95]);
    text.extend_from_slice(&NAMES_VA.to_le_bytes());

    let mut rdata = vec![0u8; 0x100];
    for (i, row) in rows.iter().enumerate() {
        rdata[i * ROW_LEN..(i + 1) * ROW_LEN].copy_from_slice(row);
    }
    let mut pointers = Vec::new();
    for name in names {
        let at = rdata.len() as u32;
        rdata.extend_from_slice(name.as_bytes());
        rdata.push(0);
        pointers.extend_from_slice(&(IMAGE_BASE + 0x2000 + at).to_le_bytes());
    }
    synthetic_image(&[
        (TEXT, 0x1000, &text),
        (RDATA, 0x2000, &rdata),
        (DATA, 0x3000, &pointers),
    ])
}

fn row(kind: u32, words: [u32; 4]) -> [u8; ROW_LEN] {
    let mut out = [0u8; ROW_LEN];
    out[0..4].copy_from_slice(&kind.to_le_bytes());
    for (i, word) in words.iter().enumerate() {
        out[4 + i * 4..8 + i * 4].copy_from_slice(&word.to_le_bytes());
    }
    out
}

fn fixture_rows() -> Vec<[u8; ROW_LEN]> {
    vec![
        // Float: [0, 1].
        row(KIND_FLOAT, [0f32.to_bits(), 1f32.to_bits(), 0, 0]),
        // Float: [0.1, 20].
        row(KIND_FLOAT, [0.1f32.to_bits(), 20f32.to_bits(), 0, 0]),
        // Int: [-10000, 0] in the last two words.
        row(KIND_INT, [0, 0, (-10000i32) as u32, 0]),
        // Sentinel: not a float or int kind, so the table ends here.
        row(0x7f80_0000, [0; 4]),
    ]
}

#[test]
fn rows_and_names_come_from_the_code_addressed_tables_in_order() {
    let exe = fixture_exe(&FIXTURE_NAMES, &fixture_rows());
    let properties = read_properties(&exe, &FIXTURE_NAMES).unwrap();
    assert_eq!(
        properties,
        vec![
            EaxProperty {
                name: "EAX effect volume".into(),
                range: PropertyRange::Float { min: 0.0, max: 1.0 },
            },
            EaxProperty {
                name: "EAX decay time".into(),
                range: PropertyRange::Float {
                    min: 0.1,
                    max: 20.0
                },
            },
            EaxProperty {
                name: "EAX room".into(),
                range: PropertyRange::Int {
                    min: -10000,
                    max: 0
                },
            },
        ]
    );
}

#[test]
fn a_name_or_row_count_that_disagrees_with_the_expected_table_is_refused() {
    let exe = fixture_exe(&FIXTURE_NAMES, &fixture_rows());
    let renamed = ["EAX effect volume", "EAX damping", "EAX room"];
    assert!(
        read_properties(&exe, &renamed)
            .unwrap_err()
            .contains("where")
    );
    assert!(
        read_properties(&exe, &FIXTURE_NAMES[..2])
            .unwrap_err()
            .contains("expected 2")
    );
}

#[test]
fn a_missing_operand_cluster_or_inverted_range_is_refused() {
    let no_operands = synthetic_image(&[(TEXT, 0x1000, &[0xc3]), (RDATA, 0x2000, &[0; 40])]);
    assert!(
        read_properties(&no_operands, &FIXTURE_NAMES)
            .unwrap_err()
            .contains("no EAX row table")
    );
    let mut inverted = fixture_rows();
    inverted[0] = row(KIND_FLOAT, [1f32.to_bits(), 0f32.to_bits(), 0, 0]);
    let exe = fixture_exe(&FIXTURE_NAMES, &inverted);
    assert!(
        read_properties(&exe, &FIXTURE_NAMES)
            .unwrap_err()
            .contains("invalid float range")
    );
}

/// Property rows with the same ranges as the executable's, for host and apply tests.
pub(crate) fn synthetic_properties() -> Vec<EaxProperty> {
    let float = |name: &str, min: f32, max: f32| EaxProperty {
        name: name.into(),
        range: PropertyRange::Float { min, max },
    };
    let int = |name: &str, min: i32, max: i32| EaxProperty {
        name: name.into(),
        range: PropertyRange::Int { min, max },
    };
    vec![
        float(EAX_NAMES[0], 0.0, 1.0),
        float(EAX_NAMES[1], 0.1, 20.0),
        float(EAX_NAMES[2], 0.0, 2.0),
        float(EAX_NAMES[3], 1.0, 100.0),
        float(EAX_NAMES[4], 0.0, 1.0),
        int(EAX_NAMES[5], -10000, 0),
        int(EAX_NAMES[6], -10000, 0),
        float(EAX_NAMES[7], 0.1, 20.0),
        int(EAX_NAMES[8], -10000, 1000),
        float(EAX_NAMES[9], 0.0, 0.3),
        int(EAX_NAMES[10], -10000, 2000),
        float(EAX_NAMES[11], 0.0, 0.1),
        float(EAX_NAMES[12], 0.1, 10.0),
        float(EAX_NAMES[13], -100.0, 0.0),
    ]
}

#[test]
fn apply_clamps_each_value_to_its_row_and_keeps_the_effect_volume_out_of_the_writes() {
    let mut dungeon = DEFAULT;
    dungeon[0] = "\"Dungeon\"";
    dungeon[3] = "3.2";
    dungeon[5] = "300";
    dungeon[7] = "-20000";
    let environments = parse(&sheet(&[dungeon])).unwrap();
    let applied = apply(&environments[0], &synthetic_properties()).unwrap();
    assert_eq!(applied.effect_volume, 1.0);
    assert_eq!(applied.writes.len(), 13);
    assert_eq!(applied.writes[0].name, "EAX decay time");
    assert_eq!(applied.writes[0].value, ProviderValue::Float(3.2));
    // Size 300 clamps to the row maximum; room -20000 clamps to the row minimum.
    assert_eq!(applied.writes[2].name, "EAX environment size");
    assert_eq!(applied.writes[2].value, ProviderValue::Float(100.0));
    assert_eq!(applied.writes[4].name, "EAX2 room");
    assert_eq!(applied.writes[4].value, ProviderValue::Int(-10000));
}

#[test]
fn apply_refuses_a_fractional_int_cell_and_a_short_property_table() {
    let mut bad = DEFAULT;
    bad[7] = "0.5";
    let environments = parse(&sheet(&[bad])).unwrap();
    assert!(apply(&environments[0], &synthetic_properties()).is_err());
    assert!(apply(&environments[0], &synthetic_properties()[..3]).is_err());
}

/// Explicit local original gate: `PEACECRAFT3_INSTALL` (or `PEACECRAFT3_LEGACY_DIR`)
/// names a Legacy install whose `Warcraft III.exe` is the selected executable.
#[test]
#[ignore = "requires PEACECRAFT3_INSTALL or PEACECRAFT3_LEGACY_DIR pointing at a local Legacy install"]
fn shipped_executable_yields_the_fourteen_eax_rows_in_order() {
    let dir = std::env::var_os("PEACECRAFT3_INSTALL")
        .or_else(|| std::env::var_os("PEACECRAFT3_LEGACY_DIR"))
        .expect("set PEACECRAFT3_INSTALL or PEACECRAFT3_LEGACY_DIR");
    let exe = std::fs::read(std::path::Path::new(&dir).join("Warcraft III.exe"))
        .expect("read Warcraft III.exe");
    let properties = eax_properties(&exe).expect("derive EAX rows");
    let names: Vec<&str> = properties.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, EAX_NAMES);
    assert_eq!(
        properties[0].range,
        PropertyRange::Float { min: 0.0, max: 1.0 }
    );
    assert_eq!(
        properties[5].range,
        PropertyRange::Int {
            min: -10000,
            max: 0
        }
    );
}
