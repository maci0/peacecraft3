//! Script cooldown controls share the implemented casts' configuration and deadlines.
use super::*;

pub(super) const NATIVES: [(&str, &[&str], &str); 5] = [
    (
        "BlzGetUnitAbilityCooldownRemaining",
        &["unit", "integer"],
        "real",
    ),
    ("BlzEndUnitAbilityCooldown", &["unit", "integer"], "nothing"),
    (
        "BlzGetUnitAbilityCooldown",
        &["unit", "integer", "integer"],
        "real",
    ),
    (
        "BlzSetUnitAbilityCooldown",
        &["unit", "integer", "integer", "real"],
        "nothing",
    ),
    ("BlzGetAbilityCooldown", &["integer", "integer"], "real"),
];

type CustomAbilityAlias = ([u8; 4], [u8; 4]);

impl ObjectHost {
    pub(crate) fn unit_cooldown_reset_declaration(&self, source: &str) -> Result<(), Error> {
        let name = "UnitResetCooldown";
        if let Some(native) = self.inner.program.natives().get(name) {
            Self::contract(&self.inner, name, &["unit"], "nothing", false, Some("unit"))?;
            if native.source != source {
                return Err(self.inner.error(name, Failure::Source));
            }
        }
        Ok(())
    }

    pub(crate) fn clear_unit_cooldowns(&mut self, unit: HandleId) {
        if let Some(holy) = &mut self.holy_light {
            holy.ready_at.remove(&unit);
        }
        self.clear_chain_cooldown(unit);
        self.clear_curse_cooldowns(unit);
        if let Some(mirror) = &mut self.mirror_image {
            mirror.ready.remove(&unit);
        }
        if let Some(summoning) = &mut self.summoning {
            summoning.ready.remove(&unit);
        }
        if let Some(shield) = &mut self.divine_shield.rules {
            shield.ready_at.remove(&unit);
        }
        self.reset_item_cooldowns(unit);
    }

    pub(crate) fn unit_reset_cooldown(&mut self, args: &[Value]) -> Result<Value, Error> {
        let name = "UnitResetCooldown";
        let unit = match args {
            [Value::Null] => return Ok(Value::Null),
            [Value::Handle(unit)] => *unit,
            _ => return Err(self.inner.error(name, Failure::Signature)),
        };
        if matches!(self.inner.entry(unit)?, Entry::RemovedUnit { .. }) {
            return Ok(Value::Null);
        }
        self.created_unit(unit, name)?;
        self.clear_unit_cooldowns(unit);
        Ok(Value::Null)
    }

    pub(super) fn ability_cooldown_call(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Result<Value, Error> {
        let fail = |detail: &str| self.hero_error(name, detail.into());
        if name == NATIVES[4].0 {
            return self.global_ability_cooldown(args);
        }
        if name == NATIVES[2].0 || name == NATIVES[3].0 {
            return self.ability_cooldown_duration_call(name, args);
        }
        let end = name == NATIVES[1].0;
        let absent = if end { Value::Null } else { Value::Real(0.0) };
        let [unit, Value::Int(ability)] = args else {
            return Err(fail("unit and ability rawcode required"));
        };
        let unit = match unit {
            Value::Null => return Ok(absent),
            Value::Handle(unit) => *unit,
            _ => return Err(fail("unit identity required")),
        };
        if matches!(self.inner.entry(unit)?, Entry::RemovedUnit { .. }) {
            return Ok(absent);
        }
        let ability = ability.to_be_bytes();
        let record = self.created_unit(unit, name)?;
        if record
            .abilities
            .as_ref()
            .is_none_or(|abilities| abilities.level(ability) <= 0)
        {
            return Ok(absent);
        }
        if let Some(answer) = self.curse_bound_cooldown(unit, ability, end)? {
            return Ok(answer);
        }
        if let Some(answer) = self.chain_bound_cooldown(unit, ability, end)? {
            return Ok(answer);
        }
        if self.mirror_alias(ability) {
            let ready = &mut self
                .mirror_image
                .as_mut()
                .expect("bound Mirror class")
                .ready;
            let deadline = if end {
                ready.remove(&unit)
            } else {
                ready.get(&unit).copied()
            };
            if end {
                return Ok(Value::Null);
            }
            let now = self
                .timer_game_time()
                .ok_or_else(|| self.hero_error(name, "clock missing".into()))?;
            return Ok(Value::Real(
                deadline.map_or(0.0, |at| (at - now).max(0.0) as f32),
            ));
        }
        // Visibility, death and player availability do not replace the wrapper's unit/ability gate.
        let admitted = match &ability {
            b"AHhb" => self.holy_light.is_some(),
            b"AHwe" => self.summoning.is_some(),
            b"AHds" => self.divine_shield.rules.is_some(),
            _ => false,
        };
        if !admitted {
            return Err(self.hero_error(
                name,
                format!("cooldown behavior for present ability {ability:?} is not admitted"),
            ));
        }
        let deadline = match &ability {
            b"AHhb" => {
                let ready = &mut self.holy_light.as_mut().expect("checked").ready_at;
                if end {
                    ready.remove(&unit)
                } else {
                    ready.get(&unit).copied()
                }
            }
            b"AHwe" => {
                let ready = &mut self.summoning.as_mut().expect("checked").ready;
                if end {
                    ready.remove(&unit)
                } else {
                    ready.get(&unit).copied()
                }
            }
            b"AHds" => {
                let ready = &mut self.divine_shield.rules.as_mut().expect("checked").ready_at;
                if end {
                    ready.remove(&unit)
                } else {
                    ready.get(&unit).copied()
                }
            }
            _ => unreachable!("checked implemented ability"),
        };
        if end {
            return Ok(Value::Null);
        }
        let Some(deadline) = deadline else {
            return Ok(absent);
        };
        let now = self
            .timer_game_time()
            .ok_or_else(|| self.inner.error(name, Failure::UnitState("clock missing")))?;
        Ok(Value::Real((deadline - now).max(0.0) as f32))
    }

    pub(crate) fn ability_parent_code(&self, alias: [u8; 4]) -> Result<[u8; 4], String> {
        let input = self.heroes_admission()?;
        let (base, _) = crate::ability_profile::base(&input.tables, &input.overlay, alias)?;
        fourcc_text(base)
    }

    /// Whether the selected tables have a buff row with this rawcode.
    pub(crate) fn ability_buff_exists(&self, buff: [u8; 4]) -> Result<bool, String> {
        let input = self.heroes_admission()?;
        let id = std::str::from_utf8(&buff).map_err(|_| "buff rawcode is not text")?;
        Ok(input.tables.object(Family::Buff, id).is_ok())
    }

    pub(crate) fn placement_buff_code(&self, buff: [u8; 4]) -> Result<[u8; 4], String> {
        let input = self.heroes_admission()?;
        let id = std::str::from_utf8(&buff).map_err(|_| "buff rawcode is not text")?;
        let object = input
            .tables
            .object(Family::Buff, id)
            .map_err(|e| e.to_string())?;
        let cell = object
            .cell(SlkId::AbilityBuffData, "code")
            .map_err(|e| e.to_string())?;
        cell.typed(FieldKind::Token).map_err(|e| e.to_string())?;
        cell.text
            .trim()
            .as_bytes()
            .try_into()
            .map_err(|_| format!("invalid buff code for {id}"))
    }

    /// Non-empty profile values of a buff row under `key`, in authored order.
    pub(crate) fn buff_profiles(&self, buff: [u8; 4], key: &str) -> Result<Vec<String>, String> {
        let input = self.heroes_admission()?;
        let id = std::str::from_utf8(&buff).map_err(|_| "buff rawcode is not text")?;
        let object = input
            .tables
            .object(Family::Buff, id)
            .map_err(|e| e.to_string())?;
        let mut values = Vec::new();
        for cell in object.profile(key) {
            if cell.parse_error.is_some() {
                return Err(format!("{buff:?}: malformed {key} profile"));
            }
            if !matches!(cell.text.trim(), "" | "_" | "-") {
                values.push(cell.text.trim().to_owned());
            }
        }
        Ok(values)
    }

    pub(crate) fn ability_metadata_text(
        &self,
        id: [u8; 4],
        column: &str,
    ) -> Result<Option<String>, String> {
        let input = self.heroes_admission()?;
        let Some(metadata) = input.tables.metadata(id) else {
            return Ok(None);
        };
        Ok(metadata
            .attribute(column)
            .map(|cell| cell.text.trim().to_owned()))
    }

    pub(crate) fn unranked_build_menu_command(
        &self,
        ability: [u8; 4],
    ) -> Result<crate::playercolor_host::BuildMenuCommand, String> {
        let (tables, overlay) = match self.unit_ability_source() {
            Some(source) => source,
            None => {
                let input = self.heroes_admission()?;
                (&*input.tables, &input.overlay)
            }
        };
        let profile = |key| -> Result<String, String> {
            match crate::ability_profile::profiles(tables, overlay, ability, key)?.as_slice() {
                [one] => Ok(one.clone()),
                [] => Err(format!("{ability:?}: missing {key} profile")),
                _ => Err(format!("{ability:?}: ambiguous {key} profile")),
            }
        };
        let position = crate::ability_profile::button_position(tables, overlay, ability)?;
        let art = profile("Art")?;
        let hotkeys = crate::ability_profile::profiles(tables, overlay, ability, "Hotkey")?;
        let hotkey = match hotkeys.as_slice() {
            [] => None,
            [one] => {
                let mut letters = one.chars();
                let key = letters
                    .next()
                    .filter(char::is_ascii_alphabetic)
                    .ok_or("invalid build-menu hotkey")?;
                if letters.next().is_some() {
                    return Err("invalid build-menu hotkey".into());
                }
                Some(key)
            }
            _ => return Err("ambiguous build-menu hotkey".into()),
        };
        let tooltip = (|| {
            Ok(crate::ability_profile::Tooltip {
                title: super::tooltip::expand(&profile("Tip")?, |row, field| {
                    self.tooltip_value(row, field)
                })?,
                description: super::tooltip::expand(&profile("Ubertip")?, |row, field| {
                    self.tooltip_value(row, field)
                })?,
            })
        })();
        Ok(crate::playercolor_host::BuildMenuCommand {
            ability,
            art,
            position,
            hotkey,
            tooltip,
        })
    }

    pub(crate) fn ability_data_text(&self, alias: [u8; 4], column: &str) -> Result<String, String> {
        let (tables, overlay) = match self.unit_ability_source() {
            Some(source) => source,
            None => {
                let input = self.heroes_admission()?;
                (&*input.tables, &input.overlay)
            }
        };
        let (base, mods) = crate::ability_profile::base(tables, overlay, alias)?;
        let metadata = tables.editor().fields().iter().filter(|field| field.from == SlkId::AbilityMetaData)
            .filter(|field| matches!(&field.source, FieldSource::Column { slk: SlkId::AbilityData, column: key } if key == column))
            .collect::<Vec<_>>();
        if metadata.len() > 1 {
            return Err(format!("{alias:?}: ambiguous {column} metadata"));
        }
        if let Some(field) = metadata.first()
            && let Some(entry) = mods.iter().rev().find(|entry| entry.field == field.fourcc)
        {
            if field.kind != FieldKind::Token
                || field.index != -1
                || entry.selectors.is_some_and(|selectors| selectors != [0, 0])
            {
                return Err(format!("{alias:?}: indexed or non-token {column} override"));
            }
            let pcx_objects::Value::Str(text) = &entry.value else {
                return Err(format!("{alias:?}: non-string {column} override"));
            };
            return std::str::from_utf8(text)
                .map(|text| text.trim().to_owned())
                .map_err(|_| format!("{alias:?}: non-UTF8 {column} override"));
        }
        let cell = tables
            .object(pcx_objects::editor::defaults::Family::Ability, base)
            .map_err(|why| why.to_string())?
            .cell(SlkId::AbilityData, column)
            .map_err(|why| why.to_string())?;
        cell.typed(FieldKind::Token)
            .map_err(|why| why.to_string())?;
        Ok(cell.text.trim().to_owned())
    }

    pub(crate) fn custom_ability_aliases(&self) -> Result<Vec<CustomAbilityAlias>, String> {
        let input = self.heroes_admission()?;
        let mut pairs = Vec::new();
        for (alias, (parent, _)) in &input.overlay.custom {
            pairs.push((fourcc_text(alias)?, fourcc_text(parent)?));
        }
        Ok(pairs)
    }

    pub(crate) fn ability_cooldown_duration(
        &self,
        unit: HandleId,
        ability: [u8; 4],
        rank: i32,
    ) -> Result<f32, Error> {
        let fail = |detail| self.hero_error("ability cooldown duration", detail);
        if let Some(bits) = self
            .created_unit(unit, "ability cooldown duration")?
            .abilities
            .as_ref()
            .and_then(|state| {
                state
                    .cooldowns
                    .iter()
                    .find(|(alias, level, _)| *alias == ability && *level == rank)
                    .map(|(_, _, bits)| *bits)
            })
        {
            return Ok(f32::from_bits(bits));
        }
        self.ability_source_cooldown(ability, rank).map_err(fail)
    }

    fn ability_source_cooldown(&self, ability: [u8; 4], rank: i32) -> Result<f32, String> {
        let raw = self.hero_ability_level_cell(ability, "Cool", rank, FieldKind::Unreal)?;
        let value = raw.parse::<f32>().map_err(|e| e.to_string())?;
        if !value.is_finite() || value < 0.0 {
            return Err("cooldown must be finite and nonnegative".into());
        }
        Ok(value)
    }

    fn global_ability_cooldown(&self, args: &[Value]) -> Result<Value, Error> {
        let name = NATIVES[4].0;
        let fail = |detail| self.hero_error(name, detail);
        let [Value::Int(ability), Value::Int(rank)] = args else {
            return Err(fail("ability rawcode and one-based level required".into()));
        };
        let ability = ability.to_be_bytes();
        let Ok(alias) = std::str::from_utf8(&ability) else {
            return Ok(Value::Real(0.0));
        };
        let input = self.heroes_admission().map_err(fail)?;
        let custom = input.overlay.new_object(alias);
        let base = custom.map_or(alias, |(base, _)| base);
        if custom.is_none()
            && !input
                .tables
                .object(Family::Ability, base)
                .is_ok_and(|row| row.rawcode() == base)
        {
            return Ok(Value::Real(0.0));
        }
        let maximum = input.number(ability, "levels").map_err(fail)?;
        if *rank < 0 || *rank > maximum {
            return Ok(Value::Real(0.0));
        }
        if *rank == 0 {
            return Err(fail(
                "level zero uses an unqualified native row index".into(),
            ));
        }
        self.ability_source_cooldown(ability, *rank)
            .map(Value::Real)
            .map_err(fail)
    }

    fn ability_cooldown_duration_call(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Result<Value, Error> {
        let fail = |detail: &str| self.hero_error(name, detail.into());
        let set = name == NATIVES[3].0;
        let absent = if set { Value::Null } else { Value::Real(0.0) };
        let (unit, ability, rank, value) = match args {
            [unit, Value::Int(ability), Value::Int(rank)] if !set => (unit, ability, rank, None),
            [
                unit,
                Value::Int(ability),
                Value::Int(rank),
                Value::Real(value),
            ] if set => (unit, ability, rank, Some(*value)),
            _ => {
                return Err(fail(
                    "unit, ability, one-based level and (for set) real duration required",
                ));
            }
        };
        if *rank < 1 {
            return Ok(absent);
        }
        let unit = match unit {
            Value::Null => return Ok(absent),
            Value::Handle(unit) => *unit,
            _ => return Err(fail("unit identity required")),
        };
        if matches!(self.inner.entry(unit)?, Entry::RemovedUnit { .. }) {
            return Ok(absent);
        }
        let ability = ability.to_be_bytes();
        if self
            .created_unit(unit, name)?
            .abilities
            .as_ref()
            .is_none_or(|state| state.level(ability) <= 0)
        {
            return Ok(absent);
        }
        if !match &ability {
            b"AHhb" => self.holy_light.is_some(),
            b"AHwe" => self.summoning.is_some(),
            b"AHds" => self.divine_shield.rules.is_some(),
            _ => false,
        } && !self.chain_alias_is_bound(ability)
            && !self.curse_alias_is_bound(ability)
            && !self.mirror_alias(ability)
        {
            return Err(fail(
                "duration behavior for this present ability is not admitted",
            ));
        }
        let maximum = self
            .hero_ability_number(ability, "levels")
            .map_err(|detail| self.hero_error(name, detail))?;
        if *rank > maximum {
            return Ok(absent);
        }
        // Resolve the authored cell before retaining an override: source ranks bound storage.
        let default = self.ability_cooldown_duration(unit, ability, *rank)?;
        let Some(value) = value else {
            return Ok(Value::Real(default));
        };
        if !value.is_finite() || value < 0.0 {
            return Err(fail("negative/nonfinite duration behavior is not admitted"));
        }
        let Entry::Unit { record } = self.inner.entry_mut(unit)? else {
            unreachable!()
        };
        let overrides = &mut record.abilities.as_mut().expect("checked").cooldowns;
        if let Some((_, _, bits)) = overrides
            .iter_mut()
            .find(|(alias, level, _)| *alias == ability && *level == *rank)
        {
            *bits = value.to_bits();
        } else {
            if overrides.try_reserve(1).is_err() {
                return Err(self.hero_error(name, "cooldown override allocation failed".into()));
            }
            overrides.push((ability, *rank, value.to_bits()));
            overrides.sort_unstable_by_key(|(alias, rank, _)| (*alias, *rank));
        }
        Ok(Value::Null)
    }
}

fn fourcc_text(text: &str) -> Result<[u8; 4], String> {
    let bytes = text.as_bytes();
    if bytes.len() != 4 {
        return Err(format!("ability id {text:?} is not four bytes"));
    }
    let mut code = [0; 4];
    code.copy_from_slice(bytes);
    Ok(code)
}
