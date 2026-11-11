//! Ordinary source-bound autocast ON and OFF transitions on canonical ability instances.
use super::*;
use pcx_script::ScriptCall;

struct PreparedToggle {
    unit: HandleId,
    alias: [u8; 4],
    revision: u64,
    /// `Some` for an ON order: the decoded ON behaviour and the game time of the first tick.
    on: Option<(crate::autocast_rules::Enable, f64)>,
    /// Same-group abilities of the unit that are ON and are switched OFF by this ON.
    peers: Vec<[u8; 4]>,
}

/// Flips `alias` away from or back to its `UnitAbilities.auto` default.
fn set_autocast(abilities: &mut UnitAbilities, alias: [u8; 4], on: bool) {
    let (back_to_default, flipped) = if on {
        (
            &mut abilities.autocast_disabled,
            &mut abilities.autocast_enabled,
        )
    } else {
        (
            &mut abilities.autocast_enabled,
            &mut abilities.autocast_disabled,
        )
    };
    match back_to_default.iter().position(|a| *a == alias) {
        Some(at) => {
            back_to_default.remove(at);
        }
        None => flipped.push(alias),
    }
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
            let Some(prepared) = self.prepare_autocast_toggle(name, args, order)? else {
                return Ok(Value::Bool(false));
            };
            self.commit_autocast_toggle(name, order, prepared)?;
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
        let Some(class) = native
            .classes
            .iter()
            .find(|c| c.code.as_slice() == code.as_bytes())
        else {
            return Ok(None);
        };
        self.autocast_state(record, alias, class.code).map(Some)
    }
    /// UnitAbilities `auto` lists ability codes, which can differ from the alias (ACrd is code Arai).
    fn autocast_state(
        &self,
        record: &ExecutedUnit,
        alias: [u8; 4],
        code: [u8; 4],
    ) -> Result<bool, Error> {
        let rules = self
            .heroes_admission()
            .map_err(|e| self.hero_error("autocast", e))?;
        let abilities = record.abilities.as_ref().expect("checked by the caller");
        let defaults = ability_list(&record.definition, "auto", rules.budget.abilities_per_unit)
            .map_err(|e| self.hero_error("autocast", e))?;
        Ok(
            (defaults.contains(&code) && !abilities.autocast_disabled.contains(&alias))
                || abilities.autocast_enabled.contains(&alias),
        )
    }
    /// The ON abilities of `unit` that share `group` with `alias`; the shared ON handler switches
    /// these OFF (`0xb99a80` calls `0x6d9270` with the group of the enabled ability).
    fn autocast_group_peers(
        &self,
        record: &ExecutedUnit,
        alias: [u8; 4],
        group: u32,
        groups: &[([u8; 4], u32)],
    ) -> Result<Vec<[u8; 4]>, Error> {
        let mut peers = Vec::new();
        if group == 0 {
            return Ok(peers);
        }
        let abilities = record.abilities.as_ref().expect("checked by the caller");
        for (other, rank) in abilities.levels() {
            if rank <= 0 || other == alias {
                continue;
            }
            let code = self
                .ability_data_text(other, "code")
                .map_err(|e| self.hero_error("autocast", e))?;
            let Some(&(code, _)) = groups
                .iter()
                .find(|&&(c, g)| c.as_slice() == code.as_bytes() && g == group)
            else {
                continue;
            };
            if self.autocast_state(record, other, code)? {
                peers.push(other);
            }
        }
        Ok(peers)
    }
    fn prepare_autocast_toggle(
        &self,
        name: &str,
        args: &[Value],
        order: i32,
    ) -> Result<Option<PreparedToggle>, Error> {
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
            // Most toggles author Orderon/Orderoff; Immolation authors only Order/Unorder.
            let (on_key, off_key) =
                if crate::ability_profile::profiles(tables, overlay, alias, "Orderoff")
                    .map_err(fail)?
                    .is_empty()
                {
                    ("Order", "Unorder")
                } else {
                    ("Orderon", "Orderoff")
                };
            for (key, wanted) in [(on_key, class.on), (off_key, class.off)] {
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
        let turning_on = order == class.on;
        let enable = if turning_on {
            Some(class.enable.ok_or_else(|| {
                fail(format!(
                    "autocast enabling of {alias:?} not implemented: its ON handler differs from the decoded shape"
                ))
            })?)
        } else {
            None
        };
        // ON of an enabled ability and OFF of a disabled one return false without a change.
        if self.unit_autocast_enabled(unit, alias)? != Some(!turning_on) {
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
                "autocast ordinary state unsupported: unit {unit:?}, ability {alias:?}"
            )));
        }
        let revision = abilities
            .autocast_revision
            .checked_add(1)
            .ok_or_else(|| fail("autocast revision exhausted".into()))?;
        let (on, peers) = match enable {
            Some(enable) => {
                let now = self
                    .timer_game_time()
                    .ok_or_else(|| fail("autocast ON needs the game clock".into()))?;
                let peers =
                    self.autocast_group_peers(record, alias, enable.group, &rules.groups)?;
                (Some((enable, now + f64::from(enable.first_tick))), peers)
            }
            None => (None, Vec::new()),
        };
        Ok(Some(PreparedToggle {
            unit,
            alias,
            revision,
            on,
            peers,
        }))
    }
    fn commit_autocast_toggle(
        &mut self,
        name: &str,
        order: i32,
        prepared: PreparedToggle,
    ) -> Result<(), Error> {
        let PreparedToggle {
            unit,
            alias,
            revision,
            on,
            peers,
        } = prepared;
        let route = self
            .prepare_steer_unit_in_range(unit, None, 0., name)?
            .ok_or_else(|| {
                self.inner.error(
                    name,
                    Failure::UnitActivity("autocast order cannot interrupt activity".into()),
                )
            })?;
        let events = self.prepare_ability_events(
            unit,
            "EVENT_UNIT_ISSUED_ORDER",
            "EVENT_PLAYER_UNIT_ISSUED_ORDER",
        )?;
        self.reserve_game_event_delivery(events.len())?;
        self.reserve_steer_commits(1, name)?;
        self.autocast_ticks
            .try_reserve(1)
            .map_err(|_| self.inner.error(name, Failure::Capacity))?;
        if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
            let abilities = record.abilities.as_mut().expect("checked");
            // Every transition pushes at most once per list: the ability and each peer.
            let reserved = abilities
                .autocast_disabled
                .try_reserve(1 + peers.len())
                .and_then(|()| abilities.autocast_enabled.try_reserve(1));
            if reserved.is_err() {
                return Err(self.inner.error(name, Failure::Capacity));
            }
        }
        self.commit_steer_unit(unit, route, name)?;
        self.unit_ordered(unit);
        self.cancel_inventory_order(unit);
        self.clear_combat_target(unit);
        let context = (unit, revision);
        if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
            let abilities = record.abilities.as_mut().expect("checked");
            set_autocast(abilities, alias, on.is_some());
            for &peer in &peers {
                set_autocast(abilities, peer, false);
            }
            abilities.autocast_revision = revision;
            abilities.autocast_order = (!events.is_empty()).then_some((revision, order));
        }
        self.autocast_ticks
            .retain(|&(u, a, _)| u != unit || !(a == alias || peers.contains(&a)));
        if let Some((_, first_tick)) = on {
            self.autocast_ticks.push((unit, alias, first_tick));
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
    /// Refuses once an armed automatic-cast evaluation falls due: the target acquisition and
    /// cast of the periodic tick (`ability-autocast-on.md`) are not implemented.
    pub(crate) fn advance_autocast(&mut self, now: f64) -> Result<(), Error> {
        let inner = &self.inner;
        self.autocast_ticks
            .retain(|&(unit, _, _)| matches!(inner.entry(unit), Ok(Entry::Unit { .. })));
        let Some(&(unit, alias, due)) = self.autocast_ticks.iter().find(|t| t.2 <= now) else {
            return Ok(());
        };
        Err(self.inner.error(
            "autocast",
            Failure::UnitActivity(format!(
                "automatic cast of {} on {unit:?} fell due at game time {due}: target acquisition and the cast are not implemented",
                String::from_utf8_lossy(&alias)
            )),
        ))
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
        let Some(prepared) = self.prepare_autocast_toggle(name, args, order)? else {
            return Ok(Some(ScriptCall::Return(Value::Bool(false))));
        };
        self.prepare_synchronous_frame(name)?;
        self.begin_synchronous_delivery();
        let result = self.commit_autocast_toggle(name, order, prepared);
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
