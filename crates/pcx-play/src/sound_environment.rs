//! Sound environment records from an authored `UI\SoundInfo` table whose header
//! row names `EnvironmentType`. Names, room types, display keys and reverb cells
//! are the table's own values; column names are the table's headers.

use crate::pe_image::{PeSections, RDATA, TEXT, i32_at, u32_at};
use pcx_slq::Table;
use std::collections::BTreeSet;

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

/// Reverb field names in declaration order, paired with the executable's property rows.
/// The names are the executable's own strings; a mismatch refuses the whole table.
const EAX_NAMES: [&str; 14] = [
    "EAX effect volume",
    "EAX decay time",
    "EAX damping",
    "EAX environment size",
    "EAX2 environment diffusion",
    "EAX2 room",
    "EAX2 room HF",
    "EAX2 decay HF Ratio",
    "EAX2 reflections",
    "EAX2 reflections delay",
    "EAX2 reverb",
    "EAX2 reverb delay",
    "EAX2 room rolloff",
    "EAX2 air absorption",
];
const MAX_PROPERTY_ROWS: usize = 64;
const ROW_LEN: usize = 20;
const KIND_FLOAT: u32 = 0;
const KIND_INT: u32 = 1;
const MAX_NAME_BYTES: usize = 128;
/// `ModRM` with mod 00 and SIB base 101 (disp32): `[index*4 + disp32]`, scale 4.
const SIB_INDEX4_DISP32: u8 = 0xbd;
const MODRM_SIB: u8 = 0x04;
/// `push dword ptr [edx*4 + disp32]`.
const PUSH_INDEX4: [u8; 3] = [0xff, 0x34, 0x95];

/// Clamp range of one property row, from the executable.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PropertyRange {
    Float { min: f32, max: f32 },
    Int { min: i32, max: i32 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct EaxProperty {
    pub name: String,
    pub range: PropertyRange,
}

/// Value handed to `AIL_set_3D_provider_preference`, after clamping.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ProviderValue {
    Float(f32),
    Int(i32),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProviderWrite {
    pub name: &'static str,
    pub value: ProviderValue,
}

/// One record applied through the native: the clamped `EAX effect volume` (kept by the
/// native in its own global, not sent to the provider) and the thirteen provider writes.
#[derive(Clone, Debug, PartialEq)]
pub struct Applied {
    pub effect_volume: f32,
    pub writes: Vec<ProviderWrite>,
}

/// Property rows of the selected executable, read from the row table and name table
/// that its `NewSoundEnvironment` path addresses. Both tables are located from code
/// operands, so no address is fixed in this crate.
pub fn eax_properties(exe: &[u8]) -> Result<Vec<EaxProperty>, String> {
    read_properties(exe, &EAX_NAMES)
}

fn read_properties(exe: &[u8], expected: &[&str]) -> Result<Vec<EaxProperty>, String> {
    let pe = PeSections::parse(exe).map_err(|e| format!("executable: {e}"))?;
    let text = pe.section(TEXT).ok_or("executable has no .text section")?;
    let rdata = pe
        .section(RDATA)
        .ok_or("executable has no .rdata section")?;
    let rdata_start = u64::from(pe.image_base) + u64::from(rdata.rva);
    let rdata_end = rdata_start + rdata.bytes.len() as u64;
    let table = row_table(text.bytes, rdata_start, rdata_end)?;
    let names = name_table(text.bytes)?;

    let mut properties = Vec::new();
    for index in 0..MAX_PROPERTY_ROWS {
        let row_va = table + (index * ROW_LEN) as u32;
        let Some(row) = pe.va_bytes(row_va, ROW_LEN) else {
            break;
        };
        let kind = u32_at(row, 0).ok_or("EAX property row is truncated")?;
        if kind != KIND_FLOAT && kind != KIND_INT {
            break;
        }
        let name_slot = names
            .checked_add((index * 4) as u32)
            .ok_or("EAX name table overflows")?;
        let name_va = u32_at(
            pe.va_bytes(name_slot, 4)
                .ok_or("EAX name table ends before its rows")?,
            0,
        )
        .ok_or("EAX name pointer is truncated")?;
        let name = c_string(&pe, name_va)?;
        let range = if kind == KIND_FLOAT {
            PropertyRange::Float {
                min: f32::from_bits(u32_at(row, 4).ok_or("EAX row")?),
                max: f32::from_bits(u32_at(row, 8).ok_or("EAX row")?),
            }
        } else {
            PropertyRange::Int {
                min: i32_at(row, 12).ok_or("EAX row")?,
                max: i32_at(row, 16).ok_or("EAX row")?,
            }
        };
        match range {
            PropertyRange::Float { min, max }
                if !(min.is_finite() && max.is_finite() && min <= max) =>
            {
                return Err(format!("EAX property {name:?} has an invalid float range"));
            }
            PropertyRange::Int { min, max } if min > max => {
                return Err(format!("EAX property {name:?} has an inverted int range"));
            }
            _ => {}
        }
        properties.push(EaxProperty { name, range });
    }

    if properties.len() != expected.len() {
        return Err(format!(
            "expected {} EAX property rows, read {}",
            expected.len(),
            properties.len()
        ));
    }
    for (property, want) in properties.iter().zip(expected) {
        if property.name != *want {
            return Err(format!(
                "EAX property {:?} where {want:?} was expected",
                property.name
            ));
        }
    }
    Ok(properties)
}

/// Displacement `base` of the unique cluster `[edi*4+base]`, `+4`, `+8`, `+12`, `+16`
/// with the cluster inside `.rdata`. Row `k` lives at `base + 20*k`.
fn row_table(code: &[u8], rdata_start: u64, rdata_end: u64) -> Result<u32, String> {
    let mut displacements = BTreeSet::new();
    for window in code.windows(6) {
        if window[0] & 0xc7 == MODRM_SIB && window[1] == SIB_INDEX4_DISP32 {
            displacements.insert(u32_at(window, 2).ok_or("truncated operand")?);
        }
    }
    let mut bases = displacements.iter().copied().filter(|&base| {
        let inside = u64::from(base) >= rdata_start && u64::from(base) + 16 < rdata_end;
        inside
            && [4, 8, 12, 16]
                .iter()
                .all(|step| displacements.contains(&base.wrapping_add(*step)))
    });
    let base = bases.next().ok_or("no EAX row table operands in .text")?;
    if bases.next().is_some() {
        return Err("ambiguous EAX row table operands".into());
    }
    Ok(base)
}

/// Unique displacement of `push dword ptr [edx*4 + disp32]` in `.text`.
fn name_table(code: &[u8]) -> Result<u32, String> {
    let mut displacements = BTreeSet::new();
    for window in code.windows(7) {
        if window[..3] == PUSH_INDEX4 {
            displacements.insert(u32_at(window, 3).ok_or("truncated operand")?);
        }
    }
    let mut it = displacements.into_iter();
    match (it.next(), it.next()) {
        (Some(base), None) => Ok(base),
        (None, _) => Err("no EAX name table operands in .text".into()),
        _ => Err("ambiguous EAX name table operands".into()),
    }
}

/// NUL-terminated printable ASCII at `va`, bounded by [`MAX_NAME_BYTES`].
fn c_string(pe: &PeSections<'_>, va: u32) -> Result<String, String> {
    let mut name = String::new();
    for offset in 0..MAX_NAME_BYTES {
        let at = va
            .checked_add(offset as u32)
            .ok_or("EAX name address overflows")?;
        let byte = pe
            .va_bytes(at, 1)
            .and_then(|b| b.first().copied())
            .ok_or("EAX name is outside the image")?;
        if byte == 0 {
            return Ok(name);
        }
        if !(0x20..0x7f).contains(&byte) {
            return Err("EAX name is not printable ASCII".into());
        }
        name.push(char::from(byte));
    }
    Err("EAX name exceeds its byte limit".into())
}

/// Applies one record the way the native does: each value is clamped to its row's
/// range, the effect volume goes to the native's own global, and the rest are writes.
pub fn apply(
    environment: &SoundEnvironment,
    properties: &[EaxProperty],
) -> Result<Applied, String> {
    if properties.len() != EAX_NAMES.len() {
        return Err("EAX property table does not match the reverb fields".into());
    }
    let reverb = &environment.reverb;
    let values = [
        reverb.effect_volume,
        reverb.decay_time,
        reverb.damping,
        reverb.size,
        reverb.diffusion,
        reverb.room,
        reverb.room_hf,
        reverb.decay_hf_ratio,
        reverb.reflections,
        reverb.reflections_delay,
        reverb.reverb,
        reverb.reverb_delay,
        reverb.room_rolloff,
        reverb.air_absorption,
    ];
    let mut effect_volume = None;
    let mut writes = Vec::new();
    for (index, (property, value)) in properties.iter().zip(values).enumerate() {
        let clamped = clamp(environment, property, value)?;
        match (index, clamped) {
            (0, ProviderValue::Float(volume)) => effect_volume = Some(volume),
            (0, ProviderValue::Int(_)) => {
                return Err("EAX effect volume row is not a float".into());
            }
            _ => writes.push(ProviderWrite {
                name: EAX_NAMES[index],
                value: clamped,
            }),
        }
    }
    Ok(Applied {
        effect_volume: effect_volume.ok_or("EAX effect volume is missing")?,
        writes,
    })
}

fn clamp(
    environment: &SoundEnvironment,
    property: &EaxProperty,
    value: f32,
) -> Result<ProviderValue, String> {
    match property.range {
        PropertyRange::Float { min, max } => Ok(ProviderValue::Float(value.clamp(min, max))),
        PropertyRange::Int { min, max } => {
            let whole =
                value.fract() == 0.0 && value >= i32::MIN as f32 && value <= i32::MAX as f32;
            if !whole {
                return Err(format!(
                    "{}: {} is not an integer for {:?}",
                    environment.name, value, property.name
                ));
            }
            Ok(ProviderValue::Int((value as i32).clamp(min, max)))
        }
    }
}

#[cfg(test)]
#[path = "sound_environment_tests.rs"]
pub(crate) mod tests;
