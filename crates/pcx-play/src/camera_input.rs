//! Player camera input: zoom and rotation tables, scroll options and the jump-key
//! cycle, read from the reviewed client image, plus the pure rules that apply them.
//!
//! The 1.29.2.9231 client builds its six zoom levels and three rotation states on
//! the stack of the camera setter (`0x5de2c0`), eases them in the key and wheel
//! handler (`0x5e12f0`, `0x5e1090`) and derives the scroll multipliers in the
//! camera constructor (`0x5dced0`). Every number here is read from the operands of
//! those instructions; the instruction addresses only bind the reviewed image.
//! Evidence labels are in `docs/re-notes/input-camera.md`.
use crate::{
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    options_defaults::{Defaults, DiscoveryBudget},
    scripts::AssetIdentity,
    selection_box::UI_FRAME_HEIGHT,
};
use std::collections::BTreeMap;

const IMAGE_BUDGET: usize = 32 * 1024 * 1024;
/// Local ceilings on the table sizes read from the image (not original values).
const MAX_LEVELS: usize = 16;
/// Rotation states of the table: base, Insert held, Delete held (`Swing`).
const SWING_STATES: usize = 3;
/// The native UI frame is 0.8 wide and 0.6 tall at 4:3 (`glue-coordinate-runtime.md`).
const UI_FRAME_WIDTH: f32 = 0.8;
/// Edge-scroll margin per axis in the UI frame. Runtime observation: the producer of
/// the edge events was not located in the image, so the value is a fit to 11 measured
/// boundaries at three resolutions (`input-live-capture.md`), not an executable operand.
pub const EDGE_MARGIN_UI: f32 = 0.004;
/// Win32 `WHEEL_DELTA`, the unit of the wheel value the client receives; used only
/// to convert line-based wheel input, never to change the divisor read from the image.
pub const WHEEL_NOTCH: i32 = 120;

/// Zoom function `0x5de2c0` operand sites.
const LEVEL_COUNT_COMPARE: u32 = 0x5de300;
const ROTATION_COUNT_COMPARE: u32 = 0x5de33e;
const STATE_STRIDE_FIRST: u32 = 0x5de323;
const STATE_STRIDE_SECOND: u32 = 0x5de32d;
const ROTATION_LOAD: u32 = 0x5de377;
const ANGLE_LOAD: u32 = 0x5de3ae;
const DISTANCE_LOAD: u32 = 0x5de400;
/// `movaps xmm0, [table]` then `movups [ebp+d], xmm0`: the pairs fill the table frame.
const FRAME_COPIES: [(u32, u32); 6] = [
    (0x5de306, 0x5de30f),
    (0x5de31c, 0x5de326),
    (0x5de337, 0x5de341),
    (0x5de34e, 0x5de357),
    (0x5de365, 0x5de36c),
    (0x5de3a3, 0x5de3aa),
];
/// `mov dword [ebp+d], imm32` stores completing the table frame.
const FRAME_IMMEDIATES: [u32; 6] = [0x5de315, 0x5de330, 0x5de347, 0x5de35e, 0x5de370, 0x5de38c];
/// Level setter `0x5e1090`: top level and the ease passed to the setter.
const TOP_LEVEL_LOAD: u32 = 0x5e1096;
const ZOOM_EASE_STORE: u32 = 0x5e10b6;
/// Insert/Delete state change in `0x5e12f0`: the ease passed to the setter.
const ROTATION_EASE_STORE: u32 = 0x5e1740;
/// Wheel handler: delta divided by the wheel unit, compiled as a multiply-shift.
const WHEEL_DIVIDE: u32 = 0x5e148b;
/// Space (`0x5e1930`) and Backspace (`0x5e1120`) repeat window: `last + window`.
const SPACE_WINDOW: u32 = 0x5e1973;
const BACKSPACE_WINDOW: u32 = 0x5e1148;
/// Camera constructor `0x5dced0`: option reads, validation and multiplier stores.
const OPTION_KEY: u32 = 0x5dd914;
const OPTION_MOUSE: u32 = 0x5dd98e;
const OPTION_DISABLE: u32 = 0x5dda0c;
const KEY_SLIDER_MAX: u32 = 0x5dd920;
const MOUSE_SLIDER_MAX: u32 = 0x5dd998;
const KEY_DIVISOR: u32 = 0x5dd940;
const MOUSE_DIVISOR: u32 = 0x5dd9b8;
const KEY_CEILING: u32 = 0x5dd94d;
const MOUSE_CEILING: u32 = 0x5dd9c5;
const KEY_OFFSET: u32 = 0x5dd95a;
const MOUSE_OFFSET: u32 = 0x5dd9ce;
const MOUSE_DEFAULT_STORE: u32 = 0x5dd743;
const KEY_DEFAULT_STORE: u32 = 0x5dd74d;
const KEY_MULTIPLIER_STORE: u32 = 0x5dd980;
const MOUSE_MULTIPLIER_STORE: u32 = 0x5dd9fe;
const DISABLE_STORE: u32 = 0x5dda16;

/// Option names the constructor's indices must resolve to in the descriptor table.
const KEY_NAME: &str = "keyscroll";
const MOUSE_NAME: &str = "mousescroll";
const DISABLE_NAME: &str = "mousescrolldisable";

/// Local resource policy for the descriptor-table scan (not original values).
const DISCOVERY: DiscoveryBudget = DiscoveryBudget {
    image: RuleInputBudget {
        image_bytes: IMAGE_BUDGET,
        profile_bytes: 128 * 1024,
        sections: 96,
    },
    string_bytes: 4096,
    candidate_limit: 32,
    work_bytes: 64 * 1024 * 1024,
};

/// Which camera pose a rotation state holds: the base state, Insert held, Delete held.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Swing {
    #[default]
    Normal,
    Insert,
    Delete,
}

impl Swing {
    fn index(self) -> usize {
        match self {
            Swing::Normal => 0,
            Swing::Insert => 1,
            Swing::Delete => 2,
        }
    }
}

/// Zoom level and rotation state, the two selectors of the camera table.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct View {
    pub level: usize,
    pub swing: Swing,
}

/// Camera fields the zoom table sets, in the script's degrees and world units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub distance: f32,
    pub angle_of_attack: f32,
    pub rotation: f32,
}

/// A pose to approach and how long to take.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ease {
    pub pose: Pose,
    pub seconds: f32,
}

/// Converts an option slider value to the scroll speed multiplier.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollRule {
    /// Largest accepted slider value.
    pub slider_max: u32,
    divisor: f32,
    ceiling: f32,
    offset: f32,
    /// Multiplier kept when the value is out of range.
    pub default: f32,
}

impl ScrollRule {
    /// `value / divisor + offset` for an in-range value, else the constructor default.
    pub fn multiplier(&self, value: u32) -> f32 {
        let ratio = value as f32 / self.divisor;
        if value > self.slider_max || !(0.0..=self.ceiling).contains(&ratio) {
            self.default
        } else {
            ratio + self.offset
        }
    }
}

/// Scroll option values (`mousescroll`, `keyscroll`, `mousescrolldisable`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScrollOptions {
    pub mouse: u32,
    pub key: u32,
    /// Any nonzero value disables edge scrolling.
    pub mouse_disable: u32,
}

/// Camera input rules of one reviewed client image.
#[derive(Clone, Debug)]
pub struct CameraInputRules {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    distance: Vec<f32>,
    angle_of_attack: Vec<f32>,
    rotation: Vec<f32>,
    top_level: usize,
    /// Seconds a zoom change takes.
    pub zoom_ease: f32,
    /// Seconds a rotation state change takes.
    pub rotation_ease: f32,
    /// Wheel units per zoom step.
    pub wheel_unit: i32,
    /// Seconds within which a repeated Space, Backspace or idle-worker press continues
    /// the previous cycle.
    pub repeat_window: f32,
    pub scroll: ScrollRule,
    /// Option values the client falls back to without a saved preference.
    pub fallback: ScrollOptions,
}

impl CameraInputRules {
    pub fn read(image: &[u8], origin: AssetIdentity) -> Result<Self, String> {
        let fail = |reason: &str| format!("camera input {}: {reason}", origin.source);
        if image.len() > IMAGE_BUDGET {
            return Err(fail("client image exceeds budget"));
        }
        if [&origin.source, &origin.edition, &origin.version]
            .into_iter()
            .any(|value| value.trim().is_empty())
        {
            return Err(fail("missing source identity"));
        }
        let digest = sha256(image);
        if digest != REVIEWED_IMAGE {
            return Err(fail(
                "unsupported client image; no camera rules substituted",
            ));
        }
        let decoded = decode(image).map_err(|e| fail(&e))?;
        let defaults = Defaults::discover(image, origin.clone(), DISCOVERY)
            .map_err(|e| fail(&e.to_string()))?;
        let rows: Vec<OptionRow<'_>> = defaults
            .entries()
            .iter()
            .map(|e| (e.index, e.name, e.fallback))
            .collect();
        let fallback = fallback_options(&rows, &decoded.indices).map_err(|e| fail(&e))?;
        Ok(Self {
            origin,
            image_sha256: digest,
            distance: decoded.table.distance,
            angle_of_attack: decoded.table.angle_of_attack,
            rotation: decoded.table.rotation,
            top_level: decoded.table.levels - 1,
            zoom_ease: decoded.zoom_ease,
            rotation_ease: decoded.rotation_ease,
            wheel_unit: decoded.wheel_unit,
            repeat_window: decoded.repeat_window,
            scroll: decoded.scroll,
            fallback,
        })
    }

    /// Rules shaped like the reviewed client's six levels and three rotation states, for
    /// tests that never read an image.
    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn fixture() -> Self {
        let mut rotation = vec![90.0; 6];
        rotation.extend([20.0; 6]);
        rotation.extend([160.0; 6]);
        Self {
            origin: AssetIdentity {
                source: "synthetic".into(),
                edition: "fixture".into(),
                version: "fixture".into(),
            },
            image_sha256: [0; 32],
            distance: vec![1650.0, 1600.0, 1500.0, 1400.0, 1275.0, 1200.0],
            angle_of_attack: vec![304.0, 311.0, 318.0, 325.0, 332.0, 335.0],
            rotation,
            top_level: 5,
            zoom_ease: 1.0,
            rotation_ease: 0.5,
            wheel_unit: 120,
            repeat_window: 1.0,
            scroll: ScrollRule {
                slider_max: 100,
                divisor: 100.0,
                ceiling: 1.0,
                offset: 0.5,
                default: 1.0,
            },
            fallback: ScrollOptions {
                mouse: 50,
                key: 50,
                mouse_disable: 0,
            },
        }
    }

    /// Number of zoom levels.
    pub fn levels(&self) -> usize {
        self.top_level + 1
    }

    /// The table pose of a level and rotation state.
    pub fn pose(&self, view: View) -> Pose {
        Pose {
            distance: self.distance[view.level],
            angle_of_attack: self.angle_of_attack[view.level],
            rotation: self.rotation[view.level + self.levels() * view.swing.index()],
        }
    }

    /// Moves to `requested`, clamped to the top level; a change eases over the zoom time.
    fn set_level(&self, view: &mut View, requested: usize) -> Option<Ease> {
        let level = requested.min(self.top_level);
        if level == view.level {
            return None;
        }
        view.level = level;
        Some(Ease {
            pose: self.pose(*view),
            seconds: self.zoom_ease,
        })
    }

    /// Page Up, or a wheel step away from the user.
    pub fn zoom_in(&self, view: &mut View) -> Option<Ease> {
        self.set_level(view, view.level + 1)
    }

    /// Page Down, or a wheel step toward the user. Nothing happens at level 0.
    pub fn zoom_out(&self, view: &mut View) -> Option<Ease> {
        view.level
            .checked_sub(1)
            .and_then(|level| self.set_level(view, level))
    }

    /// Mouse wheel: the value is divided by the wheel unit, truncating toward zero; a
    /// positive quotient zooms in, a negative one out, a smaller magnitude does nothing.
    pub fn wheel(&self, view: &mut View, delta: i32) -> Option<Ease> {
        match delta / self.wheel_unit {
            ..=-1 => self.zoom_out(view),
            0 => None,
            _ => self.zoom_in(view),
        }
    }

    /// Insert or Delete pressed: ignored while any state is held.
    pub fn swing_press(&self, view: &mut View, swing: Swing) -> Option<Ease> {
        if view.swing != Swing::Normal {
            return None;
        }
        view.swing = swing;
        Some(self.rotation_ease_to(*view))
    }

    /// Insert or Delete released (also Numpad 0 for Insert and Numpad Decimal for
    /// Delete): returns to the base state only from the state it started.
    pub fn swing_release(&self, view: &mut View, swing: Swing) -> Option<Ease> {
        if view.swing != swing {
            return None;
        }
        view.swing = Swing::Normal;
        Some(self.rotation_ease_to(*view))
    }

    fn rotation_ease_to(&self, view: View) -> Ease {
        Ease {
            pose: self.pose(view),
            seconds: self.rotation_ease,
        }
    }
}

/// Everything the instruction operands give; the option fallbacks need the descriptor table.
struct Decoded {
    table: ZoomTable,
    zoom_ease: f32,
    rotation_ease: f32,
    wheel_unit: i32,
    repeat_window: f32,
    scroll: ScrollRule,
    indices: OptionIndices,
}

fn decode(image: &[u8]) -> Result<Decoded, String> {
    let pe = Pe::parse(
        image,
        RuleInputBudget {
            image_bytes: IMAGE_BUDGET,
            profile_bytes: 128,
            sections: 96,
        },
    )
    .map_err(|e| e.to_string())?;
    let table = zoom_table(&pe)?;
    let top = imm32(&pe, TOP_LEVEL_LOAD, &[0xb8])? as usize;
    if top + 1 != table.levels {
        return Err("top level disagrees with the zoom table length".into());
    }
    let (scroll, indices) = scroll_rule(&pe)?;
    Ok(Decoded {
        table,
        zoom_ease: ease(&pe, ZOOM_EASE_STORE)?,
        rotation_ease: ease(&pe, ROTATION_EASE_STORE)?,
        wheel_unit: wheel_unit(&pe)?,
        repeat_window: repeat_window(&pe)?,
        scroll,
        indices,
    })
}

struct ZoomTable {
    levels: usize,
    distance: Vec<f32>,
    angle_of_attack: Vec<f32>,
    rotation: Vec<f32>,
}

fn code<'a>(pe: &Pe<'a>, va: u32, len: usize) -> Result<&'a [u8], String> {
    pe.read_from(va, true)
        .map_err(|e| e.to_string())?
        .get(..len)
        .ok_or_else(|| format!("truncated code at {va:#x}"))
}

/// The four bytes after `prefix` at `va`.
fn imm32(pe: &Pe<'_>, va: u32, prefix: &[u8]) -> Result<u32, String> {
    let bytes = code(pe, va, prefix.len() + 4)?;
    let (head, tail) = bytes.split_at(prefix.len());
    if head != prefix {
        return Err(format!("unexpected instruction at {va:#x}"));
    }
    Ok(u32::from_le_bytes(tail.try_into().expect("four bytes")))
}

/// The byte after `prefix` at `va`.
fn imm8(pe: &Pe<'_>, va: u32, prefix: &[u8]) -> Result<u8, String> {
    let bytes = code(pe, va, prefix.len() + 1)?;
    if bytes[..prefix.len()] != *prefix {
        return Err(format!("unexpected instruction at {va:#x}"));
    }
    Ok(bytes[prefix.len()])
}

/// A float in the data section addressed by the absolute operand after `prefix`.
fn abs_f32(pe: &Pe<'_>, va: u32, prefix: &[u8]) -> Result<f32, String> {
    let address = imm32(pe, va, prefix)?;
    let word = pe
        .read_from(address, false)
        .map_err(|e| e.to_string())?
        .get(..4)
        .ok_or_else(|| format!("truncated constant at {address:#x}"))?;
    let value = f32::from_le_bytes(word.try_into().expect("four bytes"));
    if value.is_finite() {
        Ok(value)
    } else {
        Err(format!("non-finite constant at {address:#x}"))
    }
}

fn ease(pe: &Pe<'_>, va: u32) -> Result<f32, String> {
    // `mov dword [esp], imm32`: the duration argument of the camera setter.
    let bits = imm32(pe, va, &[0xc7, 0x04, 0x24])?;
    let seconds = f32::from_bits(bits);
    if seconds.is_finite() && seconds > 0.0 {
        Ok(seconds)
    } else {
        Err(format!("invalid ease duration at {va:#x}"))
    }
}

/// Reads the stack frame the setter fills and the three tables indexed in it.
fn zoom_table(pe: &Pe<'_>) -> Result<ZoomTable, String> {
    let levels = usize::from(imm8(pe, LEVEL_COUNT_COMPARE, &[0x83, 0xfa])?);
    let rotation_count = usize::from(imm8(pe, ROTATION_COUNT_COMPARE, &[0x83, 0xf9])?);
    // `lea eax,[eax+eax*2]` then `lea ecx,[edx+eax*2]`: a state index of s selects
    // table entry level + 3 * 2 * s. The low SIB bits name the registers checked here.
    let stride_first = imm8(pe, STATE_STRIDE_FIRST, &[0x8d, 0x04])?;
    let stride_second = imm8(pe, STATE_STRIDE_SECOND, &[0x8d, 0x0c])?;
    if (stride_first & 0x3f, stride_second & 0x3f) != (0x00, 0x02) {
        return Err("rotation state index registers are not as reviewed".into());
    }
    let stride = (1 + (1usize << (stride_first >> 6))) * (1usize << (stride_second >> 6));
    if levels == 0
        || levels > MAX_LEVELS
        || stride != levels
        || rotation_count != levels * SWING_STATES
    {
        return Err("zoom table shape is not levels x three rotation states".into());
    }
    let mut frame: BTreeMap<i32, u32> = BTreeMap::new();
    for (load, store) in FRAME_COPIES {
        let source = imm32(pe, load, &[0x0f, 0x28, 0x05])?;
        let bytes = pe
            .read_from(source, false)
            .map_err(|e| e.to_string())?
            .get(..16)
            .ok_or("truncated table words")?;
        let disp = frame_disp(pe, store, &[0x0f, 0x11])?;
        for (i, word) in bytes.as_chunks::<4>().0.iter().enumerate() {
            frame.insert(disp + 4 * i as i32, u32::from_le_bytes(*word));
        }
    }
    for site in FRAME_IMMEDIATES {
        let bytes = code(pe, site, 7)?;
        let [0xc7, 0x45, disp, a, b, c, d] = bytes[..] else {
            return Err(format!("unexpected table store at {site:#x}"));
        };
        frame.insert(i32::from(disp as i8), u32::from_le_bytes([a, b, c, d]));
    }
    let load_base = |site: u32, prefix: &[u8], disp32: bool| -> Result<i32, String> {
        let bytes = code(pe, site, prefix.len() + if disp32 { 4 } else { 1 })?;
        if bytes[..prefix.len()] != *prefix {
            return Err(format!("unexpected table index at {site:#x}"));
        }
        let tail = &bytes[prefix.len()..];
        Ok(if disp32 {
            i32::from_le_bytes(tail.try_into().expect("four bytes"))
        } else {
            i32::from(tail[0] as i8)
        })
    };
    // `movss xmm0,[ebp+reg*4+disp]`.
    let rotation_base = load_base(ROTATION_LOAD, &[0xf3, 0x0f, 0x10, 0x84, 0x85], true)?;
    let angle_base = load_base(ANGLE_LOAD, &[0xf3, 0x0f, 0x10, 0x44, 0x85], false)?;
    let distance_base = load_base(DISTANCE_LOAD, &[0xf3, 0x0f, 0x10, 0x44, 0xb5], false)?;
    let words = |base: i32, count: usize| -> Result<Vec<f32>, String> {
        (0..count)
            .map(|i| {
                frame
                    .get(&(base + 4 * i as i32))
                    .map(|bits| f32::from_bits(*bits))
                    .filter(|value| value.is_finite())
                    .ok_or_else(|| "zoom table slot is not filled by the setter".to_string())
            })
            .collect()
    };
    let distance = words(distance_base, levels)?;
    let angle_of_attack = words(angle_base, levels)?;
    let rotation = words(rotation_base, rotation_count)?;
    if distance.iter().any(|d| *d <= 0.0) {
        return Err("zoom distance is not positive".into());
    }
    Ok(ZoomTable {
        levels,
        distance,
        angle_of_attack,
        rotation,
    })
}

/// Displacement of an `ebp`-relative `movups [ebp+disp], xmm0` store.
fn frame_disp(pe: &Pe<'_>, site: u32, opcode: &[u8]) -> Result<i32, String> {
    let bytes = code(pe, site, 7)?;
    if bytes[..2] != *opcode {
        return Err(format!("unexpected table store at {site:#x}"));
    }
    match bytes[2] {
        0x45 => Ok(i32::from(bytes[3] as i8)),
        0x85 => Ok(i32::from_le_bytes(
            bytes[3..7].try_into().expect("four bytes"),
        )),
        _ => Err(format!("unexpected table store at {site:#x}")),
    }
}

/// Divisor of the wheel handler's `imul`, add, shift sequence.
fn wheel_unit(pe: &Pe<'_>) -> Result<i32, String> {
    let bytes = code(pe, WHEEL_DIVIDE, 12)?;
    let [0xb8, a, b, c, d, 0xf7, 0xe9, 0x03, 0xd1, 0xc1, 0xfa, shift] = bytes[..] else {
        return Err("unexpected wheel division".into());
    };
    let magic = u32::from_le_bytes([a, b, c, d]);
    if !(1..32).contains(&shift) {
        return Err("wheel division shift is out of range".into());
    }
    let scale = 1u64 << (32 + u32::from(shift));
    let unit = ((scale + u64::from(magic) / 2) / u64::from(magic)) as i32;
    // A compiler's magic for dividing by `unit` is the rounded-up reciprocal; a signed
    // quotient confirms the sequence divides by `unit` at the sign and unit boundaries.
    if unit < 2 || scale.div_ceil(unit as u64) != u64::from(magic) {
        return Err("wheel division magic is not the reciprocal of a constant".into());
    }
    let divide = |n: i32| -> i32 {
        let high = ((i64::from(n) * i64::from(magic as i32)) >> 32) as i32;
        let sum = high.wrapping_add(n);
        let shifted = sum >> shift;
        shifted.wrapping_add(((shifted as u32) >> 31) as i32)
    };
    if [(unit, 1), (unit - 1, 0), (-unit, -1), (1 - unit, 0)]
        .into_iter()
        .any(|(n, q)| divide(n) != q)
    {
        return Err("wheel division is not a division by a constant".into());
    }
    Ok(unit)
}

fn repeat_window(pe: &Pe<'_>) -> Result<f32, String> {
    // `addss xmm0, [window]`.
    let prefix = [0xf3, 0x0f, 0x58, 0x05];
    let space = abs_f32(pe, SPACE_WINDOW, &prefix)?;
    let backspace = abs_f32(pe, BACKSPACE_WINDOW, &prefix)?;
    if space != backspace || space <= 0.0 {
        return Err("Space and Backspace windows disagree".into());
    }
    Ok(space)
}

/// Option indices of the key, mouse and disable reads.
struct OptionIndices {
    key: u32,
    mouse: u32,
    disable: u32,
}

fn scroll_rule(pe: &Pe<'_>) -> Result<(ScrollRule, OptionIndices), String> {
    let slider_max = |site, prefix: &[u8]| imm8(pe, site, prefix).map(u32::from);
    let max = slider_max(KEY_SLIDER_MAX, &[0x83, 0xf9])?;
    if max != slider_max(MOUSE_SLIDER_MAX, &[0x83, 0xf8])? || max == 0 || max > 0x7f {
        return Err("scroll slider maxima disagree".into());
    }
    let pair = |key: u32, mouse: u32, prefix: &[u8]| -> Result<f32, String> {
        let (key, mouse) = (abs_f32(pe, key, prefix)?, abs_f32(pe, mouse, prefix)?);
        if key == mouse {
            Ok(key)
        } else {
            Err("key and mouse scroll constants disagree".into())
        }
    };
    let divisor = pair(KEY_DIVISOR, MOUSE_DIVISOR, &[0xf3, 0x0f, 0x5e, 0x0d])?;
    // The key branch loads the ceiling with `movss xmm2,[abs]`, the mouse branch compares
    // against it with `comiss xmm1,[abs]`.
    let ceiling = abs_f32(pe, KEY_CEILING, &[0xf3, 0x0f, 0x10, 0x15])?;
    if ceiling != abs_f32(pe, MOUSE_CEILING, &[0x0f, 0x2f, 0x0d])? {
        return Err("key and mouse scroll constants disagree".into());
    }
    let offset = pair(KEY_OFFSET, MOUSE_OFFSET, &[0xf3, 0x0f, 0x58, 0x0d])?;
    // `mov dword [edi+0x4ec], imm32` (mouse) and `[edi+0x4f0]` (key): initial multipliers.
    let default = f32::from_bits(imm32(
        pe,
        MOUSE_DEFAULT_STORE,
        &[0xc7, 0x87, 0xec, 0x04, 0x00, 0x00],
    )?);
    if default.to_bits() != imm32(pe, KEY_DEFAULT_STORE, &[0xc7, 0x87, 0xf0, 0x04, 0x00, 0x00])? {
        return Err("key and mouse scroll defaults disagree".into());
    }
    // `movss [edi+0x4f0], xmm1` (key) and `[edi+0x4ec], xmm0` (mouse) store the result.
    for (site, tail) in [
        (KEY_MULTIPLIER_STORE, [0x8f, 0xf0, 0x04, 0x00, 0x00]),
        (MOUSE_MULTIPLIER_STORE, [0x87, 0xec, 0x04, 0x00, 0x00]),
    ] {
        if code(pe, site, 8)? != [&[0xf3, 0x0f, 0x11][..], &tail].concat() {
            return Err(format!("unexpected multiplier store at {site:#x}"));
        }
    }
    if code(pe, DISABLE_STORE, 3)? != [0x89, 0x47, 0x30] {
        return Err("unexpected scroll disable store".into());
    }
    let index = |site| imm8(pe, site, &[0x6a]).map(u32::from);
    let rule = ScrollRule {
        slider_max: max,
        divisor,
        ceiling,
        offset,
        default,
    };
    if !(divisor > 0.0 && ceiling > 0.0 && offset >= 0.0 && default > 0.0) {
        return Err("scroll constants are out of range".into());
    }
    Ok((
        rule,
        OptionIndices {
            key: index(OPTION_KEY)?,
            mouse: index(OPTION_MOUSE)?,
            disable: index(OPTION_DISABLE)?,
        },
    ))
}

/// Descriptor rows as (index, name, fallback text).
type OptionRow<'a> = (u32, &'a str, &'a [u8]);

fn fallback_options(
    rows: &[OptionRow<'_>],
    indices: &OptionIndices,
) -> Result<ScrollOptions, String> {
    let read = |index: u32, name: &str| -> Result<u32, String> {
        let (_, found, fallback) = rows
            .iter()
            .find(|row| row.0 == index)
            .ok_or_else(|| format!("option {index} is absent from the descriptor table"))?;
        if *found != name {
            return Err(format!("option {index} is {found:?}, not {name}"));
        }
        std::str::from_utf8(fallback)
            .ok()
            .and_then(|text| text.trim().parse().ok())
            .ok_or_else(|| format!("{name} fallback is not an unsigned integer"))
    };
    Ok(ScrollOptions {
        mouse: read(indices.mouse, MOUSE_NAME)?,
        key: read(indices.key, KEY_NAME)?,
        mouse_disable: read(indices.disable, DISABLE_NAME)?,
    })
}

/// Direction bits of an edge-scroll event (the client's mask values).
pub mod edge {
    pub const UP: u8 = 1;
    pub const DOWN: u8 = 2;
    pub const LEFT: u8 = 4;
    pub const RIGHT: u8 = 8;
}

/// Edge directions for a pointer at `cursor` in a window of `size` (same unit, origin
/// top left): strictly inside the margin of the 0.8 x 0.6 UI frame on each axis.
pub fn edge_mask(cursor: [f32; 2], size: [f32; 2]) -> u8 {
    let horizontal = EDGE_MARGIN_UI * size[0] / UI_FRAME_WIDTH;
    let vertical = EDGE_MARGIN_UI * size[1] / UI_FRAME_HEIGHT;
    let mut mask = 0;
    if cursor[1] < vertical {
        mask |= edge::UP;
    }
    if cursor[1] > size[1] - vertical {
        mask |= edge::DOWN;
    }
    if cursor[0] < horizontal {
        mask |= edge::LEFT;
    }
    if cursor[0] > size[0] - horizontal {
        mask |= edge::RIGHT;
    }
    mask
}

/// Repeat-window cycling shared by Space, Backspace and the idle-worker keys. Times
/// are real seconds.
#[derive(Clone, Copy, Debug)]
pub struct Cycle {
    last: f64,
    index: usize,
}

impl Default for Cycle {
    fn default() -> Self {
        Self {
            last: f64::NEG_INFINITY,
            index: 0,
        }
    }
}

impl Cycle {
    fn within(&self, now: f64, window: f32) -> bool {
        now < self.last + f64::from(window)
    }

    fn visit(&mut self, now: f64, index: usize) -> usize {
        self.last = now;
        self.index = index;
        index
    }

    /// Backspace and idle workers: the entry after the previous one inside the window,
    /// otherwise the first. `None` for an empty list, which leaves the cycle untouched.
    pub fn next(&mut self, now: f64, window: f32, len: usize) -> Option<usize> {
        if len == 0 {
            return None;
        }
        let index = if self.within(now, window) {
            (self.index + 1) % len
        } else {
            0
        };
        Some(self.visit(now, index))
    }

    /// Space: the entry before the previous one inside the window, otherwise `newest`.
    pub fn previous(&mut self, now: f64, window: f32, len: usize, newest: usize) -> Option<usize> {
        if len == 0 {
            return None;
        }
        let index = if self.within(now, window) {
            (self.index + len - 1) % len
        } else {
            newest
        };
        Some(self.visit(now, index))
    }
}

#[cfg(test)]
#[path = "camera_input_tests.rs"]
mod tests;
