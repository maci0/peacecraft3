//! Unit progress moves through the existing profile and creation pipeline.
use super::{Entry, ObjectHost};
use crate::game_cache::{CachedItem, CachedUnit, Kind, Stored, key};
use pcx_script::{HandleId, Value};

impl ObjectHost {
    pub(super) fn unit_cache_call(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
        let budget = self.game_caches.as_ref().expect("admitted cache").budget;
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
        let (Value::Str(mission), Value::Str(entry)) = (&args[1], &args[2]) else {
            return Err("expected mission and entry keys".into());
        };
        let mission = key(mission, budget.text_bytes)?;
        let entry = key(entry, budget.text_bytes)?;
        if name == "StoreUnit" {
            let Some(filename) = filename else {
                return Ok(Value::Bool(false));
            };
            let unit = match args[3] {
                Value::Null => return Ok(Value::Bool(false)),
                Value::Handle(unit) => unit,
                _ => return Err("expected unit handle".into()),
            };
            if matches!(
                self.inner.entry(unit).map_err(|e| e.to_string())?,
                Entry::RemovedUnit
            ) {
                return Ok(Value::Bool(false));
            }
            let saved = self.capture_cached_unit(unit)?;
            let bytes = saved.encode(budget.storage_bytes)?;
            self.game_caches.as_mut().expect("cache").profile.put(
                &filename,
                mission,
                entry,
                Stored::Unit(bytes),
                budget,
            )?;
            return Ok(Value::Bool(true));
        }
        if name == "FlushStoredUnit" {
            if let Some(filename) = filename {
                self.game_caches.as_mut().expect("cache").profile.flush(
                    &filename,
                    Some(&mission),
                    Some((&entry, Kind::Unit)),
                );
            }
            return Ok(Value::Null);
        }
        let stored = filename.as_ref().and_then(|filename| {
            self.game_caches.as_ref().expect("cache").profile.get(
                filename,
                &mission,
                &entry,
                Kind::Unit,
            )
        });
        if name == "HaveStoredUnit" {
            return Ok(Value::Bool(stored.is_some()));
        }
        let Some(Stored::Unit(bytes)) = stored else {
            return Ok(Value::Null);
        };
        let saved = CachedUnit::decode(bytes, budget.storage_bytes)?;
        self.create_cached_unit(
            &[
                args[3].clone(),
                Value::Int(i32::from_be_bytes(saved.rawcode)),
                args[4].clone(),
                args[5].clone(),
                args[6].clone(),
            ],
            &saved,
        )
        .map_err(|e| e.to_string())
    }

    fn capture_cached_unit(&self, unit: HandleId) -> Result<CachedUnit, String> {
        let now = self
            .timer_game_time()
            .ok_or("unit cache requires the game clock")?;
        self.check_activity_boundary(now, "StoreUnit")
            .map_err(|e| e.to_string())?;
        let record = self.executed_unit(unit).map_err(|e| e.to_string())?;
        if record.summoned {
            return Err("unit cache does not encode summoned-unit lifetime".into());
        }
        let abilities = record
            .abilities
            .as_ref()
            .ok_or("unit-cache abilities not admitted")?;
        let vitals = record.vitals.ok_or("unit-cache vitals not admitted")?;
        if record.combat.as_ref().is_some_and(|c| c.dead) {
            return Err("cannot cache a dead unit".into());
        }
        let policy = self.inventory_policy(unit).map_err(|e| e.to_string())?;
        let mut items = Vec::new();
        for (id, item) in self.executed_items() {
            let Some((holder, slot)) = item.held_by else {
                continue;
            };
            if holder != unit {
                continue;
            }
            if item.source.install != record.source.install
                || item.source.edition != record.source.edition
            {
                return Err("held item belongs to a different install/edition".into());
            }
            if slot >= policy.slots || items.len() >= policy.slots {
                return Err("unit-cache inventory exceeds admitted slots".into());
            }
            items
                .try_reserve(1)
                .map_err(|_| "unit-cache inventory allocation failed")?;
            items.push(CachedItem {
                slot,
                rawcode: item.definition.rawcode,
                charges: self.cached_item_charges(id).map_err(|e| e.to_string())?,
                flags: [item.invulnerable, item.droppable, item.drop_on_death],
            });
        }
        items.sort_unstable_by_key(|item| item.slot);
        let mut levels: Vec<_> = abilities.levels().collect();
        levels.sort_unstable_by_key(|(alias, _)| *alias);
        let saved = CachedUnit {
            rawcode: record.definition.rawcode,
            client_sha256: self
                .order_registry
                .as_ref()
                .ok_or("cached unit requires selected client identity")?
                .binding()
                .image_sha256,
            install: record.source.install.clone(),
            edition: record.source.edition.clone(),
            hero: abilities
                .hero
                .as_ref()
                .map(|hero| [hero.level, hero.skill_points]),
            abilities: levels,
            permanent_strength: record.permanent_strength,
            pools: [vitals.life, vitals.max_life, vitals.mana, vitals.max_mana].map(f32::to_bits),
            items,
        };
        saved.validate()?;
        Ok(saved)
    }
}
