//! Weather definitions from the selected TerrainArt/Weather.slk, without stock defaults.
use crate::{neutral_player::sha256, scripts::AssetIdentity};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub struct Definition {
    pub id: [u8; 4],
    pub name: String,
    pub texture: String,
    pub alpha_mode: i32,
    pub use_fog: bool,
    pub height: f32,
    pub angles: [f32; 2],
    pub emission_rate: f32,
    pub lifespan: f32,
    pub particles: u32,
    pub velocity: f32,
    pub acceleration: f32,
    pub variation: f32,
    pub texture_grid: [u32; 2],
    pub head: bool,
    pub tail: bool,
    pub tail_length: f32,
    pub latitude: f32,
    pub longitude: f32,
    pub mid_time: f32,
    /// Authored byte channels, start/mid/end; normalization belongs to the renderer.
    pub colors: [[u8; 4]; 3],
    pub scales: [f32; 3],
    pub head_uv: [f32; 3],
    pub tail_uv: [f32; 3],
    pub ambient_sound: Option<String>,
    pub version: i32,
}

#[derive(Debug)]
pub struct Catalog {
    pub origin: AssetIdentity,
    pub sha256: [u8; 32],
    definitions: BTreeMap<[u8; 4], Definition>,
}

impl Catalog {
    pub fn read(
        bytes: &[u8],
        origin: AssetIdentity,
        max_bytes: usize,
        max_rows: u32,
    ) -> Result<Self, String> {
        let read = || -> Result<BTreeMap<[u8; 4], Definition>, String> {
            if bytes.len() > max_bytes || max_rows == 0 || !crate::label_sounds::identity(&origin) {
                return Err("weather input budget or identity".into());
            }
            let table = crate::label_sounds::checked_table(
                crate::scripts::ScriptAsset {
                    label: &origin.source,
                    bytes,
                },
                pcx_slq::CoordinateLimits {
                    max_row: max_rows,
                    max_column: 256,
                },
                &[
                    "effectID",
                    "name",
                    "texDir",
                    "texFile",
                    "alphaMode",
                    "useFog",
                    "height",
                    "angx",
                    "angy",
                    "emrate",
                    "lifespan",
                    "particles",
                    "veloc",
                    "accel",
                    "var",
                    "texr",
                    "texc",
                    "head",
                    "tail",
                    "taillen",
                    "lati",
                    "long",
                    "midTime",
                    "redStart",
                    "greenStart",
                    "blueStart",
                    "redMid",
                    "greenMid",
                    "blueMid",
                    "redEnd",
                    "greenEnd",
                    "blueEnd",
                    "alphaStart",
                    "alphaMid",
                    "alphaEnd",
                    "scaleStart",
                    "scaleMid",
                    "scaleEnd",
                    "hUVStart",
                    "hUVMid",
                    "hUVEnd",
                    "tUVStart",
                    "tUVMid",
                    "tUVEnd",
                    "AmbientSound",
                    "version",
                ],
            )
            .map_err(|e| e.to_string())?;
            let mut definitions = BTreeMap::new();
            for row in table.data_rows() {
                let text = |name: &str| -> Result<&str, String> {
                    table
                        .column_of(name)
                        .and_then(|c| row.get(c))
                        .ok_or_else(|| format!("row {}: missing {name}", row.index()))
                };
                let real = |name: &str| -> Result<f32, String> {
                    text(name)?
                        .parse::<f32>()
                        .ok()
                        .filter(|v| v.is_finite())
                        .ok_or_else(|| format!("row {}: invalid {name}", row.index()))
                };
                let integer = |name: &str| -> Result<i32, String> {
                    text(name)?
                        .parse()
                        .map_err(|_| format!("row {}: invalid {name}", row.index()))
                };
                let unsigned = |name: &str| -> Result<u32, String> {
                    u32::try_from(integer(name)?)
                        .map_err(|_| format!("row {}: negative {name}", row.index()))
                };
                let boolean = |name: &str| -> Result<bool, String> {
                    match integer(name)? {
                        0 => Ok(false),
                        1 => Ok(true),
                        _ => Err(format!("row {}: invalid {name} boolean", row.index())),
                    }
                };
                let id: [u8; 4] = text("effectID")?
                    .as_bytes()
                    .try_into()
                    .map_err(|_| "weather effectID must be a FOURCC")?;
                if !id.iter().all(u8::is_ascii_graphic) {
                    return Err("invalid weather effectID".into());
                }
                let texture = format!(
                    "{}\\{}",
                    text("texDir")?.trim_end_matches(['/', '\\']),
                    text("texFile")?
                );
                pcx_assets::AssetPath::new(&texture).map_err(|e| e.to_string())?;
                let colors = ["Start", "Mid", "End"]
                    .map(|stage| {
                        ["red", "green", "blue", "alpha"]
                            .map(|channel| {
                                u8::try_from(integer(&format!("{channel}{stage}"))?)
                                    .map_err(|_| format!("invalid {channel}{stage}"))
                            })
                            .into_iter()
                            .collect::<Result<Vec<_>, String>>()
                            .map(|v| v.try_into().expect("four channels"))
                    })
                    .into_iter()
                    .collect::<Result<Vec<_>, String>>()?
                    .try_into()
                    .expect("three stages");
                let stages = |prefix: &str| -> Result<[f32; 3], String> {
                    Ok([
                        real(&format!("{prefix}Start"))?,
                        real(&format!("{prefix}Mid"))?,
                        real(&format!("{prefix}End"))?,
                    ])
                };
                let sound = text("AmbientSound")?;
                let definition = Definition {
                    id,
                    name: text("name")?.into(),
                    texture,
                    alpha_mode: integer("alphaMode")?,
                    use_fog: boolean("useFog")?,
                    height: real("height")?,
                    angles: [real("angx")?, real("angy")?],
                    emission_rate: real("emrate")?,
                    lifespan: real("lifespan")?,
                    particles: unsigned("particles")?,
                    velocity: real("veloc")?,
                    acceleration: real("accel")?,
                    variation: real("var")?,
                    texture_grid: [unsigned("texr")?, unsigned("texc")?],
                    head: boolean("head")?,
                    tail: boolean("tail")?,
                    tail_length: real("taillen")?,
                    latitude: real("lati")?,
                    longitude: real("long")?,
                    mid_time: real("midTime")?,
                    colors,
                    scales: stages("scale")?,
                    head_uv: stages("hUV")?,
                    tail_uv: stages("tUV")?,
                    ambient_sound: (!matches!(sound, "" | "_" | "-")).then(|| sound.into()),
                    version: integer("version")?,
                };
                if definition.emission_rate < 0.0
                    || definition.lifespan <= 0.0
                    || definition.texture_grid.contains(&0)
                    || !(0.0..=1.0).contains(&definition.mid_time)
                    || definition.scales.iter().any(|v| *v < 0.0)
                {
                    return Err(format!(
                        "row {}: invalid weather particle bounds",
                        row.index()
                    ));
                }
                if definitions.insert(id, definition).is_some() {
                    return Err("duplicate weather effectID".into());
                }
            }
            if definitions.is_empty() {
                return Err("empty weather table".into());
            }
            Ok(definitions)
        };
        let definitions = read().map_err(|e| format!("{}: {e}", origin.source))?;
        Ok(Self {
            origin,
            sha256: sha256(bytes),
            definitions,
        })
    }

    pub fn get(&self, id: [u8; 4]) -> Option<&Definition> {
        self.definitions.get(&id)
    }
    pub fn definitions(&self) -> impl Iterator<Item = &Definition> {
        self.definitions.values()
    }
}

pub struct Input {
    pub catalog: Catalog,
    pub initial_enabled: bool,
    pub world_bounds: [f32; 4],
}

/// Constructor local -> published area member -> EnableWeatherEffect's same member.
pub fn initial_enabled(image: &[u8]) -> Result<bool, String> {
    if image.len() > 32 << 20 || sha256(image) != crate::menu_lighting::REVIEWED_IMAGE {
        return Err("unsupported weather client image".into());
    }
    let pe = crate::pe_image::PeSections::parse(image)?;
    initial_value(
        pe.va_bytes(0x763b9f, 7).ok_or("weather initializer")?,
        pe.va_bytes(0x763cb1, 6).ok_or("weather publication")?,
        pe.va_bytes(0x764c90, 10).ok_or("weather enable member")?,
    )
    .ok_or_else(|| "weather constructor/enable relationship differs".into())
}

fn initial_value(init: &[u8], publish: &[u8], enable: &[u8]) -> Option<bool> {
    let [0xc7, 0x45, local, value @ 0..=1, 0, 0, 0] = init else {
        return None;
    };
    let [0x8b, 0x45, same_local, 0x89, 0x47, member] = publish else {
        return None;
    };
    let [
        0x39,
        0x4c,
        0x02,
        read_member,
        0x74,
        _,
        0x89,
        0x4c,
        0x02,
        write_member,
    ] = enable
    else {
        return None;
    };
    (local == same_local && member == read_member && member == write_member).then_some(*value != 0)
}

#[cfg(test)]
#[path = "weather_tests.rs"]
mod tests;
