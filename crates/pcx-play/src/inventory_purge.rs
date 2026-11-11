//! Purge (`Aprg`) as an orb casts it: the lightning orb `AIlb` through its sibling `AIlp`, and the
//! spell orb `AIsb` through the wrapped `AIpg`. The registrations come from the reviewed image
//! and the row values from the ability table; the decode is in
//! `docs/re-notes/passive-held-items.md`, "Purge".
//!
//! The cast dispels the target, may hit a summoned enemy, and slows an enemy: buff `Bprg`
//! removes `1.0` from the movement multiplier and gives it back in `DataA` equal steps over the
//! buff duration. The rows' `DataA` becomes the step and the step interval here, through the
//! client's own divide, so the arithmetic matches the reviewed routine.
use super::*;
use crate::script_random::real;
use spell_orb::{cell_text, fourcc, row_real};

/// Registration call of Purge (source `Aprg`, computed base `AAsm`).
pub(super) const PURGE_WINDOW: u32 = 0xba1310;
/// Registration call of the lightning orb (source `AIlb`, base `AIDB`).
pub(super) const LIGHTNING_ORB_WINDOW: u32 = 0xca6da0;
/// Registration call of the lightning purge (source `AIlp`, computed base `Aprg`).
pub(super) const LIGHTNING_PURGE_WINDOW: u32 = 0xca6dc0;
/// The initialized float the buff apply adds to the duration (`0x10db058`, `0xb9d5f0`): the
/// same global the poison buff reads as its first tick delay.
pub(super) const DURATION_SLACK_VA: u32 = 0x10db058;
/// The movement multiplier the buff removes when it applies, `[0x112d810]` (`0xb9d71f`).
pub(crate) const FULL_SLOW: f32 = 1.0;

/// Bytes of the reviewed image the Purge rules are read from.
#[derive(Clone, Debug)]
pub(crate) struct PurgeImage {
    pub(crate) purge_window: Vec<u8>,
    pub(crate) purge_base: Vec<u8>,
    pub(crate) orb_window: Vec<u8>,
    pub(crate) sibling_window: Vec<u8>,
    pub(crate) sibling_base: Vec<u8>,
    pub(crate) slack: [u8; 4],
    pub(crate) reciprocal: Vec<u8>,
}

impl PurgeImage {
    pub(crate) fn read(pe: &Pe<'_>) -> Result<Self, String> {
        let bytes = |va: u32, len: usize, code: bool| -> Result<Vec<u8>, String> {
            pe.read_from(va, code)
                .map_err(|e| e.to_string())?
                .get(..len)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| format!("truncated image bytes at {va:#x}"))
        };
        let purge_window = bytes(PURGE_WINDOW, HEAL_WINDOW_LEN, true)?;
        let sibling_window = bytes(LIGHTNING_PURGE_WINDOW, HEAL_WINDOW_LEN, true)?;
        let purge_base_va = call_target(&purge_window, PURGE_WINDOW, 5)?;
        let sibling_base_va = call_target(&sibling_window, LIGHTNING_PURGE_WINDOW, 5)?;
        Ok(Self {
            purge_base: bytes(purge_base_va, BASE_FN_LEN, true)?,
            sibling_base: bytes(sibling_base_va, BASE_FN_LEN, true)?,
            orb_window: bytes(LIGHTNING_ORB_WINDOW, CLASS_WINDOW_LEN, true)?,
            slack: bytes(DURATION_SLACK_VA, 4, false)?
                .try_into()
                .expect("four bytes"),
            reciprocal: bytes(
                spell_orb::RECIPROCAL_TABLE,
                spell_orb::RECIPROCAL_ENTRIES * 4,
                false,
            )?,
            purge_window,
            sibling_window,
        })
    }
}

/// The registered Purge classes: Purge itself, the lightning orb and its Purge sibling, and the
/// image values the buff arithmetic reads.
#[derive(Clone, Debug)]
pub(super) struct PurgeClasses {
    pub(super) purge: String,
    pub(super) orb: String,
    pub(super) sibling: String,
    slack: f32,
    reciprocal: Vec<u32>,
}

/// Which units a Purge cast may affect: the class tokens of its `targs1`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PurgeTargets {
    pub air: bool,
    pub ground: bool,
    pub structure: bool,
}

/// The movement slow of the buff: it removes [`FULL_SLOW`] and gives back `step` every
/// `interval` seconds (`hero_interval` on a hero), `DataA` times in all.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SlowDecay {
    pub step: f32,
    pub interval: f32,
    pub hero_interval: f32,
}

/// One Purge ability row at the effect level.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Purge {
    pub buff: [u8; 4],
    /// `Dur1` plus the image's duration slack; the buff ends this long after the cast.
    pub duration: f32,
    /// `HeroDur1` plus the same slack.
    pub hero_duration: f32,
    /// `None` when `DataA` is zero: the buff then carries no slow.
    pub slow: Option<SlowDecay>,
    /// `DataC`: damage to a summoned enemy.
    pub summoned_damage: f32,
    /// `DataD` and `DataE`: seconds a non-hero and a hero cannot move.
    pub unit_pause: f32,
    pub hero_pause: f32,
    pub targets: PurgeTargets,
    /// `Rng1`: the range the spell orb's cast test (not decoded) would hold the target to.
    pub range: f32,
    /// `Cost1` or `Cool1` is set: a spell orb's generic cast start spends them (not decoded).
    pub costs: bool,
}

impl Rules {
    /// Attaches the lightning orb, Purge and its lightning sibling. The orb registers over the
    /// orb base and the sibling over Purge; the reciprocal table, the duration slack and the
    /// registrations come from the image.
    pub(crate) fn with_purge(mut self, image: &PurgeImage) -> Result<Self, String> {
        let (purge, _) =
            class_from_call_window_at(&image.purge_window, PURGE_WINDOW, &image.purge_base)?;
        let (sibling, sibling_base) = class_from_call_window_at(
            &image.sibling_window,
            LIGHTNING_PURGE_WINDOW,
            &image.sibling_base,
        )?;
        let (orb, orb_base) = class_from_window(&image.orb_window)?;
        let orbs = self
            .orbs
            .as_ref()
            .ok_or("the lightning orb registration needs the orb base")?;
        if orb_base != orbs.base {
            return Err("lightning orb registration must name the orb base class".into());
        }
        if sibling_base != purge {
            return Err("lightning purge registration must name the Purge class".into());
        }
        let distinct = [&purge, &orb, &sibling];
        if orbs.purge.is_some()
            || distinct.iter().any(|class| self.is_registered(class))
            || orb == purge
            || orb == sibling
            || purge == sibling
        {
            return Err("Purge registrations must name distinct classes".into());
        }
        let slack = f32::from_le_bytes(image.slack);
        if !slack.is_finite() || slack < 0.0 {
            return Err("the Purge duration slack is not a nonnegative real".into());
        }
        let reciprocal: Vec<u32> = image
            .reciprocal
            .as_chunks::<4>()
            .0
            .iter()
            .map(|chunk| u32::from_le_bytes(*chunk))
            .collect();
        if reciprocal.len() != spell_orb::RECIPROCAL_ENTRIES {
            return Err("unsupported reciprocal table".into());
        }
        let orbs = self.orbs.as_mut().expect("checked above");
        orbs.purge = Some(PurgeClasses {
            purge,
            orb,
            sibling,
            slack,
            reciprocal,
        });
        Ok(self)
    }

    /// One Purge row. `DataB` (the attack-speed slow) and `DataF` (mana loss) must be zero: their
    /// hosts are not decoded. A negative frequency or duration refuses.
    pub(super) fn purge(&self, alias: &str) -> Result<Purge, String> {
        let class = self
            .orbs
            .as_ref()
            .and_then(|orbs| orbs.purge.as_ref())
            .ok_or("Purge needs its registration")?;
        self.single_level(alias)?;
        let row = self.effect_row(alias, "Purge")?;
        let real = |column: &str| row_real(row, alias, column);
        for (column, why) in [
            ("DataB1", "the attack speed slow"),
            ("DataF1", "the mana loss"),
        ] {
            if real(column)? != 0.0 {
                return Err(format!(
                    "Purge {alias} {column} is not zero; {why} is not hosted"
                ));
            }
        }
        let (duration, hero_duration) = (real("Dur1")?, real("HeroDur1")?);
        let frequency = real("DataA1")?;
        let summoned_damage = real("DataC1")?;
        let unit_pause = real("DataD1")?;
        let hero_pause = real("DataE1")?;
        let range = real("Rng1")?;
        if [
            duration,
            hero_duration,
            frequency,
            summoned_damage,
            unit_pause,
            hero_pause,
            range,
        ]
        .iter()
        .any(|v| !v.is_finite() || *v < 0.0)
        {
            return Err(format!(
                "Purge {alias} durations, frequency, damage, pauses or range are negative or not numbers"
            ));
        }
        if frequency != 0.0 && (duration <= 0.0 || hero_duration <= 0.0) {
            return Err(format!(
                "Purge {alias} duration is not positive; a repeating timer of no length is not decoded"
            ));
        }
        let slow = if frequency == 0.0 {
            None
        } else {
            let bits = |value: f32| real::from_f32(value).map_err(|e| e.to_string());
            let divide = |numerator: f32, denominator: u32| {
                real::divide(bits(numerator)?, denominator, &class.reciprocal)
                    .map(f32::from_bits)
                    .map_err(|e| e.to_string())
            };
            let frequency = bits(frequency)?;
            Some(SlowDecay {
                step: divide(FULL_SLOW, frequency)?,
                interval: divide(duration, frequency)?,
                hero_interval: divide(hero_duration, frequency)?,
            })
        };
        let buffs = row
            .cell(SlkId::AbilityData, "BuffID1")
            .map_err(|e| e.to_string())?
            .text
            .trim()
            .to_owned();
        Ok(Purge {
            buff: fourcc(alias, "BuffID1", &buffs)?,
            duration: duration + class.slack,
            hero_duration: hero_duration + class.slack,
            slow,
            summoned_damage,
            unit_pause,
            hero_pause,
            targets: purge_targets(alias, &cell_text(row, alias, "targs1")?)?,
            range,
            costs: real("Cost1")? != 0.0 || real("Cool1")? != 0.0,
        })
    }

    /// The Purge sibling an `AIlb` orb casts: the one `AIlp` ability on the same item.
    pub(super) fn lightning_purge(
        &self,
        orb: &str,
        abilities: &str,
    ) -> Result<(Purge, OrbTargets), String> {
        let class = self
            .orbs
            .as_ref()
            .and_then(|orbs| orbs.purge.as_ref())
            .ok_or_else(|| format!("orb {orb} needs the Purge registration"))?;
        let siblings = self.aliases_of_class(abilities, &class.sibling)?;
        let [sibling] = siblings.as_slice() else {
            return Err(format!(
                "orb {orb} needs exactly one {} ability on its item",
                class.sibling
            ));
        };
        let purge = self.purge(sibling)?;
        let targets = OrbTargets {
            ground: purge.targets.ground,
            air: purge.targets.air,
            structure: purge.targets.structure,
            organic: false,
        };
        Ok((purge, targets))
    }
}

/// Class tokens of a Purge row's `targs1`. Tokens that name wards, trees, destructables or
/// protection stances have no effect on the unit targets an attack reaches; relationship tokens
/// are refused.
fn purge_targets(alias: &str, targs: &str) -> Result<PurgeTargets, String> {
    let mut targets = PurgeTargets::default();
    for token in targs.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        match token.to_ascii_lowercase().as_str() {
            "ground" => targets.ground = true,
            "air" => targets.air = true,
            "structure" => targets.structure = true,
            "ward" | "vuln" | "invu" | "tree" | "debris" | "item" | "wall" | "decoration"
            | "bridge" => {}
            other => {
                return Err(format!(
                    "Purge {alias} targs1 token {other:?} is not decoded"
                ));
            }
        }
    }
    if !(targets.air || targets.ground) {
        return Err(format!("Purge {alias} targs1 needs a movement token"));
    }
    Ok(targets)
}

/// Synthetic image bytes with the shapes read above, for tests. The slack is the fixture's own;
/// nothing here is a client value.
#[cfg(test)]
pub(crate) mod fixture {
    use super::spell_orb::fixture::{computed_at, reciprocal_table, registration};
    use super::*;

    /// The duration slack of the synthetic image.
    pub(crate) const SLACK: f32 = 0.01;

    fn base(name: &str) -> Vec<u8> {
        let mut code = vec![MOV_EAX_IMM32];
        code.extend(name.bytes().rev());
        code.push(RET);
        code
    }

    /// An image with `Aprg` over `AAsm`, `AIlp` over `Aprg` and `AIlb` over `AIDB`.
    pub(crate) fn image() -> PurgeImage {
        PurgeImage {
            purge_window: computed_at("Aprg", PURGE_WINDOW),
            purge_base: base("AAsm"),
            orb_window: registration("AIlb", "AIDB"),
            sibling_window: computed_at("AIlp", LIGHTNING_PURGE_WINDOW),
            sibling_base: base("Aprg"),
            slack: SLACK.to_le_bytes(),
            reciprocal: reciprocal_table(),
        }
    }
}
