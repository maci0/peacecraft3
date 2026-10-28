//! Sound environment records from an authored `UI\SoundInfo` table whose header
//! row names `EnvironmentType`. Names, room types, display keys and reverb cells
//! are the table's own values; column names are the table's headers.

use crate::pe_image::{PeSections, RDATA, TEXT, i32_at, u32_at, windows_with_byte};
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
/// First entry of the room-name table. Its string is unique in `.rdata`.
const ROOM_FIRST: &[u8] = b"GENERIC\0";
/// Room-name table length. The native's search loop bound is `cmp esi, 0x1a`.
const ROOM_COUNT: usize = 26;
/// Room-name table end; the last entry of the native's list.
const ROOM_LAST: &str = "PSYCHOTIC";

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

/// Executable tables that `NewSoundEnvironment` reads: the property rows and the room
/// names. Index `k` of `rooms` is the room index the native passes to the 3D provider.
#[derive(Clone, Debug, PartialEq)]
pub struct EnvironmentTables {
    pub properties: Vec<EaxProperty>,
    pub rooms: Vec<String>,
    pub window: Window,
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

/// Values one record produces. `room` is `None` when its `RoomType` is not in the
/// executable's room table: the native stores -1 and skips the room call.
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    pub effect_volume: f32,
    pub writes: Vec<ProviderWrite>,
    pub room: Option<usize>,
}

/// Everything `NewSoundEnvironment` needs from the selected sources: the records from the
/// sound tables, the executable tables, and the executable tables, which carry the scheduling window.
#[derive(Clone, Debug, PartialEq)]
pub struct SoundEnvironmentInput {
    pub records: Vec<SoundEnvironment>,
    pub tables: EnvironmentTables,
}

/// Reads both executable tables. Each is located from code or data anchors, and each
/// must match its expected shape exactly.
pub fn tables(exe: &[u8]) -> Result<EnvironmentTables, String> {
    let pe = PeSections::parse(exe).map_err(|e| format!("executable: {e}"))?;
    Ok(EnvironmentTables {
        properties: read_properties(&pe, &EAX_NAMES)?,
        rooms: read_rooms(&pe)?,
        window: read_window(&pe)?,
    })
}

/// Property rows of the selected executable, read from the row table and name table
/// that its `NewSoundEnvironment` path addresses. Both tables are located from code
/// operands, so no address is fixed in this crate.
fn read_properties(pe: &PeSections<'_>, expected: &[&str]) -> Result<Vec<EaxProperty>, String> {
    let text = pe.section(TEXT)?.ok_or("executable has no .text section")?;
    let rdata = pe
        .section(RDATA)?
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
        let name = c_string(pe, name_va)?;
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

/// The room names the native matches a `RoomType` against, in index order. The array
/// is the unique run of pointers whose first entry is the unique `GENERIC` string.
fn read_rooms(pe: &PeSections<'_>) -> Result<Vec<String>, String> {
    let first = pe.find_va(ROOM_FIRST);
    let [first] = first.as_slice() else {
        return Err(format!("room table anchor occurs {} times", first.len()));
    };
    let slots = pe.find_va(&first.to_le_bytes());
    let [slot] = slots.as_slice() else {
        return Err(format!("room table pointer occurs {} times", slots.len()));
    };
    let mut rooms = Vec::with_capacity(ROOM_COUNT);
    for index in 0..ROOM_COUNT {
        let at = slot
            .checked_add((index * 4) as u32)
            .ok_or("room table overflows")?;
        let pointer = u32_at(
            pe.va_bytes(at, 4)
                .ok_or("room table ends before its entries")?,
            0,
        )
        .ok_or("room table pointer is truncated")?;
        let name = c_string(pe, pointer)?;
        if rooms.contains(&name) {
            return Err(format!("duplicate room name {name:?}"));
        }
        rooms.push(name);
    }
    if rooms.last().map(String::as_str) != Some(ROOM_LAST) {
        return Err(format!("room table ends with {:?}", rooms.last()));
    }
    Ok(rooms)
}

/// Displacement `base` of the unique cluster `[edi*4+base]`, `+4`, `+8`, `+12`, `+16`
/// with the cluster inside `.rdata`. Row `k` lives at `base + 20*k`.
fn row_table(code: &[u8], rdata_start: u64, rdata_end: u64) -> Result<u32, String> {
    let mut displacements = BTreeSet::new();
    for (_, window) in windows_with_byte(code, 1, SIB_INDEX4_DISP32, 6) {
        if window[0] & 0xc7 == MODRM_SIB {
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
    for (_, window) in windows_with_byte(code, 0, PUSH_INDEX4[0], 7) {
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
            .ok_or("name address overflows")?;
        let byte = pe
            .va_bytes(at, 1)
            .and_then(|b| b.first().copied())
            .ok_or("name is outside the image")?;
        if byte == 0 {
            return Ok(name);
        }
        if !(0x20..0x7f).contains(&byte) {
            return Err("name is not printable ASCII".into());
        }
        name.push(char::from(byte));
    }
    Err("name exceeds its byte limit".into())
}

/// Applies one record the way the native does: each value is clamped to its row's
/// range, the effect volume is the native's own global, and the rest are provider writes.
pub fn apply(
    environment: &SoundEnvironment,
    tables: &EnvironmentTables,
) -> Result<Request, String> {
    let properties = &tables.properties;
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
    let room = tables
        .rooms
        .iter()
        .position(|name| *name == environment.room_type);
    Ok(Request {
        effect_volume: effect_volume.ok_or("EAX effect volume is missing")?,
        writes,
        room,
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

/// Constants of the scheduler call, read from the executable's code. `ms` is the
/// window, and `provenance` names the code it came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Window {
    pub ms: u32,
    pub provenance: WindowProvenance,
}

/// Addresses behind a [`Window`]: the global the window test reads, the one store that
/// writes it, the setter that contains the store, and every call that passes the
/// constant. All are virtual addresses in the selected executable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowProvenance {
    pub global: u32,
    pub writer: u32,
    pub setter: u32,
    pub call_sites: Vec<u32>,
}

/// Opcode bytes of the instructions the window derivation matches: `mov ecx, [abs]`,
/// `add ecx, [abs]`, `mov [abs], ecx`, the setter prologue, `push imm32` and `call rel32`.
const MOV_ECX_FROM: [u8; 2] = [0x8b, 0x0d];
const ADD_ECX_FROM: [u8; 2] = [0x03, 0x0d];
const MOV_TO: [u8; 2] = [0x89, 0x0d];
const PROLOGUE: [u8; 3] = [0x55, 0x8b, 0xec];
const PUSH_IMM32: u8 = 0x68;
const CALL_REL32: u8 = 0xe8;
/// Bytes between the last-change load and the window add in the shared body (8).
const READ_PAIR_SPAN: usize = 16;
/// `cmp dword ptr [ebp+8], -1`: the setter keeps its current value for a -1 argument.
const FIRST_ARG_KEEP_TEST: [u8; 4] = [0x83, 0x7d, 0x08, 0xff];

/// Derives the window from the scheduler code. The shared body reads the window global
/// `G` with `add ecx, [G]` shortly after `mov ecx, [last]`. Exactly one store writes
/// `G`, and the setter is the nearest prologue before that store. Every call to the
/// setter must push its window as an immediate. Any other shape is refused.
fn read_window(pe: &PeSections<'_>) -> Result<Window, String> {
    let text = pe.section(TEXT)?.ok_or("executable has no .text section")?;
    let code = text.bytes;
    let base = pe
        .image_base
        .checked_add(text.rva)
        .ok_or("text address overflows")?;
    let va = |offset: usize| {
        base.checked_add(offset as u32)
            .ok_or("text address overflows")
    };

    let mut globals = BTreeSet::new();
    for (at, window) in code.windows(6).enumerate() {
        if window[..2] != ADD_ECX_FROM {
            continue;
        }
        let read_before = (at.saturating_sub(READ_PAIR_SPAN)..at)
            .any(|j| code.get(j..j + 2) == Some(&MOV_ECX_FROM));
        if read_before {
            globals.insert(u32_at(window, 2).ok_or("truncated operand")?);
        }
    }
    // The window global is the one whose only store sits in a function that tests its
    // first argument against -1 before storing it: the setter.
    let mut found: Option<(u32, usize, usize)> = None;
    let mut rejected = 0usize;
    for global in globals.iter().copied() {
        let mut writes = Vec::new();
        for (at, store) in code.windows(6).enumerate() {
            if store[..2] == MOV_TO && u32_at(store, 2) == Some(global) {
                writes.push(at);
            }
        }
        let [writer] = writes.as_slice() else {
            rejected += 1;
            continue;
        };
        let Some(setter_at) = code[..*writer].windows(3).rposition(|w| w == PROLOGUE) else {
            rejected += 1;
            continue;
        };
        if !code[setter_at..*writer]
            .windows(4)
            .any(|w| w == FIRST_ARG_KEEP_TEST)
        {
            rejected += 1;
            continue;
        }
        if found.replace((global, *writer, setter_at)).is_some() {
            return Err("more than one scheduler window global qualifies".into());
        }
    }
    let Some((global, writer, setter_at)) = found else {
        return Err(format!(
            "no scheduler window global qualifies ({rejected} candidates rejected)"
        ));
    };
    let setter = va(setter_at)?;

    let mut call_sites = Vec::new();
    let mut window_ms: Option<u32> = None;
    for at in 5..code.len().saturating_sub(4) {
        if code[at] != CALL_REL32 {
            continue;
        }
        let rel = i32_at(code, at + 1).ok_or("truncated call")?;
        let next = va(at + 5)?;
        if next.wrapping_add_signed(rel) != setter {
            continue;
        }
        if code[at - 5] != PUSH_IMM32 {
            return Err(format!(
                "scheduler call at {:#x} does not push a constant window",
                va(at)?
            ));
        }
        let ms = u32_at(code, at - 4).ok_or("truncated window immediate")?;
        if window_ms.is_some_and(|known| known != ms) {
            return Err("scheduler calls push different windows".into());
        }
        window_ms = Some(ms);
        call_sites.push(va(at)?);
    }
    let ms = window_ms.ok_or("no call passes a window to the scheduler")?;
    if ms == 0 {
        return Err("scheduler window is zero".into());
    }
    Ok(Window {
        ms,
        provenance: WindowProvenance {
            global,
            writer: va(writer)?,
            setter,
            call_sites,
        },
    })
}

#[cfg(test)]
#[path = "sound_environment_tests.rs"]
pub(crate) mod tests;
