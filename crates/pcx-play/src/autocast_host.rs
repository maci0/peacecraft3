//! Ordinary source-bound OFF transitions on canonical ability instances.
use super::*;
use pcx_script::ScriptCall;

struct PreparedOff {
    unit: HandleId,
    alias: [u8; 4],
    revision: u64,
}

impl ObjectHost {
    fn autocast_order_id(&self, name: &str, args: &[Value]) -> Option<i32> {
        let [_, order] = args else { return None };
        match (name, order) {
            ("IssueImmediateOrder", Value::Str(s)) => self.order_registry.as_ref()?.id(s),
            ("IssueImmediateOrderById", Value::Int(id)) => Some(*id),
            _ => None,
        }
    }
    fn recognizes_autocast(&self, order: i32) -> bool {
        self.units
            .as_ref()
            .and_then(|u| u.heroes.as_ref())
            .and_then(|a| a.rules.autocast.as_ref().ok())
            .is_some_and(|r| r.classes.iter().any(|c| c.on == order || c.off == order))
    }
    pub(crate) fn autocast_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        let order = self.autocast_order_id(name, args)?;
        if !self.recognizes_autocast(order) {
            return None;
        }
        Some((|| {
            let Some(prepared) = self.prepare_autocast_off(name, args, order)? else {
                return Ok(Value::Bool(false));
            };
            self.commit_autocast_off(name, order, prepared)?;
            Ok(Value::Bool(true))
        })())
    }
    /// No implicit false default: untouched state comes from the unit's authored auto list.
    pub fn unit_autocast_enabled(
        &self,
        unit: HandleId,
        alias: [u8; 4],
    ) -> Result<Option<bool>, Error> {
        let record = self.executed_unit(unit)?;
        let Some(abilities) = &record.abilities else {
            return Ok(None);
        };
        if abilities.level(alias) <= 0 {
            return Ok(None);
        }
        let rules = self
            .heroes_admission()
            .map_err(|e| self.hero_error("autocast", e))?;
        let native = rules
            .rules
            .autocast
            .as_ref()
            .map_err(|e| self.hero_error("autocast", e.clone()))?;
        let code = self
            .ability_data_text(alias, "code")
            .map_err(|e| self.hero_error("autocast", e))?;
        if !native
            .classes
            .iter()
            .any(|c| c.code.as_slice() == code.as_bytes())
        {
            return Ok(None);
        }
        let defaults = ability_list(&record.definition, "auto", rules.budget.abilities_per_unit)
            .map_err(|e| self.hero_error("autocast", e))?;
        Ok(Some(
            defaults.contains(&alias) && !abilities.autocast_disabled.contains(&alias),
        ))
    }
    fn prepare_autocast_off(
        &self,
        name: &str,
        args: &[Value],
        order: i32,
    ) -> Result<Option<PreparedOff>, Error> {
        let fail = |s: String| self.inner.error(name, Failure::UnitActivity(s));
        let unit = match args.first() {
            Some(Value::Null) => return Ok(None),
            Some(Value::Handle(u)) => *u,
            _ => return Err(fail("unit required".into())),
        };
        match self.inner.entry(unit)? {
            Entry::RemovedUnit { .. } => return Ok(None),
            Entry::Unit { .. } => {}
            _ => return Err(self.inner.error(name, Failure::WrongFamily)),
        }
        let input = self
            .units
            .as_ref()
            .and_then(|u| u.heroes.as_ref())
            .ok_or_else(|| fail("autocast ability source not admitted".into()))?;
        let rules = input.rules.autocast.as_ref().map_err(|e| fail(e.clone()))?;
        let class = rules
            .classes
            .iter()
            .find(|c| c.on == order || c.off == order)
            .ok_or_else(|| fail("autocast order is not admitted".into()))?
            .clone();
        if self
            .order_registry
            .as_ref()
            .is_none_or(|r| r.binding().image_sha256 != rules.image_sha256)
        {
            return Err(fail("autocast/order source differs".into()));
        }
        let record = self.executed_unit(unit)?;
        let Some(abilities) = &record.abilities else {
            return Err(fail("autocast unit abilities not admitted".into()));
        };
        let (tables, overlay) = (&input.tables, &input.overlay);
        let mut selected = None;
        for (alias, rank) in abilities.levels() {
            if rank <= 0 {
                continue;
            }
            let code = self.ability_data_text(alias, "code").map_err(fail)?;
            if code.as_bytes() != class.code {
                continue;
            }
            for (key, wanted) in [("Orderon", class.on), ("Orderoff", class.off)] {
                let profile =
                    crate::ability_profile::profiles(tables, overlay, alias, key).map_err(fail)?;
                if profile.len() != 1
                    || self.order_registry.as_ref().and_then(|r| r.id(&profile[0])) != Some(wanted)
                {
                    return Err(fail(format!(
                        "autocast {alias:?} {key} differs from selected native class"
                    )));
                }
            }
            if selected.replace(alias).is_some() {
                return Err(fail("ambiguous autocast ability instances".into()));
            }
        }
        let Some(alias) = selected else {
            return Ok(None);
        };
        if order == class.on {
            return Err(fail(format!(
                "autocast enabling/effects not implemented for {alias:?}"
            )));
        }
        if self.unit_autocast_enabled(unit, alias)? != Some(true) {
            return Ok(None);
        }
        if record.combat.as_ref().is_some_and(|c| c.dead)
            || record.vitals.is_some_and(|v| v.life <= 0.)
            || record.paused_since.is_some()
        {
            return Ok(None);
        }
        if record.cached
            || record.summoned
            || record.illusion.is_some()
            || record.admitted_order.is_some()
            || self.unit_has_effect_history(unit)
        {
            return Err(fail(format!(
                "autocast OFF ordinary state unsupported: unit {unit:?}, ability {alias:?}"
            )));
        }
        let revision = abilities
            .autocast_revision
            .checked_add(1)
            .ok_or_else(|| fail("autocast revision exhausted".into()))?;
        Ok(Some(PreparedOff {
            unit,
            alias,
            revision,
        }))
    }
    fn commit_autocast_off(
        &mut self,
        name: &str,
        order: i32,
        prepared: PreparedOff,
    ) -> Result<(), Error> {
        let PreparedOff {
            unit,
            alias,
            revision,
        } = prepared;
        let route = self
            .prepare_steer_unit_in_range(unit, None, 0., name)?
            .ok_or_else(|| {
                self.inner.error(
                    name,
                    Failure::UnitActivity("autocast OFF cannot interrupt activity".into()),
                )
            })?;
        let events = self.prepare_ability_events(
            unit,
            "EVENT_UNIT_ISSUED_ORDER",
            "EVENT_PLAYER_UNIT_ISSUED_ORDER",
        )?;
        self.reserve_game_event_delivery(events.len())?;
        self.reserve_steer_commits(1, name)?;
        if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
            record
                .abilities
                .as_mut()
                .expect("checked")
                .autocast_disabled
                .try_reserve(1)
                .map_err(|_| self.inner.error(name, Failure::Capacity))?;
        }
        self.commit_steer_unit(unit, route, name)?;
        self.unit_ordered(unit);
        self.cancel_inventory_order(unit);
        self.clear_combat_target(unit);
        let context = (unit, revision);
        if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
            let abilities = record.abilities.as_mut().expect("checked");
            abilities.autocast_disabled.push(alias);
            abilities.autocast_revision = revision;
            abilities.autocast_order = (!events.is_empty()).then_some((revision, order));
        }
        self.commit_ability_events(
            events,
            crate::playercolor_host::EventSubject {
                unit: Some(unit),
                issued_order: Some(order),
                autocast: Some(context),
                ..Default::default()
            },
        );
        Ok(())
    }
    pub(crate) fn begin_synchronous_autocast_call(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Result<Option<ScriptCall>, Error> {
        let Some(order) = self.autocast_order_id(name, args) else {
            return Ok(None);
        };
        if !self.recognizes_autocast(order) {
            return Ok(None);
        }
        let Some(prepared) = self.prepare_autocast_off(name, args, order)? else {
            return Ok(Some(ScriptCall::Return(Value::Bool(false))));
        };
        self.prepare_synchronous_frame(name)?;
        self.begin_synchronous_delivery();
        let result = self.commit_autocast_off(name, order, prepared);
        let events = self.take_synchronous_delivery().ok_or_else(|| {
            self.inner.error(
                name,
                Failure::TriggerState("autocast synchronous batch missing"),
            )
        })?;
        result?;
        self.begin_synchronous_events_with_result(events, Value::Bool(true))
            .map(Some)
    }
    pub(crate) fn settle_autocast_event(&mut self, context: Option<(HandleId, u64)>) {
        let Some(context) = context else { return };
        if self.autocast_event_pending(context) {
            return;
        }
        if let Ok(Entry::Unit { record }) = self.inner.entry_mut(context.0)
            && let Some(abilities) = &mut record.abilities
            && abilities
                .autocast_order
                .is_some_and(|(revision, _)| revision == context.1)
        {
            abilities.autocast_order = None;
        }
    }
}
