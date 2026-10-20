//! Sound environment records from an authored `UI\SoundInfo` table whose header
//! row names `EnvironmentType`. Names, room types, display keys and reverb cells
//! are the table's own values; column names are the table's headers.

use pcx_slq::Table;

const NAME: &str = "EnvironmentType";
const ROOM: &str = "RoomType";
const DISPLAY: &str = "DisplayText";
const REVERB: [&str; 14] = [
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
];

/// Authored reverb cells of one environment row, in table column order.
#[derive(Clone, Debug, PartialEq)]
pub struct Reverb {
    pub effect_volume: f32,
    pub decay_time: f32,
    pub damping: f32,
    pub size: f32,
    pub diffusion: f32,
    pub room: f32,
    pub room_hf: f32,
    pub decay_hf_ratio: f32,
    pub reflections: f32,
    pub reflections_delay: f32,
    pub reverb: f32,
    pub reverb_delay: f32,
    pub room_rolloff: f32,
    pub air_absorption: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SoundEnvironment {
    pub name: String,
    pub room_type: String,
    pub display_text: String,
    pub reverb: Reverb,
}

/// Every data row of an `EnvironmentType` table. Each row needs all columns and
/// a unique name; a missing column, non-finite number or repeated name is refused.
pub fn from_table(table: &Table) -> Result<Vec<SoundEnvironment>, String> {
    let has = |column: &str| table.headers().any(|(_, name)| name == column);
    for column in [NAME, ROOM, DISPLAY].into_iter().chain(REVERB) {
        if !has(column) {
            return Err(format!("sound environment table lacks column {column}"));
        }
    }
    let mut environments: Vec<SoundEnvironment> = Vec::new();
    for row in table.data_rows() {
        let cell = |column: &str| {
            row.by_name(column)
                .ok_or_else(|| format!("sound environment row {}: missing {column}", row.index()))
        };
        let name = cell(NAME)?;
        if name.is_empty() || name.contains('\0') {
            return Err(format!(
                "sound environment row {}: invalid name",
                row.index()
            ));
        }
        if environments.iter().any(|known| known.name == name) {
            return Err(format!("duplicate sound environment {name}"));
        }
        let number = |column: &str| -> Result<f32, String> {
            let text = cell(column)?;
            text.parse::<f32>()
                .ok()
                .filter(|value| value.is_finite())
                .ok_or_else(|| format!("{name}: {column} is not a finite number: {text:?}"))
        };
        let reverb = Reverb {
            effect_volume: number("EffectVolume")?,
            decay_time: number("DecayTime")?,
            damping: number("Damping")?,
            size: number("Size")?,
            diffusion: number("Diffusion")?,
            room: number("Room")?,
            room_hf: number("RoomHF")?,
            decay_hf_ratio: number("DecayHFRatio")?,
            reflections: number("Reflections")?,
            reflections_delay: number("ReflectionsDelay")?,
            reverb: number("Reverb")?,
            reverb_delay: number("ReverbDelay")?,
            room_rolloff: number("RoomRolloff")?,
            air_absorption: number("AirAbsorption")?,
        };
        environments.push(SoundEnvironment {
            name: name.to_owned(),
            room_type: cell(ROOM)?.to_owned(),
            display_text: cell(DISPLAY)?.to_owned(),
            reverb,
        });
    }
    if environments.is_empty() {
        return Err("sound environment table has no data rows".into());
    }
    Ok(environments)
}

#[cfg(test)]
#[path = "sound_environment_tests.rs"]
pub(crate) mod tests;
