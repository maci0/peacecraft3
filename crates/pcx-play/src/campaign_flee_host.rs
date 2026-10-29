//! Canonical per-player campaign AI flags shared by targeting and flee policy.
//! Admission only stores source operands; it never invents a flag for an
//! unadmitted policy or mints a player identity.
use super::{Entry, Error, Failure, ObjectHost};
use crate::campaign_flee_policy::FleePolicy;
use crate::map_admission::ParsedMapContext;
use pcx_script::HandleId;
use std::collections::BTreeMap;
use std::sync::Arc;

const NATIVE: &str = "SetUnitsFlee";
const HEROES_FLEE_NATIVE: &str = "SetHeroesFlee";
const GROUPS_FLEE_NATIVE: &str = "SetGroupsFlee";
const TARGETING_NATIVE: &str = "SetTargetHeroes";
const START_CAMPAIGN_AI: &str = "StartCampaignAI";

#[derive(Clone, Debug, PartialEq)]
pub(super) struct TownReference {
    pub unit: HandleId,
    pub position: [f32; 2],
    pub mines: Vec<(HandleId, [f32; 2])>,
    pub center: Result<[f32; 2], String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct PlayerFlags {
    target_heroes: Option<bool>,
    units_flee: Option<bool>,
    heroes_flee: Option<bool>,
    groups_flee: Option<bool>,
    new_heroes: Option<bool>,
    campaign_mode: Option<bool>,
    replacement_count: Option<i32>,
    slow_chopping: Option<bool>,
    gathering: Option<bool>,
    gathering_unit: Option<HandleId>,
    captains_created: bool,
    town_reference: Option<TownReference>,
    captain_homes: [Option<[f32; 2]>; 2],
    group_timed_life: Option<bool>,
    captain_members: [Vec<HandleId>; 2],
    captain_focus: [Option<HandleId>; 2],
    captain_routes: Vec<HandleId>,
    assault_full: Option<bool>,
    assault_baseline: Option<i32>,
    assault_target: Option<HandleId>,
    guard_posts: Vec<super::unit_training::GuardPost>,
}

#[derive(Default)]
pub(super) struct Registry {
    players: BTreeMap<i32, PlayerFlags>,
}

impl Registry {
    pub(super) fn activate(
        &mut self,
        player: i32,
        target_heroes: Option<bool>,
        units_flee: Option<bool>,
    ) -> bool {
        if self.players.contains_key(&player) {
            return false;
        }
        self.players.insert(
            player,
            PlayerFlags {
                target_heroes,
                units_flee,
                heroes_flee: None,
                groups_flee: None,
                new_heroes: None,
                campaign_mode: None,
                replacement_count: None,
                slow_chopping: None,
                gathering: None,
                gathering_unit: None,
                captains_created: false,
                town_reference: None,
                captain_homes: [None; 2],
                group_timed_life: None,
                captain_members: [Vec::new(), Vec::new()],
                captain_focus: [None; 2],
                captain_routes: Vec::new(),
                assault_full: None,
                assault_baseline: None,
                assault_target: None,
                guard_posts: Vec::new(),
            },
        );
        true
    }

    pub(super) fn deactivate(&mut self, player: i32) {
        self.players.remove(&player);
    }

    pub(super) fn guard_posts(&self, player: i32) -> Option<&[super::unit_training::GuardPost]> {
        self.players
            .get(&player)
            .map(|row| row.guard_posts.as_slice())
    }

    pub(super) fn guard_posts_mut(
        &mut self,
        player: i32,
    ) -> Option<&mut Vec<super::unit_training::GuardPost>> {
        self.players
            .get_mut(&player)
            .map(|row| &mut row.guard_posts)
    }

    pub(super) fn is_active(&self, player: i32) -> bool {
        self.players.contains_key(&player)
    }

    pub(super) fn target_heroes(&self, player: i32) -> Option<bool> {
        self.players
            .get(&player)
            .and_then(|flags| flags.target_heroes)
    }

    pub(super) fn units_flee(&self, player: i32) -> Option<bool> {
        self.players.get(&player).and_then(|flags| flags.units_flee)
    }

    pub(super) fn heroes_flee(&self, player: i32) -> Option<bool> {
        self.players
            .get(&player)
            .and_then(|flags| flags.heroes_flee)
    }

    pub(super) fn groups_flee(&self, player: i32) -> Option<bool> {
        self.players
            .get(&player)
            .and_then(|flags| flags.groups_flee)
    }

    pub(super) fn new_heroes(&self, player: i32) -> Option<bool> {
        self.players.get(&player).and_then(|flags| flags.new_heroes)
    }

    pub(super) fn campaign_mode(&self, player: i32) -> Option<bool> {
        self.players
            .get(&player)
            .and_then(|flags| flags.campaign_mode)
    }

    pub(super) fn set_campaign_mode(&mut self, player: i32, enabled: bool) {
        self.players
            .get_mut(&player)
            .expect("active player preflighted")
            .campaign_mode = Some(enabled);
    }

    pub(super) fn replacement_count(&self, player: i32) -> Option<i32> {
        self.players
            .get(&player)
            .and_then(|flags| flags.replacement_count)
    }

    pub(super) fn set_replacement_count(&mut self, player: i32, count: i32) {
        self.players
            .get_mut(&player)
            .expect("active player preflighted")
            .replacement_count = Some(count);
    }

    pub(super) fn slow_chopping(&self, player: i32) -> Option<bool> {
        self.players
            .get(&player)
            .and_then(|flags| flags.slow_chopping)
    }

    pub(super) fn gathering(&self, player: i32) -> Option<(Option<bool>, Option<HandleId>)> {
        self.players
            .get(&player)
            .map(|flags| (flags.gathering, flags.gathering_unit))
    }

    pub(super) fn stop_gathering(&mut self, player: i32, unit: Option<HandleId>) -> bool {
        let Some(flags) = self.players.get_mut(&player) else {
            return false;
        };
        flags.gathering = Some(false);
        flags.gathering_unit = unit;
        true
    }

    pub(super) fn captains(&self, player: i32) -> Option<(bool, [Option<[f32; 2]>; 2])> {
        self.players
            .get(&player)
            .map(|flags| (flags.captains_created, flags.captain_homes))
    }

    pub(super) fn group_timed_life(&self, player: i32) -> Option<bool> {
        self.players
            .get(&player)
            .and_then(|flags| flags.group_timed_life)
    }

    pub(super) fn set_group_timed_life(&mut self, player: i32, enabled: bool) -> bool {
        let Some(flags) = self.players.get_mut(&player) else {
            return false;
        };
        flags.group_timed_life = Some(enabled);
        true
    }

    pub(super) fn captain_members(&self, player: i32) -> Option<&[HandleId]> {
        self.captain_members_for(player, true)
    }

    pub(super) fn captain_members_for(&self, player: i32, attack: bool) -> Option<&[HandleId]> {
        self.players
            .get(&player)
            .map(|flags| flags.captain_members[usize::from(!attack)].as_slice())
    }

    pub(super) fn assault_state(
        &self,
        player: i32,
    ) -> Option<(Option<bool>, Option<i32>, Option<HandleId>)> {
        self.players
            .get(&player)
            .map(|row| (row.assault_full, row.assault_baseline, row.assault_target))
    }
    pub(super) fn initialize_assault(&mut self, player: i32, count: i32) {
        if let Some(row) = self.players.get_mut(&player) {
            row.assault_full = Some(true);
            row.assault_baseline.get_or_insert(count);
        }
    }
    pub(super) fn assault_shortfall(&mut self, player: i32) {
        if let Some(row) = self.players.get_mut(&player) {
            row.assault_full = Some(false);
        }
    }
    pub(super) fn launch_assault(&mut self, player: i32, target: HandleId) {
        if let Some(row) = self.players.get_mut(&player) {
            row.assault_target = Some(target);
        }
    }
    pub(super) fn assault_intents(&self) -> Vec<(i32, HandleId)> {
        self.players
            .iter()
            .filter_map(|(&player, row)| row.assault_target.map(|target| (player, target)))
            .collect()
    }

    pub(super) fn captain_focus(&self, player: i32, attack: bool) -> Option<Option<HandleId>> {
        self.players
            .get(&player)
            .map(|flags| flags.captain_focus[usize::from(!attack)])
    }

    pub(super) fn refresh_captain_focus(
        &mut self,
        mut resolve: impl FnMut(i32, HandleId) -> Option<HandleId>,
    ) {
        for (&player, flags) in &mut self.players {
            for (focus, members) in flags.captain_focus.iter_mut().zip(&flags.captain_members) {
                *focus = members.iter().find_map(|&unit| resolve(player, unit));
            }
        }
    }

    pub(super) fn reserve_captain_member(
        &mut self,
        player: i32,
        attack: bool,
    ) -> Result<(), std::collections::TryReserveError> {
        self.reserve_captain_members(player, attack, 1)
    }

    pub(super) fn reserve_captain_members(
        &mut self,
        player: i32,
        attack: bool,
        count: usize,
    ) -> Result<(), std::collections::TryReserveError> {
        if let Some(flags) = self.players.get_mut(&player) {
            flags.captain_members[usize::from(!attack)].try_reserve(count)?;
            flags.captain_routes.try_reserve(count)?;
        }
        Ok(())
    }

    pub(super) fn reserve_captain_routes(
        &mut self,
        player: i32,
        count: usize,
    ) -> Result<(), std::collections::TryReserveError> {
        if let Some(flags) = self.players.get_mut(&player) {
            flags.captain_routes.try_reserve(count)?;
        }
        Ok(())
    }

    pub(super) fn attach_captain_member(&mut self, player: i32, attack: bool, unit: HandleId) {
        self.detach_captain_member(unit);
        let flags = self
            .players
            .get_mut(&player)
            .expect("active AI player preflighted");
        let members = &mut flags.captain_members[usize::from(!attack)];
        if let Err(at) = members.binary_search(&unit) {
            members.insert(at, unit);
            if attack {
                flags.assault_baseline = i32::try_from(members.len()).ok();
            }
        }
    }

    pub(super) fn detach_captain_member(&mut self, unit: HandleId) {
        for flags in self.players.values_mut() {
            for members in &mut flags.captain_members {
                if let Ok(at) = members.binary_search(&unit) {
                    members.remove(at);
                }
            }
            if let Ok(at) = flags.captain_routes.binary_search(&unit) {
                flags.captain_routes.remove(at);
            }
        }
    }

    pub(super) fn clear_captain_route(&mut self, unit: HandleId) {
        for flags in self.players.values_mut() {
            if let Ok(at) = flags.captain_routes.binary_search(&unit) {
                flags.captain_routes.remove(at);
            }
        }
    }

    pub(super) fn mark_captain_route(&mut self, player: i32, unit: HandleId) {
        let flags = self
            .players
            .get_mut(&player)
            .expect("active AI player preflighted");
        if let Err(at) = flags.captain_routes.binary_search(&unit) {
            flags.captain_routes.insert(at, unit);
        }
    }

    /// Canonical route ownership is disjoint across the existing player rosters.
    pub(super) fn captain_route_members(&self) -> impl Iterator<Item = (i32, HandleId)> + '_ {
        self.players.iter().flat_map(|(&player, flags)| {
            flags.captain_routes.iter().map(move |&unit| (player, unit))
        })
    }

    pub(super) fn has_captain_route(&self, player: i32, unit: HandleId) -> bool {
        self.players
            .get(&player)
            .is_some_and(|flags| flags.captain_routes.binary_search(&unit).is_ok())
    }

    pub(super) fn captain_member_owner(&self, unit: HandleId) -> Option<i32> {
        self.captain_member_role(unit).map(|(player, _)| player)
    }

    pub(super) fn captain_member_role(&self, unit: HandleId) -> Option<(i32, bool)> {
        self.players.iter().find_map(|(&player, flags)| {
            flags
                .captain_members
                .iter()
                .enumerate()
                .find_map(|(role, members)| {
                    members
                        .binary_search(&unit)
                        .is_ok()
                        .then_some((player, role == 0))
                })
        })
    }

    pub(super) fn create_captains(&mut self, player: i32, home: Option<[f32; 2]>) -> bool {
        let Some(flags) = self.players.get_mut(&player) else {
            return false;
        };
        if flags.captains_created {
            return false;
        }
        flags.captains_created = true;
        flags.captain_homes = [home; 2];
        true
    }

    pub(super) fn set_town_reference(&mut self, player: i32, reference: Option<TownReference>) {
        if let Some(flags) = self.players.get_mut(&player) {
            flags.town_reference = reference;
        }
    }

    pub(super) fn town_reference(&self, player: i32) -> Option<&TownReference> {
        self.players.get(&player)?.town_reference.as_ref()
    }

    pub(super) fn set_captain_home(
        &mut self,
        player: i32,
        first: bool,
        second: bool,
        home: [f32; 2],
    ) -> bool {
        let Some(flags) = self.players.get_mut(&player) else {
            return false;
        };
        if (first || second) && !flags.captains_created
            || (first && flags.captain_homes[0].is_none())
            || (second && flags.captain_homes[1].is_none())
        {
            return false;
        }
        if first {
            flags.captain_homes[0] = Some(home);
        }
        if second {
            flags.captain_homes[1] = Some(home);
        }
        true
    }

    pub(super) fn set_target_heroes(&mut self, player: i32, enabled: bool) -> bool {
        let Some(flags) = self.players.get_mut(&player) else {
            return false;
        };
        flags.target_heroes = Some(enabled);
        true
    }

    pub(super) fn set_units_flee(&mut self, player: i32, enabled: bool) -> bool {
        let Some(flags) = self.players.get_mut(&player) else {
            return false;
        };
        flags.units_flee = Some(enabled);
        true
    }

    pub(super) fn set_heroes_flee(&mut self, player: i32, enabled: bool) -> bool {
        let Some(flags) = self.players.get_mut(&player) else {
            return false;
        };
        flags.heroes_flee = Some(enabled);
        true
    }

    pub(super) fn set_groups_flee(&mut self, player: i32, enabled: bool) -> bool {
        let Some(flags) = self.players.get_mut(&player) else {
            return false;
        };
        flags.groups_flee = Some(enabled);
        true
    }

    pub(super) fn set_new_heroes(&mut self, player: i32, enabled: bool) -> bool {
        let Some(flags) = self.players.get_mut(&player) else {
            return false;
        };
        flags.new_heroes = Some(enabled);
        true
    }

    pub(super) fn set_slow_chopping(&mut self, player: i32, enabled: bool) -> bool {
        let Some(flags) = self.players.get_mut(&player) else {
            return false;
        };
        flags.slow_chopping = Some(enabled);
        true
    }

    pub(super) fn admit_target_heroes_default(&mut self, default: bool) {
        for flags in self.players.values_mut() {
            if flags.target_heroes.is_none() {
                flags.target_heroes = Some(default);
            }
        }
    }

    pub(super) fn admit_units_flee_default(&mut self, default: bool) {
        for flags in self.players.values_mut() {
            if flags.units_flee.is_none() {
                flags.units_flee = Some(default);
            }
        }
    }
}

impl ObjectHost {
    /// Admits the selected flee operands for this session. The policy is only
    /// accepted for the same parsed map context; a refusal leaves all admitted
    /// state unchanged.
    pub fn admit_campaign_flee(
        &mut self,
        context: &Arc<ParsedMapContext>,
        policy: FleePolicy,
    ) -> Result<(), Error> {
        if self.campaign_flee.is_some() {
            return Err(self
                .inner
                .error(NATIVE, Failure::ConfigState("already admitted")));
        }
        let units_mask = policy.context_units_flee_mask;
        let heroes_mask = policy.context_heroes_flee_mask;
        let groups_mask = policy.context_groups_flee_mask;
        if units_mask == 0
            || !units_mask.is_power_of_two()
            || heroes_mask == 0
            || !heroes_mask.is_power_of_two()
            || groups_mask == 0
            || !groups_mask.is_power_of_two()
            || units_mask == heroes_mask
            || units_mask == groups_mask
            || heroes_mask == groups_mask
        {
            return Err(self.inner.error(
                NATIVE,
                Failure::ConfigState("flee context masks are not distinct single bits"),
            ));
        }
        if !self
            .player_context()
            .is_some_and(|current| Arc::ptr_eq(current, context))
        {
            return Err(self
                .inner
                .error(NATIVE, Failure::ConfigState("map context differs")));
        }
        self.campaign_ai
            .admit_units_flee_default(policy.units_flee_default);
        self.campaign_flee = Some(policy);
        Ok(())
    }

    pub fn campaign_flee_policy(&self) -> Option<&FleePolicy> {
        self.campaign_flee.as_ref()
    }

    /// `None` when flee operands are not admitted, the owner has no active
    /// campaign AI context, or that player is not active. An invalid owner
    /// handle is an `Err`, not `None`.
    pub fn campaign_flee_enabled(&self, owner: HandleId) -> Result<Option<bool>, Error> {
        if self.campaign_flee.is_none() {
            return Ok(None);
        }
        let number = self.player_settings(owner)?.number;
        Ok(self.campaign_ai.units_flee(number))
    }

    /// `None` until this AI player explicitly calls `SetHeroesFlee`.
    pub fn campaign_heroes_flee_enabled(&self, owner: HandleId) -> Result<Option<bool>, Error> {
        if self.campaign_flee.is_none() {
            return Ok(None);
        }
        let number = self.player_settings(owner)?.number;
        Ok(self.campaign_ai.heroes_flee(number))
    }

    /// `None` until this AI player explicitly calls `SetGroupsFlee`.
    // shortcut: group controller membership is unmodeled; bind a canonical consumer before issuing retreats.
    pub fn campaign_groups_flee_enabled(&self, owner: HandleId) -> Result<Option<bool>, Error> {
        if self.campaign_flee.is_none() {
            return Ok(None);
        }
        let number = self.player_settings(owner)?.number;
        Ok(self.campaign_ai.groups_flee(number))
    }

    /// Registers an active campaign AI player using the defaults of every
    /// admitted policy. Returns whether this call created the context; an
    /// already-active player keeps its current flags.
    pub(crate) fn activate_campaign_ai_player(&mut self, player: i32) -> Result<bool, Error> {
        let target_heroes = self
            .campaign_targeting
            .as_ref()
            .map(|state| state.target_heroes_default());
        let units_flee = self
            .campaign_flee
            .as_ref()
            .map(|policy| policy.units_flee_default);
        if self.player_identity(player).is_none() {
            return Err(self.inner.error(
                START_CAMPAIGN_AI,
                Failure::ConfigState("player is not admitted"),
            ));
        }
        if target_heroes.is_none()
            && units_flee.is_none()
            && self.campaign_gathering.is_none()
            && self.campaign_captains.is_none()
            && self.campaign_new_heroes.is_none()
            && self.campaign_mode.is_none()
            && self.campaign_slow_chopping.is_none()
            && self.campaign_group_timed_life.is_none()
        {
            return Ok(false);
        }
        if self.campaign_ai.is_active(player) {
            return Ok(false);
        }
        let posts = self.prepare_initial_guard_posts(player)?;
        let created = self.campaign_ai.activate(player, target_heroes, units_flee);
        *self
            .campaign_ai
            .guard_posts_mut(player)
            .expect("activated player") = posts;
        Ok(created)
    }

    pub(crate) fn deactivate_campaign_ai_player(&mut self, player: i32) -> Result<(), Error> {
        let members = self
            .campaign_ai
            .captain_members_for(player, true)
            .unwrap_or_default()
            .iter()
            .chain(
                self.campaign_ai
                    .captain_members_for(player, false)
                    .unwrap_or_default(),
            );
        let mut stops = Vec::new();
        for &unit in members {
            if !self.captain_route_owned(unit) {
                continue;
            }
            stops
                .try_reserve(1)
                .map_err(|_| self.inner.error(START_CAMPAIGN_AI, Failure::Capacity))?;
            let stop = self
                .prepare_steer_unit_in_range(unit, None, 0., START_CAMPAIGN_AI)?
                .ok_or_else(|| {
                    self.inner.error(
                        START_CAMPAIGN_AI,
                        Failure::UnitActivity("captain route cannot stop".into()),
                    )
                })?;
            stops.push((unit, stop));
        }
        if !stops.is_empty() {
            self.reserve_steer_commits(stops.len(), START_CAMPAIGN_AI)?;
        }
        for (unit, stop) in stops {
            self.commit_steer_unit(unit, stop, START_CAMPAIGN_AI)?;
            self.clear_combat_target(unit);
        }
        if let Some(owner) = self.player_identity(player) {
            for entry in &mut self.inner.arena {
                if let Entry::Unit { record } = entry
                    && record.owner == owner
                {
                    record.ai_harvest_flags = None;
                }
            }
        }
        self.campaign_ai.deactivate(player);
        Ok(())
    }

    pub(crate) fn set_ai_target_heroes(&mut self, player: i32, enabled: bool) -> Result<(), Error> {
        if self.campaign_targeting.is_none() {
            return Err(self.inner.error(
                TARGETING_NATIVE,
                Failure::ConfigState("operands are not admitted"),
            ));
        }
        self.require_active_campaign_ai_player(player, TARGETING_NATIVE)?;
        self.campaign_ai.set_target_heroes(player, enabled);
        Ok(())
    }

    pub(crate) fn set_ai_units_flee(&mut self, player: i32, enabled: bool) -> Result<(), Error> {
        if self.campaign_flee.is_none() {
            return Err(self
                .inner
                .error(NATIVE, Failure::ConfigState("operands are not admitted")));
        }
        if self.player_identity(player).is_none() {
            return Err(self
                .inner
                .error(NATIVE, Failure::ConfigState("player is not admitted")));
        }
        self.require_active_campaign_ai_player(player, NATIVE)?;
        self.campaign_ai.set_units_flee(player, enabled);
        Ok(())
    }

    pub(crate) fn set_ai_heroes_flee(&mut self, player: i32, enabled: bool) -> Result<(), Error> {
        if self.campaign_flee.is_none() {
            return Err(self.inner.error(
                HEROES_FLEE_NATIVE,
                Failure::ConfigState("operands are not admitted"),
            ));
        }
        if self.player_identity(player).is_none() {
            return Err(self.inner.error(
                HEROES_FLEE_NATIVE,
                Failure::ConfigState("player is not admitted"),
            ));
        }
        self.require_active_campaign_ai_player(player, HEROES_FLEE_NATIVE)?;
        self.campaign_ai.set_heroes_flee(player, enabled);
        Ok(())
    }

    pub(crate) fn set_ai_groups_flee(&mut self, player: i32, enabled: bool) -> Result<(), Error> {
        if self.campaign_flee.is_none() {
            return Err(self.inner.error(
                GROUPS_FLEE_NATIVE,
                Failure::ConfigState("operands are not admitted"),
            ));
        }
        if self.player_identity(player).is_none() {
            return Err(self.inner.error(
                GROUPS_FLEE_NATIVE,
                Failure::ConfigState("player is not admitted"),
            ));
        }
        self.require_active_campaign_ai_player(player, GROUPS_FLEE_NATIVE)?;
        self.campaign_ai.set_groups_flee(player, enabled);
        Ok(())
    }

    pub(super) fn require_active_campaign_ai_player(
        &self,
        player: i32,
        native: &'static str,
    ) -> Result<(), Error> {
        if !self.campaign_ai.players.contains_key(&player) {
            return Err(self
                .inner
                .error(native, Failure::ConfigState("AI player is not active")));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Registry;

    #[test]
    fn players_keep_independent_target_heroes_and_flee_flags() {
        let mut registry = Registry::default();
        assert!(registry.activate(0, Some(true), Some(false)));
        assert!(registry.activate(1, Some(false), Some(true)));
        assert_eq!(registry.target_heroes(0), Some(true));
        assert_eq!(registry.units_flee(0), Some(false));
        assert_eq!(registry.target_heroes(1), Some(false));
        assert_eq!(registry.units_flee(1), Some(true));

        assert!(registry.set_target_heroes(0, false));
        assert!(registry.set_units_flee(1, false));
        assert_eq!(registry.target_heroes(0), Some(false));
        assert_eq!(registry.units_flee(0), Some(false));
        assert_eq!(registry.target_heroes(1), Some(false));
        assert_eq!(registry.units_flee(1), Some(false));
    }

    #[test]
    fn unadmitted_policy_fields_remain_none_until_admission_backfills_them() {
        let mut registry = Registry::default();
        assert!(registry.activate(0, None, Some(true)));
        assert_eq!(registry.target_heroes(0), None);
        assert_eq!(registry.units_flee(0), Some(true));

        registry.admit_target_heroes_default(false);
        assert_eq!(registry.target_heroes(0), Some(false));
        assert_eq!(registry.units_flee(0), Some(true));

        assert!(registry.activate(1, None, None));
        assert_eq!(registry.target_heroes(1), None);
        assert_eq!(registry.units_flee(1), None);

        registry.admit_units_flee_default(false);
        assert_eq!(registry.units_flee(0), Some(true));
        assert_eq!(registry.units_flee(1), Some(false));
        assert_eq!(registry.target_heroes(1), None);
    }

    #[test]
    fn repeated_activation_keeps_first_flags_and_deactivation_forgets_them() {
        let mut registry = Registry::default();
        assert!(registry.activate(0, Some(false), Some(false)));
        assert!(registry.set_target_heroes(0, true));
        assert!(registry.set_units_flee(0, true));
        assert!(!registry.activate(0, Some(false), Some(false)));
        assert_eq!(registry.target_heroes(0), Some(true));
        assert_eq!(registry.units_flee(0), Some(true));

        registry.deactivate(0);
        assert_eq!(registry.target_heroes(0), None);
        assert_eq!(registry.units_flee(0), None);
        registry.deactivate(0);
        assert!(registry.activate(0, Some(false), Some(false)));
        assert_eq!(registry.target_heroes(0), Some(false));
        assert_eq!(registry.units_flee(0), Some(false));
    }

    #[test]
    fn missing_player_refuses_setters_without_creating_a_record() {
        let mut registry = Registry::default();
        assert!(!registry.set_target_heroes(7, true));
        assert!(!registry.set_units_flee(7, true));
        assert_eq!(registry.target_heroes(7), None);
        assert_eq!(registry.units_flee(7), None);
        assert!(registry.activate(7, Some(true), Some(true)));
        assert!(registry.set_target_heroes(7, false));
        assert!(registry.set_units_flee(7, false));
        assert_eq!(registry.target_heroes(7), Some(false));
        assert_eq!(registry.units_flee(7), Some(false));
    }

    #[test]
    fn captain_homes_are_per_player_and_selected_slots_preflight_together() {
        let mut registry = Registry::default();
        assert!(registry.activate(0, None, None));
        assert!(registry.activate(1, None, None));
        assert!(registry.create_captains(0, Some([8., 16.])));
        assert!(registry.create_captains(1, None));
        assert!(!registry.set_captain_home(1, true, true, [32., 40.]));
        assert_eq!(registry.captains(1), Some((true, [None; 2])));
        assert!(registry.set_captain_home(0, true, true, [24., 32.]));
        assert_eq!(registry.captains(0), Some((true, [Some([24., 32.]); 2])));
        assert!(registry.set_captain_home(1, false, false, [24., 32.]));
        assert_eq!(registry.captains(1), Some((true, [None; 2])));
        registry.deactivate(0);
        assert_eq!(registry.captains(0), None);
        assert_eq!(registry.captains(1), Some((true, [None; 2])));
    }
}
