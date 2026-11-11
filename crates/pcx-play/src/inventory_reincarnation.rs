//! The Ankh of Reincarnation ability (`AIrc`) and the hero reincarnation classes it ranks beside
//! (`AOre`, `ACrn`). The registrations come from the reviewed image and the row values from the
//! ability table; the decode is in `docs/re-notes/passive-held-items.md`, "`AIrc`".
//!
//! A held `AIrc` is a passive death handler: when its holder dies, the item spends a charge and a
//! revive record brings the unit back `DataA` seconds later at the place it died, with `DataB` life
//! and `DataC` mana (a negative `DataC` keeps the mana the unit died with). A death handler runs in
//! the stage its slot `+0x280` returns, `AOre` in 2 (`0xb8b2d0`) and `AIrc` in 3 (`0xb8b860`).
use super::*;
use spell_orb::{cell_text, fourcc, row_real};

/// Registration call of the item class (source `AIrc`, computed base `abil`).
pub(super) const ANKH_WINDOW: u32 = 0xb8c1e0;
/// Registration call of the hero class (source `AOre`, computed base `AAsp`).
pub(super) const HERO_WINDOW: u32 = 0xb8c200;
/// Registration call of the creep class (source `ACrn`, base `AOre`).
pub(super) const CREEP_WINDOW: u32 = 0xb8c1c0;
/// The life a revive restores is at least this much (`0x69bf44`, the global `0x10db0a8`), and a
/// life within [`LIFE_UNSET`] of zero means the maximum (`0x69bf17`, `0x10db054`).
pub(super) const MIN_LIFE: f32 = 0.5;
pub(super) const LIFE_UNSET: f32 = 0.001;

/// Bytes of the reviewed image the reincarnation rules are read from.
#[derive(Clone, Debug)]
pub(crate) struct ReincarnationImage {
    pub(crate) ankh_window: Vec<u8>,
    pub(crate) ankh_base: Vec<u8>,
    pub(crate) hero_window: Vec<u8>,
    pub(crate) hero_base: Vec<u8>,
    pub(crate) creep_window: Vec<u8>,
}

impl ReincarnationImage {
    pub(crate) fn read(pe: &Pe<'_>) -> Result<Self, String> {
        let bytes = |va: u32, len: usize| -> Result<Vec<u8>, String> {
            pe.read_from(va, true)
                .map_err(|e| e.to_string())?
                .get(..len)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| format!("truncated image bytes at {va:#x}"))
        };
        let ankh_window = bytes(ANKH_WINDOW, HEAL_WINDOW_LEN)?;
        let hero_window = bytes(HERO_WINDOW, HEAL_WINDOW_LEN)?;
        let ankh_base_va = call_target(&ankh_window, ANKH_WINDOW, 5)?;
        let hero_base_va = call_target(&hero_window, HERO_WINDOW, 5)?;
        Ok(Self {
            ankh_base: bytes(ankh_base_va, BASE_FN_LEN)?,
            hero_base: bytes(hero_base_va, BASE_FN_LEN)?,
            creep_window: bytes(CREEP_WINDOW, CLASS_WINDOW_LEN)?,
            ankh_window,
            hero_window,
        })
    }
}

/// The registered classes: the item ability and the two classes whose death handler runs a
/// stage earlier.
#[derive(Clone, Debug)]
pub(super) struct ReincarnationClasses {
    pub(super) ankh: String,
    pub(super) hero: String,
    pub(super) creep: String,
}

/// One `AIrc` row at the effect level.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reincarnation {
    /// `DataA1`: seconds from the death to the revive.
    pub delay: f32,
    /// `DataB1` raised to zero at least: the life restored, or the maximum for zero.
    pub life: f32,
    /// `DataC1`: the mana restored, or the mana the unit died with when it is negative.
    pub mana: f32,
}

impl Reincarnation {
    /// The life the revive sets (`0x69bf17`..`0x69bf64`): the maximum for a value within
    /// [`LIFE_UNSET`] of zero, else the value raised to [`MIN_LIFE`], and the unit's stat clamps
    /// it to the maximum.
    pub fn restored_life(&self, max_life: f32) -> f32 {
        if self.life.abs() < LIFE_UNSET {
            max_life
        } else {
            self.life.max(MIN_LIFE).min(max_life)
        }
    }

    /// The mana the revive sets: `DataC1`, or `at_death` when `DataC1` is negative
    /// (`0xb8b9db`, `0xb20d20`), clamped to the unit's range.
    pub fn restored_mana(&self, at_death: f32, max_mana: f32) -> f32 {
        let mana = if self.mana < 0.0 { at_death } else { self.mana };
        mana.clamp(0.0, max_mana)
    }
}

impl Rules {
    /// Attaches the reincarnation classes. All three must be distinct from every registered
    /// class, `ACrn` must register over `AOre`, and `AIrc` is read over the generic ability base.
    pub(crate) fn with_reincarnation(mut self, image: &ReincarnationImage) -> Result<Self, String> {
        let (ankh, _) =
            class_from_call_window_at(&image.ankh_window, ANKH_WINDOW, &image.ankh_base)?;
        let (hero, _) =
            class_from_call_window_at(&image.hero_window, HERO_WINDOW, &image.hero_base)?;
        let (creep, creep_base) = class_from_window(&image.creep_window)?;
        if creep_base != hero {
            return Err("the creep reincarnation registration must name the hero class".into());
        }
        let classes = [&ankh, &hero, &creep];
        if self.reincarnation.is_some()
            || classes.iter().any(|class| self.is_registered(class))
            || ankh == hero
            || ankh == creep
            || hero == creep
        {
            return Err("reincarnation registrations must name distinct classes".into());
        }
        self.reincarnation = Some(ReincarnationClasses { ankh, hero, creep });
        Ok(self)
    }

    /// Whether `class` is one of the hero reincarnation classes, whose death handler runs a stage
    /// before the item's.
    pub fn is_hero_reincarnation(&self, class: &str) -> bool {
        self.reincarnation
            .as_ref()
            .is_some_and(|classes| classes.hero == class || classes.creep == class)
    }

    /// One `AIrc` row. The effect id is read as a rawcode and not used: the client creates a
    /// special effect from it that the recreation does not present.
    pub(super) fn reincarnation_row(&self, alias: &str) -> Result<Reincarnation, String> {
        self.single_level(alias)?;
        let row = self.effect_row(alias, "reincarnation")?;
        let real = |column: &str| row_real(row, alias, column);
        let (delay, life, mana) = (real("DataA1")?, real("DataB1")?, real("DataC1")?);
        if !(delay.is_finite() && life.is_finite() && mana.is_finite()) || delay < 0.0 {
            return Err(format!(
                "reincarnation {alias} delay, life or mana is not a number or the delay is negative"
            ));
        }
        fourcc(alias, "EfctID1", &cell_text(row, alias, "EfctID1")?)?;
        Ok(Reincarnation {
            delay,
            life: life.max(0.0),
            mana,
        })
    }
}

/// Synthetic image bytes with the shapes read above, for tests. Nothing here is a client value.
#[cfg(test)]
pub(crate) mod fixture {
    use super::spell_orb::fixture::{computed_at, registration};
    use super::*;

    fn base(name: &str) -> Vec<u8> {
        let mut code = vec![MOV_EAX_IMM32];
        code.extend(name.bytes().rev());
        code.push(RET);
        code
    }

    /// An image with `AIrc` over `abil`, `AOre` over `AAsp` and `ACrn` over `AOre`.
    pub(crate) fn image() -> ReincarnationImage {
        ReincarnationImage {
            ankh_window: computed_at("AIrc", ANKH_WINDOW),
            ankh_base: base("abil"),
            hero_window: computed_at("AOre", HERO_WINDOW),
            hero_base: base("AAsp"),
            creep_window: registration("ACrn", "AOre"),
        }
    }
}
