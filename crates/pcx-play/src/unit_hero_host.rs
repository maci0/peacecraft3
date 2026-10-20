//! Ability learning on the existing executed unit, using selected stock/map data.
use super::*;
use crate::hero_skills::Rules;
use pcx_objects::editor::Overlay;
use std::collections::BTreeSet;

const LEARN: &str = "SelectHeroSkill";
const LEVEL: &str = "GetHeroLevel";
const POINTS: &str = "GetHeroSkillPoints";
const ABILITY: &str = "GetUnitAbilityLevel";

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
}
impl UnitAbilities {
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
}

impl ObjectHost {
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
        let mut present = [false; 4];
        for (index, (name, args, result)) in [
            (LEARN, &["unit", "integer"][..], "nothing"),
            (LEVEL, &["unit"][..], "integer"),
            (POINTS, &["unit"][..], "integer"),
            (ABILITY, &["unit", "integer"][..], "integer"),
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
        let [learn, level, points, ability] = present;
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
        self.hero_event_guard(
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
        let Entry::Unit { record } = self.inner.entry_mut(*unit)? else {
            unreachable!()
        };
        let state = record.abilities.as_mut().expect("admitted");
        if let Some((_, rank)) = state.levels.iter_mut().find(|(a, _)| *a == alias) {
            *rank += 1;
        } else {
            state.levels.push((alias, first_rank));
        }
        state.hero.as_mut().expect("hero").skill_points -= 1;
        Ok(Value::Null)
    }

    fn hero_event_guard(
        &self,
        unit: HandleId,
        name: &str,
        unit_global: &str,
        player_global: &str,
    ) -> Result<(), Error> {
        let owner = self.current_unit_owner(unit, name)?.0;
        let event =
            self.unit_event_enumerand(name, unit_global, "unitevent", "ConvertUnitEvent")?;
        let player_event = self.unit_event_enumerand(
            name,
            player_global,
            "playerunitevent",
            "ConvertPlayerUnitEvent",
        )?;
        if self.inner.arena.iter().any(|entry| {
            let trigger = match entry {
                Entry::RegisteredUnitEvent {trigger,unit: watched,enumerand,..}
                    if *watched == unit && event.is_none_or(|event| event == *enumerand) => Some(*trigger),
                Entry::RegisteredPlayerUnitEvent(registration)
                    if registration.player == owner && player_event.is_none_or(|event| event == registration.enumerand) => Some(registration.trigger),
                _ => None,
            };
            trigger.is_some_and(|id| matches!(self.inner.entry(id), Ok(Entry::Trigger {enabled}) if *enabled != Some(false)))
        }) {
            return Err(self.hero_error(name, "matching or unqualified hero listeners require event delivery".into()));
        }
        Ok(())
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
        self.hero_event_guard(
            *unit,
            SET_HERO_LEVEL,
            "EVENT_UNIT_HERO_LEVEL",
            "EVENT_PLAYER_HERO_LEVEL",
        )?;
        let Entry::Unit { record } = self.inner.entry_mut(*unit)? else {
            unreachable!()
        };
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
        Ok(Value::Null)
    }
}

impl Admission {
    fn base(&self, alias: [u8; 4]) -> Result<(&str, &[pcx_objects::Mod]), String> {
        let alias = std::str::from_utf8(&alias).map_err(|_| "non-UTF8 ability alias")?;
        let (base, mods) = self
            .overlay
            .new_object(alias)
            .unwrap_or((alias, self.overlay.on(alias)));
        if self.overlay.new_object(base).is_some() {
            return Err("custom ability base chain unsupported".into());
        }
        // Returned base borrows the overlay for custom rows, or must borrow the table's spelling.
        let object = self
            .tables
            .object(Family::Ability, base)
            .map_err(|e| e.to_string())?;
        if object.rawcode() != base {
            return Err("ability rawcode spelling differs from the selected row".into());
        }
        Ok((object.rawcode(), mods))
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
