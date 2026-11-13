//! Whole-world ability state preserves live iteration; campaign-cache projection is separate.
use super::{HeroBudget, HeroProgress, Skill, UnitAbilities};
use pcx_script::checkpoint::{Reader, Writer};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

fn source_key(state: &UnitAbilities) -> Result<[u8; 32], String> {
    let mut hash = Sha256::new();
    hash.update([u8::from(state.hero.is_some())]);
    hash.update(
        u64::try_from(state.learnable.len())
            .map_err(|_| "ability slot count overflow")?
            .to_le_bytes(),
    );
    for Skill {
        alias,
        maximum,
        required,
        skip,
        dependency,
    } in &state.learnable
    {
        hash.update(alias);
        for value in [maximum, required, skip] {
            hash.update(value.to_le_bytes());
        }
        hash.update([u8::from(dependency.is_some())]);
        if let Some(dependency) = dependency {
            hash.update(
                u64::try_from(dependency.len())
                    .map_err(|_| "ability source length overflow")?
                    .to_le_bytes(),
            );
            hash.update(dependency.as_bytes());
        }
    }
    Ok(hash.finalize().into())
}
fn slot_bytes(skills: &[Skill]) -> Result<usize, String> {
    skills
        .iter()
        .try_fold(std::mem::size_of_val(skills), |size, skill| {
            size.checked_add(skill.dependency.as_ref().map_or(0, String::len))
                .ok_or("ability slot size overflow".into())
        })
}
fn aliases_out(out: &mut Writer, values: &[[u8; 4]]) -> Result<(), String> {
    out.list(values.len())?;
    out.owned(std::mem::size_of_val(values))?;
    for value in values {
        out.raw(value)?;
    }
    Ok(())
}
fn aliases_in(input: &mut Reader<'_>) -> Result<Vec<[u8; 4]>, String> {
    let count = input.list()?;
    input.owned(count.checked_mul(4).ok_or("ability list size overflow")?)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| "ability list allocation failed")?;
    for _ in 0..count {
        values.push(input.raw(4)?.try_into().expect("four ability bytes"));
    }
    Ok(values)
}
fn validate(
    state: &UnitAbilities,
    rules: &crate::hero_skills::Rules,
    budget: HeroBudget,
    mut alias: impl FnMut([u8; 4], Option<i32>) -> Result<(), String>,
) -> Result<(), String> {
    if let Some(hero) = &state.hero {
        if hero.level < rules.initial_level || hero.level > rules.progression.max_level {
            return Err("checkpoint hero level differs from its source range".into());
        }
        if let (Some(experience), Some(policy)) = (&hero.experience, &rules.experience)
            && policy
                .thresholds
                .level_for(rules.initial_level, experience.xp)?
                != hero.level
        {
            return Err("checkpoint hero experience and level differ".into());
        }
    }
    if state.levels.len() > budget.abilities_per_unit {
        return Err("checkpoint ability count exceeds candidate admission".into());
    }
    let mut seen = BTreeSet::new();
    for (ability, rank) in &state.levels {
        if !seen.insert(*ability) {
            return Err("duplicate checkpoint ability".into());
        }
        alias(*ability, Some(*rank))?;
    }
    seen.clear();
    for list in [
        &state.autocast_disabled,
        &state.autocast_enabled,
        &state.alarm_ignored,
    ] {
        if list.len() > budget.abilities_per_unit {
            return Err("checkpoint ability flags exceed candidate admission".into());
        }
        seen.clear();
        for ability in list {
            if !seen.insert(*ability) {
                return Err("duplicate checkpoint ability flag".into());
            }
            alias(*ability, None)?;
        }
    }
    if state
        .autocast_disabled
        .iter()
        .any(|ability| state.autocast_enabled.contains(ability))
    {
        return Err("checkpoint ability is both explicitly enabled and disabled".into());
    }
    if state
        .autocast_order
        .is_some_and(|(revision, _)| revision != state.autocast_revision)
    {
        return Err("checkpoint autocast order revision differs".into());
    }
    let mut cooldowns = BTreeSet::new();
    for (ability, rank, bits) in &state.cooldowns {
        if !cooldowns.insert((*ability, *rank)) {
            return Err("duplicate checkpoint cooldown rank".into());
        }
        let duration = f32::from_bits(*bits);
        if !duration.is_finite() || duration < 0. {
            return Err("checkpoint cooldown is not finite nonnegative".into());
        }
        alias(*ability, Some(*rank))?;
    }
    Ok(())
}

impl UnitAbilities {
    pub fn write_world_checkpoint(
        &self,
        out: &mut Writer,
        rules: &crate::hero_skills::Rules,
        budget: HeroBudget,
        alias: impl FnMut([u8; 4], Option<i32>) -> Result<(), String>,
    ) -> Result<(), String> {
        validate(self, rules, budget, alias)?;
        let Self {
            autocast_disabled,
            autocast_enabled,
            autocast_revision,
            autocast_order,
            alarm_ignored,
            hero,
            levels,
            learnable: _,
            cooldowns,
        } = self;
        out.items(self.learnable.len())?;
        out.owned(slot_bytes(&self.learnable)?)?;
        out.raw(&source_key(self)?)?;
        out.boolean(hero.is_some())?;
        if let Some(HeroProgress {
            level,
            skill_points,
            experience,
        }) = hero
        {
            out.i32(*level)?;
            out.i32(*skill_points)?;
            out.boolean(experience.is_some())?;
            if let Some(experience) = experience {
                out.i32(experience.xp)?;
                out.boolean(experience.suspended)?;
            }
        }
        out.list(levels.len())?;
        out.owned(std::mem::size_of_val(levels.as_slice()))?;
        for (ability, rank) in levels {
            out.raw(ability)?;
            out.i32(*rank)?;
        }
        aliases_out(out, autocast_disabled)?;
        aliases_out(out, autocast_enabled)?;
        out.u64(*autocast_revision)?;
        out.boolean(autocast_order.is_some())?;
        if let Some((revision, order)) = autocast_order {
            out.u64(*revision)?;
            out.i32(*order)?;
        }
        aliases_out(out, alarm_ignored)?;
        out.list(cooldowns.len())?;
        out.owned(std::mem::size_of_val(cooldowns.as_slice()))?;
        for (ability, rank, bits) in cooldowns {
            out.raw(ability)?;
            out.i32(*rank)?;
            out.u32(*bits)?;
        }
        Ok(())
    }

    /// `self` supplies freshly reconstructed source learn slots; live ordinary abilities
    /// come from the payload, including removal, addition and their observable order.
    pub fn read_world_checkpoint(
        &self,
        input: &mut Reader<'_>,
        rules: &crate::hero_skills::Rules,
        budget: HeroBudget,
        alias: impl FnMut([u8; 4], Option<i32>) -> Result<(), String>,
    ) -> Result<Self, String> {
        if input.raw(32)? != source_key(self)? {
            return Err("checkpoint ability source slots differ".into());
        }
        let hero = if input.boolean()? {
            let level = input.i32()?;
            let skill_points = input.i32()?;
            let experience = if input.boolean()? {
                Some(crate::hero_experience::State {
                    xp: input.i32()?,
                    suspended: input.boolean()?,
                })
            } else {
                None
            };
            Some(HeroProgress {
                level,
                skill_points,
                experience,
            })
        } else {
            None
        };
        if hero.is_some() != self.hero.is_some() {
            return Err("checkpoint hero class differs from its source".into());
        }
        let count = input.list()?;
        if count > budget.abilities_per_unit {
            return Err("checkpoint ability count exceeds candidate admission".into());
        }
        input.owned(
            count
                .checked_mul(std::mem::size_of::<([u8; 4], i32)>())
                .ok_or("ability list size overflow")?,
        )?;
        let mut levels = Vec::new();
        levels
            .try_reserve_exact(count)
            .map_err(|_| "ability level allocation failed")?;
        for _ in 0..count {
            levels.push((
                input.raw(4)?.try_into().expect("four ability bytes"),
                input.i32()?,
            ));
        }
        let autocast_disabled = aliases_in(input)?;
        let autocast_enabled = aliases_in(input)?;
        let autocast_revision = input.u64()?;
        let autocast_order = if input.boolean()? {
            Some((input.u64()?, input.i32()?))
        } else {
            None
        };
        let alarm_ignored = aliases_in(input)?;
        let count = input.list()?;
        input.owned(
            count
                .checked_mul(std::mem::size_of::<([u8; 4], i32, u32)>())
                .ok_or("cooldown list size overflow")?,
        )?;
        let mut cooldowns = Vec::new();
        cooldowns
            .try_reserve_exact(count)
            .map_err(|_| "ability cooldown allocation failed")?;
        for _ in 0..count {
            cooldowns.push((
                input.raw(4)?.try_into().expect("four ability bytes"),
                input.i32()?,
                input.u32()?,
            ));
        }
        input.items(self.learnable.len())?;
        input.owned(slot_bytes(&self.learnable)?)?;
        let mut learnable = Vec::new();
        learnable
            .try_reserve_exact(self.learnable.len())
            .map_err(|_| "ability slot allocation failed")?;
        for Skill {
            alias,
            maximum,
            required,
            skip,
            dependency,
        } in &self.learnable
        {
            let dependency = dependency
                .as_deref()
                .map(|text| {
                    let mut result = String::new();
                    result
                        .try_reserve_exact(text.len())
                        .map_err(|_| "ability dependency allocation failed")?;
                    result.push_str(text);
                    Ok::<_, String>(result)
                })
                .transpose()?;
            learnable.push(Skill {
                alias: *alias,
                maximum: *maximum,
                required: *required,
                skip: *skip,
                dependency,
            });
        }
        let state = Self {
            hero,
            levels,
            autocast_disabled,
            autocast_enabled,
            autocast_revision,
            autocast_order,
            alarm_ignored,
            cooldowns,
            learnable,
        };
        validate(&state, rules, budget, alias)?;
        Ok(state)
    }
}

impl super::ObjectHost {
    fn checkpoint_ability_reference(
        &self,
        alias: [u8; 4],
        rank: Option<i32>,
    ) -> Result<(), String> {
        let input = self.heroes_admission()?;
        if alias == input.rules.hero_alias || input.rules.other_implicit.contains(&alias) {
            return if rank == Some(input.rules.first_rank) {
                Ok(())
            } else {
                Err("checkpoint implicit ability rank differs from its source".into())
            };
        }
        if !input.defined(alias)? {
            return Err("checkpoint ability has no candidate source row".into());
        }
        let maximum = input.number(alias, "levels")?;
        if rank.is_some_and(|rank| {
            rank != input.rules.first_rank && (rank < input.rules.first_rank || rank > maximum)
        }) {
            return Err("checkpoint ability rank exceeds its candidate source".into());
        }
        Ok(())
    }
    fn checkpoint_ability_flags(&self, state: &UnitAbilities) -> Result<(), String> {
        if state.autocast_disabled.is_empty()
            && state.autocast_enabled.is_empty()
            && state.alarm_ignored.is_empty()
        {
            return Ok(());
        }
        let input = self.heroes_admission()?;
        let rules = input.rules.autocast.as_ref().map_err(Clone::clone)?;
        for alias in state
            .autocast_disabled
            .iter()
            .chain(&state.autocast_enabled)
        {
            let code = self.ability_data_text(*alias, "code")?;
            if !rules
                .classes
                .iter()
                .any(|class| class.code.as_slice() == code.as_bytes())
            {
                return Err("checkpoint autocast flag has no candidate ability class".into());
            }
        }
        for alias in &state.alarm_ignored {
            let code = self.ability_data_text(*alias, "code")?;
            if !rules
                .alarm_code
                .is_some_and(|alarm| alarm.as_slice() == code.as_bytes())
            {
                return Err("checkpoint alarm flag has no candidate ability class".into());
            }
        }
        Ok(())
    }
    /// Ability section of the complete arena, in creation order. No unit is created here.
    pub fn write_unit_abilities_checkpoint(&self, out: &mut Writer) -> Result<(), String> {
        let units = self
            .inner
            .unit_slots
            .iter()
            .filter(|index| matches!(self.inner.arena[**index], super::Entry::Unit { .. }))
            .count();
        out.list(units)?;
        out.owned(
            units
                .checked_mul(std::mem::size_of::<(usize, Option<UnitAbilities>)>())
                .ok_or("unit ability staging size overflow")?,
        )?;
        for &index in &self.inner.unit_slots {
            let super::Entry::Unit { record } = &self.inner.arena[index] else {
                continue;
            };
            out.u64(
                self.inner
                    .id(index, "world checkpoint")
                    .map_err(|e| e.to_string())?
                    .token(),
            )?;
            out.boolean(record.abilities.is_some())?;
            if let Some(abilities) = &record.abilities {
                self.checkpoint_ability_flags(abilities)?;
                let input = self.heroes_admission()?;
                abilities.write_world_checkpoint(
                    out,
                    &input.rules,
                    input.budget,
                    |alias, rank| self.checkpoint_ability_reference(alias, rank),
                )?;
            }
        }
        Ok(())
    }
    /// The candidate arena already contains source-created ability slots. Decode every unit
    /// before replacing any section, and preserve its allocation and revision counters.
    pub fn restore_unit_abilities_checkpoint(
        &mut self,
        input: &mut Reader<'_>,
    ) -> Result<(), String> {
        let count = input.list()?;
        let units = self
            .inner
            .unit_slots
            .iter()
            .filter(|index| matches!(self.inner.arena[**index], super::Entry::Unit { .. }))
            .count();
        if count != units {
            return Err("checkpoint unit ability count differs from its candidate arena".into());
        }
        input.owned(
            count
                .checked_mul(std::mem::size_of::<(usize, Option<UnitAbilities>)>())
                .ok_or("unit ability staging size overflow")?,
        )?;
        let mut prepared = Vec::new();
        prepared
            .try_reserve_exact(count)
            .map_err(|_| "unit ability staging allocation failed")?;
        for &index in &self.inner.unit_slots {
            let super::Entry::Unit { record } = &self.inner.arena[index] else {
                continue;
            };
            if input.u64()?
                != self
                    .inner
                    .id(index, "world checkpoint")
                    .map_err(|e| e.to_string())?
                    .token()
            {
                return Err(
                    "checkpoint unit ability identity differs from its candidate arena".into(),
                );
            }
            let present = input.boolean()?;
            if present != record.abilities.is_some() {
                return Err(
                    "checkpoint unit ability admission differs from its candidate source".into(),
                );
            }
            let state = if let Some(source) = &record.abilities {
                let admission = self.heroes_admission()?;
                let state = source.read_world_checkpoint(
                    input,
                    &admission.rules,
                    admission.budget,
                    |alias, rank| self.checkpoint_ability_reference(alias, rank),
                )?;
                self.checkpoint_ability_flags(&state)?;
                Some(state)
            } else {
                None
            };
            prepared.push((index, state));
        }
        for (index, state) in prepared {
            let super::Entry::Unit { record } = &mut self.inner.arena[index] else {
                unreachable!("prepared candidate unit")
            };
            record.abilities = state;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pcx_script::checkpoint::Budget;

    const BUDGET: Budget = Budget {
        bytes: 16384,
        items: 256,
        depth: 16,
    };
    const HERO: HeroBudget = HeroBudget {
        abilities_per_unit: 8,
        overlay_objects: 0,
        modifications_per_ability: 0,
    };
    fn rules() -> crate::hero_skills::Rules {
        let mut rules = crate::hero_skills::tests::rules();
        rules.initial_level = 1;
        rules.first_rank = 1;
        rules.progression.max_level = 10;
        rules.experience = None;
        rules
    }
    fn source() -> UnitAbilities {
        UnitAbilities {
            hero: Some(HeroProgress {
                level: 1,
                skill_points: 1,
                experience: None,
            }),
            levels: vec![(*b"B001", 1), (*b"C001", 1)],
            learnable: vec![Skill {
                alias: *b"A001",
                maximum: 3,
                required: 1,
                skip: 2,
                dependency: Some("synthetic dependency".into()),
            }],
            cooldowns: vec![],
            autocast_disabled: vec![],
            autocast_enabled: vec![],
            autocast_revision: 0,
            autocast_order: None,
            alarm_ignored: vec![],
        }
    }
    fn admitted(alias: [u8; 4], rank: Option<i32>) -> Result<(), String> {
        if ![*b"A001", *b"B001", *b"C001", *b"D001"].contains(&alias)
            || rank.is_some_and(|rank| !(1..=3).contains(&rank))
        {
            return Err("synthetic source does not admit ability/rank".into());
        }
        Ok(())
    }
    fn encode(state: &UnitAbilities) -> Result<Vec<u8>, String> {
        let mut out = Writer::new(BUDGET)?;
        state.write_world_checkpoint(&mut out, &rules(), HERO, admitted)?;
        Ok(out.finish())
    }
    fn decode(source: &UnitAbilities, bytes: &[u8]) -> Result<UnitAbilities, String> {
        let mut input = Reader::new(bytes, BUDGET)?;
        let result = source.read_world_checkpoint(&mut input, &rules(), HERO, admitted)?;
        input.finish()?;
        Ok(result)
    }
    fn live() -> UnitAbilities {
        let mut state = source();
        // Ordinary removal and re-addition is transient for StoreUnit, but live world state.
        assert!(state.withdraw(*b"B001"));
        assert!(state.withdraw(*b"C001"));
        assert!(state.grant(*b"D001", 2, 8).unwrap());
        assert!(state.grant(*b"B001", 1, 8).unwrap());
        assert!(state.grant(*b"A001", 2, 8).unwrap());
        state.hero = Some(HeroProgress {
            level: 3,
            skill_points: -17,
            experience: None,
        });
        state.cooldowns = vec![
            (*b"D001", 3, (-0.0f32).to_bits()),
            (*b"A001", 2, 1.375f32.to_bits()),
        ];
        state.autocast_disabled = vec![*b"D001"];
        state.autocast_enabled = vec![*b"B001"];
        state.autocast_revision = 13;
        state.autocast_order = Some((13, 17));
        state.alarm_ignored = vec![*b"D001"];
        state
    }
    #[test]
    fn world_ability_section_retains_transient_live_order_and_all_pending_flags() {
        let source = source();
        let live = live();
        let bytes = encode(&live).unwrap();
        let loaded = decode(&source, &bytes).unwrap();
        assert_eq!(loaded, live);
        assert_eq!(
            loaded.levels().collect::<Vec<_>>(),
            vec![(*b"D001", 2), (*b"B001", 1), (*b"A001", 2)]
        );
        assert_eq!(loaded.level(*b"C001"), 0);
        assert_eq!(loaded.cooldowns[0].2, (-0.0f32).to_bits());
        assert_eq!(encode(&loaded).unwrap(), bytes);
        assert_ne!(
            loaded.levels().collect::<Vec<_>>(),
            source.levels().collect::<Vec<_>>()
        );
        let mut with_xp = live;
        with_xp.hero.as_mut().unwrap().experience = Some(crate::hero_experience::State {
            xp: 91,
            suspended: true,
        });
        assert_eq!(
            decode(&source, &encode(&with_xp).unwrap()).unwrap(),
            with_xp
        );
    }
    #[test]
    fn world_ability_section_refuses_foreign_source_aliases_ranks_and_partial_records() {
        let source = source();
        let live = live();
        let bytes = encode(&live).unwrap();
        for end in 0..bytes.len() {
            assert!(decode(&source, &bytes[..end]).is_err(), "length {end}");
        }
        let mut other = source.clone();
        other.learnable[0].skip += 1;
        assert!(decode(&other, &bytes).unwrap_err().contains("source slots"));
        let mut bad = bytes.clone();
        let at = bad.windows(4).position(|w| w == b"D001").unwrap();
        bad[at..at + 4].copy_from_slice(b"Z999");
        assert!(
            decode(&source, &bad)
                .unwrap_err()
                .contains("synthetic source")
        );
        for case in 0..6 {
            let mut invalid = live.clone();
            match case {
                0 => invalid.levels.push(invalid.levels[0]),
                1 => invalid.levels[0].1 = 4,
                2 => invalid.autocast_enabled.push(*b"D001"),
                3 => invalid.autocast_order = Some((12, 17)),
                4 => invalid.cooldowns[0].2 = f32::NAN.to_bits(),
                _ => invalid.hero.as_mut().unwrap().level = 11,
            }
            assert!(encode(&invalid).is_err(), "invalid case {case}");
        }
        assert_eq!(source, super::tests::source());
        assert_eq!(encode(&live).unwrap(), bytes);
    }
}
