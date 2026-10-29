//! In-game cursor rules read from the reviewed client image: which animation sequence and
//! tint each cursor mode shows, and which mode the pointer state selects.
//!
//! The 1.29.2.9231 client keeps one cursor sprite whose mode setter (`0x616660`) turns a
//! mode number into a list of animation tags (names registered at `0x5d1d30`) and a
//! tint (a three-byte table at `0xe94210`). Hover (`0x5fbdf0`), armed targeting
//! (`0x643280`) and edge scrolling (`0x5dfd50`) call the setter with immediates. Every
//! number here is read from those instructions; the addresses only bind the reviewed
//! image. Evidence labels are in `docs/re-notes/input-cursor.md`.
use crate::{
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, sha256},
    neutral_rule_input::Pe,
    playercolor_host::Relation,
    scripts::AssetIdentity,
};
use std::collections::BTreeMap;

const IMAGE_BUDGET: usize = 32 * 1024 * 1024;

/// Animation tag registration `0x5d1d30`: `push index; push name; push [table]; call`.
const TAG_REGISTRATION: std::ops::Range<u32> = 0x5d1d30..0x5d1e0e;
/// Tag indices the registration fills (`Normal` to `Down`).
const TAG_COUNT: usize = 11;
/// Length bound on a tag name read from the image (local policy, not an original value).
const TAG_NAME_BYTES: usize = 32;
/// Mode setter `0x616660`: range check, group index table, group jump table.
const MODE_LIMIT_COMPARE: u32 = 0x6166d8;
const GROUP_INDEX_LOAD: u32 = 0x6166e1;
const GROUP_JUMP: u32 = 0x6166e8;
/// The sprite call that ends every mode case: `0x966c10(sprite, tags, count, 0)`.
const SPRITE_SET_TAGS: u32 = 0x966c10;
/// Tint table loads in `0x6165a0`: red, green and blue of `mode * 3 + base`.
const TINT_LOADS: [u32; 3] = [0x616616, 0x61661e, 0x616631];
/// Hover mode pushes in `0x5fbdf0`.
const HOVER_NONE: u32 = 0x5fbfab;
const HOVER_ALLY: u32 = 0x5fbf89;
const HOVER_NEUTRAL: u32 = 0x5fbf97;
const HOVER_ENEMY: u32 = 0x5fbfa3;
/// The same neutral mode, pushed for an object carrying flag `0x100`.
const HOVER_FLAGGED: u32 = 0x5fbf7b;
/// The hold-item case replaces texture slot `imm8` of the sprite with the carried item's
/// icon: `push 0x15; push icon; push sprite; call 0x967800`.
const ITEM_ICON_SLOT: u32 = 0x616881;
/// Targeting mode pushes in `0x643280`.
const TARGET_PLAIN: u32 = 0x64358c;
const TARGET_NEUTRAL: u32 = 0x64357a;
const TARGET_ENEMY: u32 = 0x643568;
const TARGET_ALLY: u32 = 0x64353c;
/// Edge scroll `0x5dfd50`: mask bound, jump table, and the first mask the table covers.
const SCROLL_MASK_BOUND: u32 = 0x5dfdf0;
const SCROLL_MASK_JUMP: u32 = 0x5dfdf5;

/// What the pointer is doing, as far as the cursor cares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pointer {
    /// Nothing selectable under the pointer, or the pointer is over the interface.
    Normal,
    /// A selectable unit under the pointer, by the local player's relation to its owner.
    Hover(Relation),
    /// A command waits for a target; `Some` colours the cursor by the target's relation.
    Target(Option<Relation>),
    /// The pointer is in a screen edge band; the value is the edge mask
    /// (`camera_input::edge`).
    Scroll(u8),
}

/// A mode's presentation: the sequence name the model must carry and the tint multiplied
/// into the model colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Look<'a> {
    pub mode: u8,
    pub sequence: &'a str,
    pub tint: [u8; 3],
}

/// Cursor rules of one reviewed client image.
#[derive(Clone, Debug)]
pub struct CursorRules {
    pub origin: AssetIdentity,
    modes: Vec<(String, [u8; 3])>,
    hover: Modes,
    target: Modes,
    /// Mode per edge mask; `None` where the client keeps the previous cursor.
    scroll: [Option<u8>; 16],
    /// The model texture slot the client fills with the carried item's icon.
    pub item_icon_slot: u32,
}

#[derive(Clone, Copy, Debug)]
struct Modes {
    plain: u8,
    ally: u8,
    neutral: u8,
    enemy: u8,
}

impl CursorRules {
    pub fn read(image: &[u8], origin: AssetIdentity) -> Result<Self, String> {
        let fail = |reason: &str| format!("cursor rules {}: {reason}", origin.source);
        if image.len() > IMAGE_BUDGET {
            return Err(fail("client image exceeds budget"));
        }
        if [&origin.source, &origin.edition, &origin.version]
            .into_iter()
            .any(|value| value.trim().is_empty())
        {
            return Err(fail("missing source identity"));
        }
        if sha256(image) != REVIEWED_IMAGE {
            return Err(fail(
                "unsupported client image; no cursor rules substituted",
            ));
        }
        decode(image, origin.clone()).map_err(|e| fail(&e))
    }

    /// The mode and look the pointer state selects. An edge mask the client has no scroll
    /// cursor for (opposite edges together, or none) keeps the normal cursor, which is the
    /// mode the client's scroll handler starts from.
    pub fn look(&self, pointer: Pointer) -> Look<'_> {
        let mode = match pointer {
            Pointer::Normal => self.hover.plain,
            Pointer::Hover(relation) => self.hover.of(relation),
            Pointer::Target(None) => self.target.plain,
            Pointer::Target(Some(relation)) => self.target.of(relation),
            Pointer::Scroll(mask) => self
                .scroll
                .get(usize::from(mask))
                .copied()
                .flatten()
                .unwrap_or(self.hover.plain),
        };
        let (sequence, tint) = &self.modes[usize::from(mode)];
        Look {
            mode,
            sequence,
            tint: *tint,
        }
    }

    /// Every mode the client has and the sequence name it asks the model for.
    pub fn modes(&self) -> impl Iterator<Item = (u8, &str)> {
        self.modes
            .iter()
            .enumerate()
            .map(|(mode, (name, _))| (mode as u8, name.as_str()))
    }

    /// Rules shaped like the reviewed client's, for tests that never read an image.
    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn fixture() -> Self {
        let tints = [[255, 255, 255], [255, 255, 0], [255, 0, 0], [0, 255, 0]];
        let named = |name: &str, tint: [u8; 3]| (name.to_string(), tint);
        let mut modes = vec![named("Normal", tints[0])];
        modes.extend((1..=3).map(|i| named("Select", tints[i])));
        modes.push(named("Target", tints[0]));
        modes.extend((1..=3).map(|i| named("TargetSelect", tints[i])));
        modes.push(named("TargetSelect", [255, 10, 32]));
        modes.push(named("HoldItem", tints[0]));
        for name in [
            "Scroll Left",
            "Scroll Right",
            "Scroll Up",
            "Scroll Down",
            "Scroll Up Left",
            "Scroll Up Right",
            "Scroll Down Left",
            "Scroll Down Right",
        ] {
            modes.push(named(name, tints[0]));
        }
        let mut scroll = [None; 16];
        for (mask, mode) in [
            (1, 12),
            (2, 13),
            (4, 10),
            (5, 14),
            (6, 16),
            (8, 11),
            (9, 15),
            (10, 17),
        ] {
            scroll[mask] = Some(mode);
        }
        Self {
            origin: AssetIdentity {
                source: "synthetic".into(),
                edition: "fixture".into(),
                version: "fixture".into(),
            },
            modes,
            hover: Modes {
                plain: 0,
                neutral: 1,
                enemy: 2,
                ally: 3,
            },
            target: Modes {
                plain: 4,
                neutral: 5,
                enemy: 6,
                ally: 7,
            },
            scroll,
            item_icon_slot: 21,
        }
    }
}

impl Modes {
    fn of(self, relation: Relation) -> u8 {
        match relation {
            Relation::Friend => self.ally,
            Relation::Neutral => self.neutral,
            Relation::Enemy => self.enemy,
        }
    }
}

fn decode(image: &[u8], origin: AssetIdentity) -> Result<CursorRules, String> {
    let pe = Pe::parse(
        image,
        RuleInputBudget {
            image_bytes: IMAGE_BUDGET,
            profile_bytes: 128,
            sections: 96,
        },
    )
    .map_err(|e| e.to_string())?;
    let tags = tag_names(&pe)?;
    let count = usize::from(imm8(&pe, MODE_LIMIT_COMPARE, &[0x83, 0xfb])?) + 1;
    let group_table = imm32(&pe, GROUP_INDEX_LOAD, &[0x0f, 0xb6, 0x83])?;
    let jump_table = imm32(&pe, GROUP_JUMP, &[0xff, 0x24, 0x85])?;
    let tints = tint_table(&pe, count)?;
    let mut modes = Vec::with_capacity(count);
    for (mode, tint) in tints.into_iter().enumerate() {
        let group = *code(&pe, group_table + mode as u32, 1)?
            .first()
            .expect("one byte");
        let entry = word(&pe, jump_table + 4 * u32::from(group))?;
        let indices = tag_indices(&pe, entry, mode as u32)?;
        let names = indices
            .iter()
            .map(|&index| {
                tags.get(index as usize).map(String::as_str).ok_or_else(|| {
                    format!("mode {mode} names tag {index}, which is not registered")
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        modes.push((names.join(" "), tint));
    }
    let mode = |site: u32| -> Result<u8, String> {
        let value = imm8(&pe, site, &[0x6a])?;
        if usize::from(value) < count {
            Ok(value)
        } else {
            Err(format!("mode {value} at {site:#x} is out of range"))
        }
    };
    let hover = Modes {
        plain: mode(HOVER_NONE)?,
        ally: mode(HOVER_ALLY)?,
        neutral: mode(HOVER_NEUTRAL)?,
        enemy: mode(HOVER_ENEMY)?,
    };
    if mode(HOVER_FLAGGED)? != hover.neutral {
        return Err("flagged hover object does not share the neutral mode".into());
    }
    let target = Modes {
        plain: mode(TARGET_PLAIN)?,
        ally: mode(TARGET_ALLY)?,
        neutral: mode(TARGET_NEUTRAL)?,
        enemy: mode(TARGET_ENEMY)?,
    };
    Ok(CursorRules {
        origin,
        modes,
        hover,
        target,
        scroll: scroll_modes(&pe, count)?,
        item_icon_slot: u32::from(imm8(&pe, ITEM_ICON_SLOT, &[0x6a])?),
    })
}

fn code<'a>(pe: &Pe<'a>, va: u32, len: usize) -> Result<&'a [u8], String> {
    pe.read_from(va, true)
        .map_err(|e| e.to_string())?
        .get(..len)
        .ok_or_else(|| format!("truncated code at {va:#x}"))
}

fn data<'a>(pe: &Pe<'a>, va: u32, len: usize) -> Result<&'a [u8], String> {
    pe.read_from(va, false)
        .map_err(|e| e.to_string())?
        .get(..len)
        .ok_or_else(|| format!("truncated data at {va:#x}"))
}

fn imm32(pe: &Pe<'_>, va: u32, prefix: &[u8]) -> Result<u32, String> {
    let bytes = code(pe, va, prefix.len() + 4)?;
    let (head, tail) = bytes.split_at(prefix.len());
    if head != prefix {
        return Err(format!("unexpected instruction at {va:#x}"));
    }
    Ok(u32::from_le_bytes(tail.try_into().expect("four bytes")))
}

fn imm8(pe: &Pe<'_>, va: u32, prefix: &[u8]) -> Result<u8, String> {
    let bytes = code(pe, va, prefix.len() + 1)?;
    if bytes[..prefix.len()] != *prefix {
        return Err(format!("unexpected instruction at {va:#x}"));
    }
    Ok(bytes[prefix.len()])
}

/// A little-endian word of a code-section jump table.
fn word(pe: &Pe<'_>, va: u32) -> Result<u32, String> {
    let bytes = code(pe, va, 4)?;
    Ok(u32::from_le_bytes(bytes.try_into().expect("four bytes")))
}

/// Tag names by index from the registration calls.
fn tag_names(pe: &Pe<'_>) -> Result<Vec<String>, String> {
    let body = code(
        pe,
        TAG_REGISTRATION.start,
        (TAG_REGISTRATION.end - TAG_REGISTRATION.start) as usize,
    )?;
    let mut names: BTreeMap<usize, String> = BTreeMap::new();
    // `push imm8 index; push imm32 name; push dword [abs]; call rel32`.
    for window in body.windows(14) {
        let [0x6a, index, 0x68, a, b, c, d, 0xff, 0x35, _, _, _, _, 0xe8] = window[..] else {
            continue;
        };
        let at = u32::from_le_bytes([a, b, c, d]);
        let rest = pe.read_from(at, false).map_err(|e| e.to_string())?;
        let end = rest
            .iter()
            .take(TAG_NAME_BYTES)
            .position(|&byte| byte == 0)
            .ok_or_else(|| format!("tag name at {at:#x} is unterminated"))?;
        let raw = &rest[..end];
        let name = std::str::from_utf8(raw)
            .ok()
            .filter(|name| !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphabetic()))
            .ok_or_else(|| format!("tag name at {at:#x} is not a plain word"))?;
        if names
            .insert(usize::from(index), name.to_string())
            .is_some_and(|old| old != name)
        {
            return Err(format!("tag {index} is registered with two names"));
        }
    }
    (0..TAG_COUNT)
        .map(|index| {
            names
                .remove(&index)
                .ok_or_else(|| format!("tag {index} is not registered"))
        })
        .collect()
}

/// The tint of each mode: `base + 3 * mode` plus 0, 1 and 2, as red, green and blue.
fn tint_table(pe: &Pe<'_>, count: usize) -> Result<Vec<[u8; 3]>, String> {
    // `movzx r32, byte [edx + edx*2 + base]`, `ecx` first, `eax` for the next two.
    let loads = [(0x8c, 0), (0x84, 1), (0x84, 2)];
    let mut base = None;
    for (site, (modrm, offset)) in TINT_LOADS.into_iter().zip(loads) {
        let at = imm32(pe, site, &[0x0f, 0xb6, modrm, 0x52])?;
        let start = at.checked_sub(offset).ok_or("tint table base underflows")?;
        if *base.get_or_insert(start) != start {
            return Err("tint channels do not share one table".into());
        }
    }
    let base = base.expect("three loads");
    let bytes = data(pe, base, 3 * count)?;
    Ok(bytes.as_chunks::<3>().0.to_vec())
}

/// Mode per edge mask from the edge-scroll switch.
fn scroll_modes(pe: &Pe<'_>, count: usize) -> Result<[Option<u8>; 16], String> {
    let last = usize::from(imm8(pe, SCROLL_MASK_BOUND, &[0x83, 0xfb])?);
    let table = imm32(pe, SCROLL_MASK_JUMP, &[0xff, 0x24, 0x9d])?;
    if last + 1 > 15 {
        return Err("scroll mask table is longer than the edge masks".into());
    }
    let mut scroll = [None; 16];
    for index in 0..=last {
        let target = word(pe, table + 4 * index as u32)?;
        // A covered mask loads its mode with `mov esi, imm32`; the others share the
        // default target, which does not.
        if let [0xbe, a, b, c, d] = code(pe, target, 5)?[..] {
            let mode = u32::from_le_bytes([a, b, c, d]);
            if mode as usize >= count {
                return Err(format!("scroll mode {mode} is out of range"));
            }
            scroll[index + 1] = Some(mode as u8);
        }
    }
    Ok(scroll)
}

/// A value the mode cases push or load.
#[derive(Clone, Copy, Debug)]
enum Value {
    Immediate(u32),
    Frame(i32),
    Opaque,
}

/// Follows one mode case of the setter to its `0x966c10` call and returns the tag indices
/// it passes. The cases use only a handful of instruction forms (frame stores, `mov eax`,
/// `lea eax`, pushes, short jumps and calls elsewhere); anything else is refused.
fn tag_indices(pe: &Pe<'_>, entry: u32, mode: u32) -> Result<Vec<u32>, String> {
    const STEP_LIMIT: usize = 64;
    let mut frame: BTreeMap<i32, u32> = BTreeMap::new();
    let mut stack: Vec<Value> = Vec::new();
    let mut eax = Value::Opaque;
    let mut index: Option<u32> = None;
    let mut bound: Option<u32> = None;
    let mut at = entry;
    for _ in 0..STEP_LIMIT {
        let bytes = code(pe, at, 7).or_else(|_| code(pe, at, 1))?;
        let rel = |len: usize, delta: i64| -> u32 { (i64::from(at) + len as i64 + delta) as u32 };
        let signed = |i: usize| i64::from(bytes[i] as i8);
        let long = |i: usize| {
            i64::from(i32::from_le_bytes(
                bytes[i..i + 4].try_into().expect("four"),
            ))
        };
        let imm = |i: usize| u32::from_le_bytes(bytes[i..i + 4].try_into().expect("four"));
        match bytes {
            // mov dword [ebp+disp8], imm32
            [0xc7, 0x45, disp, ..] if bytes.len() >= 7 => {
                frame.insert(i32::from(*disp as i8), imm(3));
                at += 7;
            }
            // mov eax, imm32
            [0xb8, ..] if bytes.len() >= 5 => {
                eax = Value::Immediate(imm(1));
                at += 5;
            }
            // lea eax, [ebp+disp8]
            [0x8d, 0x45, disp, ..] => {
                eax = Value::Frame(i32::from(*disp as i8));
                at += 3;
            }
            // lea ecx, [ebx+disp8]: the scroll group's index is mode + disp
            [0x8d, 0x4b, disp, ..] => {
                index = Some(mode.wrapping_add_signed(i32::from(*disp as i8)));
                at += 3;
            }
            // cmp ecx, imm8, then ja rel32: out of range takes the default case
            [0x83, 0xf9, limit, ..] => {
                bound = Some(u32::from(*limit));
                at += 3;
            }
            [0x0f, 0x87, ..] if bytes.len() >= 6 => {
                let (Some(index), Some(bound)) = (index, bound) else {
                    return Err(format!("mode {mode}: range branch without a compare"));
                };
                at = if index > bound {
                    rel(6, long(2))
                } else {
                    at + 6
                };
            }
            // jmp dword [ecx*4 + table]
            [0xff, 0x24, 0x8d, ..] if bytes.len() >= 7 => {
                let Some(index) = index else {
                    return Err(format!("mode {mode}: table jump without an index"));
                };
                at = word(pe, imm(3) + 4 * index)?;
            }
            [0x6a, value, ..] => {
                stack.push(Value::Immediate(u32::from(*value)));
                at += 2;
            }
            [0x50, ..] => {
                stack.push(eax);
                at += 1;
            }
            [0x56, ..] => {
                stack.push(Value::Opaque);
                at += 1;
            }
            // mov ecx, edi
            [0x8b, 0xcf, ..] => at += 2,
            [0xeb, ..] if bytes.len() >= 2 => at = rel(2, signed(1)),
            [0xe8, ..] if bytes.len() >= 5 => {
                if rel(5, long(1)) == SPRITE_SET_TAGS {
                    let [
                        ..,
                        Value::Immediate(0),
                        Value::Immediate(count),
                        Value::Frame(first),
                        Value::Opaque,
                    ] = stack[..]
                    else {
                        return Err(format!("mode {mode}: unexpected tag call arguments"));
                    };
                    return (0..count)
                        .map(|i| {
                            frame
                                .get(&(first + 4 * i as i32))
                                .copied()
                                .ok_or_else(|| format!("mode {mode}: tag {i} is never stored"))
                        })
                        .collect();
                }
                at += 5;
            }
            _ => return Err(format!("mode {mode}: unexpected instruction at {at:#x}")),
        }
    }
    Err(format!(
        "mode {mode}: no tag call within {STEP_LIMIT} steps"
    ))
}

#[cfg(test)]
#[path = "cursor_rules_tests.rs"]
mod tests;
