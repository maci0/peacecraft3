//! The spell orb class (`AIsb`: Orb of Slow `AIsb`, Orb of Darkness `AIdf`, Orb of Lightning
//! `AIll`): the registration, the proc rule read from the reviewed client, and the rows of the
//! wrapped ability. The decode is in `docs/re-notes/passive-held-items.md`, "`AIsb`".
//!
//! A held orb of this class adds `DataA1` flat damage and the air switch like every orb. At the
//! start of each swing the client rolls a proc per target kind with a counter that raises the
//! chance after each miss; a hit that lands while the proc is held casts the ability named by
//! `UnitID1` on the target. The chance of the row (`DataB1` units, `DataC1` heroes, `DataD1`
//! summons) selects an increment from a table the image builds at first use.
use super::*;
use crate::script_random::real;

/// Registration call of the spell orb class (source `AIsb`, base `AIDB`): window `0xca6e20`.
pub(super) const SPELL_ORB_WINDOW: u32 = 0xca6e20;
/// Registration call of the Orb of Darkness spell class (source `ANbs`, computed base `AAsm`).
pub(super) const BLACK_ARROW_WINDOW: u32 = 0xc73ab0;
/// `AIsb` slot 73, the swing-start proc roll. The call of the secondary real draw sits at
/// [`ROLL_DRAW_AT`] and its stream is the byte pushed just before it.
pub(super) const ROLL_FN: u32 = 0xc9f2d0;
pub(super) const ROLL_LEN: usize = 0xc4;
const ROLL_DRAW_AT: usize = 0x8a;
/// The secondary real draw (`0x69a030`): stream number and out pointer are its stack arguments.
const SECONDARY_REAL_FN: u32 = 0x69a030;
/// The chance scaling and increment table builder (`0xcaec30`): its window covers the constants,
/// the 21 table stores and the final lookup.
pub(super) const CHANCE_FN: u32 = 0xcaec30;
pub(super) const CHANCE_LEN: usize = 0x47e;
/// Reciprocal table of the divide helper (`0x9a1d00`): 1024 entries plus the interpolation end.
pub(super) const RECIPROCAL_TABLE: u32 = 0xed6cf0;
pub(super) const RECIPROCAL_ENTRIES: usize = 1025;
/// Number of 5% steps the increment table has (0 through 100 percent).
pub(super) const STEPS: usize = 21;
/// Proc counter of every target kind when the orb is attached, after a proc and after a miss it
/// is set to [`PROC_COUNTER_AFTER_HIT`] and raised by [`PROC_COUNTER_STEP`] (`0xc9eeb5`,
/// `0xc9f373`, `0xc9f388`).
const PROC_COUNTER_START: i32 = 1;
const PROC_COUNTER_AFTER_HIT: i32 = 1;
const PROC_COUNTER_STEP: i32 = 1;
/// Target kinds the chance distinguishes: units, heroes and summoned units (`DataB1`, `DataC1`,
/// `DataD1`).
pub(super) const KINDS: usize = 3;
const CHANCE_COLUMNS: [&str; KINDS] = ["DataB1", "DataC1", "DataD1"];
/// Stack-cleanup and operand bytes of the shapes read below.
const PUSH_IMM8: u8 = 0x6a;
const LEA_EAX_EBP: [u8; 2] = [0x8d, 0x45];
const ADD_ESP_8: [u8; 3] = [0x83, 0xc4, 0x08];
const LOOKUP: [u8; 3] = [0x8b, 0x0c, 0xb5];
const MOV_EAX_IMM32_OPCODE: u8 = 0xb8;

/// Who a wrapped ability may be cast on, from its `targs1`. Tokens are the ones the shipped
/// wrapped rows list; relationship tokens follow the target query of the reviewed client.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WrappedTargets {
    pub air: bool,
    pub ground: bool,
    pub structure: bool,
    pub enemy: bool,
    pub neutral: bool,
    pub friend: bool,
    pub organic: bool,
}

/// Black Arrow as the orb casts it (`ANbs`, class base `AAsm`). The cast marks the target with
/// `buff` for `duration` (`hero_duration` for a hero); a marked unit that dies makes `count`
/// units of `summon` for `summon_duration` that carry `marker`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlackArrow {
    pub buff: [u8; 4],
    pub marker: [u8; 4],
    pub duration: f32,
    pub hero_duration: f32,
    pub targets: WrappedTargets,
    pub summon: [u8; 4],
    pub count: i32,
    pub summon_duration: f32,
    pub range: f32,
}

/// The ability an armed hit casts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WrappedAbility {
    BlackArrow(BlackArrow),
    Purge(Purge),
    /// A class whose cast effect is not decoded; a proc that reaches it refuses.
    Undecoded {
        alias: [u8; 4],
        code: [u8; 4],
    },
}

/// The proc rule and wrapped ability of one spell orb row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpellOrb {
    /// Secondary random stream of the proc draw.
    pub channel: usize,
    /// Increment of the proc chance per counter step, as real bits, for units, heroes and
    /// summons.
    pub increments: [u32; KINDS],
    pub wrapped: WrappedAbility,
}

/// The registered spell orb class and what the image supplies to the proc rule.
#[derive(Clone, Debug)]
pub(super) struct SpellClass {
    pub(super) class: String,
    /// Wrapped Black Arrow class code and its distinct registration.
    pub(super) black_arrow: String,
    channel: usize,
    /// Increment per 5% step, as real bits.
    steps: Vec<u32>,
    scale: u32,
    round: u32,
}

/// Bytes of the reviewed image the spell orb rules are read from.
#[derive(Clone, Debug)]
pub(crate) struct SpellOrbImage {
    pub(crate) window: Vec<u8>,
    pub(crate) black_arrow_window: Vec<u8>,
    pub(crate) black_arrow_base: Vec<u8>,
    pub(crate) roll: Vec<u8>,
    pub(crate) chance: Vec<u8>,
    /// The two real constants the chance function pushes: the step scale and the rounding bias.
    pub(crate) scale: [u8; 4],
    pub(crate) round: [u8; 4],
    pub(crate) reciprocal: Vec<u8>,
}

impl SpellOrbImage {
    /// Reads every window from the parsed image. Addresses are the ones named above; the shapes
    /// are checked when [`Rules::with_spell_orb`] reads them.
    pub(crate) fn read(pe: &Pe<'_>) -> Result<Self, String> {
        let bytes = |va: u32, len: usize, code: bool| -> Result<Vec<u8>, String> {
            pe.read_from(va, code)
                .map_err(|e| e.to_string())?
                .get(..len)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| format!("truncated image bytes at {va:#x}"))
        };
        let window = bytes(SPELL_ORB_WINDOW, CLASS_WINDOW_LEN, true)?;
        let black_arrow_window = bytes(BLACK_ARROW_WINDOW, HEAL_WINDOW_LEN, true)?;
        let base_va = call_target(&black_arrow_window, BLACK_ARROW_WINDOW, 5)?;
        let chance = bytes(CHANCE_FN, CHANCE_LEN, true)?;
        let (scale_va, round_va) = chance_constants(&chance)?;
        Ok(Self {
            window,
            black_arrow_base: bytes(base_va, BASE_FN_LEN, true)?,
            black_arrow_window,
            roll: bytes(ROLL_FN, ROLL_LEN, true)?,
            chance,
            scale: bytes(scale_va, 4, false)?.try_into().expect("four bytes"),
            round: bytes(round_va, 4, false)?.try_into().expect("four bytes"),
            reciprocal: bytes(RECIPROCAL_TABLE, RECIPROCAL_ENTRIES * 4, false)?,
        })
    }
}

fn le32(bytes: &[u8], at: usize) -> Result<u32, String> {
    bytes
        .get(at..at + 4)
        .and_then(|b| <[u8; 4]>::try_from(b).ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| "truncated spell orb code".to_string())
}

/// The scale and rounding constants pushed at the head of the chance function: `push round`,
/// `push scale`, two immediates 5 bytes apart, before the argument pushes (`0xcaec56`).
fn chance_constants(chance: &[u8]) -> Result<(u32, u32), String> {
    const AT: usize = 0x26;
    if chance.get(AT) != Some(&PUSH_IMM32) || chance.get(AT + 5) != Some(&PUSH_IMM32) {
        return Err("unsupported spell orb chance function".into());
    }
    Ok((le32(chance, AT + 6)?, le32(chance, AT + 1)?))
}

/// Stream of the proc draw and the shape around it: the roll function pushes the stream number
/// and an out pointer, then calls the secondary real draw.
pub(super) fn roll_channel(roll: &[u8]) -> Result<usize, String> {
    let fail = || "unsupported spell orb roll function".to_string();
    let call_at = ROLL_DRAW_AT + 3;
    if roll.len() != ROLL_LEN
        || roll.get(ROLL_DRAW_AT) != Some(&PUSH_IMM8)
        || roll.get(ROLL_DRAW_AT + 2) != Some(&PUSH_EAX)
        || call_target(roll, ROLL_FN, call_at).map_err(|_| fail())? != SECONDARY_REAL_FN
    {
        return Err(fail());
    }
    Ok(usize::from(roll[ROLL_DRAW_AT + 1]))
}

/// Numerators of the 21 table stores. Each store divides `numerator` by the common denominator
/// (`0x9a02d0`) into one slot of the table; the slots are consecutive and the final lookup
/// (`mov ecx, [esi*4 + base]`) names the first. The last numerator equals the denominator, so
/// the last step is one.
pub(super) fn step_numerators(chance: &[u8]) -> Result<(Vec<u32>, u32), String> {
    let fail = |why: &str| format!("unsupported spell orb chance function: {why}");
    // The lookup ends the window.
    let lookup_at = chance
        .len()
        .checked_sub(LOOKUP.len() + 4)
        .ok_or_else(|| fail("truncated"))?;
    if chance[lookup_at..lookup_at + 3] != LOOKUP {
        return Err(fail("lookup"));
    }
    let base = le32(chance, lookup_at + 3)?;
    // The clamp of the step index is the table length minus one.
    let clamp_at = chance
        .windows(5)
        .position(|w| w[0] == MOV_EAX_IMM32_OPCODE && le32(w, 1) == Ok((STEPS - 1) as u32))
        .ok_or_else(|| fail("step clamp"))?;
    let mut denominator = None;
    let mut from = clamp_at;
    let mut numerators = Vec::with_capacity(STEPS);
    for step in 0..STEPS {
        let mut marker = vec![PUSH_IMM32];
        marker.extend((base + 4 * step as u32).to_le_bytes());
        let at = chance[from..]
            .windows(marker.len())
            .position(|w| w == marker.as_slice())
            .map(|p| p + from)
            .ok_or_else(|| fail("table store"))?;
        numerators
            .push(numerator_before(chance, clamp_at, at, &mut denominator).map_err(|e| fail(&e))?);
        from = at + marker.len();
    }
    let denominator = denominator.ok_or_else(|| fail("denominator"))?;
    if numerators.last() != Some(&denominator) || numerators.windows(2).any(|w| w[0] > w[1]) {
        return Err(fail("steps do not rise to one"));
    }
    Ok((numerators, denominator))
}

/// The numerator of the store whose slot push is at `at`: the immediate converted just before the
/// division. The same conversion of the denominator opens each store; the first one found names it.
fn numerator_before(
    chance: &[u8],
    start: usize,
    at: usize,
    denominator: &mut Option<u32>,
) -> Result<u32, String> {
    // push eax; push slot; ... call convert; add esp, 8; push eax; push slot
    let shape = at >= 10
        && chance[at - 1] == PUSH_EAX
        && chance[at - 4..at - 1] == ADD_ESP_8
        && chance[at - 9] == CALL_REL32
        && chance[at - 10] == PUSH_EAX;
    if !shape {
        return Err("store shape".into());
    }
    let mut end = at - 10;
    if end >= 3 && chance[end - 3..end - 1] == LEA_EAX_EBP {
        end -= 3;
    }
    let numerator = immediate_push(chance, end)?;
    // The denominator conversion: the nearest earlier `push imm32; push eax; call` pair.
    if denominator.is_none() {
        let first = chance[start..at]
            .windows(7)
            .position(|w| w[0] == PUSH_IMM32 && w[5] == PUSH_EAX && w[6] == CALL_REL32)
            .ok_or("denominator conversion")?;
        *denominator = Some(le32(chance, start + first + 1)?);
    }
    Ok(numerator)
}

/// The immediate of the push that ends at `end` (`push imm8` or `push imm32`).
fn immediate_push(code: &[u8], end: usize) -> Result<u32, String> {
    if end >= 2 && code[end - 2] == PUSH_IMM8 {
        let value = code[end - 1];
        if value > 0x7f {
            return Err("negative immediate".into());
        }
        return Ok(u32::from(value));
    }
    if end >= 5 && code[end - 5] == PUSH_IMM32 {
        return le32(code, end - 4);
    }
    Err("immediate push".into())
}

/// The table's word for a numerator: `numerator / denominator` by the reviewed divide.
pub(super) fn step_bits(
    numerator: u32,
    denominator: u32,
    reciprocal: &[u32],
) -> Result<u32, String> {
    let convert = |n: u32| -> Result<u32, String> {
        let n = i32::try_from(n).map_err(|_| "step out of range".to_string())?;
        real::from_i32(n).map_err(|e| e.to_string())
    };
    real::divide(convert(numerator)?, convert(denominator)?, reciprocal).map_err(|e| e.to_string())
}

impl Rules {
    /// Attaches the spell orb class. Its registration names the orb base; the proc stream, the
    /// increment table and the real constants come from the image windows, and the wrapped Black
    /// Arrow class from its own registration.
    pub(crate) fn with_spell_orb(mut self, image: &SpellOrbImage) -> Result<Self, String> {
        let (class, base) = class_from_window(&image.window)?;
        let orbs = self
            .orbs
            .as_ref()
            .ok_or("the spell orb registration needs the orb base")?;
        if base != orbs.base {
            return Err("spell orb registration must name the orb base class".into());
        }
        if orbs.spell.is_some() || self.is_registered(&class) {
            return Err("spell orb registration must name a distinct class".into());
        }
        let (black_arrow, _) = class_from_call_window_at(
            &image.black_arrow_window,
            BLACK_ARROW_WINDOW,
            &image.black_arrow_base,
        )?;
        if black_arrow == class || self.is_registered(&black_arrow) {
            return Err("the wrapped Black Arrow class must be distinct".into());
        }
        let channel = roll_channel(&image.roll)?;
        let reciprocal: Vec<u32> = image
            .reciprocal
            .as_chunks::<4>()
            .0
            .iter()
            .map(|chunk| u32::from_le_bytes(*chunk))
            .collect();
        if reciprocal.len() != RECIPROCAL_ENTRIES {
            return Err("unsupported reciprocal table".into());
        }
        let (numerators, denominator) = step_numerators(&image.chance)?;
        let steps = numerators
            .iter()
            .map(|n| step_bits(*n, denominator, &reciprocal))
            .collect::<Result<Vec<_>, _>>()?;
        let orbs = self.orbs.as_mut().expect("checked above");
        orbs.spell = Some(SpellClass {
            class,
            black_arrow,
            channel,
            steps,
            scale: u32::from_le_bytes(image.scale),
            round: u32::from_le_bytes(image.round),
        });
        Ok(self)
    }

    /// One spell orb row at the effect level: the chances, the proc increments and the wrapped
    /// ability named by `UnitID1`.
    pub(super) fn spell_orb(&self, alias: &str) -> Result<SpellOrb, String> {
        let class = self
            .orbs
            .as_ref()
            .and_then(|orbs| orbs.spell.as_ref())
            .ok_or("the spell orb needs its registration")?;
        let ability = self.effect_row(alias, "orb")?;
        let mut increments = [0; KINDS];
        for (increment, column) in increments.iter_mut().zip(CHANCE_COLUMNS) {
            let chance = row_real(ability, alias, column)?;
            if !chance.is_finite() {
                return Err(format!("orb {alias} {column} is not finite"));
            }
            *increment = class
                .increment(chance)
                .map_err(|e| format!("orb {alias}: {e}"))?;
        }
        let wrapped = cell_text(ability, alias, "UnitID1")?;
        let wrapped = fourcc(alias, "UnitID1", &wrapped)?;
        let wrapped_alias = std::str::from_utf8(&wrapped).expect("alphanumeric");
        let code = self.ability_code(wrapped_alias)?;
        let purge_class = self
            .orbs
            .as_ref()
            .and_then(|orbs| orbs.purge.as_ref())
            .map(|purge| purge.purge.as_str());
        let ability = if code == class.black_arrow {
            WrappedAbility::BlackArrow(self.black_arrow(wrapped_alias)?)
        } else if purge_class == Some(code.as_str()) {
            let purge = self.purge(wrapped_alias)?;
            // The generic cast start spends the cost and sets the cooldown; neither is decoded.
            if purge.costs {
                return Err(format!(
                    "wrapped ability {wrapped_alias} Cost1 or Cool1 is not zero; the generic \
                     spell cost is not decoded"
                ));
            }
            WrappedAbility::Purge(purge)
        } else {
            WrappedAbility::Undecoded {
                alias: wrapped,
                code: fourcc(wrapped_alias, "code", &code)?,
            }
        };
        Ok(SpellOrb {
            channel: class.channel,
            increments,
            wrapped: ability,
        })
    }

    fn black_arrow(&self, alias: &str) -> Result<BlackArrow, String> {
        self.single_level(alias)?;
        let row = self.effect_row(alias, "wrapped")?;
        let real = |column: &str| row_real(row, alias, column);
        // The cast spends mana and sets a cooldown through the generic spell, neither decoded.
        for column in ["Cost1", "Cool1"] {
            if real(column)? != 0.0 {
                return Err(format!(
                    "wrapped ability {alias} {column} is not zero; the generic spell cost is not decoded"
                ));
            }
        }
        let duration = real("Dur1")?;
        let hero_duration = real("HeroDur1")?;
        let summon_duration = real("DataC1")?;
        let range = real("Rng1")?;
        let count = real("DataB1")?;
        if [duration, hero_duration, summon_duration, range, count]
            .iter()
            .any(|v| !v.is_finite() || *v < 0.0)
            || count.fract() != 0.0
            || count > f32::from(u16::MAX)
        {
            return Err(format!(
                "wrapped ability {alias} durations, range or count are negative or not whole"
            ));
        }
        let buffs = cell_text(row, alias, "BuffID1")?;
        let mut buffs = buffs.split(',').map(str::trim);
        let (Some(buff), Some(marker), None) = (buffs.next(), buffs.next(), buffs.next()) else {
            return Err(format!(
                "wrapped ability {alias} BuffID1 must list the mark and the summon buff"
            ));
        };
        Ok(BlackArrow {
            buff: fourcc(alias, "BuffID1", buff)?,
            marker: fourcc(alias, "BuffID1", marker)?,
            duration,
            hero_duration,
            targets: wrapped_targets(alias, &cell_text(row, alias, "targs1")?)?,
            summon: fourcc(alias, "UnitID1", &cell_text(row, alias, "UnitID1")?)?,
            count: count as i32,
            summon_duration,
            range,
        })
    }
}

impl SpellClass {
    /// Increment of the proc chance per counter step for a row percent: the table entry at
    /// `trunc(percent * scale + round)`, clamped to the table (`0xcaec30`).
    fn increment(&self, percent: f32) -> Result<u32, String> {
        let bits = real::from_f32(percent).map_err(|e| e.to_string())?;
        let scaled = real::mul(bits, self.scale).map_err(|e| e.to_string())?;
        let rounded = real::add(scaled, self.round).map_err(|e| e.to_string())?;
        let step = real::truncate(rounded)
            .map_err(|e| e.to_string())?
            .clamp(0, (STEPS - 1) as i32);
        Ok(self.steps[step as usize])
    }
}

pub(super) fn row_real(row: Object<'_>, alias: &str, column: &str) -> Result<f32, String> {
    let cell = row
        .cell(SlkId::AbilityData, column)
        .map_err(|e| e.to_string())?;
    match cell.typed(FieldKind::Real) {
        Ok(Value::Real(value)) => Ok(value as f32),
        // An absent number reads as zero, the SLK default.
        Ok(Value::Marker(_)) => Ok(0.0),
        _ => Err(format!("{column} of {alias} is not a real")),
    }
}

pub(super) fn cell_text(row: Object<'_>, _alias: &str, column: &str) -> Result<String, String> {
    Ok(row
        .cell(SlkId::AbilityData, column)
        .map_err(|e| e.to_string())?
        .text
        .trim()
        .to_owned())
}

pub(super) fn fourcc(alias: &str, column: &str, text: &str) -> Result<[u8; 4], String> {
    <[u8; 4]>::try_from(text.as_bytes())
        .ok()
        .filter(|code| code.iter().all(u8::is_ascii_alphanumeric))
        .ok_or_else(|| format!("{column} of {alias} is not a rawcode"))
}

/// Class and relationship tokens of a wrapped ability's `targs1`.
fn wrapped_targets(alias: &str, targs: &str) -> Result<WrappedTargets, String> {
    let mut targets = WrappedTargets::default();
    for token in targs.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        match token.to_ascii_lowercase().as_str() {
            "air" => targets.air = true,
            "ground" => targets.ground = true,
            "structure" => targets.structure = true,
            "enemy" => targets.enemy = true,
            "neutral" => targets.neutral = true,
            "friend" => targets.friend = true,
            "organic" => targets.organic = true,
            other => {
                return Err(format!(
                    "wrapped ability {alias} targs1 token {other:?} is not decoded"
                ));
            }
        }
    }
    if !(targets.air || targets.ground) || !(targets.enemy || targets.neutral || targets.friend) {
        return Err(format!(
            "wrapped ability {alias} targs1 needs a movement and a relationship token"
        ));
    }
    Ok(targets)
}

/// Target kind of a proc roll, in the order of the chance columns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProcKind {
    Unit = 0,
    Hero = 1,
    Summon = 2,
}

/// The per-kind miss counters of one attached spell orb. The chance of the next roll is the
/// counter times the row's increment, so it rises with each miss and falls back after a proc.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProcCounters([i32; KINDS]);

impl ProcCounters {
    /// The counters when the orb is attached to its holder.
    pub(crate) const fn attached() -> Self {
        Self([PROC_COUNTER_START; KINDS])
    }

    /// Chance of the next roll for `kind` with the row's `increment` (real bits).
    pub(crate) fn chance(&self, kind: ProcKind, increment: u32) -> Result<f32, real::MathError> {
        let counter = real::from_i32(self.0[kind as usize])?;
        real::mul(counter, increment).map(f32::from_bits)
    }

    /// Counter after a roll: a proc resets it, a miss raises it.
    pub(crate) fn rolled(&mut self, kind: ProcKind, procced: bool) {
        let counter = &mut self.0[kind as usize];
        *counter = if procced {
            PROC_COUNTER_AFTER_HIT
        } else {
            counter.saturating_add(PROC_COUNTER_STEP)
        };
    }
}

/// Synthetic image bytes with the shapes read above, for tests. The numerators are the
/// fixture's own; nothing here is a client value.
#[cfg(test)]
pub(crate) mod fixture {
    use super::*;

    /// Real constants of the synthetic chance function.
    pub(crate) const SCALE: f32 = 0.2;
    pub(crate) const ROUND: f32 = 0.5;
    /// Rising numerators over a denominator of 10000, ending at one.
    pub(crate) const NUMERATORS: [u32; STEPS] = [
        0, 40, 150, 320, 550, 820, 1130, 1460, 1810, 2180, 2560, 2950, 3330, 3780, 4200, 4600,
        5000, 5800, 6670, 7700, 10000,
    ];
    pub(crate) const DENOMINATOR: u32 = 10_000;
    const SLOTS: u32 = 0x4000;

    /// `table[i]` for the reciprocal of `1 + i / 1024`, in the layout the reviewed routine reads.
    pub(crate) fn reciprocal_table() -> Vec<u8> {
        (0..RECIPROCAL_ENTRIES)
            .flat_map(|i| {
                let value = 2.0 / (1.0 + i as f64 / 1024.0) - 1.0;
                ((value * f64::from(1_u32 << 23)).round() as u32).to_le_bytes()
            })
            .collect()
    }

    fn push(code: &mut Vec<u8>, value: u32) {
        if value < 0x80 {
            code.extend([PUSH_IMM8, value as u8]);
        } else {
            code.push(PUSH_IMM32);
            code.extend(value.to_le_bytes());
        }
    }

    fn call(code: &mut Vec<u8>) {
        code.push(CALL_REL32);
        code.extend([0; 4]);
    }

    /// The chance function: constants, clamp, 21 table stores in both operand orders, lookup.
    fn chance(numerators: &[u32; STEPS]) -> Vec<u8> {
        let mut code = vec![0x90; 0x26];
        code.push(PUSH_IMM32);
        code.extend(0x10d_b0a8_u32.to_le_bytes());
        code.push(PUSH_IMM32);
        code.extend(0x10d_b088_u32.to_le_bytes());
        code.resize(0x5c, 0x90);
        code.push(MOV_EAX_IMM32_OPCODE);
        code.extend(((STEPS - 1) as u32).to_le_bytes());
        code.resize(0xa0, 0x90);
        for (step, numerator) in numerators.iter().enumerate() {
            // Denominator conversion: lea, push imm, push eax, call, cleanup, push eax.
            code.extend([0x8d, 0x45, 0xec]);
            push(&mut code, DENOMINATOR);
            code.push(PUSH_EAX);
            call(&mut code);
            code.extend(ADD_ESP_8);
            code.push(PUSH_EAX);
            // Numerator conversion in the two operand orders the client uses.
            if step % 2 == 0 {
                code.extend([0x8d, 0x45, 0xf0]);
                push(&mut code, *numerator);
                code.push(PUSH_EAX);
            } else {
                push(&mut code, *numerator);
                code.extend([0x8d, 0x45, 0xf0]);
                code.push(PUSH_EAX);
            }
            call(&mut code);
            code.extend(ADD_ESP_8);
            code.push(PUSH_EAX);
            code.push(PUSH_IMM32);
            code.extend((SLOTS + 4 * step as u32).to_le_bytes());
            call(&mut code);
        }
        code.extend(LOOKUP);
        code.extend(SLOTS.to_le_bytes());
        code
    }

    pub(crate) fn registration(source: &str, base: &str) -> Vec<u8> {
        let mut w = vec![PUSH_IMM32, 1, 2, 3, 4, PUSH_IMM32];
        w.extend(base.bytes().rev());
        w.push(PUSH_IMM32);
        w.extend(source.bytes().rev());
        w.extend([CALL_REL32, 0, 0, 0, 0]);
        w.extend(CLEANUP_RET);
        w
    }

    /// The computed-base registration at `BLACK_ARROW_WINDOW` and its base function.
    fn computed(source: &str) -> Vec<u8> {
        computed_at(source, BLACK_ARROW_WINDOW)
    }

    /// The computed-base registration at `window`.
    pub(crate) fn computed_at(source: &str, window: u32) -> Vec<u8> {
        let rel_base: i32 = 0x100 - 10;
        let rel_register = (i64::from(REGISTER_FN) - (i64::from(window) + 21)) as i32;
        let mut w = vec![PUSH_IMM32, 1, 2, 3, 4, CALL_REL32];
        w.extend(rel_base.to_le_bytes());
        w.push(PUSH_EAX);
        w.push(PUSH_IMM32);
        w.extend(source.bytes().rev());
        w.push(CALL_REL32);
        w.extend(rel_register.to_le_bytes());
        w.extend(CLEANUP_RET);
        w
    }

    pub(crate) fn roll(channel: u8) -> Vec<u8> {
        let mut roll = vec![0x90; ROLL_LEN];
        let call_at = ROLL_DRAW_AT + 3;
        roll[ROLL_DRAW_AT] = PUSH_IMM8;
        roll[ROLL_DRAW_AT + 1] = channel;
        roll[ROLL_DRAW_AT + 2] = PUSH_EAX;
        roll[call_at] = CALL_REL32;
        let rel = i64::from(SECONDARY_REAL_FN) - (i64::from(ROLL_FN) + call_at as i64 + 5);
        roll[call_at + 1..call_at + 5].copy_from_slice(&(rel as i32).to_le_bytes());
        roll
    }

    /// An image with the spell orb class `AIsb` over `AIDB` and the wrapped class `ANbs`.
    pub(crate) fn image(channel: u8) -> SpellOrbImage {
        image_with(channel, &NUMERATORS)
    }

    pub(crate) fn image_with(channel: u8, numerators: &[u32; STEPS]) -> SpellOrbImage {
        let mut base = vec![MOV_EAX_IMM32];
        base.extend(b"AAsm".iter().rev());
        base.push(RET);
        SpellOrbImage {
            window: registration("AIsb", "AIDB"),
            black_arrow_window: computed("ANbs"),
            black_arrow_base: base,
            roll: roll(channel),
            chance: chance(numerators),
            scale: SCALE.to_le_bytes(),
            round: ROUND.to_le_bytes(),
            reciprocal: reciprocal_table(),
        }
    }
}
