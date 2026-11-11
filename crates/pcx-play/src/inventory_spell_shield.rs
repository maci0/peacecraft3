//! The Amulet of Spell Shield ability class (`ANss`, RTTI `CAbilitySpellShield`). The
//! registration comes from the reviewed image and the cooldown from the ability row; the decode
//! is in `docs/re-notes/passive-held-items.md`, "`ANss`".
//!
//! A held `ANss` is read and retained on the holder. The client consults it when an enemy's
//! targeted spell reaches the holder, which the recreation reports where it hosts such a spell
//! (`spell_shield_host.rs`) instead of deciding it.
use super::*;
use spell_orb::row_real;

/// Registration call of the class (source `ANss`, computed base `AAsm`).
pub(super) const SPELL_SHIELD_WINDOW: u32 = 0xc71d20;
/// The ability row column of the cooldown (`Cool1`), the number the item tooltip prints as
/// `<ANss,Cool1>`.
const COOLDOWN_COLUMN: &str = "Cool1";

/// Bytes of the reviewed image the spell shield rules are read from.
#[derive(Clone, Debug)]
pub(crate) struct SpellShieldImage {
    pub(crate) window: Vec<u8>,
    pub(crate) base: Vec<u8>,
}

impl SpellShieldImage {
    pub(crate) fn read(pe: &Pe<'_>) -> Result<Self, String> {
        let bytes = |va: u32, len: usize| -> Result<Vec<u8>, String> {
            pe.read_from(va, true)
                .map_err(|e| e.to_string())?
                .get(..len)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| format!("truncated image bytes at {va:#x}"))
        };
        let window = bytes(SPELL_SHIELD_WINDOW, HEAL_WINDOW_LEN)?;
        let base_va = call_target(&window, SPELL_SHIELD_WINDOW, 5)?;
        Ok(Self {
            base: bytes(base_va, BASE_FN_LEN)?,
            window,
        })
    }
}

/// The registered class.
#[derive(Clone, Debug)]
pub(super) struct SpellShieldClass {
    pub(super) class: String,
}

/// One `ANss` row at the effect level.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpellShield {
    /// `Cool1`: seconds before the shield blocks again, per the item text.
    pub cooldown: f32,
}

impl Rules {
    /// Attaches the spell shield class, which must be distinct from every registered class.
    pub(crate) fn with_spell_shield(mut self, image: &SpellShieldImage) -> Result<Self, String> {
        let (class, _) =
            class_from_call_window_at(&image.window, SPELL_SHIELD_WINDOW, &image.base)?;
        if self.spell_shield.is_some() || self.is_registered(&class) {
            return Err("the spell shield registration must name a distinct class".into());
        }
        self.spell_shield = Some(SpellShieldClass { class });
        Ok(self)
    }

    /// One `ANss` row.
    pub(super) fn spell_shield_row(&self, alias: &str) -> Result<SpellShield, String> {
        self.single_level(alias)?;
        let row = self.effect_row(alias, "spell shield")?;
        let cooldown = row_real(row, alias, COOLDOWN_COLUMN)?;
        if !cooldown.is_finite() || cooldown < 0.0 {
            return Err(format!(
                "spell shield {alias} {COOLDOWN_COLUMN} is not a number or is negative"
            ));
        }
        Ok(SpellShield { cooldown })
    }
}

/// Synthetic image bytes with the shapes read above, for tests. Nothing here is a client value.
#[cfg(test)]
pub(crate) mod fixture {
    use super::spell_orb::fixture::computed_at;
    use super::*;

    /// An image with `ANss` over `Base`.
    pub(crate) fn image() -> SpellShieldImage {
        let mut base = vec![MOV_EAX_IMM32];
        base.extend(b"Base".iter().rev());
        base.push(RET);
        SpellShieldImage {
            window: computed_at("ANss", SPELL_SHIELD_WINDOW),
            base,
        }
    }
}
