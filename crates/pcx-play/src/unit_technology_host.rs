//! Authored unit technology providers, separate from pending training counts.
use super::{Entry, Error, Failure, ObjectHost, unit_training::profile};
use pcx_objects::editor::ObjectKind;
use pcx_script::HandleId;
use pcx_sim::world::{ObjectCatalog, ResolutionProblem};
use std::collections::HashSet;

// Per-query safety work (resolved nodes plus references), not a game technology limit.
const GRAPH_WORK: usize = 65536;
const QUERY: &str = "GetPlayerTechCount";

// shortcut: custom overlap/cycle graphs use one provider union; qualify those native cases before claiming parity.
#[derive(Debug)]
struct Providers {
    completed: HashSet<[u8; 4]>,
    pending_upgrade: HashSet<[u8; 4]>,
}

fn provider_types(
    catalog: &dyn ObjectCatalog,
    requested: [u8; 4],
    specific: bool,
    mut work: usize,
) -> Result<Option<Providers>, String> {
    if !requested.iter().all(u8::is_ascii_graphic) {
        return Ok(None);
    }
    let pay = |work: &mut usize| {
        *work = work
            .checked_sub(1)
            .ok_or("technology relationship work budget")?;
        Ok::<_, String>(())
    };
    pay(&mut work)?;
    let mut root = match catalog.resolve(requested) {
        Ok(definition) if definition.kind == ObjectKind::Unit => Some(definition),
        Ok(_) | Err(ResolutionProblem::MissingRawcode(_)) => return Ok(None),
        Err(ResolutionProblem::UnsupportedKind(kind)) if kind != ObjectKind::Unit => {
            return Ok(None);
        }
        Err(error) => return Err(format!("technology catalog: {error:?}")),
    };
    let mut pending = vec![(requested, false)];
    let mut visited = HashSet::new();
    let mut completed = HashSet::new();
    let mut pending_upgrade = HashSet::new();
    while let Some((code, after_upgrade)) = pending.pop() {
        if visited.contains(&(code, after_upgrade)) {
            continue;
        }
        let definition = if code == requested && root.is_some() {
            root.take().expect("first root visit")
        } else {
            pay(&mut work)?;
            catalog
                .resolve(code)
                .map_err(|error| format!("technology catalog {code:?}: {error:?}"))?
        };
        if definition.rawcode != code || definition.kind != ObjectKind::Unit {
            return Err(format!("technology catalog identity differs for {code:?}"));
        }
        visited
            .try_reserve(1)
            .map_err(|_| "technology visited capacity")?;
        completed
            .try_reserve(1)
            .map_err(|_| "technology provider capacity")?;
        visited.insert((code, after_upgrade));
        completed.insert(code);
        if after_upgrade {
            pending_upgrade
                .try_reserve(1)
                .map_err(|_| "technology pending provider capacity")?;
            pending_upgrade.insert(code);
        }
        for key in ["DependencyOr", "Upgrade"] {
            if specific && key == "Upgrade" {
                continue;
            }
            let list = profile(&definition, key, "unitList")?;
            if list.is_empty() {
                continue;
            }
            for token in list.split(',') {
                pay(&mut work)?;
                let next: [u8; 4] = token
                    .trim()
                    .as_bytes()
                    .try_into()
                    .map_err(|_| format!("{code:?}: malformed {key} unit identifier"))?;
                if !next.iter().all(u8::is_ascii_graphic) {
                    return Err(format!("{code:?}: malformed {key} unit identifier"));
                }
                pending
                    .try_reserve(1)
                    .map_err(|_| "technology relationship capacity")?;
                pending.push((next, after_upgrade || key == "Upgrade"));
            }
        }
    }
    Ok(Some(Providers {
        completed,
        pending_upgrade,
    }))
}

pub(super) fn live(record: &super::ExecutedUnit) -> Result<bool, &'static str> {
    if record.combat.is_none() && record.vitals.is_none() {
        return Err("unit technology liveness is not admitted");
    }
    if record.vitals.is_some_and(|v| !v.life.is_finite()) {
        return Err("unit technology life is not finite");
    }
    Ok(record.construction.is_none()
        && !record.combat.as_ref().is_some_and(|c| c.dead)
        && record.vitals.is_none_or(|v| v.life > 0.))
}

impl ObjectHost {
    pub(super) fn unit_technology_count(
        &self,
        owner: HandleId,
        kind: i32,
        specific: bool,
    ) -> Result<Option<i32>, Error> {
        let fail = |detail: String| self.inner.error(QUERY, Failure::UnitActivity(detail));
        self.player_settings(owner)?;
        if self.units.is_none() {
            return Ok(None);
        }
        let catalog = self.unit_catalog()?;
        let Some(providers) =
            provider_types(catalog.as_ref(), kind.to_be_bytes(), specific, GRAPH_WORK)
                .map_err(fail)?
        else {
            return Ok(None);
        };
        let mut count = 0i32;
        for (slot, entry) in self.inner.arena.iter().enumerate() {
            let counted = match entry {
                Entry::Unit { record } if record.owner == owner => {
                    let pending =
                        self.pending_structure_upgrade_target(self.inner.id(slot, QUERY)?);
                    let provider = if let Some(kind) = pending {
                        providers.pending_upgrade.contains(&kind.to_be_bytes())
                    } else {
                        providers.completed.contains(&record.definition.rawcode)
                    };
                    provider && live(record).map_err(|detail| fail(detail.into()))?
                }
                Entry::RemovedUnit {
                    owner: previous,
                    kind,
                    technology,
                    removed_at,
                    ..
                } if *previous == owner => {
                    use super::super::RemovedUnitTechnology;
                    if removed_at.is_some() && !self.removed_unit_state_retained(*removed_at) {
                        continue;
                    }
                    if removed_at.is_none()
                        && (providers.completed.contains(kind)
                            || matches!(technology, Some(RemovedUnitTechnology::PendingUpgrade(target)) if providers.pending_upgrade.contains(target)))
                    {
                        return Err(fail(
                            "removed technology needs the admitted game clock".into(),
                        ));
                    }
                    let provider = match technology {
                        Some(RemovedUnitTechnology::Inactive) => false,
                        Some(RemovedUnitTechnology::Completed) => {
                            providers.completed.contains(kind)
                        }
                        Some(RemovedUnitTechnology::PendingUpgrade(target)) => {
                            providers.pending_upgrade.contains(target)
                        }
                        None if providers.completed.contains(kind) => {
                            return Err(fail("removed technology liveness is not admitted".into()));
                        }
                        None => false,
                    };
                    provider && self.removed_unit_state_retained(*removed_at)
                }
                _ => false,
            };
            if counted {
                count = count
                    .checked_add(1)
                    .ok_or_else(|| fail("unit technology count overflow".into()))?;
            }
        }
        Ok(Some(count))
    }
}

#[cfg(test)]
#[path = "unit_technology_tests.rs"]
mod tests;
