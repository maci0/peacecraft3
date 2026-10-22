//! Ability learning on the existing executed unit, using selected stock/map data.
use super::*;
use crate::hero_skills::Rules;
use crate::inventory_powerup::AttributeBonus;
use crate::playercolor_host::{EventSubject, HeroEvent};
use pcx_objects::editor::Overlay;
use pcx_script::value::FunctionId;
use std::collections::BTreeSet;

#[path = "ability_cooldown.rs"]
mod cooldown;
#[path = "hero_experience_host.rs"]
mod experience;
#[path = "hero_experience_kills.rs"]
mod experience_kills;
#[path = "ability_tooltip.rs"]
mod tooltip;

impl ObjectHost {
    pub(crate) fn ability_readiness_at(
        unit: &ExecutedUnit,
        mana_cost: f32,
        ready_at: Option<f64>,
        now: f64,
    ) -> crate::ability_profile::Readiness {
        crate::ability_profile::Readiness {
            mana_cost,
            mana: unit.vitals.map(|vitals| vitals.mana),
            cooldown_remaining: ready_at.map_or(0.0, |due| (due - now).max(0.0)),
            paused: unit.paused_since.is_some(),
        }
    }
}

struct HeroDelivery {
    trigger: HandleId,
    filter: Option<FunctionId>,
    event: HandleId,
    player: Option<HandleId>,
}

const LEARN: &str = "SelectHeroSkill";
const LEVEL: &str = "GetHeroLevel";
const POINTS: &str = "GetHeroSkillPoints";
const ABILITY: &str = "GetUnitAbilityLevel";
/// Attribute queries and the source stat each reads from the unit balance columns.
const ATTRIBUTES: [(&str, &str); 3] = [
    ("GetHeroStr", "STR"),
    ("GetHeroAgi", "AGI"),
    ("GetHeroInt", "INT"),
];

#[cfg(test)]
mod cached_tests {
    use super::*;
    #[test]
    fn cached_learned_ranks_follow_current_source_rules_and_refusals_preserve_progress() {
        let mut rules = crate::hero_skills::tests::rules();
        rules.initial_level = 1;
        rules.first_rank = 1;
        rules.progression.max_level = 10;
        let budget = HeroBudget {
            abilities_per_unit: 8,
            overlay_objects: 0,
            modifications_per_ability: 0,
        };
        let original = UnitAbilities {
            hero: Some(HeroProgress {
                level: 1,
                skill_points: 1,
                experience: None,
            }),
            levels: vec![(*b"B001", 1)],
            cooldowns: vec![],
            learnable: vec![Skill {
                alias: *b"A001",
                maximum: 3,
                required: 1,
                skip: 2,
                dependency: None,
            }],
        };
        let saved = crate::game_cache::CachedUnit {
            rawcode: *b"H001",
            client_sha256: [0; 32],
            install: "synthetic".into(),
            edition: "fixture".into(),
            hero: Some([3, 1]),
            experience: None,
            abilities: vec![(*b"A001", 2), (*b"B001", 1)],
            cooldowns: vec![(*b"A001", 2, 1.125f32.to_bits())],
            permanent_strength: 0,
            pools: [5f32, 10., 1., 2.].map(f32::to_bits),
            items: vec![],
        };
        let mut restored = original.clone();
        restored.restore_cached(&saved, &rules, budget).unwrap();
        assert_eq!(restored.level(*b"A001"), 2);
        assert_eq!(restored.cooldowns, saved.cooldowns);
        assert_eq!(
            restored.hero,
            Some(HeroProgress {
                level: 3,
                skill_points: 1,
                experience: None,
            })
        );
        for case in 0..5 {
            let mut invalid = saved.clone();
            match case {
                0 => invalid.hero = Some([2, 1]),
                1 => invalid.abilities[0].1 = 4,
                2 => {
                    invalid.abilities.pop();
                }
                3 => invalid.hero = Some([3, 2]),
                _ => invalid.hero = None,
            }
            let mut untouched = original.clone();
            assert!(
                untouched.restore_cached(&invalid, &rules, budget).is_err(),
                "case {case}"
            );
            assert_eq!(untouched, original);
        }
        rules.experience = Some(std::sync::Arc::new(crate::hero_experience::tests::rules()));
        let mut with_xp = saved.clone();
        with_xp.experience = Some(crate::hero_experience::State {
            xp: 39,
            suspended: true,
        });
        restored.restore_cached(&with_xp, &rules, budget).unwrap();
        assert_eq!(
            restored.hero.as_ref().unwrap().experience,
            with_xp.experience
        );
        let before = restored.clone();
        with_xp.experience.as_mut().unwrap().xp = 0;
        assert!(restored.restore_cached(&with_xp, &rules, budget).is_err());
        assert_eq!(restored, before);
        restored.restore_cached(&saved, &rules, budget).unwrap();
        assert_eq!(restored.hero.as_ref().unwrap().experience, None);
    }
}

#[derive(Clone, Copy, Debug)]
pub struct HeroBudget {
    pub abilities_per_unit: usize,
    pub overlay_objects: usize,
    pub modifications_per_ability: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeroProgress {
    pub level: i32,
    pub skill_points: i32,
    /// None is unadmitted or absent from an older recreation cache; never fabricated XP.
    pub experience: Option<crate::hero_experience::State>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Skill {
    alias: [u8; 4],
    maximum: i32,
    required: i32,
    skip: i32,
    dependency: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitAbilities {
    pub hero: Option<HeroProgress>,
    levels: Vec<([u8; 4], i32)>,
    learnable: Vec<Skill>,
    /// Per-unit, one-based rank overrides; durations retain their exact f32 bits.
    pub(super) cooldowns: Vec<([u8; 4], i32, u32)>,
}
impl UnitAbilities {
    pub(crate) fn cooldowns(&self) -> &[([u8; 4], i32, u32)] {
        &self.cooldowns
    }
    pub fn levels(&self) -> impl Iterator<Item = ([u8; 4], i32)> + '_ {
        self.levels.iter().copied()
    }
    pub fn level(&self, ability: [u8; 4]) -> i32 {
        self.levels
            .iter()
            .find(|(alias, _)| *alias == ability)
            .map_or(0, |(_, level)| *level)
    }
    fn capacity(&self) -> Result<i32, String> {
        self.learnable.iter().try_fold(0i32, |sum, skill| {
            sum.checked_add((skill.maximum - self.level(skill.alias)).max(0))
                .ok_or_else(|| "hero skill capacity overflow".into())
        })
    }
    fn restore_cached(
        &mut self,
        saved: &crate::game_cache::CachedUnit,
        rules: &Rules,
        budget: HeroBudget,
    ) -> Result<(), String> {
        saved.validate()?;
        if saved.abilities.len() > budget.abilities_per_unit {
            return Err("cached ability count exceeds current admission".into());
        }
        let hero = match (&self.hero, saved.hero) {
            (Some(_), Some([level, skill_points]))
                if level >= rules.initial_level && level <= rules.progression.max_level =>
            {
                Some(HeroProgress {
                    level,
                    skill_points,
                    experience: saved.experience,
                })
            }
            (None, None) => None,
            _ => return Err("cached hero class/level differs from current source rules".into()),
        };
        for (alias, initial) in &self.levels {
            if !self.learnable.iter().any(|skill| skill.alias == *alias)
                && !saved.abilities.contains(&(*alias, *initial))
            {
                return Err("cached unit lacks a current source ability".into());
            }
        }
        for (alias, level) in &saved.abilities {
            if let Some(skill) = self.learnable.iter().find(|skill| skill.alias == *alias) {
                let hero = hero.as_ref().ok_or("cached learned rank without a hero")?;
                let required = skill
                    .skip
                    .checked_mul((*level - rules.first_rank).max(0))
                    .and_then(|n| n.checked_add(skill.required))
                    .ok_or("cached skill requirement overflow")?;
                if *level < rules.first_rank || *level > skill.maximum || hero.level < required {
                    return Err("cached rank exceeds current learning rules".into());
                }
            } else if !self.levels.contains(&(*alias, *level)) {
                return Err("cached ability is absent from current unit data".into());
            }
        }
        let mut levels = Vec::new();
        levels
            .try_reserve_exact(saved.abilities.len())
            .map_err(|_| "cached ability allocation failed")?;
        levels.extend_from_slice(&saved.abilities);
        let mut candidate = self.clone();
        if let (Some(hero), Some(experience_rules), Some(experience)) =
            (&hero, &rules.experience, saved.experience)
            && experience_rules
                .thresholds
                .level_for(rules.initial_level, experience.xp)?
                != hero.level
        {
            return Err("cached XP differs from source hero level".into());
        }
        candidate.levels = levels;
        candidate.hero = hero;
        candidate.cooldowns = saved.cooldowns.clone();
        let capacity = candidate.capacity()?;
        if candidate
            .hero
            .as_ref()
            .is_some_and(|hero| hero.skill_points > capacity)
        {
            return Err("cached skill points exceed current capacity".into());
        }
        *self = candidate;
        Ok(())
    }
}

pub(super) struct Admission {
    tables: std::sync::Arc<DefaultTables>,
    catalog_origin: crate::scripts::AssetIdentity,
    overlay: Overlay,
    rules: Rules,
    budget: HeroBudget,
    learn: bool,
    level: bool,
    points: bool,
    ability: bool,
    /// Admitted `GetHeroStr`, `GetHeroAgi`, `GetHeroInt` in `ATTRIBUTES` order.
    attributes: [bool; 3],
    cooldown_natives: [bool; cooldown::NATIVES.len()],
    experience_natives: [bool; experience::NATIVES.len()],
}

impl ObjectHost {
    pub(super) fn restore_cached_abilities(
        &self,
        state: &mut UnitAbilities,
        saved: &crate::game_cache::CachedUnit,
    ) -> Result<(), Error> {
        let fail = |detail: &str| self.hero_error("RestoreUnit", detail.into());
        let input = self
            .admitted_units()?
            .heroes
            .as_ref()
            .ok_or_else(|| fail("cached ability rules missing"))?;
        for (alias, rank, _) in &saved.cooldowns {
            if *rank
                > input
                    .number(*alias, "levels")
                    .map_err(|e| self.hero_error("RestoreUnit", e))?
            {
                return Err(fail("cached cooldown rank exceeds current source levels"));
            }
            self.hero_ability_level_cell(*alias, "Cool", *rank, FieldKind::Unreal)
                .map_err(|e| self.hero_error("RestoreUnit", e))?;
        }
        state
            .restore_cached(saved, &input.rules, input.budget)
            .map_err(|detail| self.hero_error("RestoreUnit", detail))
    }
    pub(crate) fn unit_implicit_hero_alias(&self) -> Option<[u8; 4]> {
        self.units
            .as_ref()?
            .heroes
            .as_ref()
            .map(|input| input.rules.hero_alias)
    }
    pub(crate) fn hero_initial_level(&self) -> Option<i32> {
        self.units
            .as_ref()?
            .heroes
            .as_ref()
            .map(|input| input.rules.initial_level)
    }

    /// Retain the caller's existing tables; scalar W3A learning-field overrides stay explicit.
    pub fn with_hero_skills(
        mut self,
        tables: impl Into<std::sync::Arc<DefaultTables>>,
        catalog_origin: crate::scripts::AssetIdentity,
        overlay: Overlay,
        rules: Rules,
        budget: HeroBudget,
    ) -> Result<Self, Error> {
        let tables = tables.into();
        let fail = |s: &str| self.inner.error(LEARN, Failure::HeroSkills(s.into()));
        let units = self.admitted_units()?;
        if units.heroes.is_some() || units.records != 0 {
            return Err(fail("admit once, before unit creation"));
        }
        if tables.identity().install != catalog_origin.source
            || tables.identity().edition_version.is_empty()
            || [
                &catalog_origin.source,
                &catalog_origin.edition,
                &catalog_origin.version,
            ]
            .iter()
            .any(|s| s.trim().is_empty())
        {
            return Err(fail(
                "ability tables do not identify the selected unit catalog",
            ));
        }
        if overlay.mods.len().saturating_add(overlay.custom.len()) > budget.overlay_objects
            || overlay
                .mods
                .values()
                .chain(overlay.custom.values().map(|(_, mods)| mods))
                .any(|mods| mods.len() > budget.modifications_per_ability)
        {
            return Err(fail("ability overlay budget exceeded"));
        }
        let mut present = [false; 7];
        for (index, (name, args, result)) in [
            (LEARN, &["unit", "integer"][..], "nothing"),
            (LEVEL, &["unit"][..], "integer"),
            (POINTS, &["unit"][..], "integer"),
            (ABILITY, &["unit", "integer"][..], "integer"),
            (ATTRIBUTES[0].0, &["unit", "boolean"][..], "integer"),
            (ATTRIBUTES[1].0, &["unit", "boolean"][..], "integer"),
            (ATTRIBUTES[2].0, &["unit", "boolean"][..], "integer"),
        ]
        .into_iter()
        .enumerate()
        {
            let Some(native) = self.inner.program.natives().get(name) else {
                continue;
            };
            Self::contract(
                &self.inner,
                name,
                args,
                result,
                name != LEARN && native.is_constant,
                Some(KIND),
            )?;
            if native.source != units.declaration_source {
                return Err(self.inner.error(name, Failure::Source));
            }
            present[index] = true;
        }
        let [
            learn,
            level,
            points,
            ability,
            strength,
            agility,
            intelligence,
        ] = present;
        for (name, result) in [
            ("GetLevelingUnit", "unit"),
            ("GetLearningUnit", "unit"),
            ("GetLearnedSkill", "integer"),
            ("GetLearnedSkillLevel", "integer"),
        ] {
            if let Some(native) = self.inner.program.natives().get(name) {
                Self::contract(&self.inner, name, &[], result, true, None)?;
                if native.source != units.declaration_source {
                    return Err(self.inner.error(name, Failure::Source));
                }
            }
        }
        let mut cooldown_natives = [false; cooldown::NATIVES.len()];
        for (index, (name, params, result)) in cooldown::NATIVES.into_iter().enumerate() {
            if let Some(native) = self.inner.program.natives().get(name) {
                Self::contract(
                    &self.inner,
                    name,
                    params,
                    result,
                    false,
                    params.contains(&KIND).then_some(KIND),
                )?;
                if native.source != units.declaration_source {
                    return Err(self.inner.error(name, Failure::Source));
                }
                cooldown_natives[index] = true;
            }
        }
        let mut experience_natives = [false; experience::NATIVES.len()];
        if rules.experience.is_some() {
            for (index, (name, params, result)) in experience::NATIVES.into_iter().enumerate() {
                if let Some(native) = self.inner.program.natives().get(name) {
                    Self::contract(
                        &self.inner,
                        name,
                        params,
                        result,
                        native.is_constant,
                        Some(KIND),
                    )?;
                    if native.source != units.declaration_source {
                        return Err(self.inner.error(name, Failure::Source));
                    }
                    experience_natives[index] = true;
                }
            }
        }
        self.units.as_mut().expect("admitted").heroes = Some(Admission {
            tables,
            catalog_origin,
            overlay,
            rules,
            budget,
            learn,
            level,
            points,
            ability,
            attributes: [strength, agility, intelligence],
            cooldown_natives,
            experience_natives,
        });
        Ok(self)
    }

    pub(super) fn prepare_unit_abilities(
        &self,
        definition: &Definition,
    ) -> Result<Option<UnitAbilities>, Error> {
        let Some(input) = self.admitted_units()?.heroes.as_ref() else {
            return Ok(None);
        };
        let result = || -> Result<UnitAbilities, String> {
            let mut state = UnitAbilities {
                hero: None,
                levels: Vec::new(),
                learnable: Vec::new(),
                cooldowns: Vec::new(),
            };
            for alias in ability_list(definition, "abilList", input.budget.abilities_per_unit)? {
                // Resolve the same alias even when a custom row shares a stock code.
                input.number(alias, "levels")?;
                state
                    .levels
                    .try_reserve(1)
                    .map_err(|_| "ability allocation")?;
                state.levels.push((alias, input.rules.first_rank));
            }
            if definition.rawcode[0].is_ascii_uppercase() {
                if state.level(input.rules.hero_alias) == 0 {
                    if state.levels.len() >= input.budget.abilities_per_unit {
                        return Err("per-unit ability budget exceeded".into());
                    }
                    state
                        .levels
                        .try_reserve(1)
                        .map_err(|_| "hero component allocation")?;
                    state
                        .levels
                        .push((input.rules.hero_alias, input.rules.first_rank));
                }
                let list = ability_list(definition, "heroAbilList", input.rules.slots)?;
                if list.len().saturating_add(state.levels.len()) > input.budget.abilities_per_unit {
                    return Err("per-unit ability budget exceeded".into());
                }
                for alias in list {
                    let maximum = input.number(alias, "levels")?;
                    let required = input.number(alias, "reqLevel")?;
                    let skip = input.number(alias, "levelSkip")?;
                    if maximum < 0 || required < 0 || skip < 0 {
                        return Err(format!("{alias:?}: negative learning rule"));
                    }
                    let dependency = input.requirements(alias)?;
                    state
                        .learnable
                        .try_reserve(1)
                        .map_err(|_| "skill allocation")?;
                    state.learnable.push(Skill {
                        alias,
                        maximum,
                        required,
                        skip: if skip == 0 {
                            input.rules.progression.ability_level_skip
                        } else {
                            skip
                        },
                        dependency,
                    });
                }
                let capacity = state.learnable.iter().try_fold(0i32, |sum, skill| {
                    sum.checked_add(skill.maximum)
                        .ok_or("initial skill capacity overflow")
                })?;
                state.hero = Some(HeroProgress {
                    level: input.rules.initial_level,
                    skill_points: input.rules.initial_points.min(capacity),
                    experience: input.rules.experience.as_ref().map(|rules| rules.initial),
                });
                state
                    .levels
                    .try_reserve(state.learnable.len())
                    .map_err(|_| "learned ability allocation")?;
            }
            Ok(state)
        };
        result()
            .map(Some)
            .map_err(|e| self.hero_error("CreateUnit", e))
    }

    fn hero_error(&self, name: &str, detail: String) -> Error {
        let mut error = self.inner.error(name, Failure::HeroSkills(detail));
        if let Some(input) = self.units.as_ref().and_then(|u| u.heroes.as_ref()) {
            error.source_context.push_str(&format!(
                "; ability tables {:?}; selected catalog {:?}; hero rules {:?}",
                input.tables.identity(),
                input.catalog_origin,
                input.rules.origin()
            ));
        }
        error
    }

    pub(super) fn hero_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        let input = self.units.as_ref()?.heroes.as_ref()?;
        if let Some(index) = experience::NATIVES
            .iter()
            .position(|(native, _, _)| *native == name)
        {
            return input.experience_natives[index].then(|| self.hero_experience_call(name, args));
        }
        if let Some(index) = cooldown::NATIVES
            .iter()
            .position(|(native, _, _)| *native == name)
        {
            return input.cooldown_natives[index].then(|| self.ability_cooldown_call(name, args));
        }
        if let Some(index) = ATTRIBUTES.iter().position(|(native, _)| *native == name) {
            let (_, stat) = ATTRIBUTES[index];
            return input.attributes[index].then(|| self.hero_attribute(name, stat, args));
        }
        if !match name {
            LEARN => input.learn,
            LEVEL => input.level,
            POINTS => input.points,
            ABILITY => input.ability,
            _ => false,
        } {
            return None;
        }
        Some(self.hero_call(name, args))
    }

    /// Growth-derived attribute of an admitted hero. Without `includeBonuses` the total holds
    /// the base and retained permanent strength only; held passive items are read only with it.
    fn hero_attribute(&self, name: &str, stat: &str, args: &[Value]) -> Result<Value, Error> {
        let [unit, Value::Bool(include_bonuses)] = args else {
            return Err(self.hero_error(name, "unit and includeBonuses required".into()));
        };
        let unit = match unit {
            Value::Null => return Ok(Value::Int(0)),
            Value::Handle(unit) => *unit,
            _ => return Err(self.hero_error(name, "unit identity required".into())),
        };
        // A removed unit fails the handle gate of the native wrapper, so it reads zero.
        // Dead units pass that gate, and their combat state is not consulted here.
        if matches!(self.inner.entry(unit)?, Entry::RemovedUnit { .. }) {
            return Ok(Value::Int(0));
        }
        let record = self.created_unit(unit, name)?;
        let state = record
            .abilities
            .as_ref()
            .ok_or_else(|| self.hero_error(name, "ability state not admitted".into()))?;
        // Non-hero units take the wrapper's uppercase-rawcode branch, which returns 0. The
        // constant-zero registration returns 0 too, so this holds for either binding.
        let Some(hero) = &state.hero else {
            return Ok(Value::Int(0));
        };
        let bonus = if *include_bonuses {
            self.unit_hero_bonus(unit)?
        } else {
            AttributeBonus {
                strength: record.permanent_strength,
                ..AttributeBonus::default()
            }
        };
        let initial = self
            .hero_initial_level()
            .ok_or_else(|| self.hero_error(name, "hero rules not admitted".into()))?;
        let value = Self::derive_hero_attribute(
            &record.definition,
            Some((initial, hero.level)),
            bonus,
            stat,
        )
        .map_err(|e| self.hero_error(name, e))?;
        if value >= i32::MAX as f32 {
            return Err(self.hero_error(name, format!("hero attribute {stat} exceeds integer")));
        }
        Ok(Value::Int(value as i32))
    }

    fn hero_call(&mut self, name: &str, args: &[Value]) -> Result<Value, Error> {
        let (unit, ability) = match (name, args) {
            (LEVEL | POINTS, [unit]) => (unit, None),
            (ABILITY | LEARN, [unit, Value::Int(ability)]) => (unit, Some(ability.to_be_bytes())),
            _ => return Err(self.hero_error(name, "unit/ability arguments required".into())),
        };
        if matches!(unit, Value::Null) {
            return Ok(if name == LEARN {
                Value::Null
            } else {
                Value::Int(0)
            });
        }
        let Value::Handle(unit) = unit else {
            return Err(self.hero_error(name, "unit identity required".into()));
        };
        let record = self.created_unit(*unit, name)?;
        let state = record
            .abilities
            .as_ref()
            .ok_or_else(|| self.hero_error(name, "ability state not admitted".into()))?;
        if name == ABILITY {
            let alias = ability.expect("argument");
            let level = state.level(alias);
            if level == 0
                && self
                    .admitted_units()?
                    .heroes
                    .as_ref()
                    .expect("admitted")
                    .rules
                    .other_implicit
                    .contains(&alias)
            {
                return Err(self.hero_error(
                    name,
                    "implicit ability component state is not derived".into(),
                ));
            }
            return Ok(Value::Int(level));
        }
        if name == LEVEL {
            let level = if let Some(hero) = &state.hero {
                hero.level
            } else {
                if state.level(
                    self.admitted_units()?
                        .heroes
                        .as_ref()
                        .expect("admitted")
                        .rules
                        .hero_alias,
                ) > 0
                {
                    return Err(self.hero_error(
                        name,
                        "explicit hero component counters are not derived".into(),
                    ));
                }
                unit_class_level(&record.definition).map_err(|e| self.hero_error(name, e))?
            };
            return Ok(Value::Int(level));
        }
        let Some(hero) = &state.hero else {
            return Ok(if name == POINTS {
                Value::Int(0)
            } else {
                Value::Null
            });
        };
        if name == POINTS {
            return Ok(Value::Int(hero.skill_points));
        }
        let alias = ability.expect("argument");
        let Some(skill) = state.learnable.iter().find(|s| s.alias == alias) else {
            return Ok(Value::Null);
        };
        let level = state.level(alias);
        if hero.skill_points <= 0 || level >= skill.maximum {
            return Ok(Value::Null);
        }
        let required = level
            .checked_mul(skill.skip)
            .and_then(|n| n.checked_add(skill.required))
            .ok_or_else(|| self.hero_error(name, "required hero level overflow".into()))?;
        if hero.level < required {
            return Ok(Value::Null);
        }
        if let Some(dependency) = &skill.dependency {
            // shortcut: dependency-controlled learning needs the authored requirement/availability consumer before it can mutate state.
            return Err(self.hero_error(
                name,
                format!("{alias:?}: dependency-controlled learning unsupported: {dependency}"),
            ));
        }
        let events = self.prepare_hero_events(
            *unit,
            name,
            "EVENT_UNIT_HERO_SKILL",
            "EVENT_PLAYER_HERO_SKILL",
        )?;
        let first_rank = self
            .admitted_units()?
            .heroes
            .as_ref()
            .expect("admitted")
            .rules
            .first_rank;
        let learned_rank = if level == 0 {
            first_rank
        } else {
            level
                .checked_add(1)
                .ok_or_else(|| self.hero_error(name, "learned rank overflow".into()))?
        };
        let Entry::Unit { record } = self.inner.entry_mut(*unit)? else {
            unreachable!()
        };
        let state = record.abilities.as_mut().expect("admitted");
        if let Some((_, rank)) = state.levels.iter_mut().find(|(a, _)| *a == alias) {
            *rank = learned_rank;
        } else {
            state.levels.push((alias, first_rank));
        }
        state.hero.as_mut().expect("hero").skill_points -= 1;
        self.push_hero_events(
            events,
            *unit,
            HeroEvent::Skill {
                ability: i32::from_be_bytes(alias),
                level: learned_rank,
            },
        );
        Ok(Value::Null)
    }

    fn prepare_hero_events(
        &mut self,
        unit: HandleId,
        name: &str,
        unit_global: &str,
        player_global: &str,
    ) -> Result<Vec<HeroDelivery>, Error> {
        let owner = self.current_unit_owner(unit, name)?.0;
        let event =
            self.unit_event_enumerand(name, unit_global, "unitevent", "ConvertUnitEvent")?;
        let player_event = self.unit_event_enumerand(
            name,
            player_global,
            "playerunitevent",
            "ConvertPlayerUnitEvent",
        )?;
        let mut fired = Vec::new();
        for entry in &self.inner.arena {
            let (trigger, filter, identity, player, qualified) = match entry {
                Entry::RegisteredUnitEvent {
                    trigger,
                    unit: watched,
                    enumerand,
                    unitevent,
                } if *watched == unit && event.is_none_or(|event| event == *enumerand) => {
                    (*trigger, None, *unitevent, None, event.is_some())
                }
                Entry::RegisteredPlayerUnitEvent(r)
                    if r.player == owner
                        && player_event.is_none_or(|event| event == r.enumerand) =>
                {
                    (
                        r.trigger,
                        r.filter,
                        r.playerunitevent,
                        Some(r.player),
                        player_event.is_some(),
                    )
                }
                _ => continue,
            };
            match self.inner.entry(trigger)? {
                Entry::DeadTrigger
                | Entry::Trigger {
                    enabled: Some(false),
                } => continue,
                Entry::Trigger { .. } => {}
                _ => return Err(self.inner.error(name, Failure::WrongFamily)),
            }
            if !qualified {
                return Err(self.hero_error(
                    name,
                    "hero event identity is not declared by the selected source".into(),
                ));
            }
            if self.trigger_runtime.is_none() {
                return Err(self.hero_error(name, "hero listeners require event runtime".into()));
            }
            let filter = self.resolve_event_filter(filter)?;
            fired
                .try_reserve(1)
                .map_err(|_| self.inner.error(name, Failure::Capacity))?;
            fired.push(HeroDelivery {
                trigger,
                filter,
                event: identity,
                player,
            });
        }
        self.reserve_event_delivery(fired.len())?;
        Ok(fired)
    }

    fn push_hero_events(&mut self, events: Vec<HeroDelivery>, unit: HandleId, kind: HeroEvent) {
        for event in events {
            self.push_reserved_event(
                event.trigger,
                event.filter,
                EventSubject {
                    unit: Some(unit),
                    player: event.player,
                    hero: Some(kind),
                    ..EventSubject::default()
                },
                Some(event.event),
            );
        }
    }

    pub(super) fn change_hero_level(&mut self, args: &[Value]) -> Result<Value, Error> {
        let [unit, Value::Int(level), Value::Bool(eye)] = args else {
            return Err(self.hero_error(
                SET_HERO_LEVEL,
                "unit, level and eye-candy flag required".into(),
            ));
        };
        if matches!(unit, Value::Null) {
            return Ok(Value::Null);
        }
        let Value::Handle(unit) = unit else {
            return Err(self.inner.error(SET_HERO_LEVEL, Failure::WrongFamily));
        };
        let record = self.created_unit(*unit, SET_HERO_LEVEL)?;
        let state = record.abilities.as_ref().expect("admitted ability state");
        let Some(hero) = &state.hero else {
            return Ok(Value::Null);
        };
        let maximum = self
            .admitted_units()?
            .heroes
            .as_ref()
            .expect("admitted")
            .rules
            .progression
            .max_level;
        let next = (*level).min(maximum);
        if next <= hero.level {
            return Ok(Value::Null);
        }
        let points = hero
            .skill_points
            .checked_add(next - hero.level)
            .ok_or_else(|| self.hero_error(SET_HERO_LEVEL, "skill point overflow".into()))?
            .min(
                state
                    .capacity()
                    .map_err(|e| self.hero_error(SET_HERO_LEVEL, e))?,
            );
        let experience = match (
            hero.experience,
            self.admitted_units()?
                .heroes
                .as_ref()
                .and_then(|h| h.rules.experience.as_ref()),
        ) {
            (Some(mut state), Some(rules)) => {
                let initial = self.hero_initial_level().expect("hero rules");
                state.xp = state.xp.max(
                    rules
                        .thresholds
                        .minimum_for(initial, next)
                        .map_err(|e| self.hero_error(SET_HERO_LEVEL, e))?,
                );
                Some(state)
            }
            _ => hero.experience,
        };
        let events = self.prepare_hero_events(
            *unit,
            SET_HERO_LEVEL,
            "EVENT_UNIT_HERO_LEVEL",
            "EVENT_PLAYER_HERO_LEVEL",
        )?;
        let vitals = self.prepare_hero_vital_level(*unit, next)?;
        let Entry::Unit { record } = self.inner.entry_mut(*unit)? else {
            unreachable!()
        };
        if let Some(vitals) = vitals {
            record.vitals = Some(vitals);
        }
        record.requested_hero_level = Some(*level);
        record.requested_show_eye_candy = Some(*eye);
        let hero = record
            .abilities
            .as_mut()
            .expect("admitted")
            .hero
            .as_mut()
            .expect("hero");
        hero.level = next;
        hero.skill_points = points;
        hero.experience = experience;
        self.push_hero_events(events, *unit, HeroEvent::Level);
        Ok(Value::Null)
    }
}

impl Admission {
    fn base(&self, alias: [u8; 4]) -> Result<(&str, &[pcx_objects::Mod]), String> {
        crate::ability_profile::base(&self.tables, &self.overlay, alias)
    }
    fn number(&self, alias: [u8; 4], column: &str) -> Result<i32, String> {
        let (base, mods) = self.base(alias)?;
        let metadata = self.tables.editor().fields().iter().filter(|f| f.from == SlkId::AbilityMetaData)
            .filter(|f| matches!(&f.source, FieldSource::Column { slk: SlkId::AbilityData, column: name } if name == column))
            .collect::<Vec<_>>();
        if metadata.len() > 1 {
            return Err(format!(
                "{alias:?}: ambiguous AbilityData/{column} metadata"
            ));
        }
        if let Some(field) = metadata.first()
            && let Some(entry) = mods.iter().rev().find(|m| m.field == field.fourcc)
        {
            if entry.selectors.is_some_and(|s| s != [0, 0])
                || field.kind != FieldKind::Int
                || field.index != -1
                || entry.value_type != pcx_objects::ValueType::Int
            {
                return Err(format!("{alias:?}: unsupported indexed {column} override"));
            }
            let pcx_objects::Value::Int(value) = entry.value else {
                return Err(format!("{alias:?}: noninteger {column} override"));
            };
            return Ok(value as i32);
        }
        if !mods.is_empty() && metadata.is_empty() {
            return Err(format!("{alias:?}: missing {column} override metadata"));
        }
        let cell = self
            .tables
            .object(Family::Ability, base)
            .map_err(|e| e.to_string())?
            .cell(SlkId::AbilityData, column)
            .map_err(|e| e.to_string())?;
        match cell.typed(FieldKind::Int).map_err(|e| e.to_string())? {
            defaults::Value::Integer(n) => {
                i32::try_from(n).map_err(|_| format!("{alias:?}: {column} exceeds integer range"))
            }
            _ => Err(format!(
                "{alias:?}: missing/inapplicable AbilityData/{column}"
            )),
        }
    }
    fn requirements(&self, alias: [u8; 4]) -> Result<Option<String>, String> {
        let (base, mods) = self.base(alias)?;
        if mods.iter().any(|m| self.tables.editor().field(m.field).is_none()
            || self.tables.editor().field(m.field).is_some_and(|f| matches!(&f.source,FieldSource::Profile {key,..} if key.eq_ignore_ascii_case("Requires") || key.eq_ignore_ascii_case("Requiresamount")))) {
            return Ok(Some("unknown or requirement-changing map overrides".into()));
        }
        let object = self
            .tables
            .object(Family::Ability, base)
            .map_err(|e| e.to_string())?;
        let requirements = object.profile("Requires");
        if requirements.iter().any(|c| c.parse_error.is_some()) {
            return Err(format!("{alias:?}: malformed Requires profile"));
        }
        if requirements
            .iter()
            .any(|c| !matches!(c.text.trim(), "" | "_" | "-"))
        {
            return Ok(Some(
                requirements
                    .iter()
                    .map(|c| c.text)
                    .collect::<Vec<_>>()
                    .join(";"),
            ));
        }
        Ok(None)
    }
}

impl ObjectHost {
    /// Whether hero skills were admitted, so ability ranks and AbilityData are readable.
    pub fn has_hero_skills(&self) -> bool {
        self.units.as_ref().is_some_and(|u| u.heroes.is_some())
    }

    /// Presentation of a learned, player-available ability; orders still validate cast readiness.
    pub fn ability_button(
        &self,
        unit: HandleId,
        ability: [u8; 4],
    ) -> Result<Option<crate::ability_profile::Button>, Error> {
        let record = self.executed_unit(unit)?;
        let rank = record
            .abilities
            .as_ref()
            .map_or(0, |abilities| abilities.level(ability));
        if rank <= 0 || !self.unit_ability_available(unit, ability)? {
            return Ok(None);
        }
        let input = self
            .heroes_admission()
            .map_err(|detail| self.hero_error(ABILITY, detail))?;
        let position =
            crate::ability_profile::button_position(&input.tables, &input.overlay, ability)
                .map_err(|detail| self.hero_error(ABILITY, detail))?;
        let art = self
            .hero_ability_profile(ability, "Art")
            .map_err(|detail| self.hero_error(ABILITY, detail))?;
        Ok(Some(crate::ability_profile::Button {
            ability,
            rank,
            art,
            position,
        }))
    }

    /// Exact ranked ability cell, including the matching W3A level override.
    pub(crate) fn hero_ability_level_cell(
        &self,
        alias: [u8; 4],
        stem: &str,
        rank: i32,
        kind: FieldKind,
    ) -> Result<String, String> {
        if rank < 1 {
            return Err("ability rank must be positive".into());
        }
        let input = self.heroes_admission()?;
        let (base, mods) = input.base(alias)?;
        for entry in mods.iter().rev() {
            let field =
                input.tables.editor().field(entry.field).ok_or_else(|| {
                    format!("{alias:?}: unknown ability override {:?}", entry.field)
                })?;
            let FieldSource::Column {
                slk: SlkId::AbilityData,
                column,
            } = &field.source
            else {
                continue;
            };
            let metadata = input
                .tables
                .metadata(field.fourcc)
                .ok_or("ability override metadata missing")?;
            let (mapped, data) = if column == "Data" {
                let ordinal = metadata
                    .attribute("data")
                    .ok_or("ability data ordinal missing")?
                    .text
                    .parse::<u32>()
                    .map_err(|_| "invalid ability data ordinal")?;
                let letter = char::from_u32(
                    u32::from(b'A')
                        .checked_add(ordinal.checked_sub(1).ok_or("zero data ordinal")?)
                        .ok_or("ability data ordinal overflow")?,
                )
                .ok_or("invalid data ordinal")?;
                (format!("Data{letter}"), Some(ordinal))
            } else {
                (column.clone(), None)
            };
            if mapped != stem || entry.selectors.is_some_and(|s| s[0] != rank as u32) {
                continue;
            }
            if field.index != -1 {
                return Err(format!("{alias:?}: indexed {stem} override unsupported"));
            }
            if let Some([_, pointer]) = entry.selectors {
                if let Some(ordinal) = data {
                    if pointer != ordinal {
                        return Err(format!(
                            "{alias:?}: {stem} override data pointer {pointer} does not match metadata data column {ordinal}"
                        ));
                    }
                } else if pointer != 0 {
                    return Err(format!("{alias:?}: indexed {stem} override unsupported"));
                }
            }
            if let Some(specific) = metadata.attribute("useSpecific")
                && !matches!(specific.text.trim(), "" | "_" | "-")
                && !specific.text.split(',').any(|id| id.trim() == base)
            {
                return Err(format!(
                    "{alias:?}: {stem} override does not apply to {base}"
                ));
            }
            let valid = matches!(
                (kind, field.kind, entry.value_type),
                (FieldKind::Int, FieldKind::Int, pcx_objects::ValueType::Int)
                    | (
                        FieldKind::Bool,
                        FieldKind::Bool,
                        pcx_objects::ValueType::Int
                    )
                    | (
                        FieldKind::Real | FieldKind::Unreal,
                        FieldKind::Real | FieldKind::Unreal,
                        pcx_objects::ValueType::Real | pcx_objects::ValueType::Unreal
                    )
                    | (
                        FieldKind::Token,
                        FieldKind::Token,
                        pcx_objects::ValueType::Str
                    )
            );
            if !valid {
                return Err(format!("{alias:?}: wrong type for {stem} override"));
            }
            return Ok(match &entry.value {
                pcx_objects::Value::Int(value) => (*value as i32).to_string(),
                pcx_objects::Value::Real(value) if value.is_finite() => value.to_string(),
                pcx_objects::Value::Str(value) => std::str::from_utf8(value)
                    .map_err(|_| format!("{alias:?}: non-UTF8 {stem} override"))?
                    .trim()
                    .to_owned(),
                _ => return Err(format!("{alias:?}: nonfinite {stem} override")),
            });
        }
        let object = input
            .tables
            .object(Family::Ability, base)
            .map_err(|e| e.to_string())?;
        let cell = object
            .level_cell(stem, rank as u32)
            .map_err(|e| e.to_string())?;
        match cell.typed(kind).map_err(|e| e.to_string())? {
            defaults::Value::Integer(_)
            | defaults::Value::Real(_)
            | defaults::Value::Text(_)
            | defaults::Value::Boolean(_) => Ok(cell.text.trim().to_owned()),
            _ => Err(format!("{alias:?}: missing {stem}{rank}")),
        }
    }

    fn heroes_admission(&self) -> Result<&Admission, String> {
        self.units
            .as_ref()
            .and_then(|u| u.heroes.as_ref())
            .ok_or_else(|| "hero skills are not admitted".into())
    }

    /// Integer `AbilityData` column of an ability alias, through the selected tables.
    pub(crate) fn hero_ability_number(&self, alias: [u8; 4], column: &str) -> Result<i32, String> {
        self.heroes_admission()?.number(alias, column)
    }

    /// Non-empty `AbilityFunc`/`AbilityData`/strings profile values under `key`, in authored order.
    pub(crate) fn hero_ability_profiles(
        &self,
        alias: [u8; 4],
        key: &str,
    ) -> Result<Vec<String>, String> {
        let input = self.heroes_admission()?;
        crate::ability_profile::profiles(&input.tables, &input.overlay, alias, key)
    }

    /// The single profile value under `key`; zero or several values refuse.
    pub(crate) fn hero_ability_profile(&self, alias: [u8; 4], key: &str) -> Result<String, String> {
        match self.hero_ability_profiles(alias, key)?.as_slice() {
            [one] => Ok(one.clone()),
            [] => Err(format!("{alias:?}: no {key} profile")),
            _ => Err(format!("{alias:?}: ambiguous {key} profile")),
        }
    }
}

fn ability_list(
    definition: &Definition,
    column: &str,
    maximum: usize,
) -> Result<Vec<[u8; 4]>, String> {
    let mut fields = definition.fields.iter().filter(|(_, f)| {
        f.metadata.as_ref().is_some_and(|m|
        matches!(&m.source,FieldSource::Column {slk:SlkId::UnitAbilities,column:c} if c==column))
    });
    let (id, field) = fields
        .next()
        .ok_or_else(|| format!("missing UnitAbilities/{column}"))?;
    if fields.next().is_some() {
        return Err(format!("ambiguous UnitAbilities/{column}"));
    }
    let metadata = field.metadata.as_ref().expect("selected metadata");
    if metadata.fourcc != *id
        || metadata.from != SlkId::UnitMetaData
        || metadata.index != -1
        || metadata.kind != FieldKind::Token
    {
        return Err(format!("unsupported UnitAbilities/{column} metadata"));
    }
    let text = match &field.value {
        Ok(FieldValue::Cell(text)) => text.as_str(),
        Ok(FieldValue::Override(pcx_objects::Value::Str(text))) => {
            std::str::from_utf8(text).map_err(|_| "invalid ability-list UTF8")?
        }
        _ => return Err(format!("unsupported UnitAbilities/{column}")),
    };
    if matches!(text.trim(), "" | "_" | "-") {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    for alias in text.split(',').map(str::trim) {
        if result.len() >= maximum {
            return Err(format!("UnitAbilities/{column} budget exceeded"));
        }
        let raw =
            one_fourcc(alias).ok_or_else(|| format!("invalid UnitAbilities/{column} alias"))?;
        if !seen.insert(raw) {
            return Err(format!("duplicate UnitAbilities/{column} alias"));
        }
        result
            .try_reserve(1)
            .map_err(|_| "ability-list allocation")?;
        result.push(raw);
    }
    Ok(result)
}
fn unit_class_level(definition: &Definition) -> Result<i32, String> {
    let mut values = definition.fields.iter().filter(|(_, f)| {
        f.metadata.as_ref().is_some_and(|m|
        matches!(&m.source,FieldSource::Column {slk:SlkId::UnitBalance,column} if column=="level"))
    });
    let (id, field) = values.next().ok_or("missing UnitBalance/level")?;
    if values.next().is_some() {
        return Err("ambiguous UnitBalance/level".into());
    }
    let metadata = field.metadata.as_ref().expect("selected metadata");
    if metadata.fourcc != *id
        || metadata.from != SlkId::UnitMetaData
        || metadata.index != -1
        || metadata.kind != FieldKind::Int
    {
        return Err("unsupported UnitBalance/level metadata".into());
    }
    match &field.value {
        Ok(FieldValue::Cell(text)) => text
            .trim()
            .parse()
            .map_err(|_| "invalid UnitBalance/level".into()),
        Ok(FieldValue::Override(pcx_objects::Value::Int(value))) => Ok(*value as i32),
        _ => Err("unsupported UnitBalance/level".into()),
    }
}

#[path = "unit_hero_attribute.rs"]
mod attributes;
