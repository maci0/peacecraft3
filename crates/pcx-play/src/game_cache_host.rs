//! Primitive game-cache operations on the existing object arena and one profile.
use super::{Entry, Error, Failure, ObjectHost};
use crate::game_cache::{Budget, Kind, Profile, Storage, Stored, key};
use pcx_script::Value;
use std::sync::Arc;

pub(super) struct State {
    profile: Profile,
    storage: Arc<dyn Storage>,
    budget: Budget,
    handle_bytes: usize,
    storage_error: Option<String>,
}

const CONTRACTS: &[(&str, &[&str], &str)] = &[
    ("InitGameCache", &["string"], "gamecache"),
    ("SaveGameCache", &["gamecache"], "boolean"),
    ("ReloadGameCachesFromDisk", &[], "boolean"),
    ("FlushGameCache", &["gamecache"], "nothing"),
    ("FlushStoredMission", &["gamecache", "string"], "nothing"),
    (
        "StoreInteger",
        &["gamecache", "string", "string", "integer"],
        "nothing",
    ),
    (
        "StoreReal",
        &["gamecache", "string", "string", "real"],
        "nothing",
    ),
    (
        "StoreBoolean",
        &["gamecache", "string", "string", "boolean"],
        "nothing",
    ),
    (
        "StoreString",
        &["gamecache", "string", "string", "string"],
        "boolean",
    ),
    (
        "GetStoredInteger",
        &["gamecache", "string", "string"],
        "integer",
    ),
    ("GetStoredReal", &["gamecache", "string", "string"], "real"),
    (
        "GetStoredBoolean",
        &["gamecache", "string", "string"],
        "boolean",
    ),
    (
        "GetStoredString",
        &["gamecache", "string", "string"],
        "string",
    ),
    (
        "HaveStoredInteger",
        &["gamecache", "string", "string"],
        "boolean",
    ),
    (
        "HaveStoredReal",
        &["gamecache", "string", "string"],
        "boolean",
    ),
    (
        "HaveStoredBoolean",
        &["gamecache", "string", "string"],
        "boolean",
    ),
    (
        "HaveStoredString",
        &["gamecache", "string", "string"],
        "boolean",
    ),
    (
        "FlushStoredInteger",
        &["gamecache", "string", "string"],
        "nothing",
    ),
    (
        "FlushStoredReal",
        &["gamecache", "string", "string"],
        "nothing",
    ),
    (
        "FlushStoredBoolean",
        &["gamecache", "string", "string"],
        "nothing",
    ),
    (
        "FlushStoredString",
        &["gamecache", "string", "string"],
        "nothing",
    ),
];

impl ObjectHost {
    /// Attach an explicit profile. Missing storage creates a fresh profile;
    /// unreadable, corrupt or foreign-format storage refuses without replacing it.
    pub fn admit_game_caches(
        &mut self,
        storage: Arc<dyn Storage>,
        budget: Budget,
    ) -> Result<(), Error> {
        let fail = |problem| {
            self.inner
                .error("InitGameCache", Failure::GameCache(problem))
        };
        if self.game_caches.is_some() {
            return Err(fail("already admitted".into()));
        }
        Self::contract(
            &self.inner,
            "InitGameCache",
            &["string"],
            "gamecache",
            false,
            Some("gamecache"),
        )?;
        let source = &self
            .inner
            .program
            .natives()
            .get("CreateTimer")
            .ok_or_else(|| fail("missing timer contract".into()))?
            .source;
        for &(name, params, result) in CONTRACTS {
            if let Some(native) = self.inner.program.natives().get(name) {
                Self::contract(&self.inner, name, params, result, false, None)?;
                if &native.source != source {
                    return Err(self.inner.error(name, Failure::Source));
                }
            }
        }
        let profile = match storage.load(budget.storage_bytes).map_err(fail)? {
            Some(bytes) => Profile::decode(&bytes, budget).map_err(fail)?,
            None => Profile::default(),
        };
        self.game_caches = Some(State {
            profile,
            storage,
            budget,
            handle_bytes: 0,
            storage_error: None,
        });
        Ok(())
    }
    pub fn game_cache_storage_error(&self) -> Option<&str> {
        self.game_caches.as_ref()?.storage_error.as_deref()
    }
    pub(super) fn game_cache_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        self.game_caches.as_ref()?;
        let &(_, parameters, _) = CONTRACTS.iter().find(|(n, _, _)| *n == name)?;
        self.inner.program.natives().get(name)?;
        Some(
            self.game_cache_call(name, args, parameters.len())
                .map_err(|problem| self.inner.error(name, Failure::GameCache(problem))),
        )
    }
    fn game_cache_call(
        &mut self,
        name: &str,
        args: &[Value],
        arity: usize,
    ) -> Result<Value, String> {
        if args.len() != arity {
            return Err("arguments do not match the native declaration".into());
        }
        let state = self.game_caches.as_mut().expect("admitted cache state");
        if name == "ReloadGameCachesFromDisk" {
            let candidate = state
                .storage
                .load(state.budget.storage_bytes)
                .and_then(|bytes| {
                    bytes.map_or(Ok(Profile::default()), |b| {
                        Profile::decode(&b, state.budget)
                    })
                });
            return match candidate {
                Ok(profile) => {
                    state.profile = profile;
                    state.storage_error = None;
                    Ok(Value::Bool(true))
                }
                Err(error) => {
                    state.storage_error = Some(error);
                    Ok(Value::Bool(false))
                }
            };
        }
        if name == "InitGameCache" {
            let filename = match &args[0] {
                Value::Null => return Ok(Value::Null),
                Value::Str(s) => s,
                _ => return Err("expected cache filename".into()),
            };
            if filename.is_empty() {
                return Ok(Value::Null);
            }
            let filename = key(filename, state.budget.text_bytes)?;
            let bytes = state
                .handle_bytes
                .checked_add(filename.len())
                .filter(|n| *n <= state.budget.handle_bytes)
                .ok_or("game-cache handle text policy exceeded")?;
            if self.inner.arena.len() >= self.inner.max_handles {
                return Err("game-cache arena identity capacity exceeded".into());
            }
            self.inner
                .arena
                .try_reserve(1)
                .map_err(|_| "game-cache identity allocation failed")?;
            state.profile.open(&filename, state.budget)?;
            let handle = self
                .inner
                .allocate(
                    Entry::GameCache {
                        filename,
                        live: true,
                    },
                    name,
                )
                .map_err(|e| e.to_string())?;
            state.handle_bytes = bytes;
            return Ok(Value::Handle(handle));
        }
        let filename = match &args[0] {
            Value::Null => None,
            Value::Handle(id) => match self.inner.entry(*id).map_err(|e| e.to_string())? {
                Entry::GameCache {
                    filename,
                    live: true,
                } => Some(filename.clone()),
                Entry::GameCache { live: false, .. } => {
                    return Err("flushed game-cache handle".into());
                }
                _ => return Err("handle is not a game cache".into()),
            },
            _ => return Err("expected game-cache identity".into()),
        };
        if name == "SaveGameCache" {
            if filename.is_none() {
                return Ok(Value::Bool(false));
            }
            let saved = state
                .profile
                .encode(state.budget)
                .and_then(|bytes| state.storage.save(&bytes));
            state.storage_error = saved.as_ref().err().cloned();
            return Ok(Value::Bool(saved.is_ok()));
        }
        if name == "FlushGameCache" {
            if let Some(filename) = filename {
                state.profile.flush(&filename, None, None);
                if let Value::Handle(id) = args[0] {
                    let Entry::GameCache { live, .. } =
                        self.inner.entry_mut(id).map_err(|e| e.to_string())?
                    else {
                        unreachable!("checked cache handle")
                    };
                    *live = false;
                }
            }
            return Ok(Value::Null);
        }
        let Value::Str(mission) = &args[1] else {
            return Err("expected mission key".into());
        };
        let mission = key(mission, state.budget.text_bytes)?;
        if name == "FlushStoredMission" {
            if let Some(filename) = filename {
                state.profile.flush(&filename, Some(&mission), None);
            }
            return Ok(Value::Null);
        }
        let Value::Str(entry_key) = &args[2] else {
            return Err("expected entry key".into());
        };
        let entry_key = key(entry_key, state.budget.text_bytes)?;
        let kind = if name.ends_with("Integer") {
            Kind::Integer
        } else if name.ends_with("Real") {
            Kind::Real
        } else if name.ends_with("Boolean") {
            Kind::Boolean
        } else {
            Kind::String
        };
        if name.starts_with("Store") {
            let value = match (kind, &args[3]) {
                (Kind::Integer, Value::Int(v)) => Stored::Integer(*v),
                (Kind::Real, Value::Real(v)) => Stored::Real(v.to_bits()),
                (Kind::Boolean, Value::Bool(v)) => Stored::Boolean(*v),
                (Kind::String, Value::Str(v))
                    if v.len() <= state.budget.text_bytes && !v.contains('\0') =>
                {
                    Stored::String(v.to_string())
                }
                _ => {
                    return Err("stored value does not match the native type or text policy".into());
                }
            };
            if let Some(filename) = filename {
                state
                    .profile
                    .put(&filename, mission, entry_key, value, state.budget)?;
                return Ok(if kind == Kind::String {
                    Value::Bool(true)
                } else {
                    Value::Null
                });
            }
            return Ok(if kind == Kind::String {
                Value::Bool(false)
            } else {
                Value::Null
            });
        }
        if name.starts_with("Flush") {
            if let Some(filename) = filename {
                state
                    .profile
                    .flush(&filename, Some(&mission), Some((&entry_key, kind)));
            }
            return Ok(Value::Null);
        }
        let stored = filename
            .as_ref()
            .and_then(|f| state.profile.get(f, &mission, &entry_key, kind));
        if name.starts_with("Have") {
            return Ok(Value::Bool(stored.is_some()));
        }
        Ok(match stored {
            Some(Stored::Integer(v)) => Value::Int(*v),
            Some(Stored::Real(v)) => Value::Real(f32::from_bits(*v)),
            Some(Stored::Boolean(v)) => Value::Bool(*v),
            Some(Stored::String(v)) => Value::Str(v.clone()),
            None => match kind {
                Kind::Integer => Value::Int(0),
                Kind::Real => Value::Real(0.0),
                Kind::Boolean => Value::Bool(false),
                Kind::String => Value::Null,
            },
        })
    }
}

#[cfg(test)]
#[path = "game_cache_host_tests.rs"]
mod tests;
