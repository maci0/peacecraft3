//! Canonical incomplete sites. Public point-order travel is a separate admission layer.
use super::{Entry, Error, EventSubject, Failure, ObjectHost};
use pcx_script::{HandleId, RealOps, Value, ast::BinOp, value::FunctionId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildUnavailable {
    Worker,
    Resources,
    Requirements,
    Limit,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildOption {
    pub kind: i32,
    pub unavailable: Option<BuildUnavailable>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildMenuCommand {
    pub ability: [u8; 4],
    pub art: String,
    pub position: [usize; 2],
    pub hotkey: Option<char>,
    pub tooltip: Result<crate::ability_profile::Tooltip, String>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ConstructionPreview {
    Allowed { position: [f32; 2] },
    Blocked,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConstructionOrder {
    pub revision: u64,
    pub kind: i32,
    pub position: [f32; 2],
    pub site: Option<HandleId>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnitConstruction {
    pub worker: Option<HandleId>,
    pub elapsed: f64,
    pub duration: f64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Start,
    Finish,
    Cancel,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Context {
    pub site: HandleId,
    pub phase: Phase,
}

/// Reserved public request, retained on its canonical worker until site arrival.
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    owner: HandleId,
    kind: i32,
    position: [f32; 2],
    gold: i32,
    lumber: i32,
    duration: i32,
    reach: f32,
    site: Option<HandleId>,
    autonomous: bool,
    linked_mine: Option<HandleId>,
}

struct PreparedBuild {
    request: Request,
    revision: u64,
    steer: super::unit_activity::PreparedSteer,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Site {
    worker: Option<HandleId>,
    duration: f64,
    elapsed: f64,
    through: f64,
    gold: i32,
    lumber: i32,
    initial_pending: bool,
    canceled: bool,
    autonomous: bool,
    last_life: f32,
    damaged: bool,
    fault: Option<String>,
    linked_mine: Option<HandleId>,
}
impl Site {
    pub(super) fn note_damage(&mut self) {
        self.damaged = true;
    }
}

struct Event {
    trigger: HandleId,
    filter: Option<FunctionId>,
    event: HandleId,
    player: Option<HandleId>,
}

impl ObjectHost {
    #[cfg(test)]
    pub(crate) fn construction_test_footprint_loader(
        &mut self,
        loader: impl FnMut(&str) -> Result<crate::unit_placement::Footprint, String>
        + Send
        + Sync
        + 'static,
    ) {
        self.unit_placement = Some(
            self.unit_placement
                .take()
                .expect("test placement")
                .with_footprint_loader(loader),
        );
    }

    fn construction_error(&self, why: impl Into<String>) -> Error {
        self.inner
            .error("construction", Failure::UnitActivity(why.into()))
    }
    fn construction_rules(&self) -> Result<&crate::construction_rules::Rules, Error> {
        self.admitted_construction_rules()
    }
    fn construction_math(&self, op: BinOp, a: f32, b: f32) -> Result<f32, Error> {
        let numeric = self
            .numeric
            .as_ref()
            .ok_or_else(|| self.construction_error("construction numeric policy not admitted"))?;
        if numeric.image_sha256() != &self.construction_rules()?.image_sha256 {
            return Err(self.construction_error("construction numeric/client identities differ"));
        }
        match numeric
            .binary(op, a, b)
            .map_err(|e| self.construction_error(e.to_string()))?
        {
            Value::Real(value) if value.is_finite() => Ok(value),
            _ => Err(self.construction_error("construction arithmetic is not finite real")),
        }
    }
    #[cfg(test)]
    pub(crate) fn clear_construction_test_player_admission(&mut self) {
        self.players = None;
    }

    /// Authored build list and current readiness; malformed admission is an error.
    pub fn unit_build_options(&self, worker: HandleId) -> Result<Vec<BuildOption>, Error> {
        self.construction_rules()?;
        let record = self.executed_unit(worker)?;
        let builds = super::unit_training::profile(&record.definition, "Builds", "unitList")
            .map_err(|why| self.construction_error(why))?;
        let unavailable_worker = self.unit_construction_order(worker)?.is_some()
            || record.construction.is_some()
            || record.construction_request.is_some()
            || record.paused_since.is_some()
            || record.combat.as_ref().is_some_and(|state| state.dead)
            || record.vitals.is_none_or(|vitals| vitals.life <= 0.);
        let owner = record.owner;
        let mut result = Vec::new();
        for token in builds.split(',').filter(|token| !token.trim().is_empty()) {
            if matches!(token.trim(), "_" | "-") {
                continue;
            }
            let code: [u8; 4] = token
                .trim()
                .as_bytes()
                .try_into()
                .map_err(|_| self.construction_error("malformed Builds rawcode"))?;
            if !code.iter().all(u8::is_ascii_graphic) {
                return Err(self.construction_error("malformed Builds rawcode"));
            }
            let kind = i32::from_be_bytes(code);
            if result
                .iter()
                .any(|option: &BuildOption| option.kind == kind)
            {
                continue;
            }
            let definition = self
                .unit_catalog()?
                .resolve(code)
                .map_err(|why| self.construction_error(format!("construction catalog: {why:?}")))?;
            if !super::unit_types::structure(&definition)
                .map_err(|why| self.construction_error(why))?
            {
                return Err(self.construction_error("Builds entry is not a structure"));
            }
            let gold = self.ai_unit_amount(owner, kind, "goldcost")?;
            let lumber = self.ai_unit_amount(owner, kind, "lumbercost")?;
            let bank_gold = self
                .player_resource_value(owner, "PLAYER_STATE_RESOURCE_GOLD")?
                .ok_or_else(|| self.construction_error("construction gold not admitted"))?;
            let bank_lumber = self
                .player_resource_value(owner, "PLAYER_STATE_RESOURCE_LUMBER")?
                .ok_or_else(|| self.construction_error("construction lumber not admitted"))?;
            let unavailable = if unavailable_worker {
                Some(BuildUnavailable::Worker)
            } else if !self.training_prerequisites_met(&definition, owner)? {
                Some(BuildUnavailable::Requirements)
            } else if !self.training_limit_allows(owner, kind)? {
                Some(BuildUnavailable::Limit)
            } else if gold > bank_gold || lumber > bank_lumber {
                Some(BuildUnavailable::Resources)
            } else {
                None
            };
            result.push(BuildOption { kind, unavailable });
        }
        Ok(result)
    }

    pub fn unit_build_menu_ability(&self, worker: HandleId) -> Result<Option<[u8; 4]>, Error> {
        let definition = &self.executed_unit(worker)?.definition;
        let has_builds = definition.fields.values().any(|field| {
            field.metadata.as_ref().is_some_and(|metadata| {
                matches!(&metadata.source, pcx_objects::editor::FieldSource::Profile { key, .. }
                    if key.eq_ignore_ascii_case("Builds"))
            })
        });
        if !has_builds {
            return Ok(None);
        }
        let builds = super::unit_training::profile(definition, "Builds", "unitList")
            .map_err(|why| self.construction_error(why))?;
        let mut builder = false;
        for token in builds.split(',').map(str::trim) {
            if matches!(token, "" | "_" | "-") {
                continue;
            }
            if token.len() != 4 || !token.as_bytes().iter().all(u8::is_ascii_graphic) {
                return Err(self.construction_error("malformed Builds rawcode"));
            }
            builder = true;
        }
        if !builder {
            return Ok(None);
        }
        // Optional card discovery must not require this capability. The retained
        // admission Result remains authoritative for actual construction requests.
        if self.admitted_construction_rules().is_err() {
            return Ok(None);
        }
        Ok(self
            .construction_worker_style(worker)?
            .map(|(ability, _)| ability))
    }

    pub fn unit_build_menu_command(
        &self,
        worker: HandleId,
    ) -> Result<Option<BuildMenuCommand>, Error> {
        let Some(ability) = self.unit_build_menu_ability(worker)? else {
            return Ok(None);
        };
        self.unranked_build_menu_command(ability)
            .map(Some)
            .map_err(|why| self.construction_error(why))
    }

    pub(super) fn construction_facing(&self) -> Result<f32, Error> {
        // Source-authored default unit facing; native construction-facing equivalence remains unqualified.
        self.stock_real_constant("bj_UNIT_FACING")
            .map(|value| value as f32)
            .filter(|value| value.is_finite() && (value / 90.).fract() == 0.)
            .ok_or_else(|| {
                self.construction_error("source default construction facing is not admitted")
            })
    }

    fn require_human_construction_worker(&self, worker: HandleId) -> Result<(), Error> {
        if !self.human_construction_worker(worker)? {
            return Err(self.construction_error("worker construction style is not admitted"));
        }
        Ok(())
    }

    fn human_construction_worker(&self, worker: HandleId) -> Result<bool, Error> {
        // shortcut: source race plus Builds selects this class; native implicit enrollment remains unqualified.
        let ability = self.construction_rules()?.human_build_ability;
        let race = self
            .ability_data_text(ability, "race")
            .map_err(|why| self.construction_error(why))?;
        let worker_race = super::unit_positions::text(
            &self.executed_unit(worker)?.definition,
            pcx_objects::editor::SlkId::UnitData,
            "race",
        )
        .map_err(|why| self.construction_error(why))?;
        if matches!(race.trim(), "" | "_" | "-") {
            return Err(self.construction_error("construction ability race is not admitted"));
        }
        Ok(worker_race.eq_ignore_ascii_case(race.trim()))
    }

    fn construction_worker_style(
        &self,
        worker: HandleId,
    ) -> Result<Option<([u8; 4], bool)>, Error> {
        if self.human_construction_worker(worker)? {
            return Ok(Some((
                self.construction_rules()?.human_build_ability,
                false,
            )));
        }
        let ability = self.construction_rules()?.undead_build_ability;
        let race = self
            .ability_data_text(ability, "race")
            .map_err(|why| self.construction_error(why))?;
        if matches!(race.trim(), "" | "_" | "-") {
            return Err(self.construction_error("construction ability race is not admitted"));
        }
        let worker_race = super::unit_positions::text(
            &self.executed_unit(worker)?.definition,
            pcx_objects::editor::SlkId::UnitData,
            "race",
        )
        .map_err(|why| self.construction_error(why))?;
        Ok(worker_race
            .eq_ignore_ascii_case(race.trim())
            .then_some((ability, true)))
    }

    pub fn preview_unit_construction(
        &mut self,
        worker: HandleId,
        kind: i32,
        point: [f32; 2],
    ) -> Result<ConstructionPreview, Error> {
        Ok(self
            .prepare_construction_request(worker, kind, point)?
            .map_or(ConstructionPreview::Blocked, |prepared| {
                ConstructionPreview::Allowed {
                    position: prepared.request.position,
                }
            }))
    }

    fn construction_preview_placement(
        &mut self,
        worker: HandleId,
        kind: i32,
        point: [f32; 2],
    ) -> Result<ConstructionPreview, Error> {
        if !self
            .unit_build_options(worker)?
            .iter()
            .any(|option| option.kind == kind && option.unavailable.is_none())
        {
            return Ok(ConstructionPreview::Blocked);
        }
        if self.construction_worker_style(worker)?.is_none() {
            return Err(self.construction_error("worker construction style is not admitted"));
        }
        self.construction_facing()?;
        self.construction_placement(worker, kind, point)
    }

    fn construction_placement(
        &mut self,
        worker: HandleId,
        kind: i32,
        point: [f32; 2],
    ) -> Result<ConstructionPreview, Error> {
        let definition = self
            .unit_catalog()?
            .resolve(kind.to_be_bytes())
            .map_err(|why| self.construction_error(format!("construction catalog: {why:?}")))?;
        if super::unit_positions::geometry(&definition)
            .map_err(|why| self.construction_error(why))?
            .radius
            .is_some()
        {
            return Err(self.construction_error("mobile structure construction is not admitted"));
        }
        let path = super::unit_positions::text(
            &definition,
            pcx_objects::editor::SlkId::UnitData,
            "pathTex",
        )
        .map_err(|why| self.construction_error(why))?;
        let mask = |key| -> Result<u8, Error> {
            let value = super::unit_positions::text(
                &definition,
                pcx_objects::editor::SlkId::UnitBalance,
                key,
            )
            .map_err(|why| self.construction_error(why))?;
            let mut mask = 0;
            for token in value.split(',').map(str::trim) {
                mask |= match token {
                    "" | "_" | "-" => 0,
                    // WPM byte flag identity, not a building's authored requirement.
                    "unbuildable" => 0x08,
                    "blighted" => 0x20,
                    _ => {
                        return Err(self.construction_error(format!(
                            "unsupported {key} pathing token {token}"
                        )));
                    }
                };
            }
            Ok(mask)
        };
        let water = super::unit_positions::field(
            &definition,
            pcx_objects::editor::SlkId::UnitData,
            "requireWaterRadius",
            &[
                pcx_objects::editor::FieldKind::Real,
                pcx_objects::editor::FieldKind::Unreal,
                pcx_objects::editor::FieldKind::Int,
            ],
        )
        .map_err(|why| self.construction_error(why))?;
        let water = match water {
            pcx_sim::world::FieldValue::Cell(value) => value.parse::<f32>().ok(),
            pcx_sim::world::FieldValue::Override(
                pcx_objects::Value::Real(value) | pcx_objects::Value::Unreal(value),
            ) => Some(*value),
            pcx_sim::world::FieldValue::Override(pcx_objects::Value::Int(value)) => {
                Some(*value as i32 as f32)
            }
            _ => None,
        }
        .filter(|value| value.is_finite() && *value >= 0.)
        .ok_or_else(|| self.construction_error("invalid construction water radius"))?;
        if water != 0. {
            return Err(self.construction_error("shoreline construction is not admitted"));
        }
        let prevent = mask("preventPlace")?;
        let require = mask("requirePlace")?;
        let face = self.construction_facing()?;
        let now = self.timer_game_time();
        let blight = self.initial_blight.as_ref().zip(now);
        let placement = self.unit_placement.as_mut().ok_or_else(|| {
            self.inner.error(
                "construction",
                Failure::UnitActivity("construction placement not admitted".into()),
            )
        })?;
        let position = if (prevent | require) & 0x20 != 0 {
            placement.construction_position_live(
                (Some(worker), blight),
                point,
                face,
                path,
                prevent,
                require,
            )
        } else {
            placement.construction_position(Some(worker), point, face, path, prevent, require)
        }
        .map_err(|why| {
            self.inner
                .error("construction", Failure::UnitActivity(why.to_string()))
        })?;
        Ok(position.map_or(ConstructionPreview::Blocked, |position| {
            ConstructionPreview::Allowed { position }
        }))
    }

    pub fn construction_request_revision(&self, worker: HandleId) -> Result<u64, Error> {
        Ok(self.executed_unit(worker)?.construction_revision)
    }
    pub fn unit_construction(&self, site: HandleId) -> Result<Option<UnitConstruction>, Error> {
        let record = self.executed_unit(site)?;
        let Some(state) = &record.construction else {
            return Ok(None);
        };
        if let Some(fault) = &state.fault {
            return Err(self.construction_error(fault.clone()));
        }
        Ok(Some(UnitConstruction {
            worker: state.worker,
            elapsed: state.elapsed,
            duration: state.duration,
        }))
    }
    pub(super) fn construction_route_revision(
        &self,
        worker: HandleId,
        goal: [f32; 2],
        reach: f32,
    ) -> Result<Option<u64>, Error> {
        let record = self.executed_unit(worker)?;
        Ok(record
            .construction_request
            .as_ref()
            .filter(|request| request.position == goal && request.reach == reach)
            .map(|_| record.construction_revision))
    }

    /// Sorted workers that have a positioned construction site, one scan for the whole roster.
    /// A worker is in the list exactly when `unit_construction_order` finds the site by its
    /// worker, which is the order it reports for a unit without a construction request.
    pub(super) fn construction_site_workers(&self) -> Vec<HandleId> {
        let mut workers: Vec<HandleId> = self
            .executed_units()
            .filter(|(_, record)| record.position.is_some())
            .filter_map(|(_, record)| record.construction.as_ref()?.worker)
            .collect();
        workers.sort_unstable();
        workers
    }

    pub fn unit_construction_order(
        &self,
        worker: HandleId,
    ) -> Result<Option<ConstructionOrder>, Error> {
        let revision = self.construction_request_revision(worker)?;
        if let Some(request) = &self.executed_unit(worker)?.construction_request {
            return Ok(Some(ConstructionOrder {
                revision,
                kind: request.kind,
                position: request.position,
                site: request.site,
            }));
        }
        Ok(self.executed_units().find_map(|(site, record)| {
            let state = record.construction.as_ref()?;
            (state.worker == Some(worker)).then_some(ConstructionOrder {
                revision,
                kind: i32::from_be_bytes(record.definition.rawcode),
                position: record.position?,
                site: Some(site),
            })
        }))
    }

    fn construction_reach(
        &self,
        worker: HandleId,
        definition: &pcx_sim::world::Definition,
    ) -> Result<f32, Error> {
        let target_radius = Self::catalog_collision_radius(definition)
            .map_err(|why| self.construction_error(why))?;
        let radius = self.unit_collision_radius(worker, "construction")?;
        let path = super::unit_positions::text(
            definition,
            pcx_objects::editor::SlkId::UnitData,
            "pathTex",
        )
        .map_err(|why| self.construction_error(why))?;
        // shortcut: source footprint/collision touching on its grid; qualify native approach geometry separately.
        self.unit_placement
            .as_ref()
            .ok_or_else(|| self.construction_error("construction grid missing"))?
            .construction_reach(path, target_radius, radius)
            .map_err(|why| self.construction_error(why.to_string()))
    }

    fn resume_unit_construction(
        &mut self,
        worker: HandleId,
        site: HandleId,
    ) -> Result<bool, Error> {
        let target = self.executed_unit(site)?;
        let Some(state) = target.construction.clone() else {
            return Ok(false);
        };
        if state.canceled
            || state.initial_pending
            || state.autonomous
            || state.worker.is_some()
            || target.vitals.is_none_or(|vitals| vitals.life <= 0.)
        {
            return Ok(false);
        }
        let record = self.executed_unit(worker)?;
        if record.owner != target.owner
            || self.unit_construction_order(worker)?.is_some()
            || record.paused_since.is_some()
            || record.vitals.is_none_or(|vitals| vitals.life <= 0.)
        {
            return Ok(false);
        }
        self.require_human_construction_worker(worker)?;
        let kind = i32::from_be_bytes(target.definition.rawcode);
        let builds = super::unit_training::profile(&record.definition, "Builds", "unitList")
            .map_err(|why| self.construction_error(why))?;
        if !builds
            .split(',')
            .any(|token| token.trim().as_bytes() == kind.to_be_bytes())
        {
            return Ok(false);
        }
        let owner = record.owner;
        let revision = record
            .construction_revision
            .checked_add(1)
            .ok_or_else(|| self.construction_error("construction revision overflow"))?;
        let position = target
            .position
            .ok_or_else(|| self.construction_error("construction position missing"))?;
        let reach = self.construction_reach(worker, &target.definition)?;
        let Some(steer) =
            self.prepare_steer_unit_in_range(worker, Some(position), reach, "construction")?
        else {
            return Ok(false);
        };
        self.reserve_steer_commits(1, "construction")?;
        self.commit_steer_unit(worker, steer, "construction")?;
        if let Entry::Unit { record } = self.inner.entry_mut(worker)? {
            record.construction_revision = revision;
            record.construction_request = Some(Request {
                owner,
                kind,
                position,
                gold: 0,
                lumber: 0,
                duration: state.duration as i32,
                reach,
                site: Some(site),
                autonomous: false,
                linked_mine: None,
            });
        }
        self.unit_ordered(worker);
        self.clear_combat_target(worker);
        Ok(true)
    }

    fn prepare_construction_request(
        &mut self,
        worker: HandleId,
        kind: i32,
        point: [f32; 2],
    ) -> Result<Option<PreparedBuild>, Error> {
        let ConstructionPreview::Allowed { position } =
            self.construction_preview_placement(worker, kind, point)?
        else {
            return Ok(None);
        };
        let record = self.executed_unit(worker)?;
        let owner = record.owner;
        let revision = record
            .construction_revision
            .checked_add(1)
            .ok_or_else(|| self.construction_error("construction revision overflow"))?;
        let definition = self
            .unit_catalog()?
            .resolve(kind.to_be_bytes())
            .map_err(|why| self.construction_error(format!("construction catalog: {why:?}")))?;
        let used = self
            .unit_food_amount(&definition, "fused")
            .map_err(|why| self.construction_error(why))?;
        if used != 0 {
            return Err(self.construction_error("construction food reservation is not qualified"));
        }
        self.unit_food_amount(&definition, "fmade")
            .map_err(|why| self.construction_error(why))?;
        let duration = self.ai_unit_amount(owner, kind, "bldtm")?;
        if duration <= 0 {
            return Err(self.construction_error("construction duration must be positive"));
        }
        let gold = self.ai_unit_amount(owner, kind, "goldcost")?;
        let lumber = self.ai_unit_amount(owner, kind, "lumbercost")?;
        let autonomous = self
            .construction_worker_style(worker)?
            .ok_or_else(|| self.construction_error("worker construction style is not admitted"))?
            .1;
        let reach = self.construction_reach(worker, &definition)?;
        let Some(steer) =
            self.prepare_steer_unit_in_range(worker, Some(position), reach, "construction")?
        else {
            return Ok(None);
        };
        Ok(Some(PreparedBuild {
            request: Request {
                owner,
                kind,
                position,
                gold,
                lumber,
                duration,
                reach,
                site: None,
                autonomous,
                linked_mine: None,
            },
            revision,
            steer,
        }))
    }

    pub(super) fn request_unit_construction(
        &mut self,
        worker: HandleId,
        kind: i32,
        point: [f32; 2],
    ) -> Result<bool, Error> {
        let Some(PreparedBuild {
            request,
            revision,
            steer,
        }) = self.prepare_construction_request(worker, kind, point)?
        else {
            return Ok(false);
        };
        self.reserve_steer_commits(1, "construction")?;
        let Some(debit) =
            self.prepare_training_debit(request.owner, request.gold, request.lumber, 0, 0)?
        else {
            return Ok(false);
        };
        self.commit_steer_unit(worker, steer, "construction")?;
        self.commit_training_debit(debit);
        if let Entry::Unit { record } = self.inner.entry_mut(worker)? {
            record.construction_revision = revision;
            record.construction_request = Some(request);
        }
        self.unit_ordered(worker);
        self.clear_combat_target(worker);
        self.cancel_inventory_order(worker);
        Ok(true)
    }

    /// Target-order construction on an existing mine. The search candidate is not
    /// the build position: travel and the site use the mine's position.
    pub(crate) fn request_linked_mine_construction(
        &mut self,
        worker: HandleId,
        kind: i32,
        mine: HandleId,
    ) -> Result<bool, Error> {
        if self.unit_construction_order(worker)?.is_some() {
            return Err(self.construction_error(
                "linked mine construction refuses a worker with an admitted build order",
            ));
        }
        let (position, gold_now) = {
            let source = self.executed_unit(mine)?;
            if !source.shown || source.combat.as_ref().is_some_and(|state| state.dead) {
                return Err(self.construction_error("linked mine is not a shown living source"));
            }
            let position = source
                .position
                .ok_or_else(|| self.construction_error("linked mine position missing"))?;
            let gold_now = source
                .resource_amount
                .ok_or_else(|| self.construction_error("linked mine has no resource amount"))?;
            (position, gold_now)
        };
        let _ = gold_now;
        let record = self.executed_unit(worker)?;
        let owner = record.owner;
        let revision = record
            .construction_revision
            .checked_add(1)
            .ok_or_else(|| self.construction_error("construction revision overflow"))?;
        let definition = self
            .unit_catalog()?
            .resolve(kind.to_be_bytes())
            .map_err(|why| self.construction_error(format!("construction catalog: {why:?}")))?;
        let used = self
            .unit_food_amount(&definition, "fused")
            .map_err(|why| self.construction_error(why))?;
        if used != 0 {
            return Err(self.construction_error("construction food reservation is not qualified"));
        }
        self.unit_food_amount(&definition, "fmade")
            .map_err(|why| self.construction_error(why))?;
        let duration = self.ai_unit_amount(owner, kind, "bldtm")?;
        if duration <= 0 {
            return Err(self.construction_error("construction duration must be positive"));
        }
        let gold = self.ai_unit_amount(owner, kind, "goldcost")?;
        let lumber = self.ai_unit_amount(owner, kind, "lumbercost")?;
        let autonomous = self
            .construction_worker_style(worker)?
            .ok_or_else(|| self.construction_error("worker construction style is not admitted"))?
            .1;
        let reach = self.construction_reach(worker, &definition)?;
        let Some(steer) =
            self.prepare_steer_unit_in_range(worker, Some(position), reach, "construction")?
        else {
            return Ok(false);
        };
        self.reserve_steer_commits(1, "construction")?;
        let Some(debit) = self.prepare_training_debit(owner, gold, lumber, 0, 0)? else {
            return Ok(false);
        };
        self.commit_steer_unit(worker, steer, "construction")?;
        self.commit_training_debit(debit);
        if let Entry::Unit { record } = self.inner.entry_mut(worker)? {
            record.construction_revision = revision;
            record.construction_request = Some(Request {
                owner,
                kind,
                position,
                gold,
                lumber,
                duration,
                reach,
                site: None,
                autonomous,
                linked_mine: Some(mine),
            });
        }
        self.unit_ordered(worker);
        self.clear_combat_target(worker);
        self.cancel_inventory_order(worker);
        Ok(true)
    }

    pub(super) fn pending_construction_count(
        &self,
        owner: HandleId,
        kind: i32,
    ) -> Result<i32, Error> {
        self.executed_units()
            .filter(|(_, record)| {
                record.construction_request.as_ref().is_some_and(|request| {
                    request.owner == owner && request.kind == kind && request.site.is_none()
                })
            })
            .try_fold(0i32, |count, _| {
                count
                    .checked_add(1)
                    .ok_or_else(|| self.construction_error("pending construction count overflow"))
            })
    }

    pub(super) fn construction_current_order(
        &self,
        worker: HandleId,
    ) -> Result<Option<i32>, Error> {
        let Some(order) = self.unit_construction_order(worker)? else {
            return Ok(None);
        };
        if order.site.is_none() {
            return Ok(Some(order.kind));
        }
        self.order_registry
            .as_ref()
            .and_then(|registry| registry.id("repair"))
            .map(Some)
            .ok_or_else(|| self.construction_error("source repair order not admitted"))
    }

    pub(crate) fn next_construction_completion(&self, until: f64) -> Option<f64> {
        self.executed_units()
            .filter_map(|(_, record)| {
                let site = record.construction.as_ref()?;
                let worker_ready = if site.autonomous {
                    true
                } else {
                    let worker = self.executed_unit(site.worker?).ok()?;
                    worker.paused_since.is_none()
                        && worker.combat.as_ref().is_none_or(|state| !state.dead)
                };
                (!site.initial_pending
                    && !site.canceled
                    && record.paused_since.is_none()
                    && worker_ready)
                    .then(|| {
                        (site.through + site.duration - site.elapsed)
                            .max(self.timer_game_time().expect("construction clock"))
                    })
            })
            .filter(|at| *at <= until)
            .min_by(f64::total_cmp)
    }

    pub(super) fn construction_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if matches!(name, "IssueTargetOrder" | "IssueTargetOrderById") {
            let [Value::Handle(worker), order, Value::Handle(site)] = args else {
                return None;
            };
            let repair = self.order_registry.as_ref()?.id("repair")?;
            let order = match order {
                Value::Int(id) if name == "IssueTargetOrderById" => Some(*id),
                Value::Str(order) if name == "IssueTargetOrder" => {
                    self.order_registry.as_ref()?.id(order)
                }
                _ => None,
            };
            if order != Some(repair) || self.executed_unit(*site).ok()?.construction.is_none() {
                return None;
            }
            return Some((|| {
                Self::contract(
                    &self.inner,
                    name,
                    &[
                        "unit",
                        if name == "IssueTargetOrder" {
                            "string"
                        } else {
                            "integer"
                        },
                        "widget",
                    ],
                    "boolean",
                    false,
                    None,
                )?;
                if self
                    .inner
                    .program
                    .natives()
                    .get(name)
                    .expect("checked native")
                    .source
                    != self.unit_declaration()?.0
                {
                    return Err(self.inner.error(name, Failure::Source));
                }
                self.resume_unit_construction(*worker, *site)
                    .map(Value::Bool)
            })());
        }
        if !matches!(name, "IssuePointOrderById" | "IssueBuildOrderById") {
            return None;
        }
        let [
            Value::Handle(worker),
            Value::Int(kind),
            Value::Real(x),
            Value::Real(y),
        ] = args
        else {
            return None;
        };
        if !kind.to_be_bytes().iter().all(u8::is_ascii_graphic) {
            return None;
        }
        // `IssueBuildOrderById` is the same build request under its own name (a build order's
        // id is the building's rawcode) and reports whether the request was accepted.
        let build = name == "IssueBuildOrderById";
        Some((|| {
            Self::contract(
                &self.inner,
                name,
                &["unit", "integer", "real", "real"],
                "boolean",
                false,
                None,
            )?;
            let declaration = self
                .inner
                .program
                .natives()
                .get(name)
                .expect("checked native");
            if declaration.source != self.unit_declaration()?.0 {
                return Err(self.inner.error(name, Failure::Source));
            }
            let accepted = self.request_unit_construction(*worker, *kind, [*x, *y])?;
            Ok(Value::Bool(build && accepted))
        })())
    }

    pub(super) fn advance_construction(&mut self, now: f64, work: &mut usize) -> Result<(), Error> {
        let sites: Vec<_> = self
            .executed_units()
            .filter_map(|(unit, record)| record.construction.as_ref().map(|_| unit))
            .collect();
        let requests: Vec<_> = self
            .executed_units()
            .filter_map(|(worker, record)| {
                record
                    .construction_request
                    .clone()
                    .map(|request| (worker, request))
            })
            .collect();
        let count = sites
            .len()
            .checked_add(requests.len())
            .ok_or_else(|| self.construction_error("construction work overflow"))?;
        *work = work
            .checked_sub(count)
            .ok_or_else(|| self.construction_error("construction work budget"))?;
        for site in sites {
            self.advance_unit_construction(site, now)?;
        }
        for (worker, request) in requests {
            if request.site.is_none() && request.linked_mine.is_none() {
                let placement = self.construction_placement(worker, request.kind, request.position);
                if !matches!(placement?, ConstructionPreview::Allowed { position } if position == request.position)
                {
                    self.cancel_construction_request(worker)?;
                    self.steer_unit(worker, None, "construction placement invalidated")?;
                    continue;
                }
            }
            let record = self.executed_unit(worker)?;
            if record.paused_since.is_some()
                || record.combat.as_ref().is_some_and(|state| state.dead)
            {
                continue;
            }
            let Some(position) = record.position else {
                continue;
            };
            if ((position[0] - request.position[0]).powi(2)
                + (position[1] - request.position[1]).powi(2))
            .sqrt()
                > request.reach
            {
                continue;
            }
            self.steer_unit(worker, None, "construction arrival")?;
            if let Some(site) = request.site {
                self.set_construction_worker(site, Some(worker))?;
                if let Entry::Unit { record } = self.inner.entry_mut(worker)? {
                    record.construction_request = None;
                }
            } else {
                // Recheck the same placement after travel; a changed site aborts the reserved request.
                // A linked mine occupies its own cell until construction start hides it.
                let preview_ok = request.linked_mine.is_some()
                    || matches!(
                        self.construction_placement(worker, request.kind, request.position)?,
                        ConstructionPreview::Allowed { position } if position == request.position
                    );
                if !preview_ok
                    || self
                        .begin_unit_construction(worker, request.kind, request.position)?
                        .is_none()
                {
                    self.cancel_construction_request(worker)?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn interrupt_unit_construction(&mut self, unit: HandleId) -> Result<(), Error> {
        self.cancel_construction_request(unit)?;
        if let Some(site) = self
            .unit_construction_order(unit)?
            .and_then(|order| order.site)
        {
            self.set_construction_worker(site, None)?;
        }
        Ok(())
    }

    pub(super) fn forget_construction_unit(&mut self, unit: HandleId) -> Result<(), Error> {
        // Qualified pre-arrival KillUnit and STOP both release the complete reservation without site events.
        self.cancel_construction_request(unit)?;
        let sites: Vec<_> = self
            .executed_units()
            .filter_map(|(site, record)| {
                record
                    .construction
                    .as_ref()
                    .filter(|state| state.worker == Some(unit))
                    .map(|_| site)
            })
            .collect();
        for site in sites {
            if let Entry::Unit { record } = self.inner.entry_mut(site)?
                && let Some(state) = &mut record.construction
            {
                state.worker = None;
            }
        }
        let workers: Vec<_> = self
            .executed_units()
            .filter_map(|(worker, record)| {
                record
                    .construction_request
                    .as_ref()
                    .filter(|request| request.site == Some(unit))
                    .map(|_| worker)
            })
            .collect();
        for worker in workers {
            self.cancel_construction_request(worker)?;
            self.steer_unit(worker, None, "construction target retired")?;
        }
        if let Entry::Unit { record } = self.inner.entry_mut(unit)?
            && let Some(state) = &mut record.construction
        {
            state.worker = None;
        }
        Ok(())
    }

    /// Pre-arrival STOP returns the complete reserved cost and publishes no site event.
    pub(super) fn cancel_construction_request(&mut self, worker: HandleId) -> Result<bool, Error> {
        let Some(request) = self.executed_unit(worker)?.construction_request.clone() else {
            return Ok(false);
        };
        let credit = self
            .prepare_training_change(request.owner, [request.gold, request.lumber], 0, 0)?
            .ok_or_else(|| self.construction_error("reserved construction refund refused"))?;
        if let Entry::Unit { record } = self.inner.entry_mut(worker)? {
            record.construction_request = None;
        }
        self.commit_training_debit(credit);
        Ok(true)
    }

    fn hide_linked_mine(&mut self, mine: HandleId) -> Result<i32, Error> {
        let source = self.executed_unit(mine)?;
        if !source.shown || source.combat.as_ref().is_some_and(|state| state.dead) {
            return Err(self.construction_error("linked mine is not a shown living source"));
        }
        let gold = source
            .resource_amount
            .ok_or_else(|| self.construction_error("linked mine has no resource amount"))?;
        let prepared = self.prepare_unit_visibility(mine, false, "construction")?;
        self.commit_unit_visibility(mine, prepared, "construction")?;
        Ok(gold)
    }

    fn show_linked_mine(&mut self, mine: HandleId) -> Result<(), Error> {
        let prepared = self.prepare_unit_visibility(mine, true, "construction")?;
        self.commit_unit_visibility(mine, prepared, "construction")?;
        Ok(())
    }

    /// Completed-site death with remaining gold restores the original mine.
    /// In-progress death and a depleted amount stay refused.
    pub(super) fn restore_linked_mine_on_death(&mut self, site: HandleId) -> Result<(), Error> {
        let record = self.executed_unit(site)?;
        let Some(mine) = record.linked_mine else {
            return Ok(());
        };
        if record.construction.is_some() {
            return Err(
                self.construction_error("haunted-mine death during construction is not qualified")
            );
        }
        let remaining = record.resource_amount.ok_or_else(|| {
            self.construction_error("linked construction site has no resource amount")
        })?;
        if remaining <= 0 {
            return Err(self.construction_error("haunted-mine depletion teardown is not qualified"));
        }
        let reveal = self.prepare_unit_visibility(mine, true, "construction")?;
        if self.executed_unit(mine)?.resource_amount.is_none() {
            return Err(self.construction_error("linked mine has no resource amount"));
        }
        if let Entry::Unit { record } = self.inner.entry_mut(mine)? {
            *record.resource_amount.as_mut().expect("checked") = remaining;
        } else {
            return Err(self.construction_error("linked mine is no longer a unit"));
        }
        self.commit_unit_visibility(mine, reveal, "construction")?;
        if let Entry::Unit { record } = self.inner.entry_mut(site)? {
            record.linked_mine = None;
        }
        Ok(())
    }

    /// Engine arrival operation, not a public JASS order or proof of worker travel.
    pub(crate) fn begin_unit_construction(
        &mut self,
        worker: HandleId,
        kind: i32,
        position: [f32; 2],
    ) -> Result<Option<HandleId>, Error> {
        let record = self.executed_unit(worker)?;
        if record.construction.is_some()
            || record.paused_since.is_some()
            || record.combat.as_ref().is_some_and(|c| c.dead)
            || record.vitals.is_none_or(|v| v.life <= 0.)
        {
            return Ok(None);
        }
        let request = record.construction_request.clone();
        if request.is_none() && self.unit_construction_order(worker)?.is_some() {
            return Ok(None);
        }
        let owner = record.owner;
        if request
            .as_ref()
            .is_some_and(|request| request.owner != owner || request.kind != kind)
        {
            return Ok(None);
        }
        let face = self.construction_facing()?;
        let definition = self
            .unit_catalog()?
            .resolve(kind.to_be_bytes())
            .map_err(|e| self.construction_error(format!("construction catalog: {e:?}")))?;
        if !super::unit_types::structure(&definition).map_err(|e| self.construction_error(e))? {
            return Ok(None);
        }
        if !self.training_prerequisites_met(&definition, owner)?
            || (request.is_none() && !self.training_limit_allows(owner, kind)?)
        {
            return Ok(None);
        }
        let used = self
            .unit_food_amount(&definition, "fused")
            .map_err(|e| self.construction_error(e))?;
        self.unit_food_amount(&definition, "fmade")
            .map_err(|e| self.construction_error(e))?;
        if used != 0 {
            return Err(self.construction_error("construction food reservation is not qualified"));
        }
        let gold = request.as_ref().map_or_else(
            || self.ai_unit_amount(owner, kind, "goldcost"),
            |request| Ok(request.gold),
        )?;
        let lumber = request.as_ref().map_or_else(
            || self.ai_unit_amount(owner, kind, "lumbercost"),
            |request| Ok(request.lumber),
        )?;
        let duration = request.as_ref().map_or_else(
            || self.ai_unit_amount(owner, kind, "bldtm"),
            |request| Ok(request.duration),
        )?;
        if duration <= 0 {
            return Err(self.construction_error("construction duration must be positive"));
        }
        let fraction = self.construction_rules()?.initial_life_fraction;
        let now = self
            .timer_game_time()
            .ok_or_else(|| self.construction_error("construction clock not admitted"))?;
        let revision = if request.is_some() {
            record.construction_revision
        } else {
            record
                .construction_revision
                .checked_add(1)
                .ok_or_else(|| self.construction_error("construction revision overflow"))?
        };
        let events = self.prepare_construction_events(None, owner, Phase::Start)?;
        let linked = request.as_ref().and_then(|request| request.linked_mine);
        let copied_gold = if let Some(mine) = linked {
            Some(self.hide_linked_mine(mine)?)
        } else {
            None
        };
        let prepared = self.prepare_unit_creation(
            &[
                Value::Handle(owner),
                Value::Int(kind),
                Value::Real(position[0]),
                Value::Real(position[1]),
                Value::Real(face),
            ],
            None,
            super::unit_creation::Birth::Construction,
        );
        let mut prepared = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                if let Some(mine) = linked {
                    self.show_linked_mine(mine)?;
                }
                return Err(error);
            }
        };
        if request.is_some() && prepared.record.position != Some(position) {
            if let Some(mine) = linked {
                self.show_linked_mine(mine)?;
            }
            return Err(
                self.construction_error("construction creation differs from prepared placement")
            );
        }
        if let Some(gold_now) = copied_gold {
            if prepared.record.resource_amount.is_none() {
                if let Some(mine) = linked {
                    self.show_linked_mine(mine)?;
                }
                return Err(
                    self.construction_error("linked construction site has no resource amount")
                );
            }
            prepared.record.resource_amount = Some(gold_now);
            prepared.record.linked_mine = linked;
        }
        let vitals = prepared
            .record
            .vitals
            .ok_or_else(|| self.construction_error("construction vitals not admitted"))?;
        self.construction_math(BinOp::Mul, vitals.max_life, fraction)?;
        let autonomous = request.as_ref().is_some_and(|request| request.autonomous);
        prepared.record.construction = Some(Site {
            worker: (!autonomous).then_some(worker),
            duration: f64::from(duration),
            elapsed: 0.,
            through: now,
            gold,
            lumber,
            initial_pending: true,
            canceled: false,
            autonomous,
            last_life: vitals.life,
            damaged: false,
            fault: None,
            linked_mine: request.as_ref().and_then(|request| request.linked_mine),
        });
        self.inner
            .arena
            .try_reserve(1)
            .map_err(|_| self.construction_error("construction arena capacity"))?;
        let [debit_gold, debit_lumber] = if request.is_some() {
            [0, 0]
        } else {
            [gold, lumber]
        };
        let Some(debit) =
            self.prepare_training_debit(owner, debit_gold, debit_lumber, 0, events.len())?
        else {
            if let Some(mine) = linked {
                self.show_linked_mine(mine)?;
            }
            return Ok(None);
        };
        let site = match self.commit_unit_creation(prepared) {
            Ok(site) => site,
            Err(error) => {
                if let Some(mine) = linked {
                    self.show_linked_mine(mine)?;
                }
                return Err(error);
            }
        };
        self.commit_training_debit(debit);
        self.ai_register_constructed_town(site, owner);
        #[cfg(feature = "test-fixtures")]
        if std::env::var_os("PCX_AI_PLACEMENT_TRACE").is_some() {
            eprintln!(
                "construction start: time={now} worker={worker:?} site={site:?} kind={:?} position={position:?} autonomous={autonomous}",
                kind.to_be_bytes()
            );
        }
        if let Entry::Unit { record } = self.inner.entry_mut(worker)? {
            record.construction_revision = revision;
            record.construction_request = None;
        }
        self.commit_construction_events(
            events,
            Context {
                site,
                phase: Phase::Start,
            },
        );
        self.settle_construction_event(Some(Context {
            site,
            phase: Phase::Start,
        }));
        Ok(Some(site))
    }

    fn completed_blight_growth(
        &self,
        site: HandleId,
        now: f64,
    ) -> Result<Option<(crate::terrain_blight::Growth, bool)>, Error> {
        use pcx_objects::editor::FieldKind;
        let Some(blight) = self.initial_blight.as_ref() else {
            return Ok(None);
        };
        let Some(rules) = blight.growth_rules.as_ref() else {
            return Err(
                self.construction_error(format!("blight growth rules unavailable for {site:?}"))
            );
        };
        let record = self.executed_unit(site)?;
        let Some(abilities) = &record.abilities else {
            return Err(self
                .construction_error(format!("blight source abilities unavailable for {site:?}")));
        };
        let mut result = None;
        for (alias, _) in abilities.levels() {
            let code = self
                .ability_data_text(alias, "code")
                .map_err(|why| self.construction_error(why))?;
            if code.as_bytes() != rules.ability_class {
                continue;
            }
            if result.is_some() {
                return Err(self.construction_error(format!("multiple blight effects on {site:?}")));
            }
            let number = |field| -> Result<f32, Error> {
                self.hero_ability_level_cell(alias, field, 1, FieldKind::Unreal)
                    .or_else(|_| self.hero_ability_level_cell(alias, field, 1, FieldKind::Int))
                    .map_err(|why| self.construction_error(why))?
                    .parse()
                    .map_err(|why| self.construction_error(format!("blight {field}: {why}")))
            };
            let removes_blight = number("DataB")? == 0.0;
            let point = record
                .position
                .ok_or_else(|| self.construction_error("blight source position unavailable"))?;
            let spec = crate::terrain_blight::GrowthSpec {
                spacing: rules.spacing,
                fine_spacing: rules.fine_spacing,
                interval: number("Dur")?.min(rules.interval_cap),
                increment: number("DataA")?,
                target: number("Area")?,
                tolerance: rules.tolerance,
            };
            #[cfg(feature = "test-fixtures")]
            if std::env::var_os("PCX_AI_PLACEMENT_TRACE").is_some() {
                eprintln!(
                    "blight growth: time={now} source={site:?} position={point:?} spec={spec:?}"
                );
            }
            result = Some((
                crate::terrain_blight::Growth::new(point, blight.growth_origin(), now, spec)
                    .map_err(|why| self.construction_error(why))?,
                removes_blight,
            ));
        }
        Ok(result)
    }

    pub(super) fn start_completed_blight_growth(
        &mut self,
        site: HandleId,
        now: f64,
    ) -> Result<(), Error> {
        if let Some((growth, removes_blight)) = self.completed_blight_growth(site, now)? {
            let owner = self.executed_unit(site)?.owner;
            let state = self.initial_blight.as_mut().expect("growth state checked");
            if removes_blight {
                state.add_unqualified_removal(site, owner, growth)
            } else {
                state.add_growth(site, owner, growth)
            }
            .map_err(|why| self.inner.error("construction", Failure::UnitActivity(why)))?;
        }
        Ok(())
    }

    /// Source-clock work projection; activity enrollment and interruption call this shared site.
    pub fn advance_unit_construction(&mut self, site: HandleId, now: f64) -> Result<(), Error> {
        let record = self.executed_unit(site)?;
        let Some(mut state) = record.construction.clone() else {
            return Ok(());
        };
        if let Some(fault) = &state.fault {
            return Err(self.construction_error(fault.clone()));
        }
        if state.initial_pending || state.canceled {
            return Ok(());
        }
        if !now.is_finite() || now < state.through || self.timer_game_time() != Some(now) {
            return Err(
                self.construction_error("construction requires current monotonic game clock")
            );
        }
        let vitals = record
            .vitals
            .ok_or_else(|| self.construction_error("construction vitals not admitted"))?;
        if vitals.life <= 0. || record.combat.as_ref().is_some_and(|c| c.dead) {
            if let Entry::Unit { record } = self.inner.entry_mut(site)?
                && let Some(state) = &mut record.construction
            {
                state.worker = None;
                state.through = now;
            }
            return Ok(());
        }
        let owner = record.owner;
        let mut delta = now - state.through;
        if let Some(worker) = state.worker {
            let worker = self.executed_unit(worker)?;
            if worker.owner != owner || worker.vitals.is_none_or(|v| v.life <= 0.) {
                return Err(
                    self.construction_error("construction worker lifecycle requires cleanup")
                );
            }
            if let Some(since) = worker.paused_since {
                delta = (since - state.through).clamp(0., delta);
            }
        } else if !state.autonomous {
            delta = 0.;
        }
        if let Some(since) = record.paused_since {
            delta = delta.min((since - state.through).max(0.));
        }
        delta = delta.min(state.duration - state.elapsed);
        state.damaged |= vitals.life < state.last_life;
        let fraction = self.construction_rules()?.initial_life_fraction;
        // shortcut: source-duration interpolation; qualify native polling before sub-tick parity claims.
        let growth = self.construction_math(
            BinOp::Mul,
            vitals.max_life,
            self.construction_math(BinOp::Sub, 1., fraction)?,
        )?;
        let growth = self.construction_math(BinOp::Mul, growth, (delta / state.duration) as f32)?;
        let life = self
            .construction_math(BinOp::Add, vitals.life, growth)?
            .min(vitals.max_life);
        state.elapsed += delta;
        state.through = now;
        state.last_life = life;
        let complete = state.elapsed >= state.duration;
        let life = if complete && !state.damaged {
            vitals.max_life
        } else {
            life
        };
        let events = if complete {
            self.prepare_construction_events(Some(site), owner, Phase::Finish)?
        } else {
            Vec::new()
        };
        if complete {
            self.start_completed_blight_growth(site, now)?;
        }
        if let Entry::Unit { record } = self.inner.entry_mut(site)? {
            record.vitals.as_mut().expect("checked").life = life;
            record.construction = (!complete).then_some(state);
        }
        if complete {
            #[cfg(feature = "test-fixtures")]
            if std::env::var_os("PCX_AI_PLACEMENT_TRACE").is_some() {
                eprintln!("construction finish: time={now} site={site:?}");
            }
            self.commit_construction_events(
                events,
                Context {
                    site,
                    phase: Phase::Finish,
                },
            );
        }
        Ok(())
    }

    /// Change participation on an existing canonical site, preserving paid costs and elapsed work.
    pub fn set_construction_worker(
        &mut self,
        site: HandleId,
        worker: Option<HandleId>,
    ) -> Result<bool, Error> {
        let owner = self.executed_unit(site)?.owner;
        if let Some(worker) = worker {
            let record = self.executed_unit(worker)?;
            if record.owner != owner
                || record.construction.is_some()
                || record.vitals.is_none_or(|v| v.life <= 0.)
            {
                return Ok(false);
            }
            if self
                .unit_construction_order(worker)?
                .is_some_and(|order| order.site != Some(site))
            {
                return Ok(false);
            }
        }
        let now = self
            .timer_game_time()
            .ok_or_else(|| self.construction_error("construction clock"))?;
        self.advance_unit_construction(site, now)?;
        if let Entry::Unit { record } = self.inner.entry_mut(site)?
            && let Some(state) = &mut record.construction
        {
            if state.canceled || state.initial_pending || state.autonomous {
                return Ok(false);
            }
            state.worker = worker;
            return Ok(true);
        }
        Ok(false)
    }

    pub fn cancel_unit_construction(&mut self, site: HandleId) -> Result<bool, Error> {
        let record = match self.inner.entry(site)? {
            Entry::Unit { record } => record,
            Entry::RemovedUnit { .. } => return Ok(false),
            _ => return Err(self.construction_error("construction target requires a unit")),
        };
        let Some(state) = &record.construction else {
            return Ok(false);
        };
        if state.canceled {
            return Ok(false);
        }
        let vitals = record
            .vitals
            .ok_or_else(|| self.construction_error("construction vitals"))?;
        if vitals.life <= 0. || record.combat.as_ref().is_some_and(|combat| combat.dead) {
            return Ok(false);
        }
        if state.damaged || vitals.life < state.last_life {
            return Err(self.construction_error("damaged construction refunds are not qualified"));
        }
        if state.linked_mine.is_some() || record.linked_mine.is_some() {
            return Err(self
                .construction_error("haunted-mine cancellation event sequence is not qualified"));
        }
        let owner = record.owner;
        let rate = self.construction_rules()?.refund_rate;
        let numeric = self
            .numeric
            .as_ref()
            .ok_or_else(|| self.construction_error("construction numeric policy"))?;
        let refund = |cost| -> Result<i32, Error> {
            let cost = numeric
                .integer(cost)
                .map_err(|e| self.construction_error(e.to_string()))?;
            let amount = self.construction_math(BinOp::Mul, cost, rate)?;
            if amount < 0. || f64::from(amount) > f64::from(i32::MAX) {
                return Err(self.construction_error("construction refund integer class"));
            }
            Ok(amount.trunc() as i32)
        };
        let money = [refund(state.gold)?, refund(state.lumber)?];
        let events = self.prepare_construction_events(Some(site), owner, Phase::Cancel)?;
        let Some(credit) = self.prepare_training_change(owner, money, 0, events.len())? else {
            return Ok(false);
        };
        if let Entry::Unit { record } = self.inner.entry_mut(site)? {
            record.vitals.as_mut().expect("checked").life = 0.;
            let state = record.construction.as_mut().expect("checked");
            state.worker = None;
            state.canceled = true;
        }
        self.commit_training_debit(credit);
        self.commit_construction_events(
            events,
            Context {
                site,
                phase: Phase::Cancel,
            },
        );
        self.settle_construction_event(Some(Context {
            site,
            phase: Phase::Cancel,
        }));
        Ok(true)
    }

    pub(super) fn settle_construction_event(&mut self, context: Option<Context>) {
        let Some(context) = context else { return };
        if self.construction_event_pending(context) {
            return;
        }
        let result = (|| -> Result<(), Error> {
            let record = match self.executed_unit(context.site) {
                Ok(record) => record,
                Err(_) => return Ok(()),
            };
            let Some(state) = &record.construction else {
                return Ok(());
            };
            if context.phase == Phase::Cancel && state.canceled {
                self.remove_unit_canonical(context.site)?;
                return Ok(());
            }
            if context.phase != Phase::Start || !state.initial_pending || state.canceled {
                return Ok(());
            }
            let vitals = record
                .vitals
                .ok_or_else(|| self.construction_error("construction start vitals"))?;
            if vitals.life <= 0. || record.combat.as_ref().is_some_and(|c| c.dead) {
                return Ok(());
            }
            // Re-read callback-modified maxima; publication preflights the same numeric capability.
            let life = self.construction_math(
                BinOp::Mul,
                vitals.max_life,
                self.construction_rules()?.initial_life_fraction,
            )?;
            if let Entry::Unit { record } = self.inner.entry_mut(context.site)? {
                record.vitals.as_mut().expect("checked").life = life;
                let state = record.construction.as_mut().expect("checked");
                state.last_life = life;
                state.initial_pending = false;
            }
            Ok(())
        })();
        // Trigger actions can mutate the world before a later action fails. Retain that fault explicitly.
        if let Err(error) = result
            && let Ok(Entry::Unit { record }) = self.inner.entry_mut(context.site)
            && let Some(site) = &mut record.construction
        {
            site.fault = Some(error.to_string());
        }
    }

    fn prepare_construction_events(
        &mut self,
        site: Option<HandleId>,
        owner: HandleId,
        phase: Phase,
    ) -> Result<Vec<Event>, Error> {
        let suffix = match phase {
            Phase::Start => "START",
            Phase::Finish => "FINISH",
            Phase::Cancel => "CANCEL",
        };
        let player = self.source_constant_enumerand(
            &format!("EVENT_PLAYER_UNIT_CONSTRUCT_{suffix}"),
            "playerunitevent",
        )?;
        let unit = if phase == Phase::Start {
            None
        } else {
            Some(self.source_constant_enumerand(
                &format!("EVENT_UNIT_CONSTRUCT_{suffix}"),
                "unitevent",
            )?)
        };
        let source = self.unit_declaration()?.0;
        for name in [
            "GetConstructingStructure",
            "GetConstructedStructure",
            "GetCancelledStructure",
        ] {
            if let Some(native) = self.inner.program.natives().get(name) {
                Self::contract(&self.inner, name, &[], "unit", true, None)?;
                if native.source != source {
                    return Err(self.inner.error(name, Failure::Source));
                }
            }
        }
        let mut events = Vec::new();
        for entry in &self.inner.arena {
            let row = match entry {
                Entry::RegisteredUnitEvent {
                    trigger,
                    unit: target,
                    unitevent,
                    enumerand,
                } if Some(*target) == site && Some(*enumerand) == unit => {
                    Some((*trigger, None, *unitevent, None))
                }
                Entry::RegisteredPlayerUnitEvent(r)
                    if r.player == owner && r.enumerand == player =>
                {
                    Some((r.trigger, r.filter, r.playerunitevent, Some(owner)))
                }
                _ => None,
            };
            if let Some((trigger, filter, event, player)) = row {
                let filter = self.resolve_event_filter(filter)?;
                events
                    .try_reserve(1)
                    .map_err(|_| self.construction_error("construction event capacity"))?;
                events.push(Event {
                    trigger,
                    filter,
                    event,
                    player,
                });
            }
        }
        self.reserve_event_delivery(events.len())?;
        Ok(events)
    }
    fn commit_construction_events(&mut self, events: Vec<Event>, context: Context) {
        for event in events {
            self.push_reserved_event(
                event.trigger,
                event.filter,
                EventSubject {
                    unit: Some(context.site),
                    player: event.player,
                    construction: Some(context),
                    ..Default::default()
                },
                Some(event.event),
            );
        }
    }
}
