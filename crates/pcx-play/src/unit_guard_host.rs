//! Managed AI guard membership and source-distance/time home returns.
use super::{Combat, Entry, Error, Failure, ObjectHost};
use crate::unit_guard_rules::Rules;
use pcx_objects::editor::misc::CombatRules;
use pcx_script::{HandleId, Value};
use pcx_sim::world::Definition;
const REMOVE: &str = "RemoveGuardPosition";
#[derive(Clone, Copy, Debug)]
pub struct GuardBudget {
    pub max_units: usize,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnitGuard {
    pub owner: HandleId,
    pub home: [f32; 2],
    pub released: bool,
    pub returning: bool,
    beyond: bool,
    chase_since: Option<f64>,
    observed_through: f64,
    settled_at: Option<[f32; 2]>,
}
#[derive(Clone, Copy)]
struct ReturnRules {
    distance: f32,
    timeout: f64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Decision {
    Continue,
    Return,
    Wait,
}
pub struct GuardState {
    native: Rules,
    source: CombatRules,
    budget: GuardBudget,
    computer: i32,
    records: Vec<(HandleId, UnitGuard)>,
}
impl GuardState {
    pub fn get(&self, unit: HandleId) -> Option<&UnitGuard> {
        self.records
            .iter()
            .find(|(id, _)| *id == unit)
            .map(|(_, g)| g)
    }
    pub fn records(&self) -> &[(HandleId, UnitGuard)] {
        &self.records
    }
    pub fn native_rules(&self) -> &Rules {
        &self.native
    }
    pub fn source_rules(&self) -> &CombatRules {
        &self.source
    }
}
fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}
impl UnitGuard {
    fn new(owner: HandleId, home: [f32; 2], now: f64) -> Self {
        Self {
            owner,
            home,
            released: false,
            returning: false,
            beyond: false,
            chase_since: None,
            observed_through: now,
            settled_at: None,
        }
    }
    fn plan(
        &mut self,
        position: [f32; 2],
        now: f64,
        chasing: bool,
        moving: bool,
        tolerance: f32,
        rules: ReturnRules,
    ) -> Result<Decision, &'static str> {
        if !now.is_finite()
            || now < self.observed_through
            || !position.iter().all(|v| v.is_finite())
            || !tolerance.is_finite()
            || tolerance < 0.
        {
            return Err("invalid guard position/clock/arrival extent");
        }
        self.observed_through = now;
        if self.released {
            return Ok(Decision::Continue);
        }
        if self.returning {
            if distance(position, self.home) <= tolerance || !moving {
                self.returning = false;
                self.beyond = false;
                self.chase_since = None;
                self.settled_at = Some(position);
                return Ok(Decision::Continue);
            }
            return Ok(Decision::Wait);
        }
        if self.settled_at == Some(position) && !chasing {
            return Ok(Decision::Continue);
        }
        self.settled_at = None;
        if distance(position, self.home) > rules.distance {
            self.beyond = true;
        }
        if !self.beyond {
            return Ok(Decision::Continue);
        }
        if chasing {
            let since = *self.chase_since.get_or_insert(now);
            if now - since < rules.timeout {
                return Ok(Decision::Continue);
            }
        } else if moving {
            // A live explicit point order is not an idle AI return request.
            return Ok(Decision::Continue);
        }
        self.returning = true;
        self.chase_since = None;
        Ok(Decision::Return)
    }
    fn attacked(&mut self, now: f64) {
        if self.beyond {
            self.chase_since = Some(now);
        }
        self.settled_at = None;
    }
    fn resumed(&mut self, since: f64, now: f64) {
        if let Some(chase) = &mut self.chase_since {
            *chase += now - since.max(*chase);
        }
        self.observed_through = now;
    }
    fn ordered(&mut self) {
        self.returning = false;
        self.beyond = false;
        self.chase_since = None;
        self.settled_at = None;
    }
}
/// Current recreation eligibility: normal mobile armed units participate in
/// their computer owner's guard AI. Native special type-mask mapping is open.
pub(super) fn candidate(definition: &Definition, combat: Option<&Combat>) -> Result<bool, String> {
    if !combat.is_some_and(|c| !c.dead && c.weapon.is_some()) {
        return Ok(false);
    }
    let movement =
        super::unit_positions::text(definition, pcx_objects::editor::SlkId::UnitData, "movetp")
            .map_err(str::to_owned)?;
    match movement {
        "foot" | "horse" => Ok(true),
        "" | "_" | "none" => Ok(false),
        _ => Err("guard movement class is not admitted".into()),
    }
}
impl ObjectHost {
    pub fn with_unit_guards(
        mut self,
        native: Rules,
        source: CombatRules,
        budget: GuardBudget,
    ) -> Result<Self, Error> {
        let fail = |detail: &str| {
            self.inner
                .error(REMOVE, Failure::UnitActivity(detail.into()))
        };
        if self.guards.is_some()
            || self.combat.is_none()
            || self.unit_activity.is_none()
            || self.executed_units().next().is_some()
        {
            return Err(fail(
                "guard admission requires combat/activity before units, once",
            ));
        }
        if self.inner.program.natives().get(REMOVE).is_none()
            && self.inner.program.natives().ambiguous(REMOVE).is_none()
        {
            return Ok(self);
        }
        Self::contract(
            &self.inner,
            REMOVE,
            &["unit"],
            "nothing",
            false,
            Some("unit"),
        )?;
        let declared = &self
            .inner
            .program
            .natives()
            .get(REMOVE)
            .expect("checked guard declaration")
            .source;
        for n in ["CreateUnit", "Player", "GetPlayerController"] {
            if self
                .inner
                .program
                .natives()
                .get(n)
                .is_none_or(|n| &n.source != declared)
            {
                return Err(self.inner.error(REMOVE, Failure::Source));
            }
        }
        let registry = self
            .order_registry
            .as_ref()
            .ok_or_else(|| fail("guard order registry is not admitted"))?;
        if registry.binding().image_sha256 != native.image_sha256
            || self.activity_stop_order() != Some(native.stop_order())
        {
            return Err(fail(
                "guard release order differs from selected order registry",
            ));
        }
        if source.sources.is_empty()
            || [
                source.guard_distance,
                source.max_guard_distance,
                source.guard_return_time,
            ]
            .iter()
            .any(|v| !v.is_finite() || *v < 0.)
            || source.max_guard_distance < source.guard_distance
        {
            return Err(fail("invalid source guard distance/time relationships"));
        }
        std::alloc::Layout::array::<(HandleId, UnitGuard)>(budget.max_units)
            .map_err(|_| self.inner.error(REMOVE, Failure::Capacity))?;
        let computer = self.source_enumerand(REMOVE, "MAP_CONTROL_COMPUTER", "mapcontrol")?;
        self.guards = Some(GuardState {
            native,
            source,
            budget,
            computer,
            records: Vec::new(),
        });
        Ok(self)
    }
    pub fn guard_state(&self) -> Option<&GuardState> {
        self.guards.as_ref()
    }
    /// Check the admitted controller before inspecting guard-specific unit fields.
    pub(super) fn guard_managed_owner(&mut self, owner: HandleId) -> Result<bool, Error> {
        let Some(computer) = self.guards.as_ref().map(|state| state.computer) else {
            return Ok(false);
        };
        let controller = self
            .player_native("GetPlayerController", &[Value::Handle(owner)])
            .ok_or_else(|| {
                self.inner.error(
                    REMOVE,
                    Failure::PlayerState("guard controller view is not admitted"),
                )
            })??;
        let Value::Handle(controller) = controller else {
            return Err(self.inner.error(REMOVE, Failure::Signature));
        };
        Ok(self.inner.symbolic_value(controller, "mapcontrol")? == computer)
    }
    /// Preflight before the unit/owner publication; publish immediately afterward.
    pub(super) fn prepare_unit_guard(
        &mut self,
        prior: Option<HandleId>,
        owner: HandleId,
        position: [f32; 2],
        eligible: bool,
    ) -> Result<Option<UnitGuard>, Error> {
        if !eligible || !self.guard_managed_owner(owner)? {
            return Ok(None);
        }
        if !position.iter().all(|v| v.is_finite()) {
            return Err(self.inner.error(
                REMOVE,
                Failure::UnitActivity("finite guard home required".into()),
            ));
        }
        let now = self.timer_game_time().ok_or_else(|| {
            self.inner.error(
                REMOVE,
                Failure::UnitActivity("guard clock is not admitted".into()),
            )
        })?;
        if !now.is_finite() || now < 0. {
            return Err(self.inner.error(
                REMOVE,
                Failure::UnitActivity("finite nonnegative guard clock required".into()),
            ));
        }
        let state = self.guards.as_mut().expect("admitted guards");
        let replacing = prior.is_some_and(|id| state.get(id).is_some());
        if !replacing {
            if state.records.len() >= state.budget.max_units {
                return Err(self
                    .inner
                    .error(REMOVE, Failure::Budget("managed unit guards")));
            }
            state
                .records
                .try_reserve(1)
                .map_err(|_| self.inner.error(REMOVE, Failure::Capacity))?;
        }
        Ok(Some(UnitGuard::new(owner, position, now)))
    }
    pub(super) fn publish_unit_guard(&mut self, unit: HandleId, guard: Option<UnitGuard>) {
        let Some(state) = &mut self.guards else {
            return;
        };
        if let Some(at) = state.records.iter().position(|(id, _)| *id == unit) {
            if let Some(guard) = guard {
                state.records[at].1 = guard;
            } else {
                state.records.remove(at);
            }
        } else if let Some(guard) = guard {
            // prepare_unit_guard reserved the slot before creation/transfer.
            state.records.push((unit, guard));
        }
    }
    pub(super) fn forget_unit_guard(&mut self, unit: HandleId) {
        if let Some(state) = &mut self.guards {
            state.records.retain(|(id, _)| *id != unit);
        }
    }
    pub(super) fn unit_ordered(&mut self, unit: HandleId) {
        self.cancel_unit_follow(unit);
        self.cancel_summon_cast(unit);
        self.cancel_patrol(unit);
        self.wake_natural_sleep(unit);
        if let Some(state) = &mut self.guards
            && let Some((_, g)) = state.records.iter_mut().find(|(id, _)| *id == unit)
        {
            g.ordered();
        }
    }
    pub(super) fn guard_attacked(&mut self, unit: HandleId, now: f64) {
        if let Some(state) = &mut self.guards
            && let Some((_, g)) = state.records.iter_mut().find(|(id, _)| *id == unit)
        {
            g.attacked(now);
        }
    }
    pub(super) fn guard_resumed(&mut self, unit: HandleId, since: f64, now: f64) {
        if let Some(state) = &mut self.guards
            && let Some((_, guard)) = state.records.iter_mut().find(|(id, _)| *id == unit)
        {
            guard.resumed(since, now);
        }
    }
    /// True means an AI return owns movement this tick, so skip acquisition.
    #[cfg_attr(
        feature = "profile-cpu",
        tracing::instrument(skip_all, target = "pcx_cpu", name = "combat.guard")
    )]
    pub(super) fn guard_combat_step(
        &mut self,
        unit: HandleId,
        now: f64,
        chasing: bool,
    ) -> Result<bool, Error> {
        let Some(state) = &self.guards else {
            return Ok(false);
        };
        let Some(mut guard) = state.get(unit).copied() else {
            return Ok(false);
        };
        let record = self.executed_unit(unit)?;
        if !record.shown || record.combat.as_ref().is_none_or(|c| c.dead) {
            return Ok(false);
        }
        let position = record.position.ok_or_else(|| {
            self.inner.error(
                REMOVE,
                Failure::UnitActivity("accepted guard position required".into()),
            )
        })?;
        let moving = record
            .activity
            .as_ref()
            .is_some_and(|a| a.moving() || (a.patrolling() || a.following()) && !guard.returning);
        let tolerance = self.unit_collision_radius(unit, REMOVE)?;
        let decision = guard
            .plan(
                position,
                now,
                chasing,
                moving,
                tolerance,
                ReturnRules {
                    distance: state.source.guard_distance,
                    timeout: f64::from(state.source.guard_return_time),
                },
            )
            .map_err(|e| self.inner.error(REMOVE, Failure::UnitActivity(e.into())))?;
        if decision == Decision::Return {
            if !self.steer_unit(unit, Some(guard.home), REMOVE)? {
                return Ok(false);
            }
            self.clear_combat_target(unit);
        }
        if let Some(state) = &mut self.guards
            && let Some((_, g)) = state.records.iter_mut().find(|(id, _)| *id == unit)
        {
            *g = guard;
        }
        Ok(decision != Decision::Continue)
    }
    pub(super) fn guard_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if name != REMOVE {
            return None;
        }
        self.guards.as_ref()?;
        Some((|| {
            let unit = match args {
                [Value::Null] => return Ok(Value::Null),
                [Value::Handle(id)] => *id,
                _ => return Err(self.inner.error(name, Failure::Signature)),
            };
            match self.inner.entry(unit)? {
                Entry::RemovedUnit => return Ok(Value::Null),
                Entry::Unit { .. } => {}
                _ => return Err(self.inner.error(name, Failure::WrongFamily)),
            }
            if self.guards.as_ref().and_then(|s| s.get(unit)).is_none() {
                return Ok(Value::Null);
            }
            // The native manager sends its stop order even for a repeated release.
            self.steer_unit(unit, None, name)?;
            self.cancel_patrol(unit);
            self.cancel_inventory_order(unit);
            self.clear_combat_target(unit);
            if let Some(state) = &mut self.guards
                && let Some((_, g)) = state.records.iter_mut().find(|(id, _)| *id == unit)
            {
                g.released = true;
                g.returning = false;
                g.chase_since = None;
            }
            Ok(Value::Null)
        })())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn id() -> HandleId {
        HandleId::new(std::num::NonZeroU64::new(7).unwrap())
    }
    #[test]
    fn pause_does_not_spend_the_guard_chase_timeout() {
        let mut guard = UnitGuard::new(id(), [0., 0.], 0.);
        let rules = ReturnRules {
            distance: 10.,
            timeout: 5.,
        };
        assert_eq!(
            guard.plan([20., 0.], 1., true, true, 1., rules).unwrap(),
            Decision::Continue
        );
        guard.resumed(2., 12.);
        assert_eq!(
            guard.plan([20., 0.], 12., true, true, 1., rules).unwrap(),
            Decision::Continue
        );
        assert_eq!(
            guard.plan([20., 0.], 16., true, true, 1., rules).unwrap(),
            Decision::Return
        );
    }
    #[test]
    fn guard_return_uses_authored_distance_time_attack_reset_and_release() {
        let rules = ReturnRules {
            distance: 10.,
            timeout: 3.,
        };
        let mut g = UnitGuard::new(id(), [0., 0.], 0.);
        assert_eq!(
            g.plan([5., 0.], 0., true, true, 1., rules).unwrap(),
            Decision::Continue
        );
        assert_eq!(
            g.plan([11., 0.], 1., true, true, 1., rules).unwrap(),
            Decision::Continue
        );
        g.attacked(3.);
        assert_eq!(
            g.plan([15., 0.], 5., true, true, 1., rules).unwrap(),
            Decision::Continue
        );
        assert_eq!(
            g.plan([15., 0.], 6., true, true, 1., rules).unwrap(),
            Decision::Return
        );
        assert_eq!(
            g.plan([10., 0.], 7., false, true, 1., rules).unwrap(),
            Decision::Wait
        );
        assert_eq!(
            g.plan([0.5, 0.], 8., false, false, 1., rules).unwrap(),
            Decision::Continue
        );
        g.released = true;
        assert_eq!(
            g.plan([100., 0.], 20., true, true, 1., rules).unwrap(),
            Decision::Continue
        );
        assert!(g.plan([0., 0.], 19., false, false, 1., rules).is_err());
    }
}
