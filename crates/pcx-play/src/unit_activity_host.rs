use super::{Entry, Error, Failure, ObjectHost};
use crate::unit_activity::Rules;
use pcx_objects::editor::{FieldKind, FieldSource, SlkId};
use pcx_script::{HandleId, Value};
use pcx_sim::world::{Definition, FieldValue};

#[path = "unit_follow_host.rs"]
mod follow;

pub(super) struct State {
    rules: Rules,
    route_work: usize,
    active: Vec<HandleId>,
    follow_positions: Vec<(HandleId, Option<[f32; 2]>)>,
    projected_through: f64,
    pending_blocked: Vec<PendingBlocked>,
    pending_refresh: Vec<PendingRefresh>,
}

impl State {
    pub(super) fn move_speed_limits(&self) -> Option<pcx_objects::editor::misc::MoveSpeedLimits> {
        self.rules.move_speed_limits()
    }
}

type PendingBlocked = (HandleId, Option<Box<Motion>>);
type PendingRefresh = (
    HandleId,
    Option<[f32; 2]>,
    Option<Follow>,
    Option<Box<Motion>>,
);

#[derive(Clone, Debug, PartialEq)]
pub struct UnitActivity {
    held: bool,
    movable: bool,
    speed: Option<f32>,
    motion: Option<Box<Motion>>,
    patrol: Option<Patrol>,
    follow: Option<Follow>,
    turn: Option<Turn>,
    /// `SetUnitPathing(false)`: no occupancy, straight motion, no collision checks.
    pathing_off: bool,
    /// Point rally from `setrally`. Target rally and the placement marker are separate.
    pub(crate) rally: Option<[f32; 2]>,
}

/// Script facing change: linear over the requested duration along the shorter
/// arc (a recreation choice; turn-rate fidelity is unqualified).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Turn {
    from: f32,
    delta: f32,
    start: f64,
    duration: f64,
}

impl Turn {
    fn at(&self, now: f64) -> f32 {
        let t = if self.duration > 0.0 {
            ((now - self.start) / self.duration).clamp(0.0, 1.0) as f32
        } else {
            1.0
        };
        (self.from + self.delta * t).rem_euclid(360.0)
    }
    fn done(&self, now: f64) -> bool {
        now - self.start >= self.duration
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Motion {
    /// Requested order destination. `path` may end short of it when no route reaches it.
    goal: [f32; 2],
    reach: f32,
    path: Vec<[f32; 2]>,
    budget_partial: bool,
    next: usize,
    base: [f32; 2],
    prefix: f64,
    last_distance: f64,
    started: f64,
}

/// A route and animation selected without mutating the unit or the active list.
/// Committing the prepared value is the fallible capacity step, kept after
/// damage so a refused or fatal hit leaves no partial motion.
pub(super) struct PreparedSteer {
    motion: Option<Box<Motion>>,
    animation: Option<crate::unit_model::Animation>,
    no_op: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Patrol {
    start: [f32; 2],
    end: [f32; 2],
    returning: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Follow {
    target: HandleId,
    range: f32,
    observed: [f32; 2],
}

impl Patrol {
    fn goal(self) -> [f32; 2] {
        if self.returning { self.start } else { self.end }
    }
}

/// One accepted interval of a unit's movement on host game time.
///
/// `moving` spans cover one straight accepted segment from `from` to `to`.
/// Non-moving spans cover a stop: the unit stood at `from` from `start` to `end`.
/// Spans of one call are contiguous per unit. No span covers time when the unit
/// has no motion, so after the last span the consumer treats the unit as standing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AcceptedSpan {
    pub unit: HandleId,
    pub from: [f32; 2],
    pub to: [f32; 2],
    pub start: f64,
    pub end: f64,
    pub moving: bool,
}

/// Caller-owned output of one advance. `limit` is the work budget on entry;
/// every span is paid for by at least one work unit, so `limit` is never reached.
struct SpanSink<'a> {
    spans: &'a mut Vec<AcceptedSpan>,
    base: usize,
    limit: usize,
}

impl SpanSink<'_> {
    fn segment(
        &mut self,
        unit: HandleId,
        from: [f32; 2],
        to: [f32; 2],
        start: f64,
        end: f64,
    ) -> Result<(), &'static str> {
        if start >= end {
            return Ok(());
        }
        if self.spans.len() - self.base >= self.limit {
            return Err("accepted span limit exceeded");
        }
        self.spans
            .try_reserve(1)
            .map_err(|_| "accepted span storage")?;
        self.spans.push(AcceptedSpan {
            unit,
            from,
            to,
            start,
            end,
            moving: from != to,
        });
        Ok(())
    }
}

/// Host time at which `motion` has credited `credited` distance at `speed`.
fn credited_time(motion: &Motion, speed: f32, credited: f64) -> f64 {
    motion.started
        + if speed > 0.0 {
            credited / f64::from(speed)
        } else {
            0.0
        }
}

fn emit(
    sink: &mut Option<SpanSink<'_>>,
    unit: HandleId,
    from: [f32; 2],
    to: [f32; 2],
    start: f64,
    end: f64,
) -> Result<(), &'static str> {
    match sink {
        Some(sink) => sink.segment(unit, from, to, start, end),
        None => Ok(()),
    }
}

impl UnitActivity {
    pub(super) fn replace_form_profile(&mut self, source: &Self) {
        self.movable = source.movable;
        self.speed = source.speed;
    }

    /// The movement class and effective speed a form transition replaces.
    pub(super) fn form_profile(&self) -> (bool, Option<f32>) {
        (self.movable, self.speed)
    }

    pub(super) fn restore_form_profile(&mut self, (movable, speed): (bool, Option<f32>)) {
        self.movable = movable;
        self.speed = speed;
    }

    pub(super) fn resume_after_pause(&mut self, since: f64, now: f64) {
        if let Some(motion) = &mut self.motion {
            motion.started += now - since.max(motion.started);
        }
        if let Some(turn) = &mut self.turn {
            turn.start += now - since.max(turn.start);
        }
    }
    pub fn held(&self) -> bool {
        self.held
    }
    pub(super) fn release_hold(&mut self) {
        self.held = false;
    }
    pub fn moving(&self) -> bool {
        self.motion.is_some()
    }
    pub(super) fn patrolling(&self) -> bool {
        self.patrol.is_some()
    }
    pub(super) fn following(&self) -> bool {
        self.follow.is_some()
    }
    pub fn followed_unit(&self) -> Option<HandleId> {
        self.follow.map(|f| f.target)
    }
    /// The admitted range retained by the current accepted follow order.
    pub fn follow_range(&self) -> Option<f32> {
        self.follow.map(|follow| follow.range)
    }
    /// Whether the unit's class admits movement orders at all.
    pub fn movable(&self) -> bool {
        self.movable
    }
    /// Whether the selected speed admits starting a new route.
    pub(super) fn can_start_motion(&self) -> bool {
        self.movable && self.speed.is_some_and(|speed| speed > 0.0)
    }
    /// The effective move speed, after any form or ability change.
    pub fn speed(&self) -> Option<f32> {
        self.speed
    }
    /// The accepted route's waypoints, in travel order, and the index of the next one. Empty
    /// when no route is pending.
    pub fn waypoints(&self) -> (&[[f32; 2]], usize) {
        self.motion
            .as_ref()
            .map_or((&[][..], 0), |m| (m.path.as_slice(), m.next))
    }
    pub fn destination(&self) -> Option<[f32; 2]> {
        self.motion.as_ref().map(|m| m.goal)
    }
    pub(super) fn relocated(&mut self, position: [f32; 2], now: f64) {
        if let Some(motion) = &mut self.motion {
            motion.path.drain(..motion.next);
            motion.next = 0;
            motion.base = position;
            motion.prefix = 0.;
            motion.last_distance = 0.;
            motion.started = now;
        }
    }
}

fn speed(definition: &Definition) -> Result<f32, String> {
    balance_real(definition, "spd")
}

fn clamp_move_speed(speed: f32, (minimum, maximum): (f32, f32)) -> f32 {
    speed.max(minimum).min(maximum)
}

/// One `UnitBalance` numeric column of a resolved unit definition, with map edits.
pub(crate) fn balance_real(definition: &Definition, wanted: &str) -> Result<f32, String> {
    let mut matches = definition.fields.iter().filter(|(id, f)| {
        f.metadata.as_ref().is_some_and(|m|
        m.fourcc==**id && m.from==SlkId::UnitMetaData && matches!(m.index,-1|0)
        && matches!(m.kind,FieldKind::Int|FieldKind::Real|FieldKind::Unreal)
        && matches!(&m.source,FieldSource::Column{slk:SlkId::UnitBalance,column}if column==wanted))
    });
    let (_, field) = matches
        .next()
        .ok_or_else(|| format!("missing unit balance {wanted}"))?;
    if matches.next().is_some() {
        return Err(format!("ambiguous unit balance {wanted}"));
    }
    let metadata = field.metadata.as_ref().expect("selected metadata");
    let value = match field
        .value
        .as_ref()
        .map_err(|e| format!("unit balance {wanted}: {e:?}"))?
    {
        FieldValue::Cell(s) => s.trim().parse::<f32>().map_err(|e| e.to_string())?,
        FieldValue::Override(pcx_objects::Value::Int(n)) if metadata.kind == FieldKind::Int => {
            f32::from_bits(
                crate::script_random::real::from_i32(*n as i32).map_err(|e| e.to_string())?,
            )
        }
        FieldValue::Override(pcx_objects::Value::Real(n)) if metadata.kind == FieldKind::Real => *n,
        FieldValue::Override(pcx_objects::Value::Unreal(n))
            if metadata.kind == FieldKind::Unreal =>
        {
            *n
        }
        _ => return Err(format!("unsupported unit balance {wanted} field")),
    };
    if !value.is_finite() || value < 0. {
        return Err(format!("finite nonnegative unit balance {wanted} required"));
    }
    Ok(value)
}

impl ObjectHost {
    /// Currently executable standard orders, using admitted identifiers and live unit capabilities.
    pub fn standard_command_order(
        &self,
        unit: HandleId,
        kind: crate::standard_command::Kind,
    ) -> Result<Option<i32>, Error> {
        use crate::standard_command::Kind;
        let record = self.executed_unit(unit)?;
        if !record.shown || record.combat.as_ref().is_some_and(|c| c.dead) {
            return Ok(None);
        }
        let Some(state) = self.unit_activity.as_ref() else {
            return Ok(None);
        };
        let movable = record.activity.as_ref().is_some_and(UnitActivity::movable);
        let attack = self.standard_attack_order(unit);
        Ok(match kind {
            Kind::Move if movable => Some(state.rules.movement),
            Kind::Patrol if movable => Some(state.rules.patrol),
            Kind::Hold if movable => Some(state.rules.hold),
            Kind::Stop if movable || attack.is_some() => Some(state.rules.stop),
            Kind::Attack => attack,
            Kind::AttackGround => self.standard_attack_ground_order(unit),
            Kind::Rally => self.standard_rally_order(unit)?,
            _ => None,
        })
    }

    pub(super) fn activity_stop_order(&self) -> Option<i32> {
        self.unit_activity.as_ref().map(|state| state.rules.stop)
    }

    pub(super) fn activity_moving(&self, unit: HandleId) -> Result<bool, Error> {
        Ok(self
            .executed_unit(unit)?
            .activity
            .as_ref()
            .is_some_and(UnitActivity::moving))
    }

    /// Read-only admitted current order, also used by the command-card highlight.
    pub fn current_unit_order(&self, unit: HandleId) -> Result<i32, Error> {
        let name = "GetUnitCurrentOrder";
        let state = self.unit_activity.as_ref().ok_or_else(|| {
            self.inner
                .error(name, Failure::UnitActivity("activity not admitted".into()))
        })?;
        let record = self.executed_unit(unit)?;
        let activity = record.activity.as_ref().ok_or_else(|| {
            self.inner.error(
                name,
                Failure::UnitActivity("activity not initialized".into()),
            )
        })?;
        Ok(record
            .abilities
            .as_ref()
            .and_then(|a| a.autocast_current_order())
            .or(self
                .construction_current_order(unit)?
                .or_else(|| self.morph_current_order(unit))
                .or_else(|| self.mirror_current_order(unit))
                .or_else(|| self.summon_current_order(unit))
                .or_else(|| self.curse_current_order(unit))
                .or_else(|| self.inventory_order(unit))
                .or_else(|| activity.patrol.map(|_| state.rules.patrol))
                .or_else(|| activity.follow.map(|_| state.rules.movement))
                .or_else(|| self.combat_attack_order(unit)))
            .unwrap_or(if activity.moving() {
                state.rules.movement
            } else {
                0
            }))
    }

    pub(super) fn activity_waypoints_empty(&self, unit: HandleId) -> Result<bool, Error> {
        let (path, next) = self
            .executed_unit(unit)?
            .activity
            .as_ref()
            .ok_or_else(|| {
                self.inner.error(
                    crate::playercolor_host::harvest_runtime::ORDER,
                    Failure::UnitActivity("activity missing".into()),
                )
            })?
            .waypoints();
        Ok(next >= path.len())
    }

    pub fn admit_unit_activity(&mut self, rules: Rules, route_work: usize) -> Result<(), Error> {
        let fail = |detail: &str| {
            self.inner
                .error("IssueImmediateOrder", Failure::UnitActivity(detail.into()))
        };
        if self.unit_activity.is_some()
            || self.units.is_none()
            || route_work == 0
            || self.timer_game_time().is_none()
            || self
                .inner
                .arena
                .iter()
                .any(|e| matches!(e, Entry::Unit { .. }))
        {
            return Err(fail(
                "admit activity once before units, with a catalog, clock and work budget",
            ));
        }
        if self
            .order_registry
            .as_ref()
            .is_none_or(|r| r.binding().image_sha256 != rules.binding.image_sha256)
        {
            return Err(fail("selected order registry required"));
        }
        let source = &self
            .inner
            .program
            .natives()
            .get("CreateUnit")
            .expect("admitted units")
            .source;
        for (name, args, result, constant) in [
            (
                "IssueImmediateOrder",
                &["unit", "string"][..],
                "boolean",
                false,
            ),
            (
                "IssueImmediateOrderById",
                &["unit", "integer"][..],
                "boolean",
                false,
            ),
            (
                "IssuePointOrder",
                &["unit", "string", "real", "real"][..],
                "boolean",
                false,
            ),
            (
                "IssuePointOrderById",
                &["unit", "integer", "real", "real"][..],
                "boolean",
                false,
            ),
            (
                "IssuePointOrderLoc",
                &["unit", "string", "location"][..],
                "boolean",
                false,
            ),
            (
                "IssuePointOrderByIdLoc",
                &["unit", "integer", "location"][..],
                "boolean",
                false,
            ),
            (
                "IssueTargetOrder",
                &["unit", "string", "widget"][..],
                "boolean",
                false,
            ),
            (
                "IssueTargetOrderById",
                &["unit", "integer", "widget"][..],
                "boolean",
                false,
            ),
            ("GetUnitCurrentOrder", &["unit"][..], "integer", false),
            ("GetUnitMoveSpeed", &["unit"][..], "real", true),
            ("GetUnitDefaultMoveSpeed", &["unit"][..], "real", true),
            ("SetUnitMoveSpeed", &["unit", "real"][..], "nothing", false),
            ("SetUnitFacing", &["unit", "real"][..], "nothing", false),
            ("SetUnitPathing", &["unit", "boolean"][..], "nothing", false),
            (
                "SetUnitPosition",
                &["unit", "real", "real"][..],
                "nothing",
                false,
            ),
            (
                "SetUnitPositionLoc",
                &["unit", "location"][..],
                "nothing",
                false,
            ),
            (
                "SetUnitFacingTimed",
                &["unit", "real", "real"][..],
                "nothing",
                false,
            ),
            ("GetUnitFacing", &["unit"][..], "real", true),
        ] {
            let Some(declaration) = self.inner.program.natives().get(name) else {
                continue;
            };
            Self::contract(&self.inner, name, args, result, constant, Some("unit"))?;
            if &declaration.source != source {
                return Err(self.inner.error(name, Failure::Source));
            }
        }
        self.unit_activity = Some(State {
            rules,
            route_work,
            active: Vec::new(),
            follow_positions: Vec::new(),
            pending_blocked: Vec::new(),
            pending_refresh: Vec::new(),
            projected_through: self.timer_game_time().expect("admitted clock"),
        });
        Ok(())
    }

    pub(super) fn prepare_unit_activity(
        &self,
        definition: &Definition,
    ) -> Result<Option<UnitActivity>, Error> {
        if self.unit_activity.is_none() {
            return Ok(None);
        }
        let fail = |detail: String| {
            self.inner
                .error("CreateUnit", Failure::UnitActivity(detail))
        };
        let movement = super::unit_positions::text(definition, SlkId::UnitData, "movetp")
            .map_err(|e| fail(e.into()))?;
        let movable = match movement {
            "foot" | "horse" | "hover" | "fly" | "float" | "amph" => true,
            "" | "_" | "none" => false,
            _ => return Err(fail(format!("movement class `{movement}` is not admitted"))),
        };
        let mut speed = movable
            .then(|| speed(definition))
            .transpose()
            .map_err(fail)?;
        if let (Some(value), Some(limits)) = (
            speed.as_mut(),
            self.unit_activity
                .as_ref()
                .and_then(|state| state.rules.move_speed_limits()),
        ) {
            let building = super::unit_types::structure(definition).map_err(fail)?;
            *value = clamp_move_speed(*value, limits.for_building(building));
        }
        Ok(Some(UnitActivity {
            held: false,
            movable,
            speed,
            motion: None,
            patrol: None,
            follow: None,
            turn: None,
            pathing_off: false,
            rally: None,
        }))
    }

    pub(super) fn activity_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        let state = self.unit_activity.as_ref()?;
        self.inner.program.natives().get(name)?;
        if matches!(
            name,
            "SetUnitFacing" | "SetUnitFacingTimed" | "GetUnitFacing"
        ) {
            return Some(self.facing_native(name, args));
        }
        if name == "SetUnitPathing" {
            return Some(self.pathing_native(args));
        }
        if matches!(
            name,
            "GetUnitMoveSpeed" | "GetUnitDefaultMoveSpeed" | "SetUnitMoveSpeed"
        ) {
            return Some(self.move_speed_native(name, args));
        }
        if matches!(name, "SetUnitPosition" | "SetUnitPositionLoc") {
            return Some(self.position_native(name, args));
        }
        let (hold, stop, movement, patrol, route_work) = (
            state.rules.hold,
            state.rules.stop,
            state.rules.movement,
            state.rules.patrol,
            state.route_work,
        );
        let getter = name == "GetUnitCurrentOrder";
        let location = matches!(name, "IssuePointOrderLoc" | "IssuePointOrderByIdLoc");
        let point = location || matches!(name, "IssuePointOrder" | "IssuePointOrderById");
        let by_id = matches!(
            name,
            "IssueImmediateOrderById" | "IssuePointOrderById" | "IssuePointOrderByIdLoc"
        );
        if !getter && !point && !matches!(name, "IssueImmediateOrder" | "IssueImmediateOrderById") {
            return None;
        }
        let fail = |detail: &str| self.inner.error(name, Failure::UnitActivity(detail.into()));
        let wanted = if getter {
            1
        } else if location {
            3
        } else if point {
            4
        } else {
            2
        };
        if args.len() != wanted {
            return Some(Err(fail("arguments do not match the declared native")));
        }
        let order = if getter {
            0
        } else {
            match &args[1] {
                Value::Str(s) if !by_id => self.order_registry.as_ref()?.id(s)?,
                Value::Int(id) if by_id => *id,
                _ => {
                    return Some(Err(fail(
                        "order argument does not match the declared native",
                    )));
                }
            }
        };
        if point
            && let Value::Int(_) | Value::Str(_) = &args[1]
            && self.is_attack_ground_order(order)
        {
            return Some(self.attack_ground_native(name, args, location));
        }
        let rally = point
            && self
                .order_registry
                .as_ref()
                .and_then(|registry| registry.id("setrally"))
                == Some(order);
        if !getter
            && !rally
            && (if point {
                order != movement && order != patrol && !self.is_attack_order(order)
            } else {
                order != hold && order != stop
            })
        {
            return None;
        }
        let unit = match args[0] {
            Value::Null => {
                return Some(Ok(if getter {
                    if name == "GetUnitMoveSpeed" {
                        Value::Real(0.)
                    } else {
                        Value::Int(0)
                    }
                } else {
                    Value::Bool(false)
                }));
            }
            Value::Handle(id) => id,
            _ => return Some(Err(fail("unit handle required"))),
        };
        Some((|| {
            // Removed units retain their family; accepted order forms refuse without reviving them.
            if !getter
                && matches!(
                    self.inner
                        .entry(unit)
                        .map_err(|e| self.inner.error(name, e.failure))?,
                    Entry::RemovedUnit { .. }
                )
            {
                return Ok(Value::Bool(false));
            }
            {
                let record = self.executed_unit(unit).map_err(|e| {
                    self.inner.error(
                        name,
                        Failure::UnitActivity(format!("unit argument {unit:?}: {}", e.failure)),
                    )
                })?;
                if record.activity.is_none() {
                    return Err(self.inner.error(
                        name,
                        Failure::UnitActivity("activity not initialized".into()),
                    ));
                }
                if getter {
                    return Ok(Value::Int(self.current_unit_order(unit)?));
                }
                if record.paused_since.is_some() || record.combat.as_ref().is_some_and(|c| c.dead) {
                    return Ok(Value::Bool(false));
                }
            }
            self.require_morph_interruption(unit, name)?;
            if rally {
                let [x, y] = if location {
                    self.location_point(&args[2], name).map_err(|e| {
                        self.inner.error(
                            name,
                            Failure::UnitActivity(format!(
                                "location argument {:?}: {}",
                                args[2], e.failure
                            )),
                        )
                    })?
                } else {
                    let [_, _, Value::Real(x), Value::Real(y)] = args else {
                        return Err(self.inner.error(name, Failure::Signature));
                    };
                    [*x, *y]
                };
                if !x.is_finite() || !y.is_finite() {
                    return Err(self.inner.error(
                        name,
                        Failure::UnitActivity("finite destination required".into()),
                    ));
                }
                // Activity presence was required above. The write is one
                // assignment. Any refusal is reported after the entry borrow ends.
                let failure = match self.inner.entry_mut(unit)? {
                    Entry::Unit { record } => match record.activity.as_mut() {
                        Some(activity) => {
                            activity.rally = Some([x, y]);
                            None
                        }
                        None => Some(Failure::UnitActivity("activity not initialized".into())),
                    },
                    _ => Some(Failure::WrongFamily),
                };
                return match failure {
                    None => Ok(Value::Bool(true)),
                    Some(failure) => Err(self.inner.error(name, failure)),
                };
            }
            let record = self.executed_unit(unit).map_err(|e| {
                self.inner.error(
                    name,
                    Failure::UnitActivity(format!("unit argument {unit:?}: {}", e.failure)),
                )
            })?;
            let activity = record.activity.as_ref().expect("checked above");
            let now = self.timer_game_time().ok_or_else(|| {
                self.inner
                    .error(name, Failure::UnitActivity("clock not admitted".into()))
            })?;
            let moving = point;
            let motion = if point {
                let [x, y] = if location {
                    self.location_point(&args[2], name).map_err(|e| {
                        self.inner.error(
                            name,
                            Failure::UnitActivity(format!(
                                "location argument {:?}: {}",
                                args[2], e.failure
                            )),
                        )
                    })?
                } else {
                    let [_, _, Value::Real(x), Value::Real(y)] = args else {
                        return Err(self.inner.error(name, Failure::Signature));
                    };
                    [*x, *y]
                };
                if !x.is_finite() || !y.is_finite() {
                    return Err(self.inner.error(
                        name,
                        Failure::UnitActivity("finite destination required".into()),
                    ));
                }
                if !activity.can_start_motion() {
                    return Ok(Value::Bool(false));
                }
                if record.admitted_order.is_some_and(|(_, active)| active) {
                    return Err(self.inner.error(
                        name,
                        Failure::UnitActivity(
                            "active ability speed modifiers are not admitted".into(),
                        ),
                    ));
                }
                let start = record.position.ok_or_else(|| {
                    self.inner.error(
                        name,
                        Failure::UnitActivity("accepted position required".into()),
                    )
                })?;
                let radius = self.unit_collision_radius(unit, name)?;
                let mut work = route_work;
                let path = if activity.pathing_off {
                    vec![[x, y]]
                } else {
                    self.unit_placement
                        .as_ref()
                        .ok_or_else(|| {
                            self.inner.error(
                                name,
                                Failure::UnitActivity("live placement required".into()),
                            )
                        })?
                        .route(unit, start, [x, y], radius, &mut work)
                        .map_err(|e| self.inner.error(name, Failure::UnitActivity(e.to_string())))?
                };
                Some(Box::new(Motion {
                    goal: [x, y],
                    reach: 0.0,
                    path,
                    budget_partial: false,
                    next: 0,
                    base: start,
                    prefix: 0.,
                    last_distance: 0.,
                    started: now,
                }))
            } else {
                if order == hold && !activity.movable {
                    return Ok(Value::Bool(false));
                }
                None
            };
            let patrol_plan = motion
                .as_ref()
                .filter(|_| order == patrol)
                .map(|motion| Patrol {
                    start: motion.base,
                    end: motion.goal,
                    returning: false,
                });
            let attack_destination = if point && (self.is_attack_order(order) || order == patrol) {
                motion.as_ref().map(|m| m.goal)
            } else {
                None
            };
            let was_moving = activity.moving();
            let mut animation = record.animation.clone();
            if (moving || was_moving)
                && let Some(animation) = &mut animation
            {
                animation
                    .select_engine(
                        if moving { "walk" } else { "stand" },
                        now,
                        if moving { activity.speed } else { None },
                    )
                    .map_err(|e| self.inner.error(name, Failure::UnitAnimation(e)))?;
            }
            if point
                && self
                    .unit_activity
                    .as_ref()
                    .expect("activity")
                    .active
                    .binary_search(&unit)
                    .is_err()
            {
                self.unit_activity
                    .as_mut()
                    .expect("activity")
                    .active
                    .try_reserve(1)
                    .map_err(|_| self.inner.error(name, Failure::Capacity))?;
            }
            self.interrupt_unit_construction(unit)?;
            if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
                let activity = record.activity.as_mut().expect("initialized activity");
                activity.held = !point && order == hold;
                activity.motion = motion;
                record.animation = animation;
            }
            let active = &mut self.unit_activity.as_mut().expect("activity").active;
            match (point, active.binary_search(&unit)) {
                (true, Err(at)) => active.insert(at, unit),
                (false, Ok(at)) => {
                    active.remove(at);
                }
                _ => {}
            }
            self.unit_ordered(unit);
            if point && order == movement {
                self.set_explicit_guard_move(unit, true);
            }
            if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
                record.activity.as_mut().expect("activity").patrol = patrol_plan;
            }
            self.cancel_inventory_order(unit);
            self.clear_combat_target(unit);
            if let Entry::Unit { record } = self.inner.entry_mut(unit)?
                && let Some(combat) = &mut record.combat
            {
                combat.attack_destination = attack_destination;
            }
            Ok(Value::Bool(true))
        })())
    }

    /// Project explicit motion at one logical game time. Work is deterministic;
    /// a failure preserves prior effects and the caller must stop this session.
    pub fn advance_unit_activity(&mut self, now: f64, work: &mut usize) -> Result<usize, Error> {
        let moved = self.advance_unit_activity_mandatory(now, work)?;
        self.flush_unit_activity_routes(now, work)?;
        Ok(moved)
    }

    /// Intermediate script wake: optional routing waits until all mandatory
    /// projections sharing this call's work allowance have finished.
    pub(crate) fn advance_unit_activity_mandatory(
        &mut self,
        now: f64,
        work: &mut usize,
    ) -> Result<usize, Error> {
        let result = self.advance_activity(now, work, None);
        if result.is_err() {
            self.discard_unit_activity_routes();
        }
        result
    }

    /// [`Self::advance_unit_activity`] that also appends the accepted spans of this
    /// projection to `spans`. Each span is paid for by at least one unit of `work`,
    /// so a call appends at most its budget on entry. On error the spans appended by
    /// this call are removed; motion effects of the failed call are not rolled back.
    pub fn advance_unit_activity_spans(
        &mut self,
        now: f64,
        work: &mut usize,
        spans: &mut Vec<AcceptedSpan>,
    ) -> Result<usize, Error> {
        let base = spans.len();
        let result = self
            .advance_activity(now, work, Some(spans))
            .and_then(|moved| {
                self.flush_unit_activity_routes(now, work)?;
                Ok(moved)
            });
        if result.is_err() {
            self.discard_unit_activity_routes();
            spans.truncate(base);
        }
        result
    }

    /// Reserve room for a unit that will advance queued animation without motion.
    /// The sorted active list is committed only after the queue append succeeds.
    pub(super) fn reserve_animation_active(&mut self, unit: HandleId) -> Result<(), Failure> {
        let Some(state) = &mut self.unit_activity else {
            return Err(Failure::UnitAnimation(
                "queue requires admitted unit activity".into(),
            ));
        };
        if state.active.binary_search(&unit).is_ok() {
            return Ok(());
        }
        state.active.try_reserve(1).map_err(|_| Failure::Capacity)
    }

    pub(super) fn commit_animation_active(&mut self, unit: HandleId) {
        let Some(state) = &mut self.unit_activity else {
            return;
        };
        if let Err(at) = state.active.binary_search(&unit) {
            state.active.insert(at, unit);
        }
    }

    fn advance_unit_animation(&mut self, unit: HandleId, now: f64) -> Result<(), Error> {
        let result = match self.inner.entry_mut(unit)? {
            Entry::Unit { record } if record.paused_since.is_none() => record
                .animation
                .as_mut()
                .map(|animation| animation.advance(now)),
            Entry::Unit { .. } => None,
            _ => return Err(self.inner.error("unit activity", Failure::WrongFamily)),
        };
        if let Some(Err(detail)) = result {
            return Err(self
                .inner
                .error("unit activity", Failure::UnitAnimation(detail)));
        }
        Ok(())
    }

    #[cfg_attr(
        feature = "profile-cpu",
        tracing::instrument(skip_all, target = "pcx_cpu", name = "simulation.activity")
    )]
    fn advance_activity(
        &mut self,
        now: f64,
        work: &mut usize,
        out: Option<&mut Vec<AcceptedSpan>>,
    ) -> Result<usize, Error> {
        let mut regen_spans = Vec::new();
        let moving_blight = self.initial_blight.is_some()
            && self.executed_units().any(|(_, record)| {
                record
                    .vitals
                    .is_some_and(|v| v.regen.life_type == super::unit_vitals::RegenType::Blight)
                    && record
                        .activity
                        .as_ref()
                        .is_some_and(|a| a.moving() || a.patrolling() || a.following())
            });
        let out = out.or_else(|| moving_blight.then_some(&mut regen_spans));
        let mut sink = out.map(|spans| {
            let base = spans.len();
            SpanSink {
                spans,
                base,
                limit: *work,
            }
        });
        let Some(state) = &self.unit_activity else {
            return Ok(0);
        };
        if !now.is_finite() || now < state.projected_through || self.timer_game_time() != Some(now)
        {
            return Err(self.inner.error(
                "unit activity",
                Failure::UnitActivity("current monotonic game time required".into()),
            ));
        }
        if now == state.projected_through {
            return Ok(0);
        }
        self.advance_unit_decay(now, work)?;
        self.advance_training(now, work)?;
        let starts: Vec<_> = if self.initial_blight.is_some() {
            self.executed_units()
                .filter(|(_, record)| {
                    record
                        .vitals
                        .is_some_and(|v| v.regen.life_type == super::unit_vitals::RegenType::Blight)
                })
                .filter_map(|(unit, record)| record.position.map(|p| (unit, p)))
                .collect()
        } else {
            Vec::new()
        };
        self.advance_unit_sleep(work)?;
        self.advance_divine_shield(now);
        let mut active = std::mem::take(&mut self.unit_activity.as_mut().expect("activity").active);
        // Followers observe the same pre-projection target positions regardless of handle order.
        let mut followed = std::mem::take(
            &mut self
                .unit_activity
                .as_mut()
                .expect("activity")
                .follow_positions,
        );
        followed.extend(active.iter().filter_map(|&unit| {
            let follow = self.executed_unit(unit).ok()?.activity.as_ref()?.follow?;
            Some((unit, self.follow_position(follow.target)))
        }));
        let mut blocked = Vec::new();
        let mut refresh = Vec::new();
        let result = (|| {
            let mut moved = 0;
            for &unit in &active {
                *work = work.checked_sub(1).ok_or_else(|| {
                    let previous = active
                        .binary_search(&unit)
                        .ok()
                        .and_then(|index| index.checked_sub(1))
                        .map(|index| active[index]);
                    self.inner.error(
                        "unit activity",
                        Failure::UnitActivity(format!(
                            "unit tick work exhausted before {unit:?} at {now}; previous active unit {previous:?}",
                        )),
                    )
                })?;
                if let Ok(index) = followed.binary_search_by_key(&unit, |(id, _)| *id)
                    && self.refresh_unit_follow(
                        unit,
                        followed[index].1,
                        work,
                        self.unit_activity
                            .as_ref()
                            .expect("activity")
                            .projected_through,
                        true,
                    )?
                {
                    refresh.push((unit, followed[index].1));
                }
                self.advance_unit_animation(unit, now)?;
                if self
                    .executed_unit(unit)?
                    .activity
                    .as_ref()
                    .is_some_and(|a| a.motion.is_none())
                {
                    let pending = self.next_patrol_motion(unit, now, None, work)?;
                    if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
                        record.activity.as_mut().expect("activity").motion = pending;
                    }
                }
                let (speed, mut motion) = match self.inner.entry_mut(unit)? {
                    Entry::Unit { record } => {
                        let record = &mut **record;
                        if record.paused_since.is_some() {
                            if let Some(position) = record.position {
                                emit(
                                    &mut sink,
                                    unit,
                                    position,
                                    position,
                                    self.unit_activity
                                        .as_ref()
                                        .expect("activity")
                                        .projected_through,
                                    now,
                                )
                                .map_err(|detail| {
                                    self.inner.error(
                                        "unit activity",
                                        Failure::UnitActivity(detail.into()),
                                    )
                                })?;
                            }
                            continue;
                        }
                        let Some(activity) = &mut record.activity else {
                            continue;
                        };
                        if let Some(turn) = activity.turn {
                            record.face = turn.at(now);
                            if turn.done(now) {
                                activity.turn = None;
                            }
                        }
                        if activity.held {
                            continue;
                        }
                        let Some(motion) = activity.motion.take() else {
                            continue;
                        };
                        (
                            activity.speed.expect("moving unit's admitted speed"),
                            motion,
                        )
                    }
                    _ => return Err(self.inner.error("unit activity", Failure::WrongFamily)),
                };
                loop {
                    let result =
                        self.advance_motion(unit, speed, now, &mut motion, work, &mut sink);
                    if motion.next < motion.path.len() {
                        if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
                            record.activity.as_mut().expect("activity").motion = Some(motion);
                        }
                        let (count, deferred) = result?;
                        moved += count;
                        if deferred {
                            let witness = self
                                .executed_unit(unit)?
                                .activity
                                .as_ref()
                                .expect("activity")
                                .motion
                                .clone();
                            blocked.push((unit, witness));
                        }
                        break;
                    }
                    moved += result?.0;
                    if motion.budget_partial && self.construction_owns_motion(unit, &motion)? {
                        if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
                            record.activity.as_mut().expect("activity").motion = Some(motion);
                        }
                        let witness = self
                            .executed_unit(unit)?
                            .activity
                            .as_ref()
                            .expect("activity")
                            .motion
                            .clone();
                        blocked.push((unit, witness));
                        break;
                    }
                    let arrival = credited_time(&motion, speed, motion.last_distance);
                    let Some(next) =
                        self.next_patrol_motion(unit, arrival, Some(motion.goal), work)?
                    else {
                        break;
                    };
                    motion = next;
                }
            }
            Ok(moved)
        })();
        active.retain(|&unit| match self.inner.entry(unit) {
            Ok(Entry::Unit { record }) => {
                record
                    .animation
                    .as_ref()
                    .is_some_and(crate::unit_model::Animation::needs_advance)
                    || record.activity.as_ref().is_some_and(|a| {
                        a.motion.is_some() && !a.held
                            || a.turn.is_some()
                            || a.patrol.is_some()
                            || a.follow.is_some()
                    })
            }
            _ => true,
        });
        self.unit_activity.as_mut().expect("activity").active = active;
        let refresh: Vec<_> = refresh
            .into_iter()
            .filter_map(|(unit, target)| {
                let activity = self.executed_unit(unit).ok()?.activity.as_ref()?;
                Some((unit, target, activity.follow, activity.motion.clone()))
            })
            .collect();
        blocked.retain(|(unit, _)| !refresh.iter().any(|(refresh, ..)| refresh == unit));
        followed.clear();
        self.unit_activity
            .as_mut()
            .expect("activity")
            .follow_positions = followed;
        let result = result.and_then(|moved| {
            self.advance_harvest(now, work)?;
            let spans = sink.as_ref().map(|s| &s.spans[s.base..]).unwrap_or(&[]);
            self.advance_unit_regen_motion(now, Some((spans, &starts)), work)?;
            self.complete_research(now)?;
            self.advance_combat()?;
            self.advance_inventory(work)?;
            self.advance_sound_environments();
            self.advance_construction(now, work)?;
            self.advance_order_queue();
            self.advance_spatial_pool(now);
            self.defer_optional_routes(blocked, refresh)?;
            Ok(moved)
        });
        if result.is_ok() {
            self.unit_activity
                .as_mut()
                .expect("activity")
                .projected_through = now;
        } else if let Some(sink) = &mut sink {
            sink.spans.truncate(sink.base);
        }
        result
    }

    /// Tracked sessions only: report each moving unit's commanded velocity to the spatial
    /// chain owner, then deliver the cleanup deadlines due at `now`. The velocity is the
    /// unit's speed toward its next waypoint in world units per second; any other
    /// movement contributor is named so the owner refuses it.
    fn advance_spatial_pool(&mut self, now: f64) {
        let Some(pool) = self
            .unit_placement
            .as_ref()
            .and_then(crate::unit_placement::Placement::spatial)
        else {
            return;
        };
        let moving: Vec<HandleId> = pool.moving_units().collect();
        let mut units = moving.clone();
        if let Some(state) = &self.unit_activity {
            units.extend_from_slice(&state.active);
        }
        units.sort_unstable();
        units.dedup();
        for unit in units {
            let commanded = self.commanded_velocity(unit);
            if commanded.is_none() && moving.binary_search(&unit).is_err() {
                continue;
            }
            let (velocity, limit) = commanded.unwrap_or(([0.; 2], 0.));
            let contributor = self.movement_contributor(unit);
            if let Some(pool) = self
                .unit_placement
                .as_mut()
                .and_then(crate::unit_placement::Placement::spatial_mut)
            {
                pool.command_velocity(unit, velocity, limit, contributor);
            }
        }
        if let Some(pool) = self
            .unit_placement
            .as_mut()
            .and_then(crate::unit_placement::Placement::spatial_mut)
        {
            pool.cleanup(now);
        }
    }

    /// The unit's speed vector toward its next waypoint and its speed limit; `None`
    /// while it follows no route.
    fn commanded_velocity(&self, unit: HandleId) -> Option<([f32; 2], f32)> {
        let record = self.executed_unit(unit).ok()?;
        let activity = record.activity.as_ref()?;
        let motion = activity.motion.as_ref().filter(|_| !activity.held)?;
        let (position, speed) = (record.position?, activity.speed?);
        let target = *motion.path.get(motion.next)?;
        let delta = [target[0] - position[0], target[1] - position[1]];
        let length = delta[0].hypot(delta[1]);
        (length > 0.0 && speed > 0.0).then(|| (delta.map(|axis| axis / length * speed), speed))
    }

    fn movement_contributor(&self, unit: HandleId) -> Option<&'static str> {
        let record = self.executed_unit(unit).ok()?;
        if record.paused_since.is_some() {
            Some("explicit pause")
        } else if record.is_sleeping() {
            Some("sleep")
        } else if self.has_unqualified_motion_buff(unit) {
            Some("active buff")
        } else {
            None
        }
    }

    fn defer_optional_routes(
        &mut self,
        blocked: Vec<PendingBlocked>,
        refresh: Vec<PendingRefresh>,
    ) -> Result<(), Error> {
        let state = self.unit_activity.as_mut().expect("activity");
        state
            .pending_blocked
            .try_reserve(blocked.len())
            .map_err(|_| self.inner.error("unit activity", Failure::Capacity))?;
        state
            .pending_refresh
            .try_reserve(refresh.len())
            .map_err(|_| self.inner.error("unit activity", Failure::Capacity))?;
        for next in refresh {
            if let Ok(at) = state
                .pending_blocked
                .binary_search_by_key(&next.0, |old| old.0)
            {
                state.pending_blocked.remove(at);
            }
            match state
                .pending_refresh
                .binary_search_by_key(&next.0, |old| old.0)
            {
                Ok(at) => state.pending_refresh[at] = next,
                Err(at) => state.pending_refresh.insert(at, next),
            }
        }
        for next in blocked {
            if let Ok(at) = state
                .pending_refresh
                .binary_search_by_key(&next.0, |old| old.0)
            {
                state.pending_refresh.remove(at);
            }
            match state
                .pending_blocked
                .binary_search_by_key(&next.0, |old| old.0)
            {
                Ok(at) => state.pending_blocked[at] = next,
                Err(at) => state.pending_blocked.insert(at, next),
            }
        }
        Ok(())
    }

    pub(crate) fn discard_unit_activity_routes(&mut self) {
        if let Some(state) = &mut self.unit_activity {
            state.pending_blocked.clear();
            state.pending_refresh.clear();
        }
    }

    #[cfg(test)]
    pub(crate) fn deferred_unit_activity_route_count(&self) -> usize {
        self.unit_activity.as_ref().map_or(0, |state| {
            state.pending_blocked.len() + state.pending_refresh.len()
        })
    }

    /// Flush optional routes once no later mandatory sub-wake can consume this
    /// allowance. Queues are deduplicated by canonical handle and revalidate the
    /// same follow/motion witnesses as immediate projections.
    pub(crate) fn flush_unit_activity_routes(
        &mut self,
        now: f64,
        work: &mut usize,
    ) -> Result<(), Error> {
        self.check_activity_boundary(now, "optional unit routes")?;
        let Some(state) = &mut self.unit_activity else {
            return Ok(());
        };
        let blocked = std::mem::take(&mut state.pending_blocked);
        let refresh = std::mem::take(&mut state.pending_refresh);
        // Optional searches spend only work left after every unit and mandatory consumer.
        let queued = blocked.len() + refresh.len();
        for (index, (unit, target, follow, motion)) in refresh.iter().enumerate() {
            let Ok(record) = self.executed_unit(*unit) else {
                continue;
            };
            if record.paused_since.is_some()
                || record.position.is_none()
                || record.combat.as_ref().is_some_and(|combat| combat.dead)
                || record
                    .activity
                    .as_ref()
                    .is_none_or(|activity| activity.follow != *follow || activity.motion != *motion)
            {
                continue;
            }
            let allowance = *work / (queued - index);
            let mut search_work = allowance;
            let target = follow
                .and_then(|follow| self.follow_position(follow.target))
                .and(*target);
            let result = self.refresh_unit_follow(*unit, target, &mut search_work, now, false);
            *work -= allowance - search_work;
            result?;
        }
        let queued = blocked.len();
        for (index, (unit, witness)) in blocked.into_iter().enumerate() {
            let Ok(record) = self.executed_unit(unit) else {
                continue;
            };
            if record.paused_since.is_some()
                || record.position.is_none()
                || record.combat.as_ref().is_some_and(|combat| combat.dead)
                || record
                    .activity
                    .as_ref()
                    .is_none_or(|activity| activity.motion != witness)
            {
                continue;
            }
            let Some(mut motion) = (match self.inner.entry_mut(unit)? {
                Entry::Unit { record } => record.activity.as_mut().and_then(|a| a.motion.take()),
                _ => None,
            }) else {
                continue;
            };
            let from = self.executed_unit(unit)?.position.expect("moving position");
            let allowance = *work / (queued - index);
            let mut search_work = allowance;
            let result = self.replan_blocked(unit, from, &mut motion, now, &mut search_work, true);
            *work -= allowance - search_work;
            let mut animation_error = None;
            if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
                if matches!(result, Ok(false)) {
                    record.activity.as_mut().expect("activity").motion = None;
                    if let Some(animation) = &mut record.animation {
                        animation_error = animation.select_engine("stand", now, None).err();
                    }
                    let still_active = record
                        .animation
                        .as_ref()
                        .is_some_and(crate::unit_model::Animation::needs_advance)
                        || record.activity.as_ref().is_some_and(|a| {
                            a.turn.is_some() || a.patrol.is_some() || a.follow.is_some()
                        });
                    if !still_active {
                        self.unit_activity
                            .as_mut()
                            .expect("activity")
                            .active
                            .retain(|id| *id != unit);
                    }
                } else {
                    record.activity.as_mut().expect("activity").motion = Some(motion);
                }
            }
            if let Some(error) = animation_error {
                return Err(self
                    .inner
                    .error("unit activity", Failure::UnitAnimation(error)));
            }
            result?;
        }
        Ok(())
    }

    // Script sessions project units before delivering a native. Direct callers must do the same.
    pub(super) fn check_activity_boundary(&self, now: f64, name: &str) -> Result<(), Error> {
        if self
            .unit_activity
            .as_ref()
            .is_some_and(|state| state.projected_through != now)
        {
            return Err(self.inner.error(
                name,
                Failure::UnitActivity(
                    "project activity to the current clock before changing pause".into(),
                ),
            ));
        }
        Ok(())
    }

    /// Route toward `destination`, or stop when `None`, keeping walk/stand animation
    /// and the sorted active list. Returns false for units that cannot move.
    pub(crate) fn steer_unit(
        &mut self,
        unit: HandleId,
        destination: Option<[f32; 2]>,
        name: &str,
    ) -> Result<bool, Error> {
        self.steer_unit_in_range(unit, destination, 0.0, name)
    }

    pub(super) fn prepare_steer_unit_in_range(
        &self,
        unit: HandleId,
        destination: Option<[f32; 2]>,
        reach: f32,
        name: &str,
    ) -> Result<Option<PreparedSteer>, Error> {
        self.prepare_steer_unit_from(unit, None, destination, reach, name)
    }

    pub(super) fn prepare_steer_unit_from(
        &self,
        unit: HandleId,
        start: Option<[f32; 2]>,
        destination: Option<[f32; 2]>,
        reach: f32,
        name: &str,
    ) -> Result<Option<PreparedSteer>, Error> {
        self.prepare_record_steer(
            unit,
            self.executed_unit(unit)?,
            start,
            destination,
            reach,
            name,
        )
    }

    pub(super) fn prepare_record_steer(
        &self,
        unit: HandleId,
        record: &super::ExecutedUnit,
        start: Option<[f32; 2]>,
        destination: Option<[f32; 2]>,
        reach: f32,
        name: &str,
    ) -> Result<Option<PreparedSteer>, Error> {
        let fail = |detail: &str| self.inner.error(name, Failure::UnitActivity(detail.into()));
        let route_work = self
            .unit_activity
            .as_ref()
            .ok_or_else(|| fail("activity not admitted"))?
            .route_work;
        let now = self
            .timer_game_time()
            .ok_or_else(|| fail("clock not admitted"))?;
        let activity = record
            .activity
            .as_ref()
            .ok_or_else(|| fail("activity missing"))?;
        let was_moving = activity.moving();
        let motion = match destination {
            Some(target) => {
                if !activity.can_start_motion() {
                    return Ok(None);
                }
                let start = start
                    .or(record.position)
                    .ok_or_else(|| fail("accepted position required"))?;
                let radius = record.collision_radius.map_err(fail)?;
                let mut work = route_work;
                let (path, budget_partial) = if activity.pathing_off {
                    (
                        vec![crate::unit_placement::approach_point(start, target, reach)],
                        false,
                    )
                } else {
                    match self
                        .unit_placement
                        .as_ref()
                        .ok_or_else(|| fail("live placement required"))?
                        .route_outcome(unit, start, target, radius, reach, &mut work)
                        .map_err(|e| self.inner.error(name, Failure::UnitActivity(e.to_string())))?
                    {
                        crate::unit_placement::RouteOutcome::PartialBudget(path) => (path, true),
                        crate::unit_placement::RouteOutcome::Reached(path)
                        | crate::unit_placement::RouteOutcome::Blocked(path) => (path, false),
                    }
                };
                let target_distance =
                    ((target[0] - start[0]).powi(2) + (target[1] - start[1]).powi(2)).sqrt();
                if path.is_empty()
                    || (path.iter().all(|point| *point == start) && target_distance > reach)
                {
                    return Ok(None);
                }
                Some(Box::new(Motion {
                    goal: target,
                    reach,
                    path,
                    budget_partial,
                    next: 0,
                    base: start,
                    prefix: 0.,
                    last_distance: 0.,
                    started: now,
                }))
            }
            None if !was_moving && !activity.held() => {
                return Ok(Some(PreparedSteer {
                    motion: None,
                    animation: None,
                    no_op: true,
                }));
            }
            None => None,
        };
        let mut animation = record.animation.clone();
        if let Some(animation) = &mut animation {
            animation
                .select_engine(
                    if motion.is_some() { "walk" } else { "stand" },
                    now,
                    if motion.is_some() {
                        activity.speed
                    } else {
                        None
                    },
                )
                .map_err(|e| self.inner.error(name, Failure::UnitAnimation(e)))?;
        }
        Ok(Some(PreparedSteer {
            motion,
            animation,
            no_op: false,
        }))
    }

    pub(super) fn commit_steer_unit(
        &mut self,
        unit: HandleId,
        prepared: PreparedSteer,
        name: &str,
    ) -> Result<(), Error> {
        if prepared.no_op {
            self.set_explicit_guard_move(unit, false);
            return Ok(());
        }
        self.unit_activity
            .as_mut()
            .expect("activity")
            .active
            .try_reserve(1)
            .map_err(|_| self.inner.error(name, Failure::Capacity))?;
        self.set_explicit_guard_move(unit, false);
        let assault_owner = self
            .campaign_ai
            .captain_member_role(unit)
            .filter(|_| self.captain_assault_owned(unit))
            .map(|(player, _)| player);
        self.campaign_ai.clear_captain_route(unit);
        let moving = prepared.motion.is_some();
        if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
            let activity = record.activity.as_mut().expect("activity");
            activity.held = false;
            activity.motion = prepared.motion;
            record.animation = prepared.animation;
        }
        let active = &mut self.unit_activity.as_mut().expect("activity").active;
        if let (true, Err(at)) = (moving, active.binary_search(&unit)) {
            active.insert(at, unit);
        }
        if let Some(player) = assault_owner {
            self.campaign_ai.mark_captain_route(player, unit);
        }
        Ok(())
    }

    pub(super) fn reserve_steer_commits(&mut self, count: usize, name: &str) -> Result<(), Error> {
        self.unit_activity
            .as_mut()
            .ok_or_else(|| {
                self.inner
                    .error(name, Failure::UnitActivity("activity not admitted".into()))
            })?
            .active
            .try_reserve(count)
            .map_err(|_| self.inner.error(name, Failure::Capacity))
    }

    pub(super) fn steer_unit_in_range(
        &mut self,
        unit: HandleId,
        destination: Option<[f32; 2]>,
        reach: f32,
        name: &str,
    ) -> Result<bool, Error> {
        let Some(prepared) = self.prepare_steer_unit_in_range(unit, destination, reach, name)?
        else {
            return Ok(false);
        };
        self.commit_steer_unit(unit, prepared, name)?;
        Ok(true)
    }

    #[cfg(test)]
    pub(crate) fn test_set_route_work(&mut self, work: usize) {
        if let Some(state) = &mut self.unit_activity {
            state.route_work = work;
        }
    }

    #[cfg(test)]
    pub(crate) fn activity_motion_path(&self, unit: HandleId) -> Option<Vec<[f32; 2]>> {
        self.executed_unit(unit)
            .ok()?
            .activity
            .as_ref()?
            .motion
            .as_ref()
            .map(|motion| motion.path.clone())
    }

    pub(super) fn forget_unit_activity(&mut self, unit: HandleId) {
        self.cancel_mirror_cast(unit);
        if let Ok(Entry::Unit { record }) = self.inner.entry_mut(unit) {
            record.ai_harvest_flags = None;
        }
        self.forget_captain_member(unit);
        self.unit_expirations.remove(&unit);
        self.forget_campaign_flee(unit);
        self.cancel_harvest(unit);
        self.cancel_unit_follow(unit);
        self.cancel_summon_cast(unit);
        self.cancel_patrol(unit);
        self.forget_holy_cast(unit);
        self.forget_chain_cast(unit);
        self.forget_curse_cast(unit);
        if let Some(state) = &mut self.unit_activity {
            state.active.retain(|&u| u != unit);
        }
    }

    pub(super) fn cancel_patrol(&mut self, unit: HandleId) {
        if let Ok(Entry::Unit { record }) = self.inner.entry_mut(unit)
            && let Some(activity) = &mut record.activity
            && activity.patrol.take().is_some()
            && let Some(combat) = &mut record.combat
        {
            combat.attack_destination = None;
        }
    }

    /// Continue the current patrol leg, or reverse it after reaching its endpoint.
    /// `at` is the arrival time, preserving unused travel time within this projection.
    fn next_patrol_motion(
        &mut self,
        unit: HandleId,
        at: f64,
        finished: Option<[f32; 2]>,
        work: &mut usize,
    ) -> Result<Option<Box<Motion>>, Error> {
        let record = self.executed_unit(unit)?;
        let Some(activity) = &record.activity else {
            return Ok(None);
        };
        let Some(mut patrol) = activity.patrol else {
            return Ok(None);
        };
        if !activity.can_start_motion() {
            return Ok(None);
        }
        if record.paused_since.is_some()
            || record
                .combat
                .as_ref()
                .is_some_and(|c| c.dead || c.target.is_some())
            || self
                .guards
                .as_ref()
                .and_then(|g| g.get(unit))
                .is_some_and(|g| g.returning)
        {
            return Ok(None);
        }
        let position = record.position.ok_or_else(|| {
            self.inner
                .error("patrol", Failure::UnitState("accepted position required"))
        })?;
        if finished.is_some_and(|goal| goal != patrol.goal() || position != goal) {
            return Ok(None);
        }
        if position == patrol.goal() {
            if patrol.start == patrol.end {
                return Ok(None);
            }
            patrol.returning = !patrol.returning;
        }
        let mut motion = Box::new(Motion {
            goal: patrol.goal(),
            reach: 0.0,
            path: Vec::new(),
            budget_partial: false,
            next: 0,
            base: position,
            prefix: 0.0,
            last_distance: 0.0,
            started: at,
        });
        let pathing_off = activity.pathing_off;
        let speed = activity.speed;
        let mut animation = record.animation.clone();
        let progress = if pathing_off {
            motion.path.push(motion.goal);
            true
        } else {
            self.replan_blocked(unit, position, &mut motion, at, work, false)?
        };
        if progress && let Some(animation) = &mut animation {
            animation
                .select_engine("walk", at, speed)
                .map_err(|e| self.inner.error("patrol", Failure::UnitAnimation(e)))?;
        }
        if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
            record.activity.as_mut().expect("activity").patrol = Some(patrol);
            record.animation = animation;
            if let Some(combat) = &mut record.combat {
                combat.attack_destination = Some(patrol.goal());
            }
        }
        Ok(progress.then_some(motion))
    }

    /// Sets a live unit's move speed after the structure clamp, re-anchoring any route
    /// and the walk animation. Returns the clamped speed. Callers own the policy about
    /// who may change speed.
    pub(super) fn apply_unit_move_speed(
        &mut self,
        unit: HandleId,
        requested: f32,
        name: &str,
    ) -> Result<f32, Error> {
        let fail = |detail: &str| self.inner.error(name, Failure::UnitActivity(detail.into()));
        let limits = self
            .unit_activity
            .as_ref()
            .and_then(|state| state.rules.move_speed_limits())
            .ok_or_else(|| fail("move speed limits not admitted"))?;
        let record = self.executed_unit(unit)?;
        let building =
            super::unit_types::structure(&record.definition).map_err(|detail| fail(&detail))?;
        let speed = clamp_move_speed(requested, limits.for_building(building));
        let position = record
            .position
            .ok_or_else(|| fail("accepted position required"))?;
        let activity = record
            .activity
            .as_ref()
            .ok_or_else(|| fail("activity not initialized"))?;
        let moving = activity.moving();
        let now = self
            .timer_game_time()
            .ok_or_else(|| fail("clock not admitted"))?;
        let mut animation = record.animation.clone();
        if moving
            && speed > 0.
            && let Some(animation) = &mut animation
        {
            animation
                .select_engine("walk", now, Some(speed))
                .map_err(|detail| self.inner.error(name, Failure::UnitAnimation(detail)))?;
        }
        let Entry::Unit { record } = self.inner.entry_mut(unit)? else {
            unreachable!("checked unit");
        };
        let activity = record.activity.as_mut().expect("checked activity");
        activity.relocated(position, now);
        activity.speed = Some(speed);
        record.animation = animation;
        Ok(speed)
    }

    fn move_speed_native(&mut self, name: &str, args: &[Value]) -> Result<Value, Error> {
        let fail = |detail: &str| self.inner.error(name, Failure::UnitActivity(detail.into()));
        let setter = name == "SetUnitMoveSpeed";
        let no_op = || {
            if setter { Value::Null } else { Value::Real(0.) }
        };
        let (unit, requested) = if setter {
            let [unit, speed] = args else {
                return Err(fail("arguments do not match the declared native"));
            };
            let unit = match unit {
                Value::Null => None,
                Value::Handle(unit) => Some(*unit),
                _ => return Err(fail("arguments do not match the declared native")),
            };
            let speed = speed
                .as_real()
                .filter(|speed| speed.is_finite())
                .ok_or_else(|| fail("finite move speed required"))?;
            (unit, Some(speed))
        } else {
            let [unit] = args else {
                return Err(fail("arguments do not match the declared native"));
            };
            let unit = match unit {
                Value::Null => None,
                Value::Handle(unit) => Some(*unit),
                _ => return Err(fail("arguments do not match the declared native")),
            };
            (unit, None)
        };
        let Some(unit) = unit else {
            return Ok(no_op());
        };
        match self
            .inner
            .entry(unit)
            .map_err(|error| self.inner.error(name, error.failure))?
        {
            Entry::Unit { .. } => {}
            Entry::RemovedUnit { .. } => return Ok(no_op()),
            _ => return Err(self.inner.error(name, Failure::WrongFamily)),
        }
        let limits = self
            .unit_activity
            .as_ref()
            .and_then(|state| state.rules.move_speed_limits());
        let record = self.executed_unit(unit)?;
        if let Some(speed) = requested {
            if record.admitted_order.is_some_and(|(_, active)| active)
                || self.unit_speed_modified(unit)
            {
                return Err(fail("active ability speed modifiers are not admitted"));
            }
            self.apply_unit_move_speed(unit, speed, name)?;
            return Ok(Value::Null);
        }
        if name == "GetUnitDefaultMoveSpeed" {
            return speed(&record.definition)
                .map(Value::Real)
                .map_err(|detail| self.inner.error(name, Failure::UnitActivity(detail)));
        }
        if record.admitted_order.is_some_and(|(_, active)| active) || self.unit_speed_modified(unit)
        {
            return Err(fail("active ability speed modifiers are not admitted"));
        }
        let activity = record
            .activity
            .as_ref()
            .ok_or_else(|| fail("activity not initialized"))?;
        let speed = match activity.speed {
            Some(speed) => match limits {
                Some(limits) => {
                    let building = super::unit_types::structure(&record.definition)
                        .map_err(|detail| fail(&detail))?;
                    clamp_move_speed(speed, limits.for_building(building))
                }
                None => speed,
            },
            None => speed(&record.definition)
                .map_err(|detail| self.inner.error(name, Failure::UnitActivity(detail)))?,
        };
        Ok(Value::Real(speed))
    }

    fn pathing_native(&mut self, args: &[Value]) -> Result<Value, Error> {
        let name = "SetUnitPathing";
        let fail = |detail: &str| self.inner.error(name, Failure::UnitActivity(detail.into()));
        let (unit, on) = match args {
            [Value::Null, Value::Bool(_)] => return Ok(Value::Null),
            [Value::Handle(unit), Value::Bool(on)] => (*unit, *on),
            _ => return Err(fail("unit and boolean required")),
        };
        // The native (Legacy 1.29.2.9231, `0x4ab970`) resolves the handle through `0x483940`
        // and jumps to its epilogue when that yields no unit, so a removed unit is a no-op.
        if matches!(
            self.inner
                .entry(unit)
                .map_err(|e| self.inner.error(name, e.failure))?,
            Entry::RemovedUnit { .. }
        ) {
            return Ok(Value::Null);
        }
        let record = self.executed_unit(unit)?;
        let activity = record
            .activity
            .as_ref()
            .ok_or_else(|| fail("activity missing"))?;
        if activity.pathing_off != on {
            return Ok(Value::Null);
        }
        let shown = record.shown;
        let (position, face) = (record.position, record.face);
        if shown && let Some(placement) = &mut self.unit_placement {
            if on {
                let position = position.ok_or_else(|| fail("accepted position required"))?;
                let definition = self.executed_unit(unit)?.definition.clone();
                let geometry = super::unit_positions::geometry(&definition)
                    .map_err(|s| self.inner.error(name, Failure::UnitState(s)))?;
                let placement = self.unit_placement.as_mut().expect("placement");
                let prepared = placement
                    .prepare_show_class(
                        unit,
                        position,
                        geometry.radius,
                        geometry.path,
                        face,
                        geometry.movement,
                    )
                    .map_err(|e| {
                        self.inner
                            .error(name, Failure::UnitPlacement(e.to_string()))
                    })?;
                let relocated = prepared.position;
                if relocated != position {
                    self.require_blight_projection(unit, name)?;
                }
                let placement = self.unit_placement.as_mut().expect("placement");
                placement.commit(unit, prepared);
                if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
                    record.position = Some(relocated);
                }
                if relocated != position {
                    self.record_placement_event(
                        unit,
                        crate::ai_placement::PlacementEventKind::Relocated,
                    );
                }
            } else {
                placement.hide(unit).map_err(|e| {
                    self.inner
                        .error(name, Failure::UnitPlacement(e.to_string()))
                })?;
            }
        }
        if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
            record.activity.as_mut().expect("activity").pathing_off = !on;
        }
        Ok(Value::Null)
    }

    /// Moves a unit to the nearest placement for the point, as `ShowUnit` places a
    /// revealed unit; hidden or pathing-off units take the point as is. A moving
    /// unit keeps its order from the new position (recreation choice).
    fn position_native(&mut self, name: &str, args: &[Value]) -> Result<Value, Error> {
        let fail = |detail: &str| self.inner.error(name, Failure::UnitActivity(detail.into()));
        let (unit, target) = match args {
            [Value::Null, ..] => return Ok(Value::Null),
            [Value::Handle(unit), x, y] => match (x.as_real(), y.as_real()) {
                (Some(x), Some(y)) if x.is_finite() && y.is_finite() => (*unit, [x, y]),
                _ => return Err(fail("finite coordinates required")),
            },
            [Value::Handle(unit), location] => (*unit, self.location_point(location, name)?),
            _ => return Err(fail("unit and point required")),
        };
        // Both selected position wrappers skip relocation after failed unit resolution.
        // Point/location validation still precedes the typed tombstone no-op.
        if matches!(
            self.inner
                .entry(unit)
                .map_err(|e| self.inner.error(name, e.failure))?,
            Entry::RemovedUnit { .. }
        ) {
            return Ok(Value::Null);
        }
        let record = self.executed_unit(unit)?;
        let activity = record
            .activity
            .as_ref()
            .ok_or_else(|| fail("activity missing"))?;
        let occupies = record.shown && !activity.pathing_off && self.unit_placement.is_some();
        let (face, moving, previous) = (record.face, activity.moving(), record.position);
        let mut position = target;
        if occupies {
            let geometry = super::unit_positions::geometry(&record.definition)
                .map_err(|s| self.inner.error(name, Failure::UnitState(s)))?;
            let (radius, path, movement) =
                (geometry.radius, geometry.path.to_owned(), geometry.movement);
            let placement = self.unit_placement.as_mut().expect("placement");
            let prepared = placement
                .prepare_relocation_class(unit, target, radius, &path, face, movement)
                .map_err(|e| {
                    self.inner
                        .error(name, Failure::UnitPlacement(e.to_string()))
                })?;
            position = prepared.position;
            if previous != Some(position) {
                self.require_blight_projection(unit, name)?;
            }
            let placement = self.unit_placement.as_mut().expect("placement");
            placement.commit(unit, prepared);
        } else if previous != Some(position) {
            self.require_blight_projection(unit, name)?;
        }
        let now = self.timer_game_time();
        if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
            record.position = Some(position);
            if let (true, Some(activity), Some(now)) = (moving, &mut record.activity, now) {
                activity.relocated(position, now);
            }
        }
        if previous != Some(position) {
            self.record_placement_event(unit, crate::ai_placement::PlacementEventKind::Relocated);
        }
        Ok(Value::Null)
    }

    /// Null units are ignored by setters and read as zero facing.
    fn facing_native(&mut self, name: &str, args: &[Value]) -> Result<Value, Error> {
        let fail = |detail: &str| self.inner.error(name, Failure::UnitActivity(detail.into()));
        let (unit, rest) = args.split_first().ok_or_else(|| fail("unit required"))?;
        let reals = rest
            .iter()
            .map(|v| v.as_real().filter(|r| r.is_finite()))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| fail("finite reals required"))?;
        let (target, duration) = match (name, reals.as_slice()) {
            ("GetUnitFacing", []) => (0.0, 0.0),
            ("SetUnitFacing", &[angle]) => (angle, 0.0),
            ("SetUnitFacingTimed", &[angle, duration]) => (angle, duration),
            _ => return Err(fail("arguments do not match the declared native")),
        };
        let unit = match unit {
            Value::Handle(unit) => *unit,
            Value::Null => {
                return Ok(if name == "GetUnitFacing" {
                    Value::Real(0.0)
                } else {
                    Value::Null
                });
            }
            _ => return Err(fail("unit required")),
        };
        if matches!(
            self.inner
                .entry(unit)
                .map_err(|e| self.inner.error(name, e.failure))?,
            Entry::RemovedUnit { .. }
        ) {
            return Ok(if name == "GetUnitFacing" {
                Value::Real(0.0)
            } else {
                Value::Null
            });
        }
        let face = self
            .executed_unit(unit)
            .map_err(|e| self.inner.error(name, e.failure))?
            .face;
        if name == "GetUnitFacing" {
            return Ok(Value::Real(face));
        }
        let now = self.timer_game_time().ok_or_else(|| fail("game clock"))?;
        let delta = (target - face + 180.0).rem_euclid(360.0) - 180.0;
        let turn = Turn {
            from: face,
            delta,
            start: now,
            duration: f64::from(duration.max(0.0)),
        };
        if self.executed_unit(unit)?.activity.is_none() {
            return Err(fail("unit has no activity state"));
        }
        let done = turn.done(now);
        if !done
            && self
                .unit_activity
                .as_mut()
                .expect("activity")
                .active
                .try_reserve(1)
                .is_err()
        {
            return Err(self.inner.error(name, Failure::Capacity));
        }
        let Entry::Unit { record } = self.inner.entry_mut(unit)? else {
            unreachable!("checked unit");
        };
        let record = &mut **record;
        let activity = record.activity.as_mut().expect("checked activity");
        if done {
            record.face = turn.at(now);
            activity.turn = None;
        } else {
            activity.turn = Some(turn);
            let state = self.unit_activity.as_mut().expect("activity");
            if let Err(at) = state.active.binary_search(&unit) {
                state.active.insert(at, unit);
            }
        }
        Ok(Value::Null)
    }

    fn construction_owns_motion(&self, unit: HandleId, motion: &Motion) -> Result<bool, Error> {
        Ok(self
            .construction_route_revision(unit, motion.goal, motion.reach)?
            .is_some())
    }

    /// Route a blocked motion from its accepted position to its order goal on the
    /// live collision cache. Work is drawn from the tick budget, capped per route.
    /// Returns false when the route makes no progress, so the unit stops.
    fn replan_blocked(
        &mut self,
        unit: HandleId,
        from: [f32; 2],
        motion: &mut Motion,
        now: f64,
        work: &mut usize,
        defer_exhaustion: bool,
    ) -> Result<bool, Error> {
        let route_work = self.unit_activity.as_ref().expect("activity").route_work;
        let radius = self.unit_collision_radius(unit, "unit activity")?;
        let allowance = route_work.min(*work);
        let mut budget = allowance;
        let construction = self.construction_owns_motion(unit, motion)?;
        let placement = self.unit_placement.as_ref().expect("admitted placement");
        // shortcut: zero-progress searches restart; retain request-owned search state
        // when progress under repeatedly insufficient allowances is required.
        let path = if construction && defer_exhaustion {
            placement.route_outcome_streaming(
                unit,
                from,
                motion.goal,
                radius,
                motion.reach,
                &mut budget,
            )
        } else {
            placement.route_outcome(unit, from, motion.goal, radius, motion.reach, &mut budget)
        };
        *work -= allowance - budget;
        let (path, budget_partial) = match path {
            Ok(crate::unit_placement::RouteOutcome::PartialBudget(path)) => (path, true),
            Ok(
                crate::unit_placement::RouteOutcome::Reached(path)
                | crate::unit_placement::RouteOutcome::Blocked(path),
            ) => (path, false),
            Err(error) if defer_exhaustion && error.work_exhausted() => return Ok(true),
            Err(error) => {
                return Err(self
                    .inner
                    .error("unit activity", Failure::UnitActivity(error.to_string())));
            }
        };
        if path.as_slice() == [from] && !(construction && budget_partial) {
            return Ok(false);
        }
        if construction
            && path.iter().any(|point| *point != from)
            && let Entry::Unit { record } = self.inner.entry_mut(unit)?
        {
            let record = &mut **record;
            if let Some(animation) = &mut record.animation {
                animation
                    .select_engine(
                        "walk",
                        now,
                        record.activity.as_ref().and_then(|activity| activity.speed),
                    )
                    .map_err(|error| {
                        self.inner
                            .error("unit activity", Failure::UnitAnimation(error))
                    })?;
            }
        }
        motion.path = path;
        motion.budget_partial = budget_partial;
        motion.next = 0;
        motion.base = from;
        motion.prefix = 0.;
        motion.last_distance = 0.;
        motion.started = now;
        Ok(true)
    }

    #[cfg_attr(
        feature = "profile-cpu",
        tracing::instrument(skip_all, target = "pcx_cpu", name = "simulation.motion")
    )]
    fn advance_motion(
        &mut self,
        unit: HandleId,
        speed: f32,
        now: f64,
        motion: &mut Motion,
        work: &mut usize,
        sink: &mut Option<SpanSink<'_>>,
    ) -> Result<(usize, bool), Error> {
        let fail = |detail: &str| {
            self.inner
                .error("unit activity", Failure::UnitActivity(detail.into()))
        };
        if now < motion.started {
            return Err(fail("game time precedes the movement order"));
        }
        if speed <= 0.0 {
            return Ok((0, false));
        }
        // Distance is measured from `motion.started`; a replan restarts it at `now`.
        let distance = (now - motion.started) * f64::from(speed);
        let radius = self.unit_collision_radius(unit, "unit activity")?;
        let mut moved = 0;
        while motion.next < motion.path.len() {
            *work = work.checked_sub(1).ok_or_else(|| {
                self.inner.error(
                    "unit activity",
                    Failure::UnitActivity(format!(
                        "movement work exhausted for {unit:?} at {now}: goal {:?}, waypoint {}/{}, started {}, credited {}",
                        motion.goal, motion.next, motion.path.len(), motion.started, motion.last_distance,
                    )),
                )
            })?;
            let target = motion.path[motion.next];
            let delta = [
                f64::from(target[0]) - f64::from(motion.base[0]),
                f64::from(target[1]) - f64::from(motion.base[1]),
            ];
            let length = delta[0].hypot(delta[1]);
            let remaining = (distance - motion.prefix).max(0.);
            let end = remaining >= length;
            let point = if end || length == 0. {
                target
            } else {
                [
                    (f64::from(motion.base[0]) + delta[0] * (remaining / length)) as f32,
                    (f64::from(motion.base[1]) + delta[1] * (remaining / length)) as f32,
                ]
            };
            let from = self.executed_unit(unit)?.position.ok_or_else(|| {
                self.inner.error(
                    "unit activity",
                    Failure::UnitActivity("accepted position required".into()),
                )
            })?;
            let free = self
                .executed_unit(unit)?
                .activity
                .as_ref()
                .is_some_and(|a| a.pathing_off);
            if free {
                let last = end && motion.next + 1 == motion.path.len();
                let stand_at = motion.started + (motion.prefix + length) / f64::from(speed);
                let credited = if end {
                    motion.prefix + length
                } else {
                    distance
                };
                emit(
                    sink,
                    unit,
                    from,
                    point,
                    credited_time(motion, speed, motion.last_distance),
                    credited_time(motion, speed, credited),
                )
                .map_err(|detail| {
                    self.inner
                        .error("unit activity", Failure::UnitActivity(detail.into()))
                })?;
                if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
                    record.position = Some(point);
                    if delta != [0., 0.] {
                        record.face =
                            delta[1].atan2(delta[0]).to_degrees().rem_euclid(360.0) as f32;
                    }
                    if last && let Some(animation) = &mut record.animation {
                        animation
                            .select_engine("stand", stand_at, None)
                            .map_err(|e| {
                                self.inner.error("unit activity", Failure::UnitAnimation(e))
                            })?;
                    }
                }
                moved += 1;
                motion.last_distance = if end {
                    motion.prefix + length
                } else {
                    distance
                };
                if !end {
                    break;
                }
                motion.prefix += length;
                motion.base = target;
                motion.next += 1;
                continue;
            }
            let fraction = self
                .unit_placement
                .as_ref()
                .expect("admitted placement")
                .clear_fraction(unit, from, point, radius, work)
                .map_err(|e| {
                    self.inner
                        .error("unit activity", Failure::UnitActivity(e.to_string()))
                })?;
            if fraction < 1. {
                // Walk to the last clear sample: distance up to it is spent, the rest of the tick is not.
                let mut origin = from;
                if fraction > 0. {
                    let offset = [
                        f64::from(point[0]) - f64::from(from[0]),
                        f64::from(point[1]) - f64::from(from[1]),
                    ];
                    let stop =
                        [0, 1].map(|axis| (f64::from(from[axis]) + offset[axis] * fraction) as f32);
                    let prepared = self
                        .unit_placement
                        .as_ref()
                        .expect("admitted placement")
                        .prepare_motion(unit, stop, radius)
                        .map_err(|e| {
                            self.inner
                                .error("unit activity", Failure::UnitActivity(e.to_string()))
                        })?
                        .ok_or_else(|| {
                            self.inner.error(
                                "unit activity",
                                Failure::UnitActivity("clear sample is no longer clear".into()),
                            )
                        })?;
                    let reached = if end {
                        motion.prefix + length
                    } else {
                        distance
                    };
                    let credited =
                        motion.last_distance + fraction * (reached - motion.last_distance);
                    emit(
                        sink,
                        unit,
                        from,
                        stop,
                        credited_time(motion, speed, motion.last_distance),
                        credited_time(motion, speed, credited),
                    )
                    .map_err(|detail| {
                        self.inner
                            .error("unit activity", Failure::UnitActivity(detail.into()))
                    })?;
                    self.unit_placement
                        .as_mut()
                        .expect("placement")
                        .commit(unit, prepared);
                    motion.last_distance = credited;
                    if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
                        record.position = Some(stop);
                        if delta != [0., 0.] {
                            record.face =
                                delta[1].atan2(delta[0]).to_degrees().rem_euclid(360.0) as f32;
                        }
                    }
                    origin = stop;
                }
                // Stationary from the stop to this tick. A pause rebases `started` below, so this must come first.
                emit(
                    sink,
                    unit,
                    origin,
                    origin,
                    credited_time(motion, speed, motion.last_distance),
                    now,
                )
                .map_err(|detail| {
                    self.inner
                        .error("unit activity", Failure::UnitActivity(detail.into()))
                })?;
                motion.started = now - motion.last_distance / f64::from(speed);
                return Ok((moved, true));
            }
            let prepared = self
                .unit_placement
                .as_ref()
                .ok_or_else(|| {
                    self.inner.error(
                        "unit activity",
                        Failure::UnitActivity("live placement required".into()),
                    )
                })?
                .prepare_motion(unit, point, radius)
                .map_err(|e| {
                    self.inner
                        .error("unit activity", Failure::UnitActivity(e.to_string()))
                })?
                .ok_or_else(|| {
                    self.inner.error(
                        "unit activity",
                        Failure::UnitActivity("clear sample is no longer clear".into()),
                    )
                })?;
            let credited = if end {
                motion.prefix + length
            } else {
                distance
            };
            emit(
                sink,
                unit,
                from,
                point,
                credited_time(motion, speed, motion.last_distance),
                credited_time(motion, speed, credited),
            )
            .map_err(|detail| {
                self.inner
                    .error("unit activity", Failure::UnitActivity(detail.into()))
            })?;
            self.unit_placement
                .as_mut()
                .expect("placement")
                .commit(unit, prepared);
            let mut animation_error = None;
            if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
                record.position = Some(point);
                // shortcut: instantaneous geometric steering; qualify authored turn-rate and acceleration before fidelity claims.
                if delta != [0., 0.] {
                    record.face = delta[1].atan2(delta[0]).to_degrees().rem_euclid(360.0) as f32;
                }
                if end
                    && motion.next + 1 == motion.path.len()
                    && let Some(animation) = &mut record.animation
                {
                    animation_error = animation
                        .select_engine(
                            "stand",
                            motion.started + (motion.prefix + length) / f64::from(speed),
                            None,
                        )
                        .err();
                }
            }
            if let Some(error) = animation_error {
                return Err(self
                    .inner
                    .error("unit activity", Failure::UnitAnimation(error)));
            }
            moved += 1;
            motion.last_distance = if end {
                motion.prefix + length
            } else {
                distance
            };
            if !end {
                break;
            }
            motion.prefix += length;
            motion.base = target;
            motion.next += 1;
        }
        Ok((moved, false))
    }
}
