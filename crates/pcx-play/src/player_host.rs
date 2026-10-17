use super::{Entry, Error, Failure, ObjectHost};
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
/// Script-assigned state only. No lobby names, colors, controllers or races are invented.
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
    pub name: Option<String>,
    pub on_score_screen: Option<bool>,
    /// Executed `SetPlayerTechMaxAllowed` pairs. Absence is unset, not a default limit.
    pub tech_max: Vec<(i32, i32)>,
    /// Executed `SetPlayerTechResearched` pairs. Absence is unset, not level 0.
    pub tech_researched: Vec<(i32, i32)>,
}
/// Caller bound on stored `(player, techid)` records. Not a native or build limit.
#[derive(Clone, Copy, Debug)]
pub struct TechLimitBudget {
    pub max_records: usize,
}
/// One stored research pair. Absence is not a numeric level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TechRead {
    /// Player rows were not admitted, or research was not admitted.
    Unavailable,
    /// The player row or the tech pair is unset.
    Absent,
    /// The executed `setToLevel`, including an executed 0.
    Level(i32),
}
struct TechLimits {
    max_records: usize,
    records: usize,
}
pub(super) struct Players {
    source: SelectedMapNeutralHost<()>,
    slots: usize,
    budget: PlayerBudget,
    tech: Option<TechLimits>,
    research: Option<TechLimits>,
    rows: Vec<PlayerSettings>,
}
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
        players.tech = Some(TechLimits {
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
    /// clamps, or upgrade effects. `GetPlayerTechResearched` and
    /// `GetPlayerTechCount` stay unsupported because their `specificonly`
    /// aggregate-versus-exact behavior is not established; an absent pair is
    /// not level 0. Argument-shape refusals are this recreation's contract
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
        players.research = Some(TechLimits {
            max_records: budget.max_records,
            records: 0,
        });
        Ok(self)
    }
    pub fn player_slots(&self) -> Option<usize> {
        self.players.as_ref().map(|p| p.slots)
    }
    pub fn player_context(&self) -> Option<&Arc<ParsedMapContext>> {
        self.players.as_ref().map(|p| p.source.context())
    }
    pub fn player_settings(&self, identity: HandleId) -> Result<&PlayerSettings, Error> {
        let at = self.player_index(&Value::Handle(identity), "players")?;
        self.players
            .as_ref()
            .and_then(|p| p.rows.get(at))
            .ok_or_else(|| self.inner.error("players", Failure::Identity))
    }
    /// Stored `SetPlayerTechResearched` level for one player number.
    ///
    /// [`TechRead::Absent`] is an unset pair. It is not level 0.
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
    pub(super) fn player_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        let players = self.players.as_ref()?;
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
                name: None,
                on_score_screen: None,
                tech_max: Vec::new(),
                tech_researched: Vec::new(),
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
                    _ => players.rows[at].controller = Some(*id),
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
            ("GetPlayerController", []) => players.rows[at]
                .controller
                .map(Value::Handle)
                .ok_or_else(|| fail(Failure::PlayerState("controller is unset"))),
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
                if let Some(stored) = players.rows[at]
                    .tech_researched
                    .iter_mut()
                    .find(|(id, _)| *id == *techid)
                {
                    stored.1 = *level;
                    return Ok(Value::Null);
                }
                let (records, max_records) = match players.research.as_ref() {
                    Some(research) => (research.records, research.max_records),
                    None => {
                        return Err(fail(Failure::PlayerState("tech research is not admitted")));
                    }
                };
                if records >= max_records {
                    return Err(fail(Failure::Budget("player tech research")));
                }
                players.rows[at]
                    .tech_researched
                    .try_reserve(1)
                    .map_err(|_| fail(Failure::Capacity))?;
                players.rows[at].tech_researched.push((*techid, *level));
                let research = players
                    .research
                    .as_mut()
                    .ok_or_else(|| fail(Failure::PlayerState("tech research is not admitted")))?;
                research.records += 1;
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
