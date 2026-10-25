//! Chain lightning: one paid cast, a bounded chain of `damage_unit` hits, one spell event.
//!
//! The bound rows are not a fixed ability id. Admission reads the `Ocl1`/`Ocl2`/`Ocl3`
//! metadata rows, their `data` ordinals, `type`, `minVal`, `maxVal` and `useSpecific`.
//! The integer row is the target count. Of the two real rows, the one whose `maxVal`
//! is at most 1 is the per-target reduction and the other is damage. Each ordinal
//! selects `DataA`..`DataZ` by the same letter mapping as ability overrides.
//! `useSpecific` mixes damage and healing classes. The class is the row's `code`.
//! A class is this spell when that class row has both `LightningEffect` and
//! `Missileart`. Healing wave has lightning and no missile art, so a tip that says
//! "damage" does not bind it. A custom alias binds when its parent's `code` is one
//! of those classes. Tooltips are not read.
//!
//! The count includes the primary. Each later hit is the previous damage times
//! `(1 - reduction)`. That product is the metadata reading. Native bytecode for it
//! is not established.
//!
//! `Rng` and `Area` are distances between unit positions, with no collision radii.
//! `Area` is measured from the previous target. Equal distances take the lower
//! handle. Damage is applied in the cast step; `Missilespeed` is kept on the cast
//! for a renderer and is not a travel delay. Those four choices are recreations.
//!
//! One `EVENT_UNIT_SPELL_EFFECT` and one `EVENT_PLAYER_UNIT_SPELL_EFFECT` name the
//! primary. Per-jump events are not queued. `targs` tokens `organic` and `invu`
//! apply only when present. `vuln` is accepted; stock rows omit both stance tokens
//! and still hit vulnerable units, so a missing `vuln` is not a refusal.
//!
//! Each accepted hop is a [`ChainSegment`]: source, target, positions, and the
//! `LightningEffect` token (`None` when the profile has none). Nothing in this crate
//! draws the bolt.
use super::{Entry, Error, Failure, ObjectHost, Value, combat};
use pcx_objects::editor::{FieldKind, SlkId};
use pcx_script::HandleId;
use std::cmp::Ordering;
use std::collections::BTreeMap;

const FAMILY_FIELDS: [[u8; 4]; 3] = [*b"Ocl1", *b"Ocl2", *b"Ocl3"];
const NATIVE: &str = "IssueTargetOrder";
const TARGET_TOKENS: [&str; 9] = [
    "air", "ground", "enemy", "neutral", "friend", "organic", "notself", "invu", "vuln",
];

#[derive(Clone)]
struct Columns {
    damage: String,
    targets: String,
    reduction: String,
    reduction_max: f32,
    targets_min: i32,
}

struct MetaField {
    ordinal: u32,
    kind: String,
    max: f64,
    min: f64,
    specific: Vec<[u8; 4]>,
}

struct Family {
    classes: Vec<[u8; 4]>,
    members: Vec<[u8; 4]>,
    columns: Columns,
}

pub(super) struct Rules {
    /// `code` values of classes that have missile art and a lightning effect.
    bases: Vec<[u8; 4]>,
    columns: Columns,
    by_order: BTreeMap<i32, Vec<[u8; 4]>>,
    ready_at: BTreeMap<(HandleId, [u8; 4]), f64>,
    pending: BTreeMap<HandleId, (HandleId, [u8; 4])>,
    casts: Vec<ChainCast>,
}

struct Filter {
    air: bool,
    ground: bool,
    enemy: bool,
    neutral: bool,
    friend: bool,
    organic: bool,
    notself: bool,
    invulnerable: bool,
    vulnerable: bool,
}

struct Spell {
    cost: f32,
    cooldown: f64,
    range: f32,
    area: f32,
    damage: f32,
    targets: i32,
    reduction: f32,
    filter: Filter,
}

struct Hit {
    unit: HandleId,
    damage: f32,
    at: [f32; 2],
}

/// One hop of an accepted cast. `effect` is the profile token for that hop.
#[derive(Clone, Debug, PartialEq)]
pub struct ChainSegment {
    pub effect: Option<String>,
    pub source: HandleId,
    pub target: HandleId,
    pub from: [f32; 2],
    pub to: [f32; 2],
}

/// Source art and hop list from one cast. A renderer drains these; this crate does not draw them.
#[derive(Clone, Debug, PartialEq)]
pub struct ChainCast {
    pub ability: [u8; 4],
    pub segments: Vec<ChainSegment>,
    pub missile_art: Option<String>,
    pub missile_speed: Option<String>,
    pub target_art: Option<String>,
}

fn target_filter(targs: &str) -> Result<Filter, String> {
    let tokens: Vec<&str> = targs
        .split(',')
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .collect();
    if let Some(unknown) = tokens.iter().find(|token| !TARGET_TOKENS.contains(token)) {
        return Err(format!(
            "unsupported chain lightning target token {unknown}"
        ));
    }
    let has = |token: &str| tokens.contains(&token);
    if !(has("air") || has("ground")) || !(has("enemy") || has("neutral") || has("friend")) {
        return Err("chain lightning targets need a movement token and an alliance token".into());
    }
    Ok(Filter {
        air: has("air"),
        ground: has("ground"),
        enemy: has("enemy"),
        neutral: has("neutral"),
        friend: has("friend"),
        organic: has("organic"),
        notself: has("notself"),
        invulnerable: has("invu"),
        vulnerable: has("vuln"),
    })
}

fn profile_tokens(values: &[String]) -> Vec<String> {
    values
        .iter()
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(str::to_owned)
        .collect()
}

fn rawcodes(text: &str) -> Result<Vec<[u8; 4]>, String> {
    let mut codes = Vec::new();
    for part in text.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let bytes = part.as_bytes();
        if bytes.len() != 4 {
            return Err(format!("ability id {part:?} is not four bytes"));
        }
        let mut code = [0; 4];
        code.copy_from_slice(bytes);
        if !codes.contains(&code) {
            codes.push(code);
        }
    }
    Ok(codes)
}

fn data_stem(ordinal: u32) -> Result<String, String> {
    let Some(offset) = ordinal.checked_sub(1) else {
        return Err("data ordinal must be positive".into());
    };
    if offset > 25 {
        return Err("data ordinal is past DataZ".into());
    }
    Ok(format!("Data{}", char::from(b'A' + offset as u8)))
}

impl ObjectHost {
    fn chain_error(&self, reason: String) -> Error {
        self.inner.error(NATIVE, Failure::HeroSkills(reason))
    }

    /// Combat and hero skills, once. Rows come from the Ocl metadata family.
    pub fn admit_chain_lightning(&mut self) -> Result<(), Error> {
        if self.chain_lightning.is_some() || self.combat.is_none() || !self.has_hero_skills() {
            return Err(self.chain_error(
                "chain lightning needs combat and hero skills, admitted once".into(),
            ));
        }
        let family = self.chain_family()?;
        let aliases = self.chain_aliases(&family.classes, &family.members)?;
        let mut by_order: BTreeMap<i32, Vec<[u8; 4]>> = BTreeMap::new();
        for alias in aliases {
            let name = self
                .hero_ability_profile(alias, "Order")
                .map_err(|e| self.chain_error(e))?;
            let order = self
                .order_registry
                .as_ref()
                .and_then(|registry| registry.id(&name))
                .filter(|id| *id != -1)
                .ok_or_else(|| self.chain_error(format!("order {name} is not registered")))?;
            by_order.entry(order).or_default().push(alias);
        }
        self.chain_lightning = Some(Rules {
            bases: family.classes,
            columns: family.columns,
            by_order,
            ready_at: BTreeMap::new(),
            pending: BTreeMap::new(),
            casts: Vec::new(),
        });
        Ok(())
    }

    fn metadata_field(&self, id: [u8; 4]) -> Result<MetaField, Error> {
        let text = |column: &str| {
            self.ability_metadata_text(id, column)
                .map_err(|e| self.chain_error(e))?
                .ok_or_else(|| self.chain_error(format!("{id:?} metadata has no {column}")))
        };
        let ordinal = text("data")?
            .parse::<u32>()
            .map_err(|_| self.chain_error(format!("{id:?} data ordinal is not an integer")))?;
        let kind = text("type")?;
        let max = text("maxVal")?
            .parse::<f64>()
            .map_err(|_| self.chain_error(format!("{id:?} maxVal is not a number")))?;
        let min = text("minVal")?
            .parse::<f64>()
            .map_err(|_| self.chain_error(format!("{id:?} minVal is not a number")))?;
        let specific = rawcodes(&text("useSpecific")?).map_err(|e| self.chain_error(e))?;
        Ok(MetaField {
            ordinal,
            kind,
            max,
            min,
            specific,
        })
    }

    fn chain_family(&self) -> Result<Family, Error> {
        let mut rows = Vec::with_capacity(FAMILY_FIELDS.len());
        for id in FAMILY_FIELDS {
            rows.push(self.metadata_field(id)?);
        }
        let mut members = rows[0].specific.clone();
        members.retain(|code| rows[1..].iter().all(|row| row.specific.contains(code)));
        let (ints, mut reals): (Vec<_>, Vec<_>) = rows
            .into_iter()
            .partition(|row| row.kind.eq_ignore_ascii_case("int"));
        if ints.len() != 1 || reals.len() != 2 {
            return Err(
                self.chain_error("Ocl metadata needs one integer field and two real fields".into())
            );
        }
        reals.sort_by(|left, right| left.max.total_cmp(&right.max));
        let reduction = &reals[0];
        let damage = &reals[1];
        if !(reduction.max <= 1.0 && damage.max > 1.0) {
            return Err(self.chain_error(
                "Ocl real fields do not separate a unit reduction from damage".into(),
            ));
        }
        let targets = &ints[0];
        let columns = Columns {
            damage: data_stem(damage.ordinal).map_err(|e| self.chain_error(e))?,
            targets: data_stem(targets.ordinal).map_err(|e| self.chain_error(e))?,
            reduction: data_stem(reduction.ordinal).map_err(|e| self.chain_error(e))?,
            reduction_max: reduction.max as f32,
            targets_min: targets.min as i32,
        };
        let mut classes = Vec::new();
        for alias in &members {
            let class = self.ability_class(*alias)?;
            if classes.contains(&class) || !self.missile_lightning(class)? {
                continue;
            }
            classes.push(class);
        }
        if classes.is_empty() {
            return Err(
                self.chain_error("Ocl metadata lists no lightning class with missile art".into())
            );
        }
        Ok(Family {
            classes,
            members,
            columns,
        })
    }

    /// Class row: both effect profiles are set. Healing wave has only lightning.
    fn missile_lightning(&self, class: [u8; 4]) -> Result<bool, Error> {
        let lightning = profile_tokens(
            &self
                .hero_ability_profiles(class, "LightningEffect")
                .map_err(|e| self.chain_error(e))?,
        );
        let missile = profile_tokens(
            &self
                .hero_ability_profiles(class, "Missileart")
                .map_err(|e| self.chain_error(e))?,
        );
        Ok(!lightning.is_empty() && !missile.is_empty())
    }

    fn ability_class(&self, alias: [u8; 4]) -> Result<[u8; 4], Error> {
        let parent = self
            .ability_parent_code(alias)
            .map_err(|e| self.chain_error(e))?;
        let code = self
            .ability_data_text(parent, "code")
            .map_err(|e| self.chain_error(e))?;
        rawcodes(&code)
            .map_err(|e| self.chain_error(e))?
            .first()
            .copied()
            .ok_or_else(|| self.chain_error(format!("{alias:?} has no code")))
    }

    fn chain_aliases(
        &self,
        classes: &[[u8; 4]],
        members: &[[u8; 4]],
    ) -> Result<Vec<[u8; 4]>, Error> {
        let mut aliases = Vec::new();
        for alias in members {
            let class = self.ability_class(*alias)?;
            if classes.contains(&class) && !aliases.contains(alias) {
                aliases.push(*alias);
            }
        }
        for (alias, _) in self
            .custom_ability_aliases()
            .map_err(|e| self.chain_error(e))?
        {
            let class = self.ability_class(alias)?;
            if classes.contains(&class) && !aliases.contains(&alias) {
                aliases.push(alias);
            }
        }
        Ok(aliases)
    }

    pub(super) fn chain_order_claimed(&self, order: i32) -> bool {
        self.chain_lightning
            .as_ref()
            .is_some_and(|rules| rules.by_order.contains_key(&order))
    }

    pub(super) fn chain_alias_is_bound(&self, alias: [u8; 4]) -> bool {
        let Some(rules) = &self.chain_lightning else {
            return false;
        };
        let Ok(class) = self.ability_class(alias) else {
            return false;
        };
        rules.bases.contains(&class)
    }

    fn learned_chain_aliases(&self, unit: HandleId) -> Result<Vec<[u8; 4]>, Error> {
        let bases = self
            .chain_lightning
            .as_ref()
            .map(|rules| rules.bases.clone())
            .unwrap_or_default();
        let Some(abilities) = self.executed_unit(unit)?.abilities.clone() else {
            return Ok(Vec::new());
        };
        let mut found = Vec::new();
        for (alias, rank) in abilities.levels() {
            if rank <= 0 || !self.unit_ability_available(unit, alias)? {
                continue;
            }
            let class = self.ability_class(alias)?;
            if bases.contains(&class) && !found.contains(&alias) {
                found.push(alias);
            }
        }
        Ok(found)
    }

    fn alias_for_order(&self, unit: HandleId, order: i32) -> Result<Option<[u8; 4]>, Error> {
        let Some(candidates) = self
            .chain_lightning
            .as_ref()
            .and_then(|rules| rules.by_order.get(&order))
            .cloned()
        else {
            return Ok(None);
        };
        let learned = self.learned_chain_aliases(unit)?;
        let matched: Vec<[u8; 4]> = candidates
            .into_iter()
            .filter(|alias| learned.contains(alias))
            .collect();
        match matched.len() {
            0 => Ok(None),
            1 => Ok(Some(matched[0])),
            _ => {
                Err(self
                    .chain_error("more than one learned chain lightning shares this order".into()))
            }
        }
    }

    /// Learned cast command for the unit's chain-lightning row, or `None` before a rank.
    pub fn chain_lightning_command(
        &self,
        unit: HandleId,
    ) -> Result<Option<super::holy_light::Command>, Error> {
        if self.chain_lightning.is_none() {
            return Ok(None);
        }
        let learned = self.learned_chain_aliases(unit)?;
        let alias = match learned.as_slice() {
            [] => return Ok(None),
            [alias] => *alias,
            _ => {
                return Err(self.chain_error(
                    "more than one learned chain lightning needs its own command".into(),
                ));
            }
        };
        let name = self
            .hero_ability_profile(alias, "Order")
            .map_err(|e| self.chain_error(e))?;
        let order = self
            .order_registry
            .as_ref()
            .and_then(|registry| registry.id(&name))
            .filter(|id| *id != -1)
            .ok_or_else(|| self.chain_error(format!("order {name} is not registered")))?;
        let hotkey = self
            .hero_ability_profile(alias, "Hotkey")
            .map_err(|e| self.chain_error(e))?;
        match hotkey.as_bytes() {
            [letter] if letter.is_ascii_alphabetic() => Ok(Some(super::holy_light::Command {
                ability: alias,
                hotkey: char::from(letter.to_ascii_uppercase()),
                order,
            })),
            _ => Err(self.chain_error(format!("Hotkey {hotkey:?} is not one letter"))),
        }
    }

    pub fn chain_lightning_readiness(
        &self,
        unit: HandleId,
    ) -> Result<Option<crate::ability_profile::Readiness>, Error> {
        if self.chain_lightning.is_none() {
            return Ok(None);
        }
        let learned = self.learned_chain_aliases(unit)?;
        let [alias] = learned.as_slice() else {
            return if learned.is_empty() {
                Ok(None)
            } else {
                Err(self.chain_error(
                    "more than one learned chain lightning needs its own readiness".into(),
                ))
            };
        };
        let Some(spell) = self.chain_spell(unit, *alias)? else {
            return Ok(None);
        };
        let now = self
            .timer_game_time()
            .ok_or_else(|| self.chain_error("clock missing".into()))?;
        let ready = self
            .chain_lightning
            .as_ref()
            .and_then(|rules| rules.ready_at.get(&(unit, *alias)).copied());
        Ok(Some(Self::ability_readiness_at(
            self.executed_unit(unit)?,
            spell.cost,
            ready,
            now,
        )))
    }

    /// Casts not yet taken by a renderer, oldest first.
    pub fn chain_lightning_casts(&self) -> &[ChainCast] {
        self.chain_lightning
            .as_ref()
            .map(|rules| rules.casts.as_slice())
            .unwrap_or(&[])
    }

    pub fn take_chain_lightning_casts(&mut self) -> Vec<ChainCast> {
        self.chain_lightning
            .as_mut()
            .map(|rules| std::mem::take(&mut rules.casts))
            .unwrap_or_default()
    }

    pub(super) fn forget_chain_cast(&mut self, caster: HandleId) {
        if let Some(rules) = &mut self.chain_lightning {
            rules.pending.remove(&caster);
        }
    }

    pub(super) fn clear_chain_cooldown(&mut self, unit: HandleId) {
        if let Some(rules) = &mut self.chain_lightning {
            rules.ready_at.retain(|(id, _), _| *id != unit);
            rules.pending.retain(|id, _| *id != unit);
        }
    }

    /// Remaining-cooldown and end-cooldown for a bound alias. `None` means some other ability.
    pub(super) fn chain_bound_cooldown(
        &mut self,
        unit: HandleId,
        alias: [u8; 4],
        end: bool,
    ) -> Result<Option<Value>, Error> {
        if !self.chain_alias_is_bound(alias) {
            return Ok(None);
        }
        let absent = if end { Value::Null } else { Value::Real(0.0) };
        let deadline = {
            let rules = self
                .chain_lightning
                .as_mut()
                .expect("bound alias has rules");
            if end {
                rules.ready_at.remove(&(unit, alias));
                return Ok(Some(Value::Null));
            }
            rules.ready_at.get(&(unit, alias)).copied()
        };
        let Some(deadline) = deadline else {
            return Ok(Some(absent));
        };
        let now = self
            .timer_game_time()
            .ok_or_else(|| self.chain_error("clock missing".into()))?;
        Ok(Some(Value::Real((deadline - now).max(0.0) as f32)))
    }

    fn chain_spell(&self, caster: HandleId, alias: [u8; 4]) -> Result<Option<Spell>, Error> {
        let Some(unit) = self.live(caster) else {
            return Ok(None);
        };
        if !self.unit_ability_available(caster, alias)? {
            return Ok(None);
        }
        let Some(abilities) = unit.abilities.as_ref() else {
            return Ok(None);
        };
        let rank = abilities.level(alias);
        if rank <= 0 {
            return Ok(None);
        }
        let levels = self
            .hero_ability_number(alias, "levels")
            .map_err(|e| self.chain_error(e))?;
        if rank > levels {
            return Err(self.chain_error(format!("rank {rank} outside {levels} levels")));
        }
        let Some(columns) = self
            .chain_lightning
            .as_ref()
            .map(|rules| rules.columns.clone())
        else {
            return Ok(None);
        };
        let number = |stem: &str, kind| {
            let raw = self
                .hero_ability_level_cell(alias, stem, rank, kind)
                .map_err(|e| self.chain_error(e))?;
            let value = if kind == FieldKind::Int {
                raw.parse::<i32>()
                    .map(|value| value as f32)
                    .map_err(|e| e.to_string())
            } else {
                raw.parse::<f32>().map_err(|e| e.to_string())
            }
            .map_err(|detail| self.chain_error(format!("invalid {stem}{rank}: {detail}")))?;
            if !value.is_finite() || value < 0.0 {
                return Err(
                    self.chain_error(format!("{stem}{rank} must be finite and nonnegative"))
                );
            }
            Ok(value)
        };
        let targets = number(&columns.targets, FieldKind::Int)? as i32;
        if targets < columns.targets_min {
            return Err(self.chain_error(format!(
                "{}{rank} is below its metadata minimum",
                columns.targets
            )));
        }
        let reduction = number(&columns.reduction, FieldKind::Unreal)?;
        if reduction > columns.reduction_max {
            return Err(self.chain_error(format!(
                "{}{rank} is above its metadata maximum",
                columns.reduction
            )));
        }
        let targs = self
            .hero_ability_level_cell(alias, "targs", rank, FieldKind::Token)
            .map_err(|e| self.chain_error(e))?;
        Ok(Some(Spell {
            cost: number("Cost", FieldKind::Int)?,
            cooldown: f64::from(self.ability_cooldown_duration(caster, alias, rank)?),
            range: number("Rng", FieldKind::Unreal)?,
            area: number("Area", FieldKind::Unreal)?,
            damage: number(&columns.damage, FieldKind::Unreal)?,
            targets,
            reduction,
            filter: target_filter(&targs).map_err(|e| self.chain_error(e))?,
        }))
    }

    fn chain_unready(&self, caster: HandleId, alias: [u8; 4], spell: &Spell, now: f64) -> bool {
        let Some(rules) = self.chain_lightning.as_ref() else {
            return true;
        };
        let Some(unit) = self.live(caster) else {
            return true;
        };
        !Self::ability_readiness_at(
            unit,
            spell.cost,
            rules.ready_at.get(&(caster, alias)).copied(),
            now,
        )
        .ready()
    }

    fn chain_position(&self, unit: HandleId) -> Result<[f32; 2], Error> {
        self.live(unit)
            .and_then(|unit| unit.position)
            .ok_or_else(|| self.chain_error("unit position missing".into()))
    }

    fn chain_neutral(&self, caster: HandleId, target: HandleId) -> bool {
        !self.is_enemy(caster, target) && !self.is_ally(caster, target)
    }

    fn chain_eligible(
        &self,
        caster: HandleId,
        target: HandleId,
        spell: &Spell,
        now: f64,
    ) -> Result<bool, Error> {
        let Some(facts) = self.chain_target_facts(target)? else {
            return Ok(false);
        };
        let protected = facts.invulnerable || self.divine_shield_protects(target, now);
        let stance_listed = spell.filter.invulnerable || spell.filter.vulnerable;
        let stance_ok = if stance_listed {
            (protected && spell.filter.invulnerable) || (!protected && spell.filter.vulnerable)
        } else {
            !protected
        };
        if !stance_ok || (spell.filter.notself && caster == target) {
            return Ok(false);
        }
        let allied = (spell.filter.enemy && self.is_enemy(caster, target))
            || (spell.filter.neutral && self.chain_neutral(caster, target))
            || (spell.filter.friend && self.is_ally(caster, target));
        if !allied {
            return Ok(false);
        }
        if facts.flying && !spell.filter.air || !facts.flying && !spell.filter.ground {
            return Ok(false);
        }
        if spell.filter.organic && (facts.mechanical || facts.structure) {
            return Ok(false);
        }
        Ok(true)
    }

    fn chain_target_facts(&self, target: HandleId) -> Result<Option<TargetFacts>, Error> {
        let Some(unit) = self.live(target) else {
            return Ok(None);
        };
        let invulnerable = unit.invulnerable == Some(true);
        let definition = &unit.definition;
        let flying = super::unit_positions::text(definition, SlkId::UnitData, "movetp")
            .unwrap_or("")
            == "fly";
        let mechanical = super::unit_types::class(definition, "UNIT_TYPE_MECHANICAL")
            .map_err(|e| self.chain_error(e))?;
        let structure = match super::unit_types::structure(definition) {
            Ok(value) => value,
            Err(detail) if detail.contains("missing") => false,
            Err(detail) => return Err(self.chain_error(detail)),
        };
        Ok(Some(TargetFacts {
            invulnerable,
            flying,
            mechanical,
            structure,
        }))
    }

    fn nearest_bounce(
        &self,
        caster: HandleId,
        from: [f32; 2],
        spell: &Spell,
        chosen: &[HandleId],
        now: f64,
    ) -> Result<Option<Hit>, Error> {
        let mut best: Option<Hit> = None;
        for (id, _) in self.executed_units() {
            if id == caster
                || chosen.contains(&id)
                || !self.chain_eligible(caster, id, spell, now)?
            {
                continue;
            }
            let at = self.chain_position(id)?;
            let gap = combat::distance(from, at);
            if gap > spell.area {
                continue;
            }
            let replace = match &best {
                None => true,
                Some(hit) => {
                    let best_gap = combat::distance(from, hit.at);
                    match gap.total_cmp(&best_gap) {
                        Ordering::Less => true,
                        Ordering::Equal => id < hit.unit,
                        Ordering::Greater => false,
                    }
                }
            };
            if replace {
                best = Some(Hit {
                    unit: id,
                    damage: 0.0,
                    at,
                });
            }
        }
        Ok(best)
    }

    fn chain_targets(
        &self,
        caster: HandleId,
        primary: HandleId,
        spell: &Spell,
        now: f64,
    ) -> Result<Vec<Hit>, Error> {
        let mut hits = vec![Hit {
            unit: primary,
            damage: spell.damage,
            at: self.chain_position(primary)?,
        }];
        let mut chosen = vec![primary];
        let mut damage = spell.damage;
        while hits.len() < spell.targets as usize {
            let from = hits.last().expect("primary").at;
            let Some(mut next) = self.nearest_bounce(caster, from, spell, &chosen, now)? else {
                break;
            };
            damage *= 1.0 - spell.reduction;
            next.damage = damage;
            chosen.push(next.unit);
            hits.push(next);
        }
        Ok(hits)
    }

    pub(super) fn order_chain_lightning(
        &mut self,
        caster: HandleId,
        target: HandleId,
        order: i32,
    ) -> Result<bool, Error> {
        if self.executed_unit(caster)?.paused_since.is_some() {
            return Ok(false);
        }
        let Some(alias) = self.alias_for_order(caster, order)? else {
            return Ok(false);
        };
        let now = self
            .timer_game_time()
            .ok_or_else(|| self.chain_error("clock missing".into()))?;
        let Some(spell) = self.chain_spell(caster, alias)? else {
            return Ok(false);
        };
        if !self.chain_eligible(caster, target, &spell, now)?
            || self.chain_unready(caster, alias, &spell, now)
        {
            return Ok(false);
        }
        self.clear_combat_target(caster);
        if let Some(rules) = &mut self.chain_lightning {
            rules.pending.insert(caster, (target, alias));
        }
        self.step_chain_cast(caster, target, alias, now)?;
        self.wake_natural_sleep(caster);
        self.cancel_patrol(caster);
        Ok(true)
    }

    pub(super) fn advance_chain_lightning(&mut self, now: f64) -> Result<(), Error> {
        let Some(rules) = self.chain_lightning.as_ref() else {
            return Ok(());
        };
        let pending: Vec<(HandleId, HandleId, [u8; 4])> = rules
            .pending
            .iter()
            .map(|(&caster, (target, alias))| (caster, *target, *alias))
            .collect();
        for (caster, target, alias) in pending {
            if self.executed_unit(caster).is_err() {
                self.forget_chain_cast(caster);
                continue;
            }
            if self.executed_unit(caster)?.paused_since.is_some() {
                continue;
            }
            self.step_chain_cast(caster, target, alias, now)?;
        }
        Ok(())
    }

    fn step_chain_cast(
        &mut self,
        caster: HandleId,
        target: HandleId,
        alias: [u8; 4],
        now: f64,
    ) -> Result<(), Error> {
        let spell = self.chain_spell(caster, alias)?;
        let eligible = match &spell {
            Some(spell) => self.chain_eligible(caster, target, spell, now)?,
            None => false,
        };
        let Some(spell) = spell else {
            return self.cancel_chain_cast(caster);
        };
        if !eligible || self.chain_unready(caster, alias, &spell, now) {
            return self.cancel_chain_cast(caster);
        }
        let (from, to) = (self.chain_position(caster)?, self.chain_position(target)?);
        if combat::distance(from, to) <= spell.range {
            if self.chain_moving(caster) {
                self.steer_unit(caster, None, NATIVE)?;
            }
            return self.apply_chain_cast(caster, target, alias, &spell, now);
        }
        if !self.chain_moving(caster) {
            self.steer_unit(caster, Some(to), NATIVE)?;
        }
        Ok(())
    }

    fn chain_moving(&self, unit: HandleId) -> bool {
        self.live(unit)
            .and_then(|unit| unit.activity.as_ref())
            .is_some_and(|activity| activity.moving())
    }

    fn cancel_chain_cast(&mut self, caster: HandleId) -> Result<(), Error> {
        self.forget_chain_cast(caster);
        if self.live(caster).is_some() {
            self.steer_unit(caster, None, NATIVE)?;
        }
        Ok(())
    }

    fn apply_chain_cast(
        &mut self,
        caster: HandleId,
        primary: HandleId,
        alias: [u8; 4],
        spell: &Spell,
        now: f64,
    ) -> Result<(), Error> {
        let hits = self.chain_targets(caster, primary, spell, now)?;
        let cast = self.chain_cast_view(caster, alias, &hits)?;
        let sequence = profile_tokens(
            &self
                .hero_ability_profiles(alias, "Animnames")
                .map_err(|e| self.chain_error(e))?,
        )
        .into_iter()
        .next();
        self.forget_chain_cast(caster);
        if let Some(rules) = &mut self.chain_lightning {
            rules.ready_at.insert((caster, alias), now + spell.cooldown);
        }
        self.clear_combat_target(caster);
        if let Entry::Unit { record } = self.inner.entry_mut(caster)?
            && let Some(vitals) = &mut record.vitals
        {
            vitals.mana -= spell.cost;
        }
        if let Some(sequence) = sequence {
            let played = match self.inner.entry_mut(caster)? {
                Entry::Unit { record } => match &mut record.animation {
                    Some(animation) if animation.has_sequence(&sequence) => {
                        animation.select(&sequence, now, None)
                    }
                    _ => Ok(()),
                },
                _ => Ok(()),
            };
            played.map_err(|e| self.inner.error(NATIVE, Failure::UnitAnimation(e)))?;
        }
        for hit in &hits {
            self.damage_unit(caster, hit.unit, hit.damage)?;
        }
        self.queue_spell_effect(caster, Some(primary))?;
        if let Some(rules) = &mut self.chain_lightning {
            rules.casts.push(cast);
        }
        Ok(())
    }

    fn chain_cast_view(
        &self,
        caster: HandleId,
        alias: [u8; 4],
        hits: &[Hit],
    ) -> Result<ChainCast, Error> {
        let one = |key: &str| {
            let values = self
                .hero_ability_profiles(alias, key)
                .map_err(|e| self.chain_error(e))?;
            match values.as_slice() {
                [] => Ok(None),
                [value] => Ok(Some(value.clone())),
                _ => Err(self.chain_error(format!("{key} profile is ambiguous"))),
            }
        };
        let tokens = profile_tokens(
            &self
                .hero_ability_profiles(alias, "LightningEffect")
                .map_err(|e| self.chain_error(e))?,
        );
        let from = self.chain_position(caster)?;
        let mut source = caster;
        let mut at = from;
        let mut segments = Vec::with_capacity(hits.len());
        for (index, hit) in hits.iter().enumerate() {
            let effect = if index == 0 {
                tokens.first()
            } else {
                tokens.get(1).or(tokens.first())
            }
            .cloned();
            segments.push(ChainSegment {
                effect,
                source,
                target: hit.unit,
                from: at,
                to: hit.at,
            });
            source = hit.unit;
            at = hit.at;
        }
        Ok(ChainCast {
            ability: alias,
            segments,
            missile_art: one("Missileart")?,
            missile_speed: one("Missilespeed")?,
            target_art: one("Targetart")?,
        })
    }

    #[cfg(test)]
    pub(crate) fn test_chain_pending(&self, unit: HandleId) -> bool {
        self.chain_lightning
            .as_ref()
            .is_some_and(|rules| rules.pending.contains_key(&unit))
    }

    #[cfg(test)]
    pub(crate) fn test_chain_effect_count(&self) -> usize {
        self.inner
            .arena
            .iter()
            .filter(|entry| matches!(entry, Entry::Effect { .. }))
            .count()
    }
}

struct TargetFacts {
    invulnerable: bool,
    flying: bool,
    mechanical: bool,
    structure: bool,
}

#[cfg(test)]
mod tests {
    use super::target_filter;

    #[test]
    fn target_filter_uses_only_the_tokens_that_are_present() {
        let plain = target_filter("air,ground,enemy,neutral").unwrap();
        assert!(!plain.organic && !plain.invulnerable && !plain.vulnerable);
        let stance = target_filter("ground,enemy,organic,invu,vuln").unwrap();
        assert!(stance.organic && stance.invulnerable && stance.vulnerable && !stance.air);
        assert!(target_filter("air,ground,organic").is_err());
        assert!(target_filter("enemy,neutral").is_err());
        assert!(target_filter("air,ground,enemy,immune").is_err());
    }
}
