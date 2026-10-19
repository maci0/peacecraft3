use super::{Error, Failure, ObjectHost, below};
use pcx_script::{HandleId, Value};

/// Caller policy for retained text bytes and distinct start-location records.
#[derive(Clone, Copy, Debug)]
pub struct MapConfigBudget {
    pub text_bytes: usize,
    pub start_locations: usize,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StartLocation {
    pub index: i32,
    pub x: f32,
    pub y: f32,
}
/// Only values actually assigned by executed script calls; no implicit game defaults.
#[derive(Debug)]
pub struct MapConfig {
    pub name: Option<String>,
    pub description: Option<String>,
    pub players: Option<i32>,
    pub teams: Option<i32>,
    pub placement: Option<HandleId>,
    pub speed_locked: Option<bool>,
    pub start_locations: Vec<StartLocation>,
    pub start_priorities: Vec<StartPriorities>,
    priority_limit: Option<usize>,
    budget: MapConfigBudget,
}
const CONTRACTS: &[(&str, &[&str], &str, bool)] = &[
    ("SetMapName", &["string"], "nothing", false),
    ("SetMapDescription", &["string"], "nothing", false),
    ("SetPlayers", &["integer"], "nothing", false),
    ("SetTeams", &["integer"], "nothing", false),
    ("GetPlayers", &[], "integer", false),
    ("GetTeams", &[], "integer", false),
    ("SetGamePlacement", &["placement"], "nothing", false),
    ("GetGamePlacement", &[], "placement", true),
    (
        "DefineStartLocation",
        &["integer", "real", "real"],
        "nothing",
        false,
    ),
    ("GetStartLocationX", &["integer"], "real", true),
    ("GetStartLocationY", &["integer"], "real", true),
];
impl ObjectHost {
    pub fn with_map_config(mut self, budget: MapConfigBudget) -> Result<Self, Error> {
        if self.config.is_some() {
            return Err(self
                .inner
                .error("config", Failure::ConfigState("already admitted")));
        }
        std::alloc::Layout::array::<u8>(budget.text_bytes)
            .and_then(|_| std::alloc::Layout::array::<StartLocation>(budget.start_locations))
            .map_err(|_| self.inner.error("config", Failure::Capacity))?;
        for &(name, parameters, result, constant) in CONTRACTS {
            Self::contract(&self.inner, name, parameters, result, constant, None)?;
        }
        if !below(&self.inner.program, "placement", "handle") {
            return Err(self.inner.error("config", Failure::Signature));
        }
        self.config = Some(MapConfig {
            name: None,
            description: None,
            players: None,
            teams: None,
            placement: None,
            speed_locked: None,
            start_locations: Vec::new(),
            start_priorities: Vec::new(),
            priority_limit: None,
            budget,
        });
        Ok(self)
    }
    pub fn map_config(&self) -> Option<&MapConfig> {
        self.config.as_ref()
    }
    pub(super) fn config_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        self.config.as_ref()?;
        if let Some(result) = self.priority_native(name, args) {
            return Some(result);
        }
        if matches!(name, "SetMapFlag" | "IsMapFlagSet") {
            return Some(self.speed_lock_native(name, args));
        }
        if !CONTRACTS.iter().any(|c| c.0 == name) {
            return None;
        }
        Some(self.apply_config(name, args))
    }
    fn speed_lock_native(&mut self, name: &str, args: &[Value]) -> Result<Value, Error> {
        let fail = |why| self.inner.error(name, why);
        let (parameters, result) = if name == "SetMapFlag" {
            (&["mapflag", "boolean"][..], "nothing")
        } else {
            (&["mapflag"][..], "boolean")
        };
        Self::contract(&self.inner, name, parameters, result, false, None)?;
        if self
            .inner
            .program
            .natives()
            .get(name)
            .expect("checked map flag")
            .source
            != self
                .inner
                .program
                .natives()
                .get("CreateTimer")
                .expect("checked Host")
                .source
        {
            return Err(fail(Failure::Source));
        }
        let flag = match args {
            [Value::Handle(flag), Value::Bool(_)] if name == "SetMapFlag" => *flag,
            [Value::Handle(flag)] if name == "IsMapFlagSet" => *flag,
            _ => return Err(fail(Failure::ConfigArguments)),
        };
        if self
            .inner
            .symbolic_value(flag, "mapflag")
            .map_err(|e| fail(e.failure))?
            != self.source_enumerand(name, "MAP_LOCK_SPEED", "mapflag")?
        {
            return Err(fail(Failure::ConfigState(
                "only MAP_LOCK_SPEED is supported",
            )));
        }
        let config = self.config.as_mut().expect("admitted config");
        if let [_, Value::Bool(locked)] = args {
            config.speed_locked = Some(*locked);
            Ok(Value::Null)
        } else {
            config
                .speed_locked
                .map(Value::Bool)
                .ok_or_else(|| fail(Failure::ConfigState("speed lock is unset")))
        }
    }
    fn apply_config(&mut self, name: &str, args: &[Value]) -> Result<Value, Error> {
        let fail = |problem| self.inner.error(name, problem);
        let config = self
            .config
            .as_mut()
            .ok_or_else(|| fail(Failure::ConfigState("not admitted")))?;
        match (name, args) {
            ("SetMapName" | "SetMapDescription", [Value::Str(text)]) => {
                let other = if name == "SetMapName" {
                    &config.description
                } else {
                    &config.name
                };
                text.len()
                    .checked_add(other.as_ref().map_or(0, String::len))
                    .filter(|&n| n <= config.budget.text_bytes)
                    .ok_or_else(|| fail(Failure::Budget("map config text")))?;
                let mut copy = String::new();
                copy.try_reserve_exact(text.len())
                    .map_err(|_| fail(Failure::Capacity))?;
                copy.push_str(text);
                if name == "SetMapName" {
                    config.name = Some(copy);
                } else {
                    config.description = Some(copy);
                }
                Ok(Value::Null)
            }
            ("SetPlayers" | "SetTeams", [Value::Int(count)]) => {
                if *count < 0 {
                    return Err(fail(Failure::ConfigState("negative count")));
                }
                if name == "SetPlayers" {
                    config.players = Some(*count);
                } else {
                    config.teams = Some(*count);
                }
                Ok(Value::Null)
            }
            ("GetPlayers" | "GetTeams", []) => {
                let value = if name == "GetPlayers" {
                    config.players
                } else {
                    config.teams
                };
                value
                    .map(Value::Int)
                    .ok_or_else(|| fail(Failure::ConfigState("count is unset")))
            }
            ("SetGamePlacement", [Value::Handle(id)]) => {
                self.inner
                    .symbolic_value(*id, "placement")
                    .map_err(|e| fail(e.failure))?;
                config.placement = Some(*id);
                Ok(Value::Null)
            }
            ("GetGamePlacement", []) => config
                .placement
                .map(Value::Handle)
                .ok_or_else(|| fail(Failure::ConfigState("placement is unset"))),
            ("DefineStartLocation", [Value::Int(index), x, y]) => {
                let (Some(x), Some(y)) = (x.as_real(), y.as_real()) else {
                    return Err(fail(Failure::ConfigArguments));
                };
                if *index < 0 || !x.is_finite() || !y.is_finite() {
                    return Err(fail(Failure::ConfigState(
                        "invalid start-location index or coordinates",
                    )));
                }
                let location = StartLocation {
                    index: *index,
                    x,
                    y,
                };
                if let Some(old) = config
                    .start_locations
                    .iter_mut()
                    .find(|row| row.index == *index)
                {
                    *old = location;
                } else {
                    if config.start_locations.len() >= config.budget.start_locations {
                        return Err(fail(Failure::Budget("map config start locations")));
                    }
                    config
                        .start_locations
                        .try_reserve(1)
                        .map_err(|_| fail(Failure::Capacity))?;
                    config.start_locations.push(location);
                }
                Ok(Value::Null)
            }
            ("GetStartLocationX" | "GetStartLocationY", [Value::Int(index)]) => {
                let location = config
                    .start_locations
                    .iter()
                    .find(|row| row.index == *index)
                    .ok_or_else(|| fail(Failure::ConfigState("start location is unset")))?;
                Ok(Value::Real(if name == "GetStartLocationX" {
                    location.x
                } else {
                    location.y
                }))
            }
            _ => Err(fail(Failure::ConfigArguments)),
        }
    }
}

#[cfg(test)]
#[path = "map_config_host_tests.rs"]
mod tests;

#[path = "start_priorities.rs"]
mod priorities;
pub use priorities::{StartPriorities, StartPriority};
