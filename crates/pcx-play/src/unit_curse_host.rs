//! Slow (`Aslo`), Polymorph (`Aply`) and Dark Conversion (`ANdc`): single-target spells with a
//! timed buff.
//!
//! Which rows are these spells is the implementation's identity: an ability binds when its
//! parent row's `code` is [`SLOW_CLASS`], [`POLYMORPH_CLASS`] or [`CONVERSION_CLASS`]. Everything else is read
//! per learned rank from the selected tables and map edits:
//!
//! - order: the row's `Order` profile, resolved through the selected order registry;
//! - `Cost`, `Cool`, `Rng`, `targs`, `Cast`, `Dur`, `HeroDur`, `BuffID`, `Requires`
//!   and `Requiresamount`;
//! - the effect columns, chosen by the `data` ordinal of the metadata rows `Slo1`/`Slo2`
//!   (movement speed factor, attack speed factor) and `Ply1`..`Ply5` (maximum creep level,
//!   then the ground, air, amphibious and water morph units). The roles are the editor
//!   strings `WESTRING_AEVAL_SLO1`/`SLO2`/`PLY1`..`PLY5` of the selected install.
//! - the cast point: the caster's `UnitWeapons` `castpt`.
//!
//! A nonzero `Cast` refuses the cast, because the ranked casting time of a non-channeled
//! spell is not decoded. Orders replace each other as for the other spell hosts. The caster
//! walks into `Rng` (center distance, no collision radii), faces the target, plays `spell`
//! and applies the effect at the cast point. Mana, cooldown and the spell-effect events are
//! taken then. Backswing (`castbsw`) is not modelled: orders cancel it at once.
//!
//! Slow multiplies the target's move speed by `1 - factor` (clamped by the selected speed
//! limits) and subtracts the attack factor from the attack speed multiplier the combat step
//! already uses (`1 + agility bonus`). Both are recreation readings of the tooltip wording
//! ("attack rate", "movement speed" reduced by a percentage), not native formulas. Duration
//! is `Dur`, or `HeroDur` for a hero target. A recast refreshes the effect.
//!
//! Polymorph replaces the target's definition, model, movement class and speed, weapon and
//! owned abilities with the ground morph unit, and keeps life, mana, armor and defense type
//! (the tooltip says hit points and armor are kept). The original is restored at expiry or
//! when `UnitRemoveBuffs` removes the buff. Air, amphibious and water forms, hero targets,
//! targets that hold items, morph or mirror state, and a slowed target refuse loudly.
//!
//! Dark Conversion reads the `data` ordinals of `Ndc1` (the target race, compared with the
//! target's `UnitData` `race`) and `Ndc2` (the unit). The buff and its `Targetart` stay on the
//! target for `Dur` (`HeroDur` for a hero, which `nonhero` targets exclude). At expiry the
//! target is removed and the unit is created at its position and facing for the caster's owner,
//! and `EVENT_UNIT_SUMMON` fires with the caster as summoner. Readings, not native evidence: that
//! `Dur` is the delay before the conversion, that the new unit belongs to the caster's owner (the
//! shipped Human05 triggers listen on the caster owner's summon event and then reassign it),
//! that a `targs` list without an alliance token admits every relation, and that a hidden
//! caster casts (the shipped script hides the caster before ordering). The new unit has no
//! lifetime. A caster removed before the conversion ends refuses loudly.
//!
//! The buff is attached through the unit buff host with the data's `BuffID`. Its dispel
//! class is not established, so only the negative flag is set.
use super::unit_buff::{ActiveBuff, BuffTraits};
use super::unit_creation::FormChange;
use super::{Entry, Error, EventSubject, Failure, ObjectHost, combat};
use pcx_objects::editor::{FieldKind, SlkId};
use pcx_script::{HandleId, Value};
use std::collections::BTreeMap;

/// Parent-row `code` of the Slow class.
pub(super) const SLOW_CLASS: [u8; 4] = *b"Aslo";
/// Parent-row `code` of the Polymorph class.
pub(super) const POLYMORPH_CLASS: [u8; 4] = *b"Aply";
/// Parent-row `code` of the Dark Conversion class.
pub(super) const CONVERSION_CLASS: [u8; 4] = *b"ANdc";
/// Metadata rows of the movement and attack factors.
const SLOW_FIELDS: [[u8; 4]; 2] = [*b"Slo1", *b"Slo2"];
/// Metadata rows of the maximum creep level and the ground, air, amphibious and water forms.
const POLYMORPH_FIELDS: [[u8; 4]; 5] = [*b"Ply1", *b"Ply2", *b"Ply3", *b"Ply4", *b"Ply5"];
/// Metadata rows of the convertible race and the created unit.
const CONVERSION_FIELDS: [[u8; 4]; 2] = [*b"Ndc1", *b"Ndc2"];
/// Metadata `field` of a spell's unit column.
const UNIT_ID_FIELD: &str = "UnitID";
const NATIVE: &str = "IssueTargetOrder";
/// `UnitData` `movetp` tokens whose morph slot is the ground form. Inference from the editor
/// label "Morph Units - Ground": every stock `hover` row has `targType` ground. The other slots
/// (air, amphibious, water) refuse.
const GROUND_MOVEMENT: [&str; 3] = ["foot", "horse", "hover"];
const CAST_SEQUENCE: &str = "spell";
const STAND_SEQUENCE: &str = "stand";
/// Attach point of the buff `Targetart` when the buff row authors none. Inference: the model root.
const DEFAULT_ATTACH: &str = "origin";
const TARGET_TOKENS: [&str; 13] = [
    "air",
    "ground",
    "enemy",
    "neutral",
    "friend",
    "self",
    "notself",
    "organic",
    "nonhero",
    "nonancient",
    "invu",
    "vuln",
    "structure",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Slow,
    Polymorph,
    DarkConversion,
}

impl Kind {
    const fn class(self) -> [u8; 4] {
        match self {
            Self::Slow => SLOW_CLASS,
            Self::Polymorph => POLYMORPH_CLASS,
            Self::DarkConversion => CONVERSION_CLASS,
        }
    }

    const fn fields(self) -> &'static [[u8; 4]] {
        match self {
            Self::Slow => &SLOW_FIELDS,
            Self::Polymorph => &POLYMORPH_FIELDS,
            Self::DarkConversion => &CONVERSION_FIELDS,
        }
    }
}

/// `Data` column stems chosen from the metadata ordinals.
#[derive(Clone)]
struct Columns {
    movement: String,
    attack: String,
    creep_level: String,
    ground_form: String,
    conversion_race: String,
    conversion_unit: String,
}

enum Stage {
    Walking,
    Casting { effect_at: f64 },
    Backswing,
}

struct Cast {
    alias: [u8; 4],
    kind: Kind,
    target: HandleId,
    stage: Stage,
    /// Sequence index and start of the cast animation, to return to stand when it is unchanged.
    animation: Option<(usize, f64)>,
}

struct ActiveSlow {
    buff: [u8; 4],
    /// Move speed before the slow, when the unit can move.
    base_speed: Option<f32>,
    attack: f32,
    expires_at: f64,
    /// Definition revision the base speed belongs to.
    revision: u64,
    art: Option<HandleId>,
}

struct ActiveConversion {
    caster: HandleId,
    buff: [u8; 4],
    unit: [u8; 4],
    expires_at: f64,
    art: Option<HandleId>,
}

struct ActivePolymorph {
    buff: [u8; 4],
    expires_at: f64,
    original: pcx_sim::world::Definition,
    abilities: Option<super::UnitAbilities>,
    profile: (bool, Option<f32>),
}

pub(super) struct State {
    columns: Columns,
    orders: BTreeMap<i32, Vec<([u8; 4], Kind)>>,
    ready_at: BTreeMap<(HandleId, [u8; 4]), f64>,
    pending: BTreeMap<HandleId, Cast>,
    slows: BTreeMap<HandleId, ActiveSlow>,
    polymorphs: BTreeMap<HandleId, ActivePolymorph>,
    conversions: BTreeMap<HandleId, ActiveConversion>,
}

struct Filter {
    air: bool,
    ground: bool,
    enemy: bool,
    neutral: bool,
    friend: bool,
    /// No alliance token at all.
    any_relation: bool,
    this: bool,
    organic: bool,
    nonhero: bool,
    nonancient: bool,
    invulnerable: bool,
    vulnerable: bool,
    structure: bool,
}

enum Effect {
    Slow {
        movement: f32,
        attack: f32,
    },
    Polymorph {
        max_creep_level: i32,
        ground_form: [u8; 4],
    },
    DarkConversion {
        race: String,
        unit: [u8; 4],
    },
}

struct Requirement {
    techid: i32,
    level: Option<i32>,
}

struct Spell {
    cost: f32,
    cooldown: f64,
    range: f32,
    duration: f64,
    hero_duration: f64,
    buff: [u8; 4],
    filter: Filter,
    requirements: Vec<Requirement>,
    effect: Effect,
}

struct TargetFacts {
    flying: bool,
    mechanical: bool,
    ancient: bool,
    structure: bool,
    hero: bool,
    protected: bool,
}

fn target_filter(targs: &str, kind: Kind) -> Result<Filter, String> {
    let tokens: Vec<&str> = targs
        .split(',')
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .collect();
    if let Some(unknown) = tokens.iter().find(|token| !TARGET_TOKENS.contains(token)) {
        return Err(format!("unsupported curse target token {unknown}"));
    }
    let has = |token: &str| tokens.contains(&token);
    let related = has("enemy") || has("neutral") || has("friend");
    if !(has("air") || has("ground")) || !(related || kind == Kind::DarkConversion) {
        return Err("curse targets need a movement token and an alliance token".into());
    }
    Ok(Filter {
        air: has("air"),
        ground: has("ground"),
        enemy: has("enemy"),
        neutral: has("neutral"),
        friend: has("friend"),
        any_relation: !related,
        this: has("self"),
        organic: has("organic"),
        nonhero: has("nonhero"),
        nonancient: has("nonancient"),
        invulnerable: has("invu"),
        vulnerable: has("vuln"),
        structure: has("structure"),
    })
}

fn rawcode(text: &str) -> Result<[u8; 4], String> {
    let bytes = text.trim().as_bytes();
    <[u8; 4]>::try_from(bytes).map_err(|_| format!("{:?} is not a four-byte rawcode", text.trim()))
}

fn rawcodes(text: &str) -> Result<Vec<[u8; 4]>, String> {
    let mut codes = Vec::new();
    for part in text.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let code = rawcode(part)?;
        if !codes.contains(&code) {
            codes.push(code);
        }
    }
    Ok(codes)
}

fn data_stem(ordinal: u32) -> Result<String, String> {
    let offset = ordinal
        .checked_sub(1)
        .filter(|offset| *offset <= 25)
        .ok_or("data ordinal is outside DataA..DataZ")?;
    Ok(format!("Data{}", char::from(b'A' + offset as u8)))
}

/// `Requires` tokens paired with `Requiresamount` levels. A missing amount list means the
/// upgrade's own research threshold.
fn requirements(required: &[String], amounts: &[String]) -> Result<Vec<Requirement>, String> {
    let split = |values: &[String]| -> Vec<String> {
        values
            .iter()
            .flat_map(|value| value.split(','))
            .map(str::trim)
            .filter(|token| !matches!(*token, "" | "_" | "-"))
            .map(str::to_owned)
            .collect()
    };
    let (required, amounts) = (split(required), split(amounts));
    if !amounts.is_empty() && amounts.len() != required.len() {
        return Err("Requiresamount does not pair with Requires".into());
    }
    required
        .iter()
        .enumerate()
        .map(|(at, token)| {
            let level = amounts
                .get(at)
                .map(|amount| {
                    amount
                        .parse::<i32>()
                        .map_err(|_| format!("Requiresamount {amount:?} is not an integer"))
                })
                .transpose()?;
            Ok(Requirement {
                techid: i32::from_be_bytes(rawcode(token)?),
                level,
            })
        })
        .collect()
}

impl ObjectHost {
    fn curse_error(&self, reason: impl Into<String>) -> Error {
        self.inner.error(NATIVE, Failure::HeroSkills(reason.into()))
    }

    /// Metadata row of one effect field: its `Data` stem, checked against the class and type.
    fn curse_field(&self, id: [u8; 4], kind: Kind, wanted: &str) -> Result<String, Error> {
        let text = |column: &str| {
            self.ability_metadata_text(id, column)
                .map_err(|e| self.curse_error(e))?
                .ok_or_else(|| self.curse_error(format!("{id:?} metadata has no {column}")))
        };
        let ordinal = text("data")?
            .parse::<u32>()
            .map_err(|_| self.curse_error(format!("{id:?} data ordinal is not an integer")))?;
        let declared = text("type")?;
        if !declared.eq_ignore_ascii_case(wanted) {
            return Err(self.curse_error(format!("{id:?} type {declared} is not {wanted}")));
        }
        let specific = rawcodes(&text("useSpecific")?).map_err(|e| self.curse_error(e))?;
        if !specific.contains(&kind.class()) {
            return Err(self.curse_error(format!("{id:?} does not list {:?}", kind.class())));
        }
        // The created unit lives in the `UnitID` column, not in a `Data` letter.
        if text("field")? == UNIT_ID_FIELD && ordinal == 0 {
            return Ok(UNIT_ID_FIELD.to_owned());
        }
        data_stem(ordinal).map_err(|e| self.curse_error(format!("{id:?}: {e}")))
    }

    fn curse_class(&self, alias: [u8; 4]) -> Result<[u8; 4], Error> {
        let parent = self
            .ability_parent_code(alias)
            .map_err(|e| self.curse_error(e))?;
        let code = self
            .ability_data_text(parent, "code")
            .map_err(|e| self.curse_error(e))?;
        rawcodes(&code)
            .map_err(|e| self.curse_error(e))?
            .first()
            .copied()
            .ok_or_else(|| self.curse_error(format!("{alias:?} has no code")))
    }

    /// Slow, Polymorph and Dark Conversion, once. Needs combat (dispatch), hero skills (ranked reads) and unit
    /// effects (the buff art).
    pub fn admit_unit_curses(&mut self) -> Result<(), Error> {
        if self.curses.is_some() || self.combat.is_none() || !self.has_hero_skills() {
            return Err(self.curse_error("Slow and Polymorph need combat and hero skills, once"));
        }
        let [movement, attack] = SLOW_FIELDS;
        let [creep, ground, air, amphibious, water] = POLYMORPH_FIELDS;
        let [race, created] = CONVERSION_FIELDS;
        let columns = Columns {
            movement: self.curse_field(movement, Kind::Slow, "unreal")?,
            attack: self.curse_field(attack, Kind::Slow, "unreal")?,
            creep_level: self.curse_field(creep, Kind::Polymorph, "int")?,
            ground_form: self.curse_field(ground, Kind::Polymorph, "unitList")?,
            conversion_race: self.curse_field(race, Kind::DarkConversion, "unitRace")?,
            conversion_unit: self.curse_field(created, Kind::DarkConversion, "unitCode")?,
        };
        // The other forms are refused, but their rows must still describe the same class.
        for other in [air, amphibious, water] {
            self.curse_field(other, Kind::Polymorph, "unitList")?;
        }
        let mut orders: BTreeMap<i32, Vec<([u8; 4], Kind)>> = BTreeMap::new();
        for kind in [Kind::Slow, Kind::Polymorph, Kind::DarkConversion] {
            let listed = self
                .ability_metadata_text(kind.fields()[0], "useSpecific")
                .map_err(|e| self.curse_error(e))?
                .unwrap_or_default();
            let mut aliases = rawcodes(&listed).map_err(|e| self.curse_error(e))?;
            for (alias, _) in self
                .custom_ability_aliases()
                .map_err(|e| self.curse_error(e))?
            {
                aliases.push(alias);
            }
            for alias in aliases {
                if self.curse_class(alias)? != kind.class() {
                    continue;
                }
                let name = self
                    .hero_ability_profile(alias, "Order")
                    .map_err(|e| self.curse_error(e))?;
                let order = self
                    .order_registry
                    .as_ref()
                    .and_then(|registry| registry.id(&name))
                    .filter(|id| *id != -1)
                    .ok_or_else(|| self.curse_error(format!("order {name} is not registered")))?;
                let bound = orders.entry(order).or_default();
                if !bound.contains(&(alias, kind)) {
                    bound.push((alias, kind));
                }
            }
        }
        self.curses = Some(State {
            columns,
            orders,
            ready_at: BTreeMap::new(),
            pending: BTreeMap::new(),
            slows: BTreeMap::new(),
            polymorphs: BTreeMap::new(),
            conversions: BTreeMap::new(),
        });
        Ok(())
    }

    pub(super) fn curse_order_claimed(&self, order: i32) -> bool {
        self.curses
            .as_ref()
            .is_some_and(|state| state.orders.contains_key(&order))
    }

    pub(crate) fn curse_alias_is_bound(&self, alias: [u8; 4]) -> bool {
        self.curses
            .as_ref()
            .is_some_and(|state| state.orders.values().flatten().any(|(a, _)| *a == alias))
    }

    /// The learned bound ability that `order` names for this caster.
    fn curse_alias_for_order(
        &self,
        unit: HandleId,
        order: i32,
    ) -> Result<Option<([u8; 4], Kind)>, Error> {
        let Some(candidates) = self
            .curses
            .as_ref()
            .and_then(|state| state.orders.get(&order))
            .cloned()
        else {
            return Ok(None);
        };
        let Some(abilities) = self.executed_unit(unit)?.abilities.as_ref() else {
            return Ok(None);
        };
        let mut matched = Vec::new();
        for (alias, kind) in candidates {
            if abilities.level(alias) > 0 && self.unit_ability_available(unit, alias)? {
                matched.push((alias, kind));
            }
        }
        match matched.as_slice() {
            [] => Ok(None),
            [one] => Ok(Some(*one)),
            _ => Err(self.curse_error("more than one learned curse shares this order")),
        }
    }

    /// The learned rank's values, or `None` when the caster has no learned rank of `alias`.
    fn curse_spell(
        &self,
        caster: HandleId,
        alias: [u8; 4],
        kind: Kind,
    ) -> Result<Option<Spell>, Error> {
        let Some(unit) = self.curse_caster(caster) else {
            return Ok(None);
        };
        let Some(columns) = self.curses.as_ref().map(|state| state.columns.clone()) else {
            return Ok(None);
        };
        let rank = unit.abilities.as_ref().map_or(0, |a| a.level(alias));
        if rank <= 0 || !self.unit_ability_available(caster, alias)? {
            return Ok(None);
        }
        let levels = self
            .hero_ability_number(alias, "levels")
            .map_err(|e| self.curse_error(e))?;
        if rank > levels {
            return Err(self.curse_error(format!("rank {rank} outside {levels} levels")));
        }
        let cell = |stem: &str, field: FieldKind| {
            self.hero_ability_level_cell(alias, stem, rank, field)
                .map_err(|e| self.curse_error(e))
        };
        let real = |stem: &str, field: FieldKind| -> Result<f32, Error> {
            let value = cell(stem, field)?
                .parse::<f32>()
                .map_err(|e| self.curse_error(format!("invalid {stem}{rank}: {e}")))?;
            if !value.is_finite() {
                return Err(self.curse_error(format!("{stem}{rank} must be finite")));
            }
            Ok(value)
        };
        let nonnegative = |stem: &str, field: FieldKind| -> Result<f32, Error> {
            let value = real(stem, field)?;
            if value < 0.0 {
                return Err(self.curse_error(format!("{stem}{rank} must be nonnegative")));
            }
            Ok(value)
        };
        if real("Cast", FieldKind::Unreal)? != 0.0 {
            return Err(self.curse_error(format!(
                "Cast{rank} is not zero; ranked casting time is not decoded"
            )));
        }
        let duration = nonnegative("Dur", FieldKind::Unreal)?;
        let hero_duration = nonnegative("HeroDur", FieldKind::Unreal)?;
        if duration <= 0.0 || hero_duration <= 0.0 {
            return Err(self.curse_error(format!("Dur{rank} and HeroDur{rank} must be positive")));
        }
        let buff = rawcode(&cell("BuffID", FieldKind::Token)?).map_err(|e| self.curse_error(e))?;
        if !self
            .ability_buff_exists(buff)
            .map_err(|e| self.curse_error(e))?
        {
            return Err(self.curse_error(format!("buff {buff:?} is not in the selected tables")));
        }
        let targs = cell("targs", FieldKind::Token)?;
        let required = self
            .hero_ability_profiles(alias, "Requires")
            .map_err(|e| self.curse_error(e))?;
        let amounts = self
            .hero_ability_profiles(alias, "Requiresamount")
            .map_err(|e| self.curse_error(e))?;
        let effect = match kind {
            Kind::Slow => Effect::Slow {
                movement: real(&columns.movement, FieldKind::Unreal)?,
                attack: real(&columns.attack, FieldKind::Unreal)?,
            },
            Kind::Polymorph => Effect::Polymorph {
                max_creep_level: real(&columns.creep_level, FieldKind::Int)? as i32,
                ground_form: rawcode(&cell(&columns.ground_form, FieldKind::Token)?)
                    .map_err(|e| self.curse_error(e))?,
            },
            Kind::DarkConversion => Effect::DarkConversion {
                race: cell(&columns.conversion_race, FieldKind::Token)?
                    .trim()
                    .to_owned(),
                unit: rawcode(&cell(&columns.conversion_unit, FieldKind::Token)?)
                    .map_err(|e| self.curse_error(e))?,
            },
        };
        Ok(Some(Spell {
            cost: nonnegative("Cost", FieldKind::Int)?,
            cooldown: f64::from(self.ability_cooldown_duration(caster, alias, rank)?),
            range: nonnegative("Rng", FieldKind::Unreal)?,
            duration: f64::from(duration),
            hero_duration: f64::from(hero_duration),
            buff,
            filter: target_filter(&targs, kind).map_err(|e| self.curse_error(e))?,
            requirements: requirements(&required, &amounts).map_err(|e| self.curse_error(e))?,
            effect,
        }))
    }

    fn curse_target_facts(&self, target: HandleId) -> Result<Option<TargetFacts>, Error> {
        let Some(unit) = self.live(target) else {
            return Ok(None);
        };
        let definition = &unit.definition;
        let flying = super::unit_positions::text(definition, SlkId::UnitData, "movetp")
            .unwrap_or("")
            == "fly";
        let structure = match super::unit_types::structure(definition) {
            Ok(value) => value,
            Err(detail) if detail.contains("missing") => false,
            Err(detail) => return Err(self.curse_error(detail)),
        };
        Ok(Some(TargetFacts {
            flying,
            mechanical: super::unit_types::class(definition, "UNIT_TYPE_MECHANICAL")
                .map_err(|e| self.curse_error(e))?,
            ancient: super::unit_types::class(definition, "UNIT_TYPE_ANCIENT")
                .map_err(|e| self.curse_error(e))?,
            structure,
            hero: unit.abilities.as_ref().is_some_and(|a| a.hero.is_some()),
            protected: self.weapon_target(target).is_none(),
        }))
    }

    fn curse_eligible(
        &self,
        caster: HandleId,
        target: HandleId,
        spell: &Spell,
    ) -> Result<bool, Error> {
        let Some(facts) = self.curse_target_facts(target)? else {
            return Ok(false);
        };
        let filter = &spell.filter;
        if caster == target && !filter.this {
            return Ok(false);
        }
        let stance_ok = if filter.invulnerable || filter.vulnerable {
            (facts.protected && filter.invulnerable) || (!facts.protected && filter.vulnerable)
        } else {
            !facts.protected
        };
        let related = filter.any_relation
            || (filter.enemy && self.is_enemy(caster, target))
            || (filter.neutral && !self.is_enemy(caster, target) && !self.is_ally(caster, target))
            || (filter.friend && self.is_ally(caster, target));
        if !stance_ok
            || !related
            || (facts.flying && !filter.air)
            || (!facts.flying && !filter.ground)
            || (facts.structure && !filter.structure)
            || (filter.organic && facts.mechanical)
            || (filter.nonhero && facts.hero)
            || (filter.nonancient && facts.ancient)
        {
            return Ok(false);
        }
        let owner = self.executed_unit(caster)?.owner;
        for need in &spell.requirements {
            if !self.technology_requirement_met(owner, need.techid, need.level)? {
                return Ok(false);
            }
        }
        if let Effect::DarkConversion { race, .. } = &spell.effect {
            let target_race = super::unit_positions::text(
                &self.executed_unit(target)?.definition,
                SlkId::UnitData,
                "race",
            )
            .map_err(|e| self.curse_error(format!("UnitData/race: {e}")))?;
            if target_race != race {
                return Ok(false);
            }
        }
        if let Effect::Polymorph {
            max_creep_level, ..
        } = spell.effect
        {
            let aggressive = self
                .combat
                .as_ref()
                .and_then(|rules| rules.neutral_aggressive);
            let target_unit = self.executed_unit(target)?;
            if aggressive == Some(self.player_settings(target_unit.owner)?.number) {
                let level = combat::column(&target_unit.definition, SlkId::UnitBalance, "level")
                    .map_err(|e| self.curse_error(e))?
                    .map(|cell| combat::number(cell, "level"))
                    .transpose()
                    .map_err(|e| self.curse_error(e))?
                    .flatten()
                    .ok_or_else(|| self.curse_error("creep target has no level"))?;
                if level > max_creep_level as f32 {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    fn curse_unready(&self, caster: HandleId, alias: [u8; 4], spell: &Spell, now: f64) -> bool {
        let Some(state) = self.curses.as_ref() else {
            return true;
        };
        let Some(unit) = self.curse_caster(caster) else {
            return true;
        };
        !Self::ability_readiness_at(
            unit,
            spell.cost,
            state.ready_at.get(&(caster, alias)).copied(),
            now,
        )
        .ready()
    }

    /// `IssueTargetOrder` for a bound curse. Accepted when the caster has a learned rank and can
    /// afford, target and ready it now; the cast then walks, casts and applies the effect.
    pub(super) fn order_curse(
        &mut self,
        caster: HandleId,
        target: HandleId,
        order: i32,
    ) -> Result<bool, Error> {
        if self.executed_unit(caster)?.paused_since.is_some() {
            return Ok(false);
        }
        let Some((alias, kind)) = self.curse_alias_for_order(caster, order)? else {
            return Ok(false);
        };
        let now = self
            .timer_game_time()
            .ok_or_else(|| self.curse_error("clock missing"))?;
        let Some(spell) = self.curse_spell(caster, alias, kind)? else {
            return Ok(false);
        };
        if !self.curse_eligible(caster, target, &spell)?
            || self.curse_unready(caster, alias, &spell, now)
        {
            return Ok(false);
        }
        self.unit_ordered(caster);
        self.cancel_inventory_order(caster);
        self.clear_combat_target(caster);
        if let Some(state) = &mut self.curses {
            state.pending.insert(
                caster,
                Cast {
                    alias,
                    kind,
                    target,
                    stage: Stage::Walking,
                    animation: None,
                },
            );
        }
        self.step_curse_cast(caster, now)?;
        self.cancel_patrol(caster);
        Ok(true)
    }

    pub(super) fn curse_current_order(&self, unit: HandleId) -> Option<i32> {
        let state = self.curses.as_ref()?;
        let cast = state.pending.get(&unit)?;
        state
            .orders
            .iter()
            .find(|(_, bound)| bound.iter().any(|(alias, _)| *alias == cast.alias))
            .map(|(order, _)| *order)
    }

    /// Drops a pending cast when its caster takes another order, dies or is removed.
    pub(super) fn forget_curse_cast(&mut self, caster: HandleId) {
        if let Some(state) = &mut self.curses {
            state.pending.remove(&caster);
        }
    }

    pub(super) fn resume_curse_cast(&mut self, caster: HandleId, elapsed: f64) {
        if let Some(Cast {
            stage: Stage::Casting { effect_at },
            ..
        }) = self
            .curses
            .as_mut()
            .and_then(|state| state.pending.get_mut(&caster))
        {
            *effect_at += elapsed;
        }
    }

    pub(super) fn clear_curse_cooldowns(&mut self, unit: HandleId) {
        if let Some(state) = &mut self.curses {
            state.ready_at.retain(|(id, _), _| *id != unit);
        }
    }

    /// Remaining-cooldown and end-cooldown natives for a bound alias; `None` means another ability.
    pub(super) fn curse_bound_cooldown(
        &mut self,
        unit: HandleId,
        alias: [u8; 4],
        end: bool,
    ) -> Result<Option<pcx_script::Value>, Error> {
        use pcx_script::Value;
        if !self.curse_alias_is_bound(alias) {
            return Ok(None);
        }
        let state = self.curses.as_mut().expect("bound alias has state");
        if end {
            state.ready_at.remove(&(unit, alias));
            return Ok(Some(Value::Null));
        }
        let Some(deadline) = state.ready_at.get(&(unit, alias)).copied() else {
            return Ok(Some(Value::Real(0.0)));
        };
        let now = self
            .timer_game_time()
            .ok_or_else(|| self.curse_error("clock missing"))?;
        Ok(Some(Value::Real((deadline - now).max(0.0) as f32)))
    }

    /// Attack speed fraction removed from the unit by active slows.
    pub(crate) fn curse_attack_penalty(&self, unit: HandleId) -> f32 {
        self.curses
            .as_ref()
            .and_then(|state| state.slows.get(&unit))
            .map_or(0.0, |slow| slow.attack)
    }

    /// Whether an active slow owns the unit's move speed.
    pub(super) fn unit_speed_modified(&self, unit: HandleId) -> bool {
        self.curses
            .as_ref()
            .is_some_and(|state| state.slows.contains_key(&unit))
    }

    pub(super) fn advance_curses(&mut self, now: f64) -> Result<(), Error> {
        let Some(state) = self.curses.as_ref() else {
            return Ok(());
        };
        let casters: Vec<HandleId> = state.pending.keys().copied().collect();
        let slows: Vec<HandleId> = state
            .slows
            .iter()
            .filter(|(_, slow)| slow.expires_at <= now)
            .map(|(unit, _)| *unit)
            .collect();
        let polymorphs: Vec<HandleId> = state
            .polymorphs
            .iter()
            .filter(|(_, polymorph)| polymorph.expires_at <= now)
            .map(|(unit, _)| *unit)
            .collect();
        let conversions: Vec<HandleId> = state
            .conversions
            .iter()
            .filter(|(_, conversion)| conversion.expires_at <= now)
            .map(|(unit, _)| *unit)
            .collect();
        for caster in casters {
            self.step_curse_cast(caster, now)?;
        }
        for unit in slows {
            self.end_slow(unit)?;
        }
        for unit in polymorphs {
            self.end_polymorph(unit)?;
        }
        for unit in conversions {
            self.complete_conversion(unit)?;
        }
        Ok(())
    }

    fn step_curse_cast(&mut self, caster: HandleId, now: f64) -> Result<(), Error> {
        let Some(cast) = self
            .curses
            .as_ref()
            .and_then(|state| state.pending.get(&caster))
        else {
            return Ok(());
        };
        let (alias, kind, target) = (cast.alias, cast.kind, cast.target);
        let stage = match &cast.stage {
            Stage::Walking => None,
            Stage::Casting { effect_at } => Some(Some(*effect_at)),
            Stage::Backswing => Some(None),
        };
        let Some(unit) = self.curse_caster(caster) else {
            self.forget_curse_cast(caster);
            return Ok(());
        };
        if unit.paused_since.is_some() {
            return Ok(());
        }
        match stage {
            None => self.walk_curse_cast(caster, target, alias, kind, now),
            Some(Some(effect_at)) if now >= effect_at => {
                self.apply_curse_cast(caster, target, alias, kind, now)
            }
            Some(Some(_)) => Ok(()),
            Some(None) => self.finish_curse_backswing(caster, now),
        }
    }

    fn cancel_curse_cast(&mut self, caster: HandleId, now: f64) -> Result<(), Error> {
        let animation = self
            .curses
            .as_mut()
            .and_then(|state| state.pending.remove(&caster))
            .and_then(|cast| cast.animation);
        if self.curse_caster(caster).is_some() {
            self.steer_unit(caster, None, NATIVE)?;
            self.reset_cast_animation(caster, animation, now)?;
        }
        Ok(())
    }

    /// Returns the caster to stand when its cast animation is still the one playing.
    fn reset_cast_animation(
        &mut self,
        caster: HandleId,
        animation: Option<(usize, f64)>,
        now: f64,
    ) -> Result<(), Error> {
        let Some((index, started)) = animation else {
            return Ok(());
        };
        let selected = match self.inner.entry_mut(caster)? {
            Entry::Unit { record } => match &mut record.animation {
                Some(current) if current.sequence == Some(index) && current.started == started => {
                    current.select_engine(STAND_SEQUENCE, now, None)
                }
                _ => Ok(()),
            },
            _ => Ok(()),
        };
        selected.map_err(|e| self.curse_error(e))
    }

    fn walk_curse_cast(
        &mut self,
        caster: HandleId,
        target: HandleId,
        alias: [u8; 4],
        kind: Kind,
        now: f64,
    ) -> Result<(), Error> {
        let spell = self.curse_spell(caster, alias, kind)?;
        let usable = match &spell {
            Some(spell) => {
                self.curse_eligible(caster, target, spell)?
                    && !self.curse_unready(caster, alias, spell, now)
            }
            None => false,
        };
        let (true, Some(spell)) = (usable, spell) else {
            return self.cancel_curse_cast(caster, now);
        };
        let (Some(from), Some(to)) = (
            self.curse_caster(caster).and_then(|u| u.position),
            self.live(target).and_then(|u| u.position),
        ) else {
            return Err(self.curse_error("unit position missing"));
        };
        if combat::distance(from, to) > spell.range {
            if !self.curse_moving(caster) {
                self.steer_unit(caster, Some(to), NATIVE)?;
            }
            return Ok(());
        }
        if self.curse_moving(caster) {
            self.steer_unit(caster, None, NATIVE)?;
        }
        let delay = self.curse_cast_point(caster)?;
        let facing = (to[1] - from[1])
            .atan2(to[0] - from[0])
            .to_degrees()
            .rem_euclid(360.0);
        let mut started = None;
        let played = match self.inner.entry_mut(caster)? {
            Entry::Unit { record } => {
                record.face = facing;
                match &mut record.animation {
                    Some(animation) if animation.has_sequence(CAST_SEQUENCE) => {
                        let selected = animation.select_engine(CAST_SEQUENCE, now, None);
                        started = animation.sequence.map(|index| (index, animation.started));
                        selected
                    }
                    _ => Ok(()),
                }
            }
            _ => Ok(()),
        };
        played.map_err(|e| self.inner.error(NATIVE, Failure::UnitAnimation(e)))?;
        if let Some(cast) = self
            .curses
            .as_mut()
            .and_then(|state| state.pending.get_mut(&caster))
        {
            cast.animation = started;
            cast.stage = Stage::Casting {
                effect_at: now + delay,
            };
        }
        if delay <= 0.0 {
            return self.apply_curse_cast(caster, target, alias, kind, now);
        }
        Ok(())
    }

    fn curse_moving(&self, unit: HandleId) -> bool {
        self.curse_caster(unit)
            .and_then(|u| u.activity.as_ref())
            .is_some_and(|a| a.moving())
    }

    /// The caster's `castpt`, in seconds.
    fn curse_cast_point(&self, caster: HandleId) -> Result<f64, Error> {
        let record = self.executed_unit(caster)?;
        let cell = combat::column(&record.definition, SlkId::UnitWeapons, "castpt")
            .map_err(|e| self.curse_error(e))?
            .ok_or_else(|| self.curse_error("caster cast point is absent"))?;
        let delay = combat::number(cell, "castpt")
            .map_err(|e| self.curse_error(e))?
            .map(f64::from)
            .ok_or_else(|| self.curse_error("caster cast point is absent"))?;
        if delay < 0.0 {
            return Err(self.curse_error("caster cast point is negative"));
        }
        Ok(delay)
    }

    fn finish_curse_backswing(&mut self, caster: HandleId, now: f64) -> Result<(), Error> {
        let Some(animation) = self
            .curses
            .as_ref()
            .and_then(|state| state.pending.get(&caster))
            .and_then(|cast| cast.animation)
        else {
            self.forget_curse_cast(caster);
            return Ok(());
        };
        let running = match self.inner.entry(caster)? {
            Entry::Unit { record } => match record.animation.as_ref() {
                Some(current)
                    if current.sequence == Some(animation.0) && current.started == animation.1 =>
                {
                    !current
                        .interval_finished(now)
                        .map_err(|e| self.curse_error(e))?
                }
                _ => false,
            },
            _ => false,
        };
        if !running {
            self.cancel_curse_cast(caster, now)?;
        }
        Ok(())
    }

    /// The cast point: recheck, pay, start the cooldown, apply the effect, queue the events.
    fn apply_curse_cast(
        &mut self,
        caster: HandleId,
        target: HandleId,
        alias: [u8; 4],
        kind: Kind,
        now: f64,
    ) -> Result<(), Error> {
        let spell = self.curse_spell(caster, alias, kind)?;
        let usable = match &spell {
            Some(spell) => {
                self.curse_eligible(caster, target, spell)?
                    && !self.curse_unready(caster, alias, spell, now)
            }
            None => false,
        };
        let (true, Some(spell)) = (usable, spell) else {
            return self.cancel_curse_cast(caster, now);
        };
        let events = self.prepare_ability_events(
            caster,
            "EVENT_UNIT_SPELL_EFFECT",
            "EVENT_PLAYER_UNIT_SPELL_EFFECT",
        )?;
        self.reserve_event_delivery(events.len())?;
        let duration = if self
            .executed_unit(target)?
            .abilities
            .as_ref()
            .is_some_and(|a| a.hero.is_some())
        {
            spell.hero_duration
        } else {
            spell.duration
        };
        let expires_at = now + duration;
        match spell.effect {
            Effect::Slow { movement, attack } => {
                self.apply_slow(target, spell.buff, movement, attack, expires_at)?;
            }
            Effect::Polymorph { ground_form, .. } => {
                self.apply_polymorph(target, spell.buff, expires_at, ground_form)?;
            }
            Effect::DarkConversion { unit, .. } => {
                self.apply_conversion(caster, target, spell.buff, unit, expires_at)?;
            }
        }
        if let Some(state) = &mut self.curses {
            state.ready_at.insert((caster, alias), now + spell.cooldown);
        }
        if let Entry::Unit { record } = self.inner.entry_mut(caster)?
            && let Some(vitals) = &mut record.vitals
        {
            vitals.mana -= spell.cost;
        }
        self.commit_ability_events(
            events,
            EventSubject {
                unit: Some(caster),
                spell_target: Some(target),
                spell_ability: Some(i32::from_be_bytes(alias)),
                ..EventSubject::default()
            },
        );
        let casting = self
            .curses
            .as_mut()
            .and_then(|state| state.pending.get_mut(&caster))
            .filter(|cast| cast.animation.is_some());
        match casting {
            Some(cast) => cast.stage = Stage::Backswing,
            None => self.forget_curse_cast(caster),
        }
        Ok(())
    }

    /// The caster of a curse: alive, shown or not (the shipped Human05 script hides Mal'Ganis
    /// before ordering the conversion).
    fn curse_caster(&self, caster: HandleId) -> Option<&super::ExecutedUnit> {
        match self.inner.entry(caster).ok()? {
            Entry::Unit { record } if record.combat.as_ref().is_some_and(|c| !c.dead) => {
                Some(record)
            }
            _ => None,
        }
    }

    fn apply_conversion(
        &mut self,
        caster: HandleId,
        target: HandleId,
        buff: [u8; 4],
        unit: [u8; 4],
        expires_at: f64,
    ) -> Result<(), Error> {
        self.end_conversion(target);
        self.detach_unit_buff(target, buff);
        self.attach_unit_buff(target, Self::curse_buff(buff))?;
        let art = self.show_buff_art(target, buff, expires_at)?;
        if let Some(state) = &mut self.curses {
            state.conversions.insert(
                target,
                ActiveConversion {
                    caster,
                    buff,
                    unit,
                    expires_at,
                    art,
                },
            );
        }
        Ok(())
    }

    /// Drops the pending conversion of `target` with its buff and art.
    fn end_conversion(&mut self, target: HandleId) -> Option<ActiveConversion> {
        let conversion = self
            .curses
            .as_mut()
            .and_then(|state| state.conversions.remove(&target))?;
        self.detach_unit_buff(target, conversion.buff);
        self.kill_effect(conversion.art);
        Some(conversion)
    }

    /// Replaces the target with the conversion unit for the caster's owner, then fires the
    /// caster's summon events.
    fn complete_conversion(&mut self, target: HandleId) -> Result<(), Error> {
        let Some(conversion) = self.end_conversion(target) else {
            return Ok(());
        };
        let Some(record) = self.live(target) else {
            return Ok(());
        };
        let (position, face) = (record.position, record.face);
        let position =
            position.ok_or_else(|| self.curse_error("converted unit position is absent"))?;
        let owner = match self.inner.entry(conversion.caster)? {
            Entry::Unit { record } => record.owner,
            _ => {
                return Err(self.curse_error(
                    "the caster was removed before the conversion ended; not modelled",
                ));
            }
        };
        self.check_unit_creation_capacity(1)?;
        // The target leaves first so the new unit takes its place.
        self.remove_unit_canonical(target)?;
        let Value::Handle(created) = self.create_summoned_unit(&[
            Value::Handle(owner),
            Value::Int(i32::from_be_bytes(conversion.unit)),
            Value::Real(position[0]),
            Value::Real(position[1]),
            Value::Real(face),
        ])?
        else {
            return Err(self.curse_error("unit factory returned no converted unit"));
        };
        self.queue_ability_event(
            conversion.caster,
            "EVENT_UNIT_SUMMON",
            "EVENT_PLAYER_UNIT_SUMMON",
            EventSubject {
                unit: Some(conversion.caster),
                summon: Some(super::event_delivery::SummonContext {
                    summoner: conversion.caster,
                    summoned: created,
                }),
                ..EventSubject::default()
            },
        )
    }

    fn curse_buff(buff: [u8; 4]) -> ActiveBuff {
        ActiveBuff::new(
            buff,
            BuffTraits {
                negative: true,
                ..BuffTraits::default()
            },
        )
    }

    /// Shows the buff row's `Targetart` on the unit until `expires_at`.
    fn show_buff_art(
        &mut self,
        unit: HandleId,
        buff: [u8; 4],
        expires_at: f64,
    ) -> Result<Option<HandleId>, Error> {
        let models = self
            .buff_profiles(buff, "Targetart")
            .map_err(|e| self.curse_error(e))?;
        let attaches = self
            .buff_profiles(buff, "Targetattach")
            .map_err(|e| self.curse_error(e))?;
        let [model] = models.as_slice() else {
            return Ok(None);
        };
        let attach = attaches.first().map_or(DEFAULT_ATTACH, String::as_str);
        self.add_unit_effect(model, unit, attach, Some(expires_at))
            .map(Some)
    }

    fn kill_effect(&mut self, effect: Option<HandleId>) {
        if let Some(effect) = effect
            && let Ok(Entry::Effect { live, .. }) = self.inner.entry_mut(effect)
        {
            *live = false;
        }
    }

    fn apply_slow(
        &mut self,
        target: HandleId,
        buff: [u8; 4],
        movement: f32,
        attack: f32,
        expires_at: f64,
    ) -> Result<(), Error> {
        if self
            .curses
            .as_ref()
            .is_some_and(|state| state.polymorphs.contains_key(&target))
        {
            return Err(self.curse_error("slow on a polymorphed unit is not modelled"));
        }
        self.end_slow(target)?;
        self.detach_unit_buff(target, buff);
        self.attach_unit_buff(target, Self::curse_buff(buff))?;
        let record = self.executed_unit(target)?;
        let (revision, base_speed) = (
            record.definition_revision,
            record.activity.as_ref().and_then(|a| a.form_profile().1),
        );
        if let Some(base) = base_speed
            && let Err(error) = self.apply_unit_move_speed(target, base * (1.0 - movement), NATIVE)
        {
            self.detach_unit_buff(target, buff);
            return Err(error);
        }
        let art = self.show_buff_art(target, buff, expires_at)?;
        if let Some(state) = &mut self.curses {
            state.slows.insert(
                target,
                ActiveSlow {
                    buff,
                    base_speed,
                    attack,
                    expires_at,
                    revision,
                    art,
                },
            );
        }
        Ok(())
    }

    /// Restores the unit's move speed and removes the buff, art and attack penalty.
    fn end_slow(&mut self, unit: HandleId) -> Result<(), Error> {
        let Some(slow) = self
            .curses
            .as_ref()
            .and_then(|state| state.slows.get(&unit))
        else {
            return Ok(());
        };
        let (buff, base, revision, art) = (slow.buff, slow.base_speed, slow.revision, slow.art);
        let restorable = self.executed_unit(unit).is_ok_and(|record| {
            record.definition_revision == revision
                && record.combat.as_ref().is_some_and(|c| !c.dead)
        });
        if let (Some(base), true) = (base, restorable) {
            self.apply_unit_move_speed(unit, base, NATIVE)?;
        }
        if let Some(state) = &mut self.curses {
            state.slows.remove(&unit);
        }
        self.detach_unit_buff(unit, buff);
        self.kill_effect(art);
        Ok(())
    }

    fn apply_polymorph(
        &mut self,
        target: HandleId,
        buff: [u8; 4],
        expires_at: f64,
        ground_form: [u8; 4],
    ) -> Result<(), Error> {
        let state = self.curses.as_mut().expect("admitted");
        if let Some(active) = state.polymorphs.get_mut(&target) {
            active.expires_at = expires_at;
            return Ok(());
        }
        if state.slows.contains_key(&target) {
            return Err(self.curse_error("polymorph on a slowed unit is not modelled"));
        }
        let record = self.executed_unit(target)?.clone();
        let refusal = if record.morph.is_some() || self.mirror_busy(target) {
            Some("a form or mirror state")
        } else if record.illusion.is_some() {
            Some("an illusion")
        } else if record.construction.is_some() || record.construction_request.is_some() {
            Some("construction")
        } else if record.abilities.as_ref().is_some_and(|a| a.hero.is_some()) {
            Some("a hero")
        } else if self
            .executed_items()
            .any(|(_, item)| item.held_by.is_some_and(|(owner, _)| owner == target))
        {
            Some("held items")
        } else {
            None
        };
        if let Some(reason) = refusal {
            return Err(
                self.curse_error(format!("polymorph of a unit with {reason} is not modelled"))
            );
        }
        let movetp = super::unit_positions::text(&record.definition, SlkId::UnitData, "movetp")
            .map_err(|e| self.curse_error(format!("UnitData/movetp: {e}")))?;
        if !GROUND_MOVEMENT.contains(&movetp) {
            return Err(self.curse_error(format!(
                "polymorph of movement type {movetp:?} is not modelled; only the ground form is"
            )));
        }
        let profile = record
            .activity
            .as_ref()
            .ok_or_else(|| self.curse_error("activity missing"))?
            .form_profile();
        let definition = self.resolve_form_definition(ground_form)?;
        let form = self.prepare_form(target, definition, FormChange::Replacement)?;
        let abilities = self.prepare_unit_abilities(&form.definition)?;
        self.detach_unit_buff(target, buff);
        self.attach_unit_buff(target, Self::curse_buff(buff))?;
        self.steer_unit(target, None, NATIVE)?;
        self.unit_ordered(target);
        self.cancel_inventory_order(target);
        self.clear_combat_target(target);
        if let Err(error) = self.commit_replacement_form(target, form, abilities) {
            self.detach_unit_buff(target, buff);
            return Err(error);
        }
        if let Some(state) = &mut self.curses {
            state.polymorphs.insert(
                target,
                ActivePolymorph {
                    buff,
                    expires_at,
                    original: record.definition,
                    abilities: record.abilities,
                    profile,
                },
            );
        }
        Ok(())
    }

    /// Restores the original definition, model, abilities and movement profile.
    fn end_polymorph(&mut self, unit: HandleId) -> Result<(), Error> {
        let Some(active) = self
            .curses
            .as_ref()
            .and_then(|state| state.polymorphs.get(&unit))
        else {
            return Ok(());
        };
        let (buff, original, abilities, profile) = (
            active.buff,
            active.original.clone(),
            active.abilities.clone(),
            active.profile,
        );
        let alive = self
            .executed_unit(unit)
            .is_ok_and(|record| record.combat.as_ref().is_some_and(|c| !c.dead));
        if alive {
            let form = self.prepare_form(unit, original, FormChange::Replacement)?;
            self.steer_unit(unit, None, NATIVE)?;
            self.commit_replacement_form(unit, form, abilities)?;
            if let Entry::Unit { record } = self.inner.entry_mut(unit)?
                && let Some(activity) = &mut record.activity
            {
                activity.restore_form_profile(profile);
            }
        }
        if let Some(state) = &mut self.curses {
            state.polymorphs.remove(&unit);
        }
        self.detach_unit_buff(unit, buff);
        Ok(())
    }

    /// Whether the buff host removed a buff that one of these spells owns.
    pub(super) fn curse_buff_removed(
        &mut self,
        unit: HandleId,
        rawcode: [u8; 4],
    ) -> Result<(), Error> {
        let Some(state) = self.curses.as_ref() else {
            return Ok(());
        };
        if state
            .slows
            .get(&unit)
            .is_some_and(|slow| slow.buff == rawcode)
        {
            self.end_slow(unit)?;
        }
        if self
            .curses
            .as_ref()
            .and_then(|state| state.polymorphs.get(&unit))
            .is_some_and(|polymorph| polymorph.buff == rawcode)
        {
            self.end_polymorph(unit)?;
        }
        if self
            .curses
            .as_ref()
            .and_then(|state| state.conversions.get(&unit))
            .is_some_and(|conversion| conversion.buff == rawcode)
        {
            self.end_conversion(unit);
        }
        Ok(())
    }

    /// A dead or removed unit keeps no cast, slow or polymorph state; a dead polymorph stays a sheep.
    pub(super) fn forget_curse_unit(&mut self, unit: HandleId) {
        let Some(state) = &mut self.curses else {
            return;
        };
        state.pending.remove(&unit);
        state.ready_at.retain(|(id, _), _| *id != unit);
        let art = state.slows.remove(&unit).and_then(|slow| slow.art);
        state.polymorphs.remove(&unit);
        self.kill_effect(art);
        self.end_conversion(unit);
    }
}
