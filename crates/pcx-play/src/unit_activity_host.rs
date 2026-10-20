use super::{Entry, Error, Failure, ObjectHost};
use crate::unit_activity::Rules;
use pcx_objects::editor::{FieldKind, FieldSource, SlkId};
use pcx_script::{HandleId, Value};
use pcx_sim::world::{Definition, FieldValue};

pub(super) struct State {
    rules: Rules,
    route_work: usize,
    active: Vec<HandleId>,
    projected_through: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UnitActivity {
    held: bool,
    movable: bool,
    speed: Option<f32>,
    motion: Option<Box<Motion>>,
    turn: Option<Turn>,
    /// `SetUnitPathing(false)`: no occupancy, straight motion, no collision checks.
    pathing_off: bool,
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
    path: Vec<[f32; 2]>,
    next: usize,
    base: [f32; 2],
    prefix: f64,
    last_distance: f64,
    started: f64,
}

impl UnitActivity {
    pub fn held(&self) -> bool {
        self.held
    }
    pub(super) fn release_hold(&mut self) {
        self.held = false;
    }
    pub fn moving(&self) -> bool {
        self.motion.is_some()
    }
    pub fn destination(&self) -> Option<[f32; 2]> {
        self.motion.as_ref().and_then(|m| m.path.last().copied())
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

/// One `UnitBalance` numeric column of a resolved unit definition, with map edits.
pub(crate) fn balance_real(definition: &Definition, wanted: &str) -> Result<f32, String> {
    balance_real_with_marker(definition, wanted, None)
}

/// Optional marker semantics must be admitted from the selected numeric reader.
pub(crate) fn balance_real_with_marker(
    definition: &Definition,
    wanted: &str,
    marker: Option<i32>,
) -> Result<f32, String> {
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
        FieldValue::Cell(s) => {
            let s = s.trim();
            if metadata.kind == FieldKind::Int && matches!(s, "-" | "_") {
                let marker =
                    marker.ok_or_else(|| format!("unsupported unit balance {wanted} marker"))?;
                f32::from_bits(
                    crate::script_random::real::from_i32(marker).map_err(|e| e.to_string())?,
                )
            } else {
                s.parse::<f32>().map_err(|e| e.to_string())?
            }
        }
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
            ("GetUnitCurrentOrder", &["unit"][..], "integer", false),
            ("GetUnitMoveSpeed", &["unit"][..], "real", true),
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
            "foot" | "horse" => true,
            "" | "_" | "none" => false,
            _ => return Err(fail("movement class is not admitted".into())),
        };
        Ok(Some(UnitActivity {
            held: false,
            movable,
            speed: movable
                .then(|| speed(definition))
                .transpose()
                .map_err(fail)?,
            motion: None,
            turn: None,
            pathing_off: false,
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
        if matches!(name, "SetUnitPosition" | "SetUnitPositionLoc") {
            return Some(self.position_native(name, args));
        }
        let (hold, stop, movement, route_work) = (
            state.rules.hold,
            state.rules.stop,
            state.rules.movement,
            state.route_work,
        );
        let getter = matches!(name, "GetUnitCurrentOrder" | "GetUnitMoveSpeed");
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
        if !getter
            && (if point {
                order != movement && !self.is_attack_order(order)
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
                    Entry::RemovedUnit
                )
            {
                return Ok(Value::Bool(false));
            }
            let record = self.executed_unit(unit).map_err(|e| {
                self.inner.error(
                    name,
                    Failure::UnitActivity(format!("unit argument {unit:?}: {}", e.failure)),
                )
            })?;
            let activity = record.activity.as_ref().ok_or_else(|| {
                self.inner.error(
                    name,
                    Failure::UnitActivity("activity not initialized".into()),
                )
            })?;
            if getter {
                return Ok(if name == "GetUnitCurrentOrder" {
                    Value::Int(
                        match self
                            .inventory_order(unit)
                            .or_else(|| self.combat_attack_order(unit))
                        {
                            Some(attack) => attack,
                            None if activity.moving() => movement,
                            None => 0,
                        },
                    )
                } else {
                    if record.admitted_order.is_some_and(|(_, active)| active) {
                        return Err(self.inner.error(
                            name,
                            Failure::UnitActivity(
                                "active ability speed modifiers are not admitted".into(),
                            ),
                        ));
                    }
                    Value::Real(match activity.speed {
                        Some(speed) => speed,
                        None => speed(&record.definition).map_err(|detail| {
                            self.inner.error(name, Failure::UnitActivity(detail))
                        })?,
                    })
                });
            }
            if record.combat.as_ref().is_some_and(|c| c.dead) {
                return Ok(Value::Bool(false));
            }
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
                if !activity.movable || activity.speed == Some(0.) {
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
                    path,
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
            let attack_destination = if point && self.is_attack_order(order) {
                motion.as_ref().and_then(|m| m.path.last().copied())
            } else {
                None
            };
            let was_moving = activity.moving();
            let mut animation = record.animation.clone();
            if (moving || was_moving)
                && let Some(animation) = &mut animation
            {
                animation
                    .select(if moving { "walk" } else { "stand" }, now)
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
        let mut active = std::mem::take(&mut self.unit_activity.as_mut().expect("activity").active);
        let result = (|| {
            let mut moved = 0;
            for &unit in &active {
                *work = work.checked_sub(1).ok_or_else(|| {
                    self.inner.error(
                        "unit activity",
                        Failure::UnitActivity("unit tick work exhausted".into()),
                    )
                })?;
                let (speed, mut motion) = match self.inner.entry_mut(unit)? {
                    Entry::Unit { record } => {
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
                let result = self.advance_motion(unit, speed, now, &mut motion, work);
                if let Entry::Unit { record } = self.inner.entry_mut(unit)?
                    && motion.next < motion.path.len()
                {
                    record.activity.as_mut().expect("activity").motion = Some(motion);
                }
                moved += result?;
            }
            Ok(moved)
        })();
        active.retain(|&unit| match self.inner.entry(unit) {
            Ok(Entry::Unit { record }) => record
                .activity
                .as_ref()
                .is_some_and(|a| a.motion.is_some() && !a.held || a.turn.is_some()),
            _ => true,
        });
        self.unit_activity.as_mut().expect("activity").active = active;
        let result = result.and_then(|moved| {
            self.advance_combat()?;
            self.advance_inventory(work)?;
            Ok(moved)
        });
        if result.is_ok() {
            self.unit_activity
                .as_mut()
                .expect("activity")
                .projected_through = now;
        }
        result
    }

    /// Route toward `destination`, or stop when `None`, keeping walk/stand animation
    /// and the sorted active list. Returns false for units that cannot move.
    pub(super) fn steer_unit(
        &mut self,
        unit: HandleId,
        destination: Option<[f32; 2]>,
        name: &str,
    ) -> Result<bool, Error> {
        let fail = |detail: &str| self.inner.error(name, Failure::UnitActivity(detail.into()));
        let route_work = self
            .unit_activity
            .as_ref()
            .ok_or_else(|| fail("activity not admitted"))?
            .route_work;
        let now = self
            .timer_game_time()
            .ok_or_else(|| fail("clock not admitted"))?;
        let record = self.executed_unit(unit)?;
        let activity = record
            .activity
            .as_ref()
            .ok_or_else(|| fail("activity missing"))?;
        let was_moving = activity.moving();
        let motion = match destination {
            Some(target) => {
                if !activity.movable || activity.speed == Some(0.) {
                    return Ok(false);
                }
                let start = record
                    .position
                    .ok_or_else(|| fail("accepted position required"))?;
                let radius = self.unit_collision_radius(unit, name)?;
                let mut work = route_work;
                let path = if activity.pathing_off {
                    vec![target]
                } else {
                    self.unit_placement
                        .as_ref()
                        .ok_or_else(|| fail("live placement required"))?
                        .route(unit, start, target, radius, &mut work)
                        .map_err(|e| self.inner.error(name, Failure::UnitActivity(e.to_string())))?
                };
                Some(Box::new(Motion {
                    path,
                    next: 0,
                    base: start,
                    prefix: 0.,
                    last_distance: 0.,
                    started: now,
                }))
            }
            None if !was_moving => return Ok(true),
            None => None,
        };
        let mut animation = record.animation.clone();
        if let Some(animation) = &mut animation {
            animation
                .select(if motion.is_some() { "walk" } else { "stand" }, now)
                .map_err(|e| self.inner.error(name, Failure::UnitAnimation(e)))?;
        }
        let state = self.unit_activity.as_mut().expect("activity");
        state
            .active
            .try_reserve(1)
            .map_err(|_| self.inner.error(name, Failure::Capacity))?;
        let moving = motion.is_some();
        if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
            let activity = record.activity.as_mut().expect("activity");
            activity.held = false;
            activity.motion = motion;
            record.animation = animation;
        }
        let active = &mut self.unit_activity.as_mut().expect("activity").active;
        if let (true, Err(at)) = (moving, active.binary_search(&unit)) {
            active.insert(at, unit);
        }
        Ok(true)
    }

    pub(super) fn forget_unit_activity(&mut self, unit: HandleId) {
        if let Some(state) = &mut self.unit_activity {
            state.active.retain(|&u| u != unit);
        }
    }

    fn pathing_native(&mut self, args: &[Value]) -> Result<Value, Error> {
        let name = "SetUnitPathing";
        let fail = |detail: &str| self.inner.error(name, Failure::UnitActivity(detail.into()));
        let (unit, on) = match args {
            [Value::Null, Value::Bool(_)] => return Ok(Value::Null),
            [Value::Handle(unit), Value::Bool(on)] => (*unit, *on),
            _ => return Err(fail("unit and boolean required")),
        };
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
                let (radius, path) = super::unit_positions::geometry(&definition)
                    .map_err(|s| self.inner.error(name, Failure::UnitState(s)))?;
                let placement = self.unit_placement.as_mut().expect("placement");
                let prepared = placement
                    .prepare_show(unit, position, radius, path, face)
                    .map_err(|e| {
                        self.inner
                            .error(name, Failure::UnitPlacement(e.to_string()))
                    })?;
                let relocated = prepared.position;
                placement.commit(unit, prepared);
                if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
                    record.position = Some(relocated);
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
        let record = self.executed_unit(unit)?;
        let activity = record
            .activity
            .as_ref()
            .ok_or_else(|| fail("activity missing"))?;
        let occupies = record.shown && !activity.pathing_off && self.unit_placement.is_some();
        let (face, moving) = (record.face, activity.moving());
        let mut position = target;
        if occupies {
            let (radius, path) = super::unit_positions::geometry(&record.definition)
                .map_err(|s| self.inner.error(name, Failure::UnitState(s)))?;
            let path = path.to_owned();
            let placement = self.unit_placement.as_mut().expect("placement");
            let prepared = placement
                .prepare_relocation(unit, target, radius, &path, face)
                .map_err(|e| {
                    self.inner
                        .error(name, Failure::UnitPlacement(e.to_string()))
                })?;
            position = prepared.position;
            placement.commit(unit, prepared);
        }
        let now = self.timer_game_time();
        if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
            record.position = Some(position);
            if let (true, Some(activity), Some(now)) = (moving, &mut record.activity, now) {
                activity.relocated(position, now);
            }
        }
        Ok(Value::Null)
    }

    /// Null units are ignored by setters and read as zero facing.
    fn facing_native(&mut self, name: &str, args: &[Value]) -> Result<Value, Error> {
        let fail = |detail: &str| self.inner.error(name, Failure::UnitActivity(detail.into()));
        let (unit, rest) = match args.split_first() {
            Some((Value::Handle(unit), rest)) => (*unit, rest),
            Some((Value::Null, _)) => {
                return Ok(if name == "GetUnitFacing" {
                    Value::Real(0.0)
                } else {
                    Value::Null
                });
            }
            _ => return Err(fail("unit required")),
        };
        let face = self
            .executed_unit(unit)
            .map_err(|e| self.inner.error(name, e.failure))?
            .face;
        let reals = rest
            .iter()
            .map(|v| v.as_real().filter(|r| r.is_finite()))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| fail("finite reals required"))?;
        let (target, duration) = match (name, reals.as_slice()) {
            ("GetUnitFacing", []) => return Ok(Value::Real(face)),
            ("SetUnitFacing", &[angle]) => (angle, 0.0),
            ("SetUnitFacingTimed", &[angle, duration]) => (angle, duration),
            _ => return Err(fail("arguments do not match the declared native")),
        };
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

    fn advance_motion(
        &mut self,
        unit: HandleId,
        speed: f32,
        now: f64,
        motion: &mut Motion,
        work: &mut usize,
    ) -> Result<usize, Error> {
        let fail = |detail: &str| {
            self.inner
                .error("unit activity", Failure::UnitActivity(detail.into()))
        };
        if now < motion.started {
            return Err(fail("game time precedes the movement order"));
        }
        let distance = (now - motion.started) * f64::from(speed);
        let radius = self.unit_collision_radius(unit, "unit activity")?;
        let mut moved = 0;
        while motion.next < motion.path.len() {
            *work = work.checked_sub(1).ok_or_else(|| {
                self.inner.error(
                    "unit activity",
                    Failure::UnitActivity("movement work exhausted".into()),
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
                if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
                    record.position = Some(point);
                    if delta != [0., 0.] {
                        record.face =
                            delta[1].atan2(delta[0]).to_degrees().rem_euclid(360.0) as f32;
                    }
                    if last && let Some(animation) = &mut record.animation {
                        animation.select("stand", stand_at).map_err(|e| {
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
            if !self
                .unit_placement
                .as_ref()
                .expect("admitted placement")
                .segment_clear(unit, from, point, radius, work)
                .map_err(|e| {
                    self.inner
                        .error("unit activity", Failure::UnitActivity(e.to_string()))
                })?
            {
                motion.started = now - motion.last_distance / f64::from(speed);
                break;
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
                })?;
            let Some(prepared) = prepared else {
                motion.started = now - motion.last_distance / f64::from(speed);
                break;
            };
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
                        .select(
                            "stand",
                            motion.started + (motion.prefix + length) / f64::from(speed),
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
        Ok(moved)
    }
}
