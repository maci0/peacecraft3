use super::*;

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
