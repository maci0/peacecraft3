//! Per-player captain home targets in the canonical campaign AI context.
use super::{Entry, Error, Failure, ObjectHost};
use crate::campaign_captain_policy::CaptainPolicy;
use crate::map_admission::ParsedMapContext;
use pcx_script::HandleId;
use std::sync::Arc;

const CREATE: &str = "CreateCaptains";
const SET_HOME: &str = "SetCaptainHome";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CaptainObservation {
    pub created: bool,
    pub homes: [Option<[f32; 2]>; 2],
}

impl ObjectHost {
    pub fn campaign_captain_in_combat(
        &self,
        owner: HandleId,
        attack: bool,
    ) -> Result<Option<bool>, Error> {
        const NAME: &str = "CaptainInCombat";
        if self.campaign_captains.is_none() {
            return Ok(None);
        }
        let player = self.player_settings(owner)?.number;
        let Some((created, homes)) = self.campaign_ai.captains(player) else {
            return Ok(None);
        };
        if !created || homes[usize::from(!attack)].is_none() {
            return Err(self.inner.error(
                NAME,
                Failure::ConfigState("selected captain is not created"),
            ));
        }
        let focus = self
            .campaign_ai
            .captain_focus(player, attack)
            .expect("active player row");
        match focus.and_then(|target| self.inner.entry(target).ok()) {
            Some(Entry::Unit { .. }) => Ok(Some(true)),
            Some(Entry::RemovedUnit { .. }) | None => Ok(Some(false)),
            Some(_) => Err(self.inner.error(
                NAME,
                Failure::ConfigState("captain target class is not qualified"),
            )),
        }
    }

    pub(super) fn refresh_captain_combat(&mut self) {
        let inner = &self.inner;
        // shortcut: publish canonical member targets at the house combat step;
        // qualify native controller cadence and target priority before timing parity.
        self.campaign_ai.refresh_captain_focus(|player, unit| {
            let Entry::Unit { record } = inner.entry(unit).ok()? else { return None; };
            if !matches!(inner.entry(record.owner).ok(), Some(Entry::Player { number }) if *number == player) {
                return None;
            }
            let combat = record.combat.as_ref()?;
            if combat.dead {
                return None;
            }
            let target = combat.target?;
            match inner.entry(target).ok()? {
                Entry::Unit { record } if record.combat.as_ref().is_none_or(|combat| !combat.dead)
                    && record.vitals.is_none_or(|vitals| vitals.life > 0.) => Some(target),
                Entry::Destructable { state, .. } if !state.dead && state.life.is_none_or(|life| life > 0.) => Some(target),
                _ => None,
            }
        });
    }

    #[cfg(test)]
    pub(crate) fn fixture_captain_members(&self, player: i32, attack: bool) -> &[HandleId] {
        self.campaign_ai
            .captain_members_for(player, attack)
            .unwrap_or_default()
    }

    #[cfg(test)]
    pub(crate) fn fixture_captain_member(&mut self, player: i32, attack: bool, unit: HandleId) {
        self.campaign_ai
            .reserve_captain_member(player, attack)
            .unwrap();
        self.campaign_ai.attach_captain_member(player, attack, unit);
    }

    pub fn admit_campaign_captains(
        &mut self,
        context: &Arc<ParsedMapContext>,
        policy: CaptainPolicy,
    ) -> Result<(), Error> {
        if self.campaign_captains.is_some() {
            return Err(self
                .inner
                .error(CREATE, Failure::ConfigState("already admitted")));
        }
        if !self
            .player_context()
            .is_some_and(|current| Arc::ptr_eq(current, context))
        {
            return Err(self
                .inner
                .error(CREATE, Failure::ConfigState("map context differs")));
        }
        if policy.first_selector_mask.count_ones() != 1
            || policy.second_selector_mask.count_ones() != 1
            || policy.first_selector_mask == policy.second_selector_mask
        {
            return Err(self.inner.error(
                CREATE,
                Failure::ConfigState("captain selectors must be distinct single bits"),
            ));
        }
        policy.defenders.validate().map_err(|error| {
            self.inner
                .error(CREATE, Failure::UnitActivity(error.to_string()))
        })?;
        if self
            .campaign_gathering
            .as_ref()
            .is_some_and(|gathering| gathering.image_sha256 != policy.image_sha256)
        {
            return Err(self
                .inner
                .error(CREATE, Failure::ConfigState("source image differs")));
        }
        self.campaign_captains = Some(policy);
        Ok(())
    }

    pub fn campaign_captain_policy(&self) -> Option<&CaptainPolicy> {
        self.campaign_captains.as_ref()
    }

    pub(crate) fn add_ai_defenders(
        &mut self,
        player: i32,
        quantity: i32,
        kind: i32,
    ) -> Result<bool, Error> {
        self.recruit_captain_members(player, quantity, kind, false)
    }

    fn prepare_captain_home(
        &self,
        unit: HandleId,
        home: [f32; 2],
        name: &str,
    ) -> Result<super::unit_activity::PreparedSteer, Error> {
        let record = self.executed_unit(unit)?;
        if record
            .activity
            .as_ref()
            .is_none_or(|activity| !activity.can_start_motion())
        {
            return Err(self.inner.error(
                name,
                Failure::UnitActivity(format!(
                    "captain member {unit:?} kind {:?} at {:?} cannot move toward home {home:?}",
                    record.definition.rawcode, record.position
                )),
            ));
        }
        if let Some(route) = self.prepare_steer_unit_in_range(unit, Some(home), 0., name)? {
            return Ok(route);
        }
        // Recruitment/association is not conditional on immediate route success.
        // Stop the former leg, then retain the existing ownership marker for retry.
        self.prepare_steer_unit_in_range(unit, None, 0., name)?
            .ok_or_else(|| {
                self.inner.error(
                    name,
                    Failure::UnitActivity("captain member cannot prepare a stop".into()),
                )
            })
    }

    fn recruit_captain_members(
        &mut self,
        player: i32,
        quantity: i32,
        kind: i32,
        attack: bool,
    ) -> Result<bool, Error> {
        let name = if attack { "AddAssault" } else { "AddDefenders" };
        let policy = self.campaign_captains.as_ref().ok_or_else(|| {
            self.inner
                .error(name, Failure::ConfigState("captain policy is not admitted"))
        })?;
        let rules = policy.defenders.clone();
        let owner = self.player_identity(player).ok_or_else(|| {
            self.inner
                .error(name, Failure::ConfigState("player is not admitted"))
        })?;
        let Some((true, homes)) = self.campaign_ai.captains(player) else {
            return Err(self.inner.error(
                name,
                Failure::ConfigState("defensive captain is not created"),
            ));
        };
        let home = homes[usize::from(!attack)].ok_or_else(|| {
            self.inner.error(
                name,
                Failure::ConfigState("selected captain home is not present"),
            )
        })?;
        let requested = kind.to_be_bytes();
        let matches = |code| {
            code == requested
                || rules
                    .alternates
                    .iter()
                    .any(|&(base, alternate)| base == requested && alternate == code)
        };
        let mut remaining = quantity;
        for &unit in self
            .campaign_ai
            .captain_members_for(player, attack)
            .unwrap_or_default()
        {
            if self
                .live(unit)
                .is_some_and(|unit| unit.owner == owner && matches(unit.definition.rawcode))
            {
                remaining = remaining.wrapping_add(rules.count_step);
                if remaining == 0 {
                    break;
                }
            }
        }
        if remaining == 0 {
            return Ok(true);
        }

        // shortcut: canonical creation order breaks equal-priority ties; qualify native controller ordering before matching formation details.
        let mut candidates = Vec::new();
        for (unit, record) in self
            .executed_units()
            .filter(|(_, r)| r.owner == owner && matches(r.definition.rawcode))
        {
            if !record.shown
                || record.combat.as_ref().is_some_and(|combat| combat.dead)
                || record.vitals.is_some_and(|vitals| vitals.life <= 0.)
                || self.campaign_ai.captain_member_owner(unit).is_some()
                || self
                    .combat_attack_order(unit)
                    .is_some_and(|order| rules.excluded_orders.contains(&order))
            {
                continue;
            }
            if self
                .guard_state()
                .and_then(|state| state.get(unit))
                .is_some_and(|guard| guard.released)
            {
                continue;
            }
            let vitals = record.vitals.ok_or_else(|| {
                self.inner.error(
                    name,
                    Failure::UnitActivity("defender life is not admitted".into()),
                )
            })?;
            if !vitals.life.is_finite() || !vitals.max_life.is_finite() || vitals.max_life <= 0. {
                return Err(self.inner.error(
                    name,
                    Failure::UnitActivity("defender life ratio is invalid".into()),
                ));
            }
            candidates
                .try_reserve(1)
                .map_err(|_| self.inner.error(name, Failure::Capacity))?;
            candidates.push((unit, vitals.life / vitals.max_life >= rules.health_priority));
        }
        let mut accepted = Vec::new();
        accepted
            .try_reserve(candidates.len())
            .map_err(|_| self.inner.error(name, Failure::Capacity))?;
        for healthy in [true, false] {
            for &(unit, priority) in &candidates {
                if priority != healthy {
                    continue;
                }
                let record = self.executed_unit(unit)?;
                if record.combat.is_none()
                    || record.cached
                    || record.summoned
                    || record.paused_since.is_some()
                    || record.admitted_order.is_some()
                    || self.unit_has_effect_history(unit)
                    || self.harvest_owns_unit(unit)
                    || self.summon_current_order(unit).is_some()
                {
                    return Err(self.inner.error(
                        name,
                        Failure::UnitActivity(
                            "defender creation, effect or activity state is unqualified".into(),
                        ),
                    ));
                }
                let guard = self.prepare_captain_guard(unit, owner, home, name)?;
                let route = self.prepare_captain_home(unit, home, name)?;
                accepted.push((unit, guard, route));
                remaining = remaining.wrapping_add(rules.count_step);
                if remaining == 0 {
                    break;
                }
            }
            if remaining == 0 {
                break;
            }
        }
        if !accepted.is_empty() {
            self.reserve_steer_commits(accepted.len(), name)?;
        }
        self.campaign_ai
            .reserve_captain_members(player, attack, accepted.len())
            .map_err(|_| self.inner.error(name, Failure::Capacity))?;
        for (unit, guard, route) in accepted {
            self.clear_combat_target(unit);
            self.commit_steer_unit(unit, route, name)?;
            self.publish_unit_guard(unit, Some(guard));
            self.campaign_ai.attach_captain_member(player, attack, unit);
            self.campaign_ai.mark_captain_route(player, unit);
        }
        Ok(remaining == 0)
    }

    pub fn campaign_captain_state(
        &self,
        owner: HandleId,
    ) -> Result<Option<CaptainObservation>, Error> {
        if self.campaign_captains.is_none() {
            return Ok(None);
        }
        let player = self.player_settings(owner)?.number;
        Ok(self
            .campaign_ai
            .captains(player)
            .map(|(created, homes)| CaptainObservation { created, homes }))
    }

    pub fn campaign_captain_group_size(&self, owner: HandleId) -> Result<Option<i32>, Error> {
        if self.campaign_captains.is_none() || self.campaign_group_timed_life.is_none() {
            return Ok(None);
        }
        let player = self.player_settings(owner)?.number;
        let Some((created, _)) = self.campaign_ai.captains(player) else {
            return Ok(None);
        };
        if !created {
            return Err(self.inner.error(
                "CaptainGroupSize",
                Failure::ConfigState("captains are not created"),
            ));
        }
        let count = self
            .campaign_ai
            .captain_members(player)
            .expect("active player row")
            .len();
        Ok(Some(i32::try_from(count).map_err(|_| {
            self.inner.error("CaptainGroupSize", Failure::Capacity)
        })?))
    }

    pub fn campaign_captain_is_empty(&self, owner: HandleId) -> Result<Option<bool>, Error> {
        Ok(self
            .campaign_captain_group_size(owner)?
            .map(|count| count == 0))
    }

    pub(crate) fn create_ai_captains(&mut self, player: i32) -> Result<(), Error> {
        let policy = self.campaign_captains.as_ref().ok_or_else(|| {
            self.inner
                .error(CREATE, Failure::ConfigState("operands are not admitted"))
        })?;
        let gathering = self.campaign_gathering.as_ref().ok_or_else(|| {
            self.inner.error(
                CREATE,
                Failure::ConfigState("gathering operands are not admitted"),
            )
        })?;
        if policy.image_sha256 != gathering.image_sha256 {
            return Err(self
                .inner
                .error(CREATE, Failure::ConfigState("source image differs")));
        }
        let owner = self.player_identity(player).ok_or_else(|| {
            self.inner
                .error(CREATE, Failure::ConfigState("player is not admitted"))
        })?;
        let Some((created, _)) = self.campaign_ai.captains(player) else {
            return Err(self
                .inner
                .error(CREATE, Failure::ConfigState("AI player is not active")));
        };
        if created {
            return Err(self
                .inner
                .error(CREATE, Failure::ConfigState("captains already created")));
        }
        // shortcut: copy the canonical position into both targets; qualify the
        // native numeric setter before claiming exact corner-case conversion.
        let home = self
            .current_gathering_anchor(owner, CREATE)?
            .map(|(_, position)| position);
        self.campaign_ai.create_captains(player, home);
        Ok(())
    }

    pub(crate) fn set_ai_captain_home(
        &mut self,
        player: i32,
        selector: i32,
        home: [f32; 2],
    ) -> Result<(), Error> {
        let policy = self.campaign_captains.as_ref().ok_or_else(|| {
            self.inner
                .error(SET_HOME, Failure::ConfigState("operands are not admitted"))
        })?;
        if home.iter().any(|value| !value.is_finite()) {
            return Err(self.inner.error(
                SET_HOME,
                Failure::UnitActivity("non-finite captain home".into()),
            ));
        }
        if self.player_identity(player).is_none() {
            return Err(self
                .inner
                .error(SET_HOME, Failure::ConfigState("player is not admitted")));
        }
        let first = (selector as u32 & u32::from(policy.first_selector_mask)) != 0;
        let second = (selector as u32 & u32::from(policy.second_selector_mask)) != 0;
        let members = self
            .campaign_ai
            .captain_members_for(player, true)
            .unwrap_or_default()
            .iter()
            .filter(|_| first)
            .map(|&unit| (unit, true))
            .chain(
                self.campaign_ai
                    .captain_members_for(player, false)
                    .unwrap_or_default()
                    .iter()
                    .filter(|_| second)
                    .map(|&unit| (unit, false)),
            );
        let mut routes = Vec::new();
        for (unit, attack) in members {
            let record = self.executed_unit(unit)?;
            if record.owner != self.player_identity(player).expect("validated owner")
                || self.live(unit).is_none()
            {
                return Err(self.inner.error(
                    SET_HOME,
                    Failure::ConfigState("captain member is no longer owned and live"),
                ));
            }
            let prepared = self.prepare_captain_home(unit, home, SET_HOME)?;
            let guard = if attack {
                None
            } else {
                Some(self.prepare_captain_guard(unit, record.owner, home, SET_HOME)?)
            };
            routes
                .try_reserve(1)
                .map_err(|_| self.inner.error(SET_HOME, Failure::Capacity))?;
            routes.push((unit, prepared, guard));
        }
        if !routes.is_empty() {
            self.reserve_steer_commits(routes.len(), SET_HOME)?;
            self.campaign_ai
                .reserve_captain_routes(player, routes.len())
                .map_err(|_| self.inner.error(SET_HOME, Failure::Capacity))?;
        }
        if !self
            .campaign_ai
            .set_captain_home(player, first, second, home)
        {
            return Err(self.inner.error(
                SET_HOME,
                Failure::ConfigState("selected captain home is not present"),
            ));
        }
        for (unit, prepared, guard) in routes {
            self.commit_steer_unit(unit, prepared, SET_HOME)?;
            if guard.is_some() {
                self.publish_unit_guard(unit, guard);
            }
            self.campaign_ai.mark_captain_route(player, unit);
        }
        Ok(())
    }
}

impl ObjectHost {
    fn assault_rules(&self) -> Result<&crate::campaign_assault_policy::Rules, Error> {
        let fail = |why: String| self.inner.error("assault", Failure::UnitActivity(why));
        let captain = self
            .campaign_captains
            .as_ref()
            .ok_or_else(|| fail("captain policy not admitted".into()))?;
        let rules = captain.assault.as_ref().map_err(|why| fail(why.clone()))?;
        if rules.image_sha256 != captain.image_sha256 {
            return Err(fail("assault/captain image differs".into()));
        }
        Ok(rules)
    }
    fn assault_player(&self, player: i32) -> Result<HandleId, Error> {
        self.assault_rules()?;
        self.require_active_campaign_ai_player(player, "assault")?;
        let owner = self.player_identity(player).ok_or_else(|| {
            self.inner
                .error("assault", Failure::ConfigState("player not admitted"))
        })?;
        if !matches!(
            self.campaign_ai.captains(player),
            Some((true, [Some(_), _]))
        ) {
            return Err(self.inner.error(
                "assault",
                Failure::ConfigState("attack captain not created"),
            ));
        }
        Ok(owner)
    }
    pub(crate) fn init_ai_assault(&mut self, player: i32) -> Result<(), Error> {
        self.assault_player(player)?;
        let initial = self.assault_rules()?.initial_member_count;
        self.campaign_ai.initialize_assault(player, initial);
        Ok(())
    }
    pub(crate) fn add_ai_assault(
        &mut self,
        player: i32,
        quantity: i32,
        kind: i32,
    ) -> Result<bool, Error> {
        self.assault_player(player)?;
        let accepted = self.recruit_captain_members(player, quantity, kind, true)?;
        if !accepted {
            self.campaign_ai.assault_shortfall(player);
        }
        Ok(accepted)
    }
    pub(crate) fn ai_captain_is_full(&self, player: i32) -> Result<bool, Error> {
        self.assault_player(player)?;
        self.campaign_ai
            .assault_state(player)
            .and_then(|(full, _, _)| full)
            .ok_or_else(|| {
                self.inner.error(
                    "CaptainIsFull",
                    Failure::ConfigState("InitAssault has not run"),
                )
            })
    }
    pub(crate) fn ai_captain_readiness(&self, player: i32) -> Result<i32, Error> {
        let owner = self.assault_player(player)?;
        let fail = |why: String| {
            self.inner
                .error("CaptainReadiness", Failure::UnitActivity(why))
        };
        let numeric = self
            .numeric
            .as_ref()
            .ok_or_else(|| fail("numeric policy not admitted".into()))?;
        let members = self
            .campaign_ai
            .captain_members_for(player, true)
            .unwrap_or_default();
        let mut vitals = Vec::with_capacity(members.len());
        for &unit in members {
            let record = self.executed_unit(unit)?;
            if record.owner != owner {
                return Err(fail("captain member owner changed without cleanup".into()));
            }
            let pools = record
                .vitals
                .ok_or_else(|| fail("member vitals not admitted".into()))?;
            vitals.push((record.definition.rawcode, pools.life, pools.max_life));
        }
        let baseline = self
            .campaign_ai
            .assault_state(player)
            .and_then(|(_, baseline, _)| baseline)
            .unwrap_or(self.assault_rules()?.initial_member_count);
        self.assault_rules()?
            .ordinary_readiness(numeric, baseline, &vitals)
            .map_err(fail)
    }
    pub(crate) fn suicide_ai_player(
        &mut self,
        player: i32,
        target: HandleId,
        check_full: bool,
    ) -> Result<bool, Error> {
        self.assault_player(player)?;
        self.player_settings(target)?;
        if check_full && !self.ai_captain_is_full(player)? {
            return Ok(false);
        }
        let members = self
            .campaign_ai
            .captain_members_for(player, true)
            .unwrap_or_default()
            .to_vec();
        self.campaign_ai
            .reserve_captain_routes(player, members.len())
            .map_err(|_| self.inner.error("SuicidePlayer", Failure::Capacity))?;
        self.campaign_ai.launch_assault(player, target);
        for unit in members {
            self.campaign_ai.mark_captain_route(player, unit);
        }
        // Native acceptance is independent of target discovery or path success.
        Ok(true)
    }
    pub(crate) fn captain_assault_owned(&self, unit: HandleId) -> bool {
        self.campaign_ai
            .captain_member_role(unit)
            .is_some_and(|(player, attack)| {
                attack
                    && self
                        .campaign_ai
                        .assault_state(player)
                        .is_some_and(|(_, _, target)| target.is_some())
                    && self.campaign_ai.has_captain_route(player, unit)
            })
    }
    /// One bounded world scan per active assault player, then one route attempt per owned member.
    pub(crate) fn project_assault_orders(&mut self) -> Result<usize, Error> {
        let intents = self.campaign_ai.assault_intents();
        // Count candidate records and member order attempts. Each intent scans at
        // most the admitted handle capacity; memberships are disjoint. Individual
        // route attempts retain the existing path planner's separate work limit.
        let limit = self
            .inner
            .max_handles
            .checked_mul(
                intents
                    .len()
                    .checked_add(1)
                    .ok_or_else(|| self.inner.error("assault", Failure::Capacity))?,
            )
            .ok_or_else(|| self.inner.error("assault", Failure::Capacity))?;
        let mut work = 0usize;
        // Home and launched-assault members are disjoint. Together their route
        // attempts fit the existing one-attempt-per-admitted-member bound.
        let mut pending = Vec::new();
        for owned in self
            .campaign_ai
            .captain_route_members()
            .filter(|&(_, unit)| self.captain_home_pending(unit))
        {
            pending
                .try_reserve(1)
                .map_err(|_| self.inner.error("captain home", Failure::Capacity))?;
            pending.push(owned);
        }
        for (player, unit) in pending {
            let record = self.executed_unit(unit)?;
            if record.paused_since.is_some()
                || record
                    .combat
                    .as_ref()
                    .is_some_and(|combat| combat.target.is_some())
            {
                continue;
            }
            let (_, homes) = self.campaign_ai.captains(player).expect("owned captain");
            let (_, attack) = self
                .campaign_ai
                .captain_member_role(unit)
                .expect("owned member");
            let home = homes[usize::from(!attack)].expect("pending home");
            work = work
                .checked_add(1)
                .filter(|&next| next <= limit)
                .ok_or_else(|| self.inner.error("assault", Failure::Capacity))?;
            let Some(route) =
                self.prepare_steer_unit_in_range(unit, Some(home), 0., "captain home")?
            else {
                continue;
            };
            self.commit_steer_unit(unit, route, "captain home")?;
            self.campaign_ai.mark_captain_route(player, unit);
        }
        for (player, requested_owner) in intents {
            let owner = self.assault_player(player)?;
            let rules = self.assault_rules()?.clone();
            let members: Vec<_> = self
                .campaign_ai
                .captain_members_for(player, true)
                .unwrap_or_default()
                .iter()
                .copied()
                .filter(|&unit| self.captain_assault_owned(unit))
                .collect();
            let Some(origin) = members.iter().find_map(|&unit| self.live(unit)?.position) else {
                continue;
            };
            let mut selected: Option<(i32, f32, HandleId, [f32; 2])> = None;
            for (unit, record) in self.executed_units() {
                work = work
                    .checked_add(1)
                    .filter(|&next| next <= limit)
                    .ok_or_else(|| self.inner.error("assault", Failure::Capacity))?;
                if !self.hostile_owners(owner, record.owner) || self.weapon_target(unit).is_none() {
                    continue;
                }
                let Some(point) = record.position else {
                    continue;
                };
                let other = usize::from(record.owner != requested_owner);
                let structure = super::unit_types::structure(&record.definition)
                    .map_err(|why| self.inner.error("assault", Failure::UnitActivity(why)))?;
                // shortcut: special-category ranking is unmapped beyond the source mine alias;
                // replace after the native flag classification is qualified.
                let tier = if record.definition.rawcode == rules.gold_mine {
                    rules.special_tiers[other]
                } else {
                    rules.owner_tiers[other][usize::from(structure)]
                };
                let distance = (point[0] - origin[0]).hypot(point[1] - origin[1]);
                if selected
                    .as_ref()
                    .is_none_or(|&(old_tier, old_distance, old_unit, _)| {
                        tier < old_tier
                            || (tier == old_tier
                                && (distance < old_distance
                                    || (distance == old_distance && unit < old_unit)))
                    })
                {
                    selected = Some((tier, distance, unit, point));
                }
            }
            let Some((_, _, _, goal)) = selected else {
                continue;
            };
            for unit in members {
                work = work
                    .checked_add(1)
                    .filter(|&next| next <= limit)
                    .ok_or_else(|| self.inner.error("assault", Failure::Capacity))?;
                let Some(record) = self.live(unit) else {
                    continue;
                };
                if record.owner != owner
                    || record.paused_since.is_some()
                    || record.construction.is_some()
                {
                    continue;
                }
                let Some(combat) = record.combat.as_ref() else {
                    continue;
                };
                if combat.target.is_some() || combat.attack_destination == Some(goal) {
                    continue;
                }
                let Some(route) =
                    self.prepare_steer_unit_in_range(unit, Some(goal), 0., "assault")?
                else {
                    continue;
                };
                self.clear_combat_target(unit);
                self.commit_steer_unit(unit, route, "assault")?;
                if let Entry::Unit { record } = self.inner.entry_mut(unit)?
                    && let Some(combat) = &mut record.combat
                {
                    combat.attack_destination = Some(goal);
                }
                self.campaign_ai.mark_captain_route(player, unit);
            }
        }
        Ok(work)
    }
}
