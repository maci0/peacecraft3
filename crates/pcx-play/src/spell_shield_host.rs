//! Where a held Spell Shield (`ANss`, `inventory_spell_shield.rs`) is consulted. The decode is in
//! `docs/re-notes/passive-held-items.md`, "`ANss`".
use super::ObjectHost;
use pcx_script::HandleId;

impl ObjectHost {
    /// Fails when `target` holds a Spell Shield that the client offers an enemy `caster`'s
    /// targeted `spell` to, before the spell takes effect. Whether the shield then blocks needs
    /// the casting ability's virtuals, which no hosted spell class has decoded, so the
    /// recreation neither blocks nor lets the spell through. Allies and holders without a
    /// shield are not affected: the client's enemy test (`0x696780`) fails first.
    pub(crate) fn refuse_spell_shield_consulted(
        &self,
        caster: HandleId,
        target: HandleId,
        spell: &str,
    ) -> Result<(), String> {
        let bonus = self.unit_hero_bonus(target).map_err(|e| e.to_string())?;
        let Some(shield) = bonus.spell_shield else {
            return Ok(());
        };
        if self.is_ally(target, caster) {
            return Ok(());
        }
        Err(format!(
            "{spell} on a unit holding a Spell Shield (ANss, cooldown {} s): the client offers an \
             enemy's targeted spell to the shield (0x6d4c90 calls virtual +0x180 at 0xc71310), which \
             blocks it by the casting ability's virtuals +0x1d8 and +0x1f8, not decoded for {spell}, \
             and starts its cooldown through virtual +0x380, not decoded",
            shield.cooldown
        ))
    }
}
