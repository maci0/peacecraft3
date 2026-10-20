use super::{Entry, Error, Failure, ObjectHost, below};
use crate::map_admission::ParsedMapContext;
use crate::neutral_player::{
    BJ_MAX_PLAYER_SLOTS_NATIVE_NAME, NeutralError, RuleProblem, SelectedMapNeutralHost,
    VerifiedNeutralRule,
};
use pcx_script::{HandleId, Value};
use std::sync::Arc;

#[derive(Clone, Copy, Debug)]
pub struct PlayerBudget {
    pub max_slots: usize,
    /// Aggregate retained name bytes, not a native name length or peak-heap bound.
    pub name_bytes: usize,
}
#[derive(Debug, thiserror::Error)]
pub enum PlayerAdmissionError {
    #[error(transparent)]
    Source(#[from] NeutralError),
    #[error(transparent)]
    Contract(#[from] Error),
}
/// Explicit player setup and caller-qualified slot state; no implicit lobby defaults.
#[derive(Debug)]
pub struct PlayerSettings {
    pub identity: HandleId,
    pub number: i32,
    pub team: Option<i32>,
    pub start_location: Option<i32>,
    /// Whether a force-start command has been executed for this player.
    pub start_is_forced: bool,
    pub color: Option<HandleId>,
    pub race_preference: Option<HandleId>,
    pub race_selectable: Option<bool>,
    pub controller: Option<HandleId>,
    /// Caller-qualified current slot state; absence is unset, not empty.
    pub slot_state: Option<HandleId>,
    pub name: Option<String>,
    pub on_score_screen: Option<bool>,
    /// Executed `SetPlayerTechMaxAllowed` pairs. Absence is unset, not a default limit.
    pub tech_max: Vec<(i32, i32)>,
    /// Executed `SetPlayerTechResearched` pairs. Absence is unset, not level 0.
    pub tech_researched: Vec<(i32, i32)>,
    /// Applied upgrade counters in the opt-in fresh-game model; raw requests stay above.
    pub research_levels: Vec<(i32, i32)>,
    /// Executed directed `(other player, alliance setting, value)` assignments.
    /// Absence is unset; `false` is an explicit assignment, never a mirrored default.
    pub alliances: Vec<(HandleId, HandleId, bool)>,
    /// Last executed `SetPlayerState` request for each symbolic setting.
    /// These signed inputs are not effective resource balances or fabricated defaults.
    pub state_requests: Vec<(HandleId, i32)>,
    /// Assigned, source-clamped XP factor. None is unknown initial player state.
    pub experience_handicap: Option<f32>,
}
/// Caller bound on stored `(player, techid)` records. Not a native or build limit.
#[derive(Clone, Copy, Debug)]
pub struct TechLimitBudget {
    pub max_records: usize,
}
/// A research level from the admitted model or an explicit setter record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TechRead {
    /// Player rows were not admitted, or research was not admitted.
    Unavailable,
    /// The player row or the tech pair is unset.
    Absent,
    /// The applied counter, or the executed `setToLevel` in record-only mode.
    Level(i32),
}
/// Caller bound on distinct directed alliance assignments across all player rows.
#[derive(Clone, Copy, Debug)]
pub struct PlayerAllianceBudget {
    pub max_records: usize,
}
/// Aggregate caller bound on distinct player-state request pairs.
#[derive(Clone, Copy, Debug)]
pub struct PlayerStateRequestBudget {
    pub max_records: usize,
}
struct RecordLimits {
    max_records: usize,
    records: usize,
}
pub(super) struct Players {
    source: SelectedMapNeutralHost<()>,
    slots: usize,
    budget: PlayerBudget,
    tech: Option<RecordLimits>,
    research: Option<RecordLimits>,
    alliance: Option<RecordLimits>,
    state_requests: Option<RecordLimits>,
    resources: Option<Resources>,
    experience_handicap: Option<crate::player_handicap::Rules>,
    slot_states: Option<Vec<(i32, HandleId)>>,
    controllers: Option<controllers::Controllers>,
    current_research: Option<Vec<ResearchObservation>>,
    upgrade_research: Option<current_research::UpgradeResearch>,
    local_player: Option<i32>,
    rows: Vec<PlayerSettings>,
}
struct Resources {
    rules: crate::player_resources::Rules,
    // Gold, lumber, food used, food supplied, food ceiling; identities come from common source.
    states: [i32; 5],
    values: Vec<(i32, i32, i32)>,
}
const RESOURCE_GLOBALS: [&str; 5] = [
    "PLAYER_STATE_RESOURCE_GOLD",
    "PLAYER_STATE_RESOURCE_LUMBER",
    "PLAYER_STATE_RESOURCE_FOOD_USED",
    "PLAYER_STATE_RESOURCE_FOOD_CAP",
    "PLAYER_STATE_FOOD_CAP_CEILING",
];
const GET_PLAYER_STATE: &str = "GetPlayerState";
const SET_PLAYER_STATE: &str = "SetPlayerState";
const CONVERT_PLAYER_STATE: &str = "ConvertPlayerState";
const PLAYER_STATE_KIND: &str = "playerstate";
const SET_ALLIANCE: &str = "SetPlayerAlliance";
const CONVERT_ALLIANCE: &str = "ConvertAllianceType";
const ALLIANCE_KIND: &str = "alliancetype";
const CONTRACTS: &[(&str, &[&str], &str, bool)] = &[
    ("Player", &["integer"], "player", true),
    ("GetPlayerId", &["player"], "integer", true),
    ("SetPlayerTeam", &["player", "integer"], "nothing", false),
    ("GetPlayerTeam", &["player"], "integer", false),
    (
        "SetPlayerStartLocation",
        &["player", "integer"],
        "nothing",
        false,
    ),
    (
        "ForcePlayerStartLocation",
        &["player", "integer"],
        "nothing",
        false,
    ),
    ("GetPlayerStartLocation", &["player"], "integer", false),
    (
        "SetPlayerColor",
        &["player", "playercolor"],
        "nothing",
        false,
    ),
    ("GetPlayerColor", &["player"], "playercolor", false),
    (
        "SetPlayerRacePreference",
        &["player", "racepreference"],
        "nothing",
        false,
    ),
    (
        "SetPlayerRaceSelectable",
        &["player", "boolean"],
        "nothing",
        false,
    ),
    ("GetPlayerSelectable", &["player"], "boolean", false),
    (
        "SetPlayerController",
        &["player", "mapcontrol"],
        "nothing",
        false,
    ),
    ("GetPlayerController", &["player"], "mapcontrol", false),
    ("SetPlayerName", &["player", "string"], "nothing", false),
    ("GetPlayerName", &["player"], "string", false),
    (
        "SetPlayerOnScoreScreen",
        &["player", "boolean"],
        "nothing",
        false,
    ),
];
impl ObjectHost {
    pub fn with_players(
        mut self,
        context: Arc<ParsedMapContext>,
        slot_rule: Arc<VerifiedNeutralRule>,
        budget: PlayerBudget,
    ) -> Result<Self, PlayerAdmissionError> {
        if self.players.is_some() {
            return Err(self
                .inner
                .error("Player", Failure::PlayerState("already admitted"))
                .into());
        }
        if slot_rule.native_name != BJ_MAX_PLAYER_SLOTS_NATIVE_NAME {
            return Err(NeutralError::Rule(RuleProblem::Binding).into());
        }
        if !Arc::ptr_eq(&self.inner.program, &context.scripts.shared_program()) {
            return Err(self
                .inner
                .error(
                    "Player",
                    Failure::PlayerState("Program differs from admitted map"),
                )
                .into());
        }
        let source = SelectedMapNeutralHost::try_new(context, slot_rule, ())?;
        let slots = usize::try_from(source.selected_result()).map_err(|_| {
            self.inner
                .error("Player", Failure::PlayerState("negative source slot count"))
        })?;
        if slots > budget.max_slots {
            return Err(self
                .inner
                .error("Player", Failure::Budget("player slots"))
                .into());
        }
        std::alloc::Layout::array::<PlayerSettings>(slots)
            .and_then(|_| std::alloc::Layout::array::<u8>(budget.name_bytes))
            .map_err(|_| self.inner.error("Player", Failure::Capacity))?;
        for &(name, parameters, result, constant) in CONTRACTS {
            Self::contract(
                &self.inner,
                name,
                parameters,
                result,
                constant,
                Some("player"),
            )?;
        }
        self.players = Some(Players {
            source,
            slots,
            budget,
            tech: None,
            research: None,
            alliance: None,
            state_requests: None,
            resources: None,
            experience_handicap: None,
            slot_states: None,
            controllers: None,
            current_research: None,
            upgrade_research: None,
            local_player: None,
            rows: Vec::new(),
        });
        Ok(self)
    }
    /// Admit the declared tech-max setter and getter on the existing player rows.
    ///
    /// Records store the executed signed `techid` and `maximum` for that player
    /// identity. They are not rawcodes, categories, or build enforcement. An
    /// unset getter refuses until a source or default policy exists. A new key
    /// is refused at the caller record bound before allocation; replacing an
    /// existing key does not allocate. Argument-shape refusals are this
    /// recreation's contract checks, not a claim about native exceptional inputs.
    pub fn with_player_tech_limits(mut self, budget: TechLimitBudget) -> Result<Self, Error> {
        let fail = |reason| self.inner.error("SetPlayerTechMaxAllowed", reason);
        match self.players.as_ref() {
            None => return Err(fail(Failure::PlayerState("not admitted"))),
            Some(players) if players.tech.is_some() => {
                return Err(fail(Failure::PlayerState("tech limits already admitted")));
            }
            Some(_) => {}
        }
        std::alloc::Layout::array::<(i32, i32)>(budget.max_records)
            .map_err(|_| fail(Failure::Capacity))?;
        for &(name, parameters, result, constant) in TECH_CONTRACTS {
            Self::contract(
                &self.inner,
                name,
                parameters,
                result,
                constant,
                Some("player"),
            )?;
        }
        let players = self
            .players
            .as_mut()
            .ok_or_else(|| fail(Failure::PlayerState("not admitted")))?;
        players.tech = Some(RecordLimits {
            max_records: budget.max_records,
            records: 0,
        });
        Ok(self)
    }
    /// Admit `SetPlayerTechResearched` on the existing player rows.
    ///
    /// Each record stores one executed signed `(techid, setToLevel)` pair for
    /// that player identity. The pair is written together: a refused new key
    /// leaves the row unchanged. Replacing an existing key updates the level in
    /// place and does not allocate. The integers are not rawcodes, categories,
    /// clamps, or upgrade effects. Current research answers require separate
    /// explicit admission. Fresh upgrade counters can additionally serve
    /// `GetPlayerTechCount`; record-only admission leaves it unsupported. An absent
    /// raw pair is not level 0. Argument-shape refusals are this recreation's contract
    /// checks, not a claim about native exceptional inputs.
    pub fn with_player_tech_researched(mut self, budget: TechLimitBudget) -> Result<Self, Error> {
        let fail = |reason| self.inner.error("SetPlayerTechResearched", reason);
        match self.players.as_ref() {
            None => return Err(fail(Failure::PlayerState("not admitted"))),
            Some(players) if players.research.is_some() => {
                return Err(fail(Failure::PlayerState("tech research already admitted")));
            }
            Some(_) => {}
        }
        std::alloc::Layout::array::<(i32, i32)>(budget.max_records)
            .map_err(|_| fail(Failure::Capacity))?;
        for &(name, parameters, result, constant) in RESEARCH_CONTRACTS {
            Self::contract(
                &self.inner,
                name,
                parameters,
                result,
                constant,
                Some("player"),
            )?;
        }
        let players = self
            .players
            .as_mut()
            .ok_or_else(|| fail(Failure::PlayerState("not admitted")))?;
        players.research = Some(RecordLimits {
            max_records: budget.max_records,
            records: 0,
        });
        Ok(self)
    }
    /// Admit executed alliance assignments on the existing shared player identities.
    ///
    /// The symbolic setting comes from the selected `ConvertAllianceType` domain
    /// and the same declaration source as `Player`. Records are directed: no
    /// reverse assignment, self restriction or team/default relationship is inferred.
    /// Updating a retained key does not allocate. New keys respect the aggregate
    /// caller bound before allocation. This does not enforce alliance gameplay;
    /// `GetPlayerAlliance` stays unsupported until its initial-state policy is bound.
    pub fn with_player_alliances(mut self, budget: PlayerAllianceBudget) -> Result<Self, Error> {
        let fail = |reason| self.inner.error(SET_ALLIANCE, reason);
        let players = self
            .players
            .as_ref()
            .ok_or_else(|| fail(Failure::PlayerState("not admitted")))?;
        if players.alliance.is_some() {
            return Err(fail(Failure::PlayerState("alliances already admitted")));
        }
        Self::contract(
            &self.inner,
            SET_ALLIANCE,
            &["player", "player", ALLIANCE_KIND, "boolean"],
            "nothing",
            false,
            Some("player"),
        )?;
        Self::contract(
            &self.inner,
            CONVERT_ALLIANCE,
            &["integer"],
            ALLIANCE_KIND,
            true,
            None,
        )?;
        let player = self
            .inner
            .program
            .natives()
            .get("Player")
            .ok_or_else(|| fail(Failure::Signature))?;
        for name in [SET_ALLIANCE, CONVERT_ALLIANCE] {
            let native = self
                .inner
                .program
                .natives()
                .get(name)
                .ok_or_else(|| fail(Failure::Signature))?;
            if native.source != player.source {
                return Err(fail(Failure::Source));
            }
        }
        let conversion = self
            .inner
            .conversions
            .get(CONVERT_ALLIANCE)
            .ok_or_else(|| fail(Failure::Signature))?;
        if conversion.source != player.source {
            return Err(fail(Failure::Source));
        }
        if conversion.kind != ALLIANCE_KIND || !below(&self.inner.program, ALLIANCE_KIND, "handle")
        {
            return Err(fail(Failure::Signature));
        }
        if conversion.domain.is_empty() {
            return Err(fail(Failure::EmptyDomain));
        }
        std::alloc::Layout::array::<(HandleId, HandleId, bool)>(budget.max_records)
            .map_err(|_| fail(Failure::Capacity))?;
        self.players
            .as_mut()
            .ok_or_else(|| fail(Failure::PlayerState("not admitted")))?
            .alliance = Some(RecordLimits {
            max_records: budget.max_records,
            records: 0,
        });
        Ok(self)
    }
    fn set_player_alliance(&mut self, args: &[Value]) -> Result<Value, Error> {
        let fail = |reason| self.inner.error(SET_ALLIANCE, reason);
        let [source, target, Value::Handle(setting), Value::Bool(value)] = args else {
            return Err(fail(Failure::PlayerArguments));
        };
        let source = self.player_index(source, SET_ALLIANCE)?;
        let target = self.player_index(target, SET_ALLIANCE)?;
        let enumerand = self
            .inner
            .symbolic_value(*setting, ALLIANCE_KIND)
            .map_err(|e| fail(e.failure))?;
        if !self
            .inner
            .conversions
            .get(CONVERT_ALLIANCE)
            .is_some_and(|conversion| conversion.domain.contains(&enumerand))
        {
            return Err(fail(Failure::OutsideDomain(enumerand)));
        }
        let players = self
            .players
            .as_mut()
            .ok_or_else(|| fail(Failure::PlayerState("not admitted")))?;
        let target = players.rows[target].identity;
        let limits = players
            .alliance
            .as_mut()
            .ok_or_else(|| fail(Failure::PlayerState("alliances not admitted")))?;
        let assignments = &mut players.rows[source].alliances;
        if let Some(stored) = assignments
            .iter_mut()
            .find(|(other, kind, _)| *other == target && *kind == *setting)
        {
            stored.2 = *value;
            return Ok(Value::Null);
        }
        if limits.records >= limits.max_records {
            return Err(fail(Failure::Budget("player alliance assignments")));
        }
        assignments
            .try_reserve(1)
            .map_err(|_| fail(Failure::Capacity))?;
        assignments.push((target, *setting, *value));
        limits.records += 1;
        Ok(Value::Null)
    }
    /// Retain executed state requests on the existing shared player identities.
    ///
    /// The selected `ConvertPlayerState` domain and setter share `Player`'s
    /// declaration source. A new pair respects the aggregate caller bound before
    /// allocation; updates reuse a retained pair. Signed inputs, including zero,
    /// stay distinct from absent requests. Resource limits, clamps, effective
    /// balances and `GetPlayerState` are not inferred from these requests.
    pub fn with_player_state_requests(
        mut self,
        budget: PlayerStateRequestBudget,
    ) -> Result<Self, Error> {
        let fail = |reason| self.inner.error(SET_PLAYER_STATE, reason);
        let players = self
            .players
            .as_ref()
            .ok_or_else(|| fail(Failure::PlayerState("not admitted")))?;
        if players.state_requests.is_some() {
            return Err(fail(Failure::PlayerState(
                "state requests already admitted",
            )));
        }
        Self::contract(
            &self.inner,
            SET_PLAYER_STATE,
            &["player", PLAYER_STATE_KIND, "integer"],
            "nothing",
            false,
            Some("player"),
        )?;
        Self::contract(
            &self.inner,
            CONVERT_PLAYER_STATE,
            &["integer"],
            PLAYER_STATE_KIND,
            true,
            None,
        )?;
        let player = self
            .inner
            .program
            .natives()
            .get("Player")
            .ok_or_else(|| fail(Failure::Signature))?;
        for name in [SET_PLAYER_STATE, CONVERT_PLAYER_STATE] {
            let native = self
                .inner
                .program
                .natives()
                .get(name)
                .ok_or_else(|| fail(Failure::Signature))?;
            if native.source != player.source {
                return Err(fail(Failure::Source));
            }
        }
        let conversion = self
            .inner
            .conversions
            .get(CONVERT_PLAYER_STATE)
            .ok_or_else(|| fail(Failure::Signature))?;
        if conversion.source != player.source {
            return Err(fail(Failure::Source));
        }
        if conversion.kind != PLAYER_STATE_KIND
            || !below(&self.inner.program, PLAYER_STATE_KIND, "handle")
        {
            return Err(fail(Failure::Signature));
        }
        if conversion.domain.is_empty() {
            return Err(fail(Failure::EmptyDomain));
        }
        std::alloc::Layout::array::<(HandleId, i32)>(budget.max_records)
            .map_err(|_| fail(Failure::Capacity))?;
        let players = self
            .players
            .as_mut()
            .ok_or_else(|| fail(Failure::PlayerState("not admitted")))?;
        players.state_requests = Some(RecordLimits {
            max_records: budget.max_records,
            records: 0,
        });
        Ok(self)
    }
    fn request_player_state(&mut self, args: &[Value]) -> Result<Value, Error> {
        let fail = |reason| self.inner.error(SET_PLAYER_STATE, reason);
        let [player, Value::Handle(setting), Value::Int(value)] = args else {
            return Err(fail(Failure::PlayerArguments));
        };
        let at = self.player_index(player, SET_PLAYER_STATE)?;
        let enumerand = self
            .inner
            .symbolic_value(*setting, PLAYER_STATE_KIND)
            .map_err(|e| fail(e.failure))?;
        if !self
            .inner
            .conversions
            .get(CONVERT_PLAYER_STATE)
            .is_some_and(|conversion| conversion.domain.contains(&enumerand))
        {
            return Err(fail(Failure::OutsideDomain(enumerand)));
        }
        let effective = self.resource_assignment(at, enumerand, *value)?;
        let players = self
            .players
            .as_mut()
            .ok_or_else(|| fail(Failure::PlayerState("not admitted")))?;
        let limits = players
            .state_requests
            .as_mut()
            .ok_or_else(|| fail(Failure::PlayerState("state requests not admitted")))?;
        let requests = &mut players.rows[at].state_requests;
        let stored = requests.iter().position(|(state, _)| *state == *setting);
        if stored.is_none() {
            if limits.records >= limits.max_records {
                return Err(fail(Failure::Budget("player-state requests")));
            }
            requests
                .try_reserve(1)
                .map_err(|_| fail(Failure::Capacity))?;
        }
        if let Some(value) = effective {
            let number = players.rows[at].number;
            let resources = players.resources.as_mut().expect("prepared resources");
            if let Some(row) = resources
                .values
                .iter_mut()
                .find(|(p, s, _)| *p == number && *s == enumerand)
            {
                row.2 = value;
            } else {
                resources
                    .values
                    .try_reserve(1)
                    .map_err(|_| fail(Failure::Capacity))?;
                resources.values.push((number, enumerand, value));
            }
        }
        let requests = &mut players.rows[at].state_requests;
        if let Some(index) = stored {
            requests[index].1 = *value;
        } else {
            requests.push((*setting, *value));
            limits.records += 1;
        }
        Ok(Value::Null)
    }
    /// Admit selected initial resource counters and assignment limits before any request.
    pub fn with_player_resources(
        mut self,
        rules: crate::player_resources::Rules,
    ) -> Result<Self, Error> {
        let fail = |s| self.inner.error(GET_PLAYER_STATE, Failure::PlayerState(s));
        let players = self
            .players
            .as_ref()
            .ok_or_else(|| fail("players not admitted"))?;
        if players.state_requests.is_none()
            || players.resources.is_some()
            || players.rows.iter().any(|p| !p.state_requests.is_empty())
        {
            return Err(fail("admit resources once before assignments"));
        }
        Self::contract(
            &self.inner,
            GET_PLAYER_STATE,
            &["player", "playerstate"],
            "integer",
            true,
            Some("player"),
        )?;
        let source = &self
            .inner
            .program
            .natives()
            .get("Player")
            .expect("admitted players")
            .source;
        if self
            .inner
            .program
            .natives()
            .get(GET_PLAYER_STATE)
            .is_none_or(|n| &n.source != source)
        {
            return Err(self.inner.error(GET_PLAYER_STATE, Failure::Source));
        }
        let mut states = [0; 5];
        for (state, name) in states.iter_mut().zip(RESOURCE_GLOBALS) {
            *state = self.source_constant_enumerand(name, "playerstate")?;
        }
        if states
            .iter()
            .any(|s| *s < 0 || *s >= rules.initialized_states)
            || states[0..2] != rules.money_states
            || states
                .iter()
                .enumerate()
                .any(|(i, v)| states[..i].contains(v))
        {
            return Err(fail("resource constants do not match selected source"));
        }
        self.players.as_mut().expect("players").resources = Some(Resources {
            rules,
            states,
            values: Vec::new(),
        });
        Ok(self)
    }
    fn live_food(&self, owner: HandleId, column: &str) -> Result<i32, Error> {
        self.executed_units()
            .filter(|(_, u)| {
                u.owner == owner
                    && u.combat.as_ref().is_none_or(|c| !c.dead)
                    && u.vitals.is_none_or(|v| v.life > 0.0)
            })
            .try_fold(0i32, |total, (_, u)| {
                let marker = self
                    .players
                    .as_ref()
                    .and_then(|p| p.resources.as_ref())
                    .map(|r| r.rules.food_marker_default);
                let value =
                    super::unit_activity::balance_real_with_marker(&u.definition, column, marker)
                        .map_err(|s| self.inner.error(GET_PLAYER_STATE, Failure::UnitActivity(s)))?;
                if value.fract() != 0.0 || value >= i32::MAX as f32 {
                    return Err(self.inner.error(
                        GET_PLAYER_STATE,
                        Failure::PlayerState("invalid integral unit food"),
                    ));
                }
                total.checked_add(value as i32).ok_or_else(|| {
                    self.inner
                        .error(GET_PLAYER_STATE, Failure::PlayerState("unit food overflow"))
                })
            })
    }
    fn resource_assignment(&self, at: usize, state: i32, value: i32) -> Result<Option<i32>, Error> {
        let Some(players) = &self.players else {
            return Ok(None);
        };
        let Some(resources) = &players.resources else {
            return Ok(None);
        };
        let Some(index) = resources.states.iter().position(|s| *s == state) else {
            return Ok(None);
        };
        let value = match index {
            0 | 1 => resources.rules.money(value),
            2 | 3 => value
                .checked_sub(self.live_food(
                    players.rows[at].identity,
                    if index == 2 { "fused" } else { "fmade" },
                )?)
                .ok_or_else(|| {
                    self.inner.error(
                        SET_PLAYER_STATE,
                        Failure::PlayerState("food assignment overflow"),
                    )
                })?,
            _ => value,
        };
        Ok(Some(value))
    }
    fn resource_value(&self, at: usize, state: i32) -> Result<Option<i32>, Error> {
        let players = self.players.as_ref().expect("admitted players");
        let Some(resources) = &players.resources else {
            return Ok(None);
        };
        let Some(index) = resources.states.iter().position(|s| *s == state) else {
            return Ok(None);
        };
        let player = &players.rows[at];
        let assigned = resources
            .values
            .iter()
            .find(|(p, s, _)| *p == player.number && *s == state)
            .map(|(_, _, v)| *v);
        let value = if let Some(value) = assigned {
            value
        } else if index == 4 {
            let version = self
                .startup_version
                .map(|id| self.inner.symbolic_value(id, "version"))
                .transpose()
                .map_err(|e| self.inner.error(GET_PLAYER_STATE, e.failure))?;
            resources.rules.food_ceiling(version).ok_or_else(|| {
                self.inner.error(
                    GET_PLAYER_STATE,
                    Failure::PlayerState("food ceiling requires the selected startup version"),
                )
            })?
        } else if index < 2 {
            resources.rules.initial / resources.rules.money_scale
        } else {
            resources.rules.initial
        };
        let value = if matches!(index, 2 | 3) {
            value
                .checked_add(
                    self.live_food(player.identity, if index == 2 { "fused" } else { "fmade" })?,
                )
                .ok_or_else(|| {
                    self.inner.error(
                        GET_PLAYER_STATE,
                        Failure::PlayerState("effective food overflow"),
                    )
                })?
        } else {
            value
        };
        if index == 3 {
            let ceiling = self
                .resource_value(at, resources.states[4])?
                .expect("ceiling");
            Ok(Some(value.min(ceiling)))
        } else {
            Ok(Some(value))
        }
    }
    /// Effective source resource counters for the selected owned player; absent admission stays unknown.
    pub fn player_resource_value(
        &self,
        player: HandleId,
        source_global: &str,
    ) -> Result<Option<i32>, Error> {
        let at = self.player_index(&Value::Handle(player), GET_PLAYER_STATE)?;
        if !RESOURCE_GLOBALS.contains(&source_global) {
            return Err(self.inner.error(
                GET_PLAYER_STATE,
                Failure::PlayerState("unsupported resource setting"),
            ));
        }
        let state = self.source_constant_enumerand(source_global, "playerstate")?;
        self.resource_value(at, state)
    }
    /// Source-selected upkeep localization binding for an effective participant.
    /// Observer/watch-target presentation is not projected by this API.
    pub fn player_upkeep_label(
        &self,
        player: HandleId,
    ) -> Result<Option<crate::player_resources::UpkeepLabel>, String> {
        let at = self
            .player_index(&Value::Handle(player), GET_PLAYER_STATE)
            .map_err(|e| e.to_string())?;
        let Some(resources) = self.players.as_ref().and_then(|p| p.resources.as_ref()) else {
            return Ok(None);
        };
        if let Some(state) = resources.rules.upkeep_food_state()
            && resources.states[2] != state
        {
            return Err("upkeep food state disagrees with selected source enum".into());
        }
        let Some(food) = self
            .resource_value(at, resources.states[2])
            .map_err(|e| e.to_string())?
        else {
            return Ok(None);
        };
        let version = self
            .startup_version
            .map(|id| self.inner.symbolic_value(id, "version"))
            .transpose()
            .map_err(|e| e.to_string())?;
        resources.rules.upkeep_label(food, version).map(Some)
    }
    fn get_player_state(&self, args: &[Value]) -> Result<Value, Error> {
        let [player, Value::Handle(setting)] = args else {
            return Err(self.inner.error(GET_PLAYER_STATE, Failure::PlayerArguments));
        };
        let at = self.player_index(player, GET_PLAYER_STATE)?;
        let state = self
            .inner
            .symbolic_value(*setting, PLAYER_STATE_KIND)
            .map_err(|e| self.inner.error(GET_PLAYER_STATE, e.failure))?;
        self.resource_value(at, state)?
            .map(Value::Int)
            .ok_or_else(|| {
                self.inner.error(
                    GET_PLAYER_STATE,
                    Failure::PlayerState("effective player state is unsupported"),
                )
            })
    }
    pub fn player_slots(&self) -> Option<usize> {
        self.players.as_ref().map(|p| p.slots)
    }
    /// Existing `Player` identity for a source slot, if that call has already run.
    pub fn player_identity(&self, number: i32) -> Option<HandleId> {
        self.players.as_ref().and_then(|players| {
            players
                .rows
                .iter()
                .find(|row| row.number == number)
                .map(|row| row.identity)
        })
    }
    pub fn player_context(&self) -> Option<&Arc<ParsedMapContext>> {
        self.players.as_ref().map(|p| p.source.context())
    }
    /// Select this client's source slot without changing participation or controller setup.
    /// The normal `Player` path creates its canonical identity lazily on the first query.
    pub fn admit_local_player(
        &mut self,
        context: &Arc<ParsedMapContext>,
        number: i32,
    ) -> Result<(), Error> {
        let name = "GetLocalPlayer";
        let fail = |reason| self.inner.error(name, reason);
        if self
            .player_context()
            .is_none_or(|selected| !Arc::ptr_eq(selected, context))
        {
            return Err(fail(Failure::PlayerState(
                "local player map context is not admitted",
            )));
        }
        Self::contract(&self.inner, name, &[], "player", true, Some("player"))?;
        let natives = self.inner.program.natives();
        let source = &natives
            .get("Player")
            .ok_or_else(|| fail(Failure::Signature))?
            .source;
        if &natives.get(name).expect("checked contract").source != source {
            return Err(fail(Failure::Source));
        }
        let mut kind = "player";
        for _ in 0..=self.inner.program.types().count() {
            if kind == "handle" {
                break;
            }
            let ty = self
                .inner
                .program
                .types()
                .find(|ty| ty.name == kind)
                .ok_or_else(|| fail(Failure::Signature))?;
            if &ty.source != source {
                return Err(fail(Failure::Source));
            }
            kind = &ty.parent;
        }
        if kind != "handle" {
            return Err(fail(Failure::Signature));
        }
        if usize::try_from(number)
            .ok()
            .is_none_or(|n| n >= self.players.as_ref().expect("checked context").slots)
        {
            return Err(fail(Failure::OutsideDomain(number)));
        }
        self.players.as_mut().expect("checked context").local_player = Some(number);
        Ok(())
    }
    pub fn local_player_number(&self) -> Option<i32> {
        self.players.as_ref()?.local_player
    }
    pub fn local_player_settings(&self) -> Option<&PlayerSettings> {
        let players = self.players.as_ref()?;
        players
            .rows
            .iter()
            .find(|row| Some(row.number) == players.local_player)
    }
    /// Constant `racepreference` global whose value the local player's executed
    /// `SetPlayerRacePreference` holds, or None while either is unset.
    pub fn local_race_preference_constant(&self) -> Result<Option<String>, Error> {
        const KIND: &str = "racepreference";
        let Some(id) = self
            .local_player_settings()
            .and_then(|row| row.race_preference)
        else {
            return Ok(None);
        };
        let value = self.inner.symbolic_value(id, KIND)?;
        let names: Vec<&str> = self
            .inner
            .program
            .globals()
            .iter()
            .filter(|g| g.is_constant && !g.array && g.ty.kind.name() == KIND)
            .map(|g| g.name.as_str())
            .filter(|name| self.source_constant_enumerand(name, KIND).ok() == Some(value))
            .collect();
        match names.as_slice() {
            [name] => Ok(Some((*name).to_string())),
            _ => Err(self.inner.error(
                "SetPlayerRacePreference",
                Failure::UnitState("race preference is not one selected constant"),
            )),
        }
    }
    pub fn player_settings(&self, identity: HandleId) -> Result<&PlayerSettings, Error> {
        let at = self.player_index(&Value::Handle(identity), "players")?;
        self.players
            .as_ref()
            .and_then(|p| p.rows.get(at))
            .ok_or_else(|| self.inner.error("players", Failure::Identity))
    }
    /// Applied upgrade level, or retained setter level without a fresh model.
    ///
    /// An admitted fresh upgrade has the model's initial level when unset.
    /// [`TechRead::Absent`] is an unset player or record-only pair, not level 0.
    /// [`TechRead::Unavailable`] means player rows or research admission are
    /// absent. This does not implement `specificonly`.
    pub(super) fn stored_tech_level(&self, number: i32, techid: i32) -> TechRead {
        let Some(players) = &self.players else {
            return TechRead::Unavailable;
        };
        if players.research.is_none() {
            return TechRead::Unavailable;
        }
        let Some(row) = players.rows.iter().find(|row| row.number == number) else {
            return TechRead::Absent;
        };
        if let Some(model) = &players.upgrade_research {
            if !model.active || model.rules.maximum(techid).is_none() {
                return TechRead::Unavailable;
            }
            return TechRead::Level(
                row.research_levels
                    .iter()
                    .find(|(id, _)| *id == techid)
                    .map_or(model.rules.initial, |(_, level)| *level),
            );
        }
        match row
            .tech_researched
            .iter()
            .find(|(stored, _)| *stored == techid)
        {
            Some((_, level)) => TechRead::Level(*level),
            None => TechRead::Absent,
        }
    }
    pub fn start_location_occupied(&self, index: i32) -> Result<bool, Error> {
        let players = self.players.as_ref().ok_or_else(|| {
            self.inner
                .error("players", Failure::PlayerState("not admitted"))
        })?;
        if !self
            .config
            .as_ref()
            .is_some_and(|c| c.start_locations.iter().any(|p| p.index == index))
        {
            return Err(self.inner.error(
                "players",
                Failure::PlayerState("start location is undefined"),
            ));
        }
        Ok(players
            .rows
            .iter()
            .any(|row| row.start_is_forced && row.start_location == Some(index)))
    }
    fn player_index(&self, value: &Value, name: &str) -> Result<usize, Error> {
        let Value::Handle(id) = value else {
            return Err(self.inner.error(name, Failure::PlayerArguments));
        };
        let number = match self
            .inner
            .entry(*id)
            .map_err(|e| self.inner.error(name, e.failure))?
        {
            Entry::Player { number } => *number,
            _ => return Err(self.inner.error(name, Failure::WrongFamily)),
        };
        self.players
            .as_ref()
            .and_then(|p| {
                p.rows
                    .iter()
                    .position(|row| row.number == number && row.identity == *id)
            })
            .ok_or_else(|| self.inner.error(name, Failure::Identity))
    }
    /// Update the effective current slot without retiring the player's identity,
    /// controller or the other explicit startup participation entries.
    pub(super) fn mark_removed_player_slot(
        &mut self,
        player: HandleId,
        left: HandleId,
    ) -> Result<(), Error> {
        let at = self.player_index(&Value::Handle(player), "RemovePlayer")?;
        let fail = |failure| self.inner.error("RemovePlayer", failure);
        let players = self.players.as_mut().expect("validated player row");
        let number = players.rows[at].number;
        let slot = players
            .slot_states
            .as_mut()
            .and_then(|states| states.iter_mut().find(|(n, _)| *n == number))
            .ok_or_else(|| {
                fail(Failure::PlayerState(
                    "current player slot snapshot is not admitted",
                ))
            })?;
        slot.1 = left;
        players.rows[at].slot_state = Some(left);
        Ok(())
    }
    pub(super) fn player_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        let players = self.players.as_ref()?;
        if name == "GetLocalPlayer" {
            let number = players.local_player?;
            return Some(if args.is_empty() {
                self.apply_player("Player", &[Value::Int(number)])
                    .map_err(|error| self.inner.error(name, error.failure))
            } else {
                Err(self.inner.error(name, Failure::PlayerArguments))
            });
        }
        if matches!(name, "SetPlayerHandicapXP" | "GetPlayerHandicapXP") {
            players.experience_handicap.as_ref()?;
            return Some(self.experience_handicap_native(name, args));
        }
        if name == GET_PLAYER_STATE {
            players.resources.as_ref()?;
            return Some(self.get_player_state(args));
        }
        if name == SET_PLAYER_STATE {
            players.state_requests.as_ref()?;
            return Some(self.request_player_state(args));
        }
        if name == SET_ALLIANCE {
            players.alliance.as_ref()?;
            return Some(self.set_player_alliance(args));
        }
        if name == "GetPlayerSlotState" {
            players.slot_states.as_ref()?;
            return Some(self.apply_player(name, args));
        }
        if name == "GetPlayerTechResearched" {
            if players.current_research.is_none() && players.upgrade_research.is_none() {
                return None;
            }
            return Some(self.current_research_query(name, args));
        }
        if name == "GetPlayerTechCount" {
            if !players.upgrade_research.as_ref()?.count_admitted {
                return None;
            }
            return Some(self.current_research_query(name, args));
        }
        let tech = TECH_CONTRACTS.iter().any(|c| c.0 == name);
        let research = RESEARCH_CONTRACTS.iter().any(|c| c.0 == name);
        if tech {
            players.tech.as_ref()?;
        } else if research {
            players.research.as_ref()?;
        } else if !CONTRACTS.iter().any(|c| c.0 == name) {
            return None;
        }
        Some(self.apply_player(name, args))
    }
    fn apply_player(&mut self, name: &str, args: &[Value]) -> Result<Value, Error> {
        if name == "Player" {
            let [Value::Int(number)] = args else {
                return Err(self.inner.error(name, Failure::PlayerArguments));
            };
            let fail = |reason| self.inner.error(name, reason);
            let players = self
                .players
                .as_mut()
                .ok_or_else(|| fail(Failure::PlayerState("not admitted")))?;
            if usize::try_from(*number)
                .ok()
                .is_none_or(|n| n >= players.slots)
            {
                return Err(fail(Failure::OutsideDomain(*number)));
            }
            if let Some(row) = players.rows.iter().find(|row| row.number == *number) {
                return Ok(Value::Handle(row.identity));
            }
            players
                .rows
                .try_reserve(1)
                .map_err(|_| fail(Failure::Capacity))?;
            let id = self
                .inner
                .allocate(Entry::Player { number: *number }, name)?;
            players.rows.push(PlayerSettings {
                identity: id,
                number: *number,
                team: None,
                start_location: None,
                start_is_forced: false,
                color: None,
                race_preference: None,
                race_selectable: None,
                controller: None,
                slot_state: players.slot_states.as_ref().and_then(|states| {
                    states
                        .iter()
                        .find(|(n, _)| n == number)
                        .map(|(_, state)| *state)
                }),
                name: None,
                on_score_screen: None,
                tech_max: Vec::new(),
                tech_researched: Vec::new(),
                research_levels: Vec::new(),
                alliances: Vec::new(),
                state_requests: Vec::new(),
                experience_handicap: None,
            });
            return Ok(Value::Handle(id));
        }
        let Some(player) = args.first() else {
            return Err(self.inner.error(name, Failure::PlayerArguments));
        };
        let at = self.player_index(player, name)?;
        let fail = |reason| self.inner.error(name, reason);
        let players = self
            .players
            .as_mut()
            .ok_or_else(|| fail(Failure::PlayerState("not admitted")))?;
        match (name, &args[1..]) {
            ("GetPlayerId", []) => Ok(Value::Int(players.rows[at].number)),
            ("SetPlayerTeam", [Value::Int(team)]) => {
                if *team < 0 {
                    return Err(fail(Failure::PlayerState("negative team index")));
                }
                players.rows[at].team = Some(*team);
                Ok(Value::Null)
            }
            ("SetPlayerStartLocation" | "ForcePlayerStartLocation", [Value::Int(index)]) => {
                if !self
                    .config
                    .as_ref()
                    .is_some_and(|c| c.start_locations.iter().any(|p| p.index == *index))
                {
                    return Err(fail(Failure::PlayerState("start location is undefined")));
                }
                players.rows[at].start_location = Some(*index);
                if name == "ForcePlayerStartLocation" {
                    players.rows[at].start_is_forced = true;
                }
                Ok(Value::Null)
            }
            (
                "SetPlayerColor" | "SetPlayerRacePreference" | "SetPlayerController",
                [Value::Handle(id)],
            ) => {
                let kind = match name {
                    "SetPlayerColor" => "playercolor",
                    "SetPlayerRacePreference" => "racepreference",
                    _ => "mapcontrol",
                };
                self.inner
                    .symbolic_value(*id, kind)
                    .map_err(|e| fail(e.failure))?;
                match name {
                    "SetPlayerColor" => players.rows[at].color = Some(*id),
                    "SetPlayerRacePreference" => players.rows[at].race_preference = Some(*id),
                    _ => {
                        players.rows[at].controller = Some(*id);
                        self.invalidate_force_enumeration();
                        self.invalidate_startup_player_slots();
                        self.invalidate_startup_controllers();
                    }
                }
                Ok(Value::Null)
            }
            ("SetPlayerRaceSelectable" | "SetPlayerOnScoreScreen", [Value::Bool(value)]) => {
                if name == "SetPlayerRaceSelectable" {
                    players.rows[at].race_selectable = Some(*value);
                } else {
                    players.rows[at].on_score_screen = Some(*value);
                }
                Ok(Value::Null)
            }
            ("SetPlayerName", [Value::Str(text)]) => {
                players
                    .rows
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i != at)
                    .try_fold(text.len(), |sum, (_, row)| {
                        sum.checked_add(row.name.as_ref().map_or(0, String::len))
                    })
                    .filter(|&bytes| bytes <= players.budget.name_bytes)
                    .ok_or_else(|| fail(Failure::Budget("player names")))?;
                let mut copy = String::new();
                copy.try_reserve_exact(text.len())
                    .map_err(|_| fail(Failure::Capacity))?;
                copy.push_str(text);
                players.rows[at].name = Some(copy);
                Ok(Value::Null)
            }
            ("GetPlayerTeam", []) => players.rows[at]
                .team
                .map(Value::Int)
                .ok_or_else(|| fail(Failure::PlayerState("team is unset"))),
            ("GetPlayerStartLocation", []) => players.rows[at]
                .start_location
                .map(Value::Int)
                .ok_or_else(|| fail(Failure::PlayerState("start location is unset"))),
            ("GetPlayerColor", []) => players.rows[at]
                .color
                .map(Value::Handle)
                .ok_or_else(|| fail(Failure::PlayerState("color is unset"))),
            ("GetPlayerSelectable", []) => players.rows[at]
                .race_selectable
                .map(Value::Bool)
                .ok_or_else(|| fail(Failure::PlayerState("selectability is unset"))),
            ("GetPlayerSlotState", []) => players.rows[at]
                .slot_state
                .map(Value::Handle)
                .ok_or_else(|| fail(Failure::PlayerState("slot state is unset"))),
            ("GetPlayerController", []) => {
                if let Some(input) = &players.controllers {
                    input
                        .get(players.rows[at].number)
                        .map(Value::Handle)
                        .map_err(fail)
                } else {
                    players.rows[at]
                        .controller
                        .map(Value::Handle)
                        .ok_or_else(|| fail(Failure::PlayerState("controller is unset")))
                }
            }
            ("GetPlayerName", []) => players.rows[at]
                .name
                .as_ref()
                .map(|s| Value::Str(s.clone()))
                .ok_or_else(|| fail(Failure::PlayerState("name is unset"))),
            ("SetPlayerTechMaxAllowed", [Value::Int(techid), Value::Int(maximum)]) => {
                if let Some(limit) = players.rows[at]
                    .tech_max
                    .iter_mut()
                    .find(|(id, _)| *id == *techid)
                {
                    limit.1 = *maximum;
                    return Ok(Value::Null);
                }
                let (records, max_records) = match players.tech.as_ref() {
                    Some(tech) => (tech.records, tech.max_records),
                    None => {
                        return Err(fail(Failure::PlayerState("tech limits are not admitted")));
                    }
                };
                if records >= max_records {
                    return Err(fail(Failure::Budget("player tech limits")));
                }
                players.rows[at]
                    .tech_max
                    .try_reserve(1)
                    .map_err(|_| fail(Failure::Capacity))?;
                players.rows[at].tech_max.push((*techid, *maximum));
                let tech = players
                    .tech
                    .as_mut()
                    .ok_or_else(|| fail(Failure::PlayerState("tech limits are not admitted")))?;
                tech.records += 1;
                Ok(Value::Null)
            }
            ("GetPlayerTechMaxAllowed", [Value::Int(techid)]) => players.rows[at]
                .tech_max
                .iter()
                .find(|(id, _)| *id == *techid)
                .map(|(_, maximum)| Value::Int(*maximum))
                .ok_or_else(|| fail(Failure::PlayerState("tech max is unset"))),
            ("SetPlayerTechResearched", [Value::Int(techid), Value::Int(level)]) => {
                let applied = if let Some(model) = &players.upgrade_research {
                    if !model.active {
                        return Err(fail(Failure::PlayerState(
                            "upgrade research is invalidated",
                        )));
                    }
                    let current = players.rows[at]
                        .research_levels
                        .iter()
                        .find(|(id, _)| id == techid)
                        .map_or(model.rules.initial, |(_, level)| *level);
                    Some(
                        model
                            .rules
                            .target(*techid, current, *level)
                            .ok_or_else(|| {
                                fail(Failure::PlayerState(
                                    "technology is not an admitted upgrade",
                                ))
                            })?,
                    )
                } else {
                    None
                };
                let stored = players.rows[at]
                    .tech_researched
                    .iter()
                    .position(|(id, _)| id == techid);
                if stored.is_none() {
                    let (records, max_records) = match players.research.as_ref() {
                        Some(research) => (research.records, research.max_records),
                        None => {
                            return Err(fail(Failure::PlayerState(
                                "tech research is not admitted",
                            )));
                        }
                    };
                    if records >= max_records {
                        return Err(fail(Failure::Budget("player tech research")));
                    }
                    players.rows[at]
                        .tech_researched
                        .try_reserve(1)
                        .map_err(|_| fail(Failure::Capacity))?;
                    if applied.is_some() {
                        players.rows[at]
                            .research_levels
                            .try_reserve(1)
                            .map_err(|_| fail(Failure::Capacity))?;
                    }
                }
                if let Some(stored) = stored {
                    if let Some(applied) = applied {
                        let effective = players.rows[at]
                            .research_levels
                            .iter_mut()
                            .find(|(id, _)| id == techid)
                            .ok_or_else(|| {
                                fail(Failure::PlayerState("upgrade counter record is missing"))
                            })?;
                        effective.1 = applied;
                    }
                    players.rows[at].tech_researched[stored].1 = *level;
                } else {
                    players.rows[at].tech_researched.push((*techid, *level));
                    if let Some(applied) = applied {
                        players.rows[at].research_levels.push((*techid, applied));
                    }
                    let research = players.research.as_mut().ok_or_else(|| {
                        fail(Failure::PlayerState("tech research is not admitted"))
                    })?;
                    research.records += 1;
                }
                let number = players.rows[at].number;
                if let Some(entries) = players.current_research.as_mut() {
                    entries.retain(|entry| entry.player_number != number);
                }
                Ok(Value::Null)
            }
            _ => Err(fail(Failure::PlayerArguments)),
        }
    }
}

const TECH_CONTRACTS: &[(&str, &[&str], &str, bool)] = &[
    (
        "SetPlayerTechMaxAllowed",
        &["player", "integer", "integer"],
        "nothing",
        true,
    ),
    (
        "GetPlayerTechMaxAllowed",
        &["player", "integer"],
        "integer",
        true,
    ),
];

const RESEARCH_CONTRACTS: &[(&str, &[&str], &str, bool)] = &[(
    "SetPlayerTechResearched",
    &["player", "integer", "integer"],
    "nothing",
    true,
)];

#[path = "player_slots_host.rs"]
mod slots;

#[path = "player_controllers_host.rs"]
mod controllers;

#[path = "player_current_research_host.rs"]
mod current_research;
pub use current_research::ResearchObservation;

#[path = "player_handicap_host.rs"]
mod handicap;

impl ObjectHost {
    /// Gold of an admitted player, or `None` when resource state is not admitted.
    pub(crate) fn owner_gold(&self, player: HandleId) -> Result<Option<i32>, Error> {
        self.player_resource_value(player, "PLAYER_STATE_RESOURCE_GOLD")
    }

    /// Lowers an admitted player's gold by `price`. The caller has checked the balance, and
    /// an overflow refuses before any row changes.
    pub(crate) fn debit_gold(&mut self, player: HandleId, price: i32) -> Result<(), Error> {
        let fail = |reason| {
            self.inner
                .error(GET_PLAYER_STATE, Failure::PlayerState(reason))
        };
        let at = self.player_index(&Value::Handle(player), GET_PLAYER_STATE)?;
        let state = self.source_constant_enumerand("PLAYER_STATE_RESOURCE_GOLD", "playerstate")?;
        let current = self
            .resource_value(at, state)?
            .ok_or_else(|| fail("gold not admitted"))?;
        let next = current
            .checked_sub(price)
            .ok_or_else(|| fail("gold debit overflow"))?;
        let players = self.players.as_mut().ok_or_else(|| fail("not admitted"))?;
        let number = players.rows[at].number;
        let resources = players
            .resources
            .as_mut()
            .ok_or_else(|| fail("resources not admitted"))?;
        if let Some(row) = resources
            .values
            .iter_mut()
            .find(|(p, s, _)| *p == number && *s == state)
        {
            row.2 = next;
        } else {
            resources
                .values
                .try_reserve(1)
                .map_err(|_| fail("resource row capacity"))?;
            resources.values.push((number, state, next));
        }
        Ok(())
    }
}
