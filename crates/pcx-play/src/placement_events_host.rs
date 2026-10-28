use super::{Entry, ObjectHost};
use crate::ai_placement::{ExclusionError, PlacementEvent, PlacementEventKind};
use pcx_script::HandleId;

impl ObjectHost {
    /// Resolve the town reference from live registrations and unit visibility.
    pub fn ai_town_reference(&self, owner: HandleId) -> Result<Option<HandleId>, ExclusionError> {
        self.player_settings(owner)
            .map_err(|_| ExclusionError::Unavailable {
                unit: owner,
                source_name: "town owner",
            })?;
        let mut peon: Option<((u64, usize), HandleId)> = None;
        for (unit, record) in self.executed_units() {
            if record.owner != owner || !record.shown {
                continue;
            }
            let Some(sequence) = self.town_registration_sequence(unit)? else {
                continue;
            };
            let is_peon =
                super::unit_types::class(&record.definition, "UNIT_TYPE_PEON").map_err(|_| {
                    ExclusionError::Unavailable {
                        unit,
                        source_name: "town peon class",
                    }
                })?;
            if is_peon && peon.is_none_or(|(old, _)| sequence > old) {
                peon = Some((sequence, unit));
            }
        }
        if let Some((_, unit)) = peon {
            return Ok(Some(unit));
        }
        let mut other: Option<((u64, usize), HandleId)> = None;
        for (unit, record) in self.executed_units() {
            if record.owner != owner || !record.shown {
                continue;
            }
            let Some(sequence) = self.town_registration_sequence(unit)? else {
                continue;
            };
            if super::unit_types::structure(&record.definition).map_err(|_| {
                ExclusionError::Unavailable {
                    unit,
                    source_name: "town structure class",
                }
            })? {
                return Err(ExclusionError::Unavailable {
                    unit,
                    source_name: "town structure root-ability state bits 0x80 and 0x8000000",
                });
            } else if other.is_none_or(|(old, _)| sequence < old) {
                other = Some((sequence, unit));
            }
        }
        Ok(other.map(|(_, unit)| unit))
    }

    /// The registration order of a living unit, or `None` after death or removal.
    pub fn town_registration_sequence(
        &self,
        unit: HandleId,
    ) -> Result<Option<(u64, usize)>, ExclusionError> {
        let entry = self
            .inner
            .entry(unit)
            .map_err(|_| ExclusionError::Unavailable {
                unit,
                source_name: "unit record",
            })?;
        let record = match entry {
            Entry::RemovedUnit { .. } => return Ok(None),
            Entry::Unit { record } => record,
            _ => {
                return Err(ExclusionError::Unavailable {
                    unit,
                    source_name: "unit record",
                });
            }
        };
        if record.vitals.is_some_and(|v| !v.life.is_finite())
            || (record.vitals.is_none() && record.combat.is_none())
        {
            return Err(ExclusionError::Unavailable {
                unit,
                source_name: "unit liveness",
            });
        }
        if record.combat.as_ref().is_some_and(|c| c.dead)
            || record.vitals.is_some_and(|v| v.life <= 0.)
        {
            return Ok(None);
        }
        if self.placement_events_incomplete {
            return Err(ExclusionError::Unavailable {
                unit,
                source_name: "town registration history",
            });
        }
        let mut sequence = (unit.token(), 0);
        for (index, event) in self.placement_event_history().enumerate() {
            if event.unit == unit
                && let PlacementEventKind::OwnerRegistration(handles) = event.kind
            {
                sequence = (u64::from(handles), index + 1);
            }
        }
        Ok(Some(sequence))
    }

    pub(super) fn record_town_registration(&mut self, unit: HandleId) {
        let Ok(handles) = u32::try_from(self.inner.handles()) else {
            self.placement_events_incomplete = true;
            return;
        };
        self.record_placement_event(unit, PlacementEventKind::OwnerRegistration(handles));
    }

    /// Set the clock for a diagnostic snapshot without replaying timer callbacks.
    #[cfg(feature = "test-fixtures")]
    pub fn diagnostic_set_snapshot_time(&mut self, at: f64) -> Result<(), super::Error> {
        self.set_timer_game_time(at)
    }

    /// Mark the end of diagnostic snapshot setup without altering the event history.
    #[cfg(feature = "test-fixtures")]
    pub fn diagnostic_placement_checkpoint(&self) -> usize {
        self.placement_event_history().count()
    }

    #[cfg(feature = "test-fixtures")]
    pub fn diagnostic_placement_events_since(
        &self,
        unit: HandleId,
        checkpoint: usize,
    ) -> Result<impl Iterator<Item = PlacementEvent> + '_, ExclusionError> {
        drop(self.placement_events(unit)?);
        if checkpoint > self.placement_event_history().count() {
            return Err(ExclusionError::Unavailable {
                unit,
                source_name: "invalid placement snapshot checkpoint",
            });
        }
        Ok(self
            .placement_event_history()
            .skip(checkpoint)
            .filter(move |event| event.unit == unit))
    }

    /// Events for this handle, in execution order. An incomplete log refuses.
    pub fn placement_events(
        &self,
        unit: HandleId,
    ) -> Result<impl Iterator<Item = PlacementEvent> + '_, ExclusionError> {
        self.executed_unit(unit)
            .map_err(|_| ExclusionError::Unavailable {
                unit,
                source_name: "unit record",
            })?;
        if self.placement_events_incomplete {
            return Err(ExclusionError::Unavailable {
                unit,
                source_name: "placement event history",
            });
        }
        Ok(self
            .placement_event_history()
            .filter(move |event| event.unit == unit))
    }

    fn placement_event_history(&self) -> impl Iterator<Item = PlacementEvent> + '_ {
        self.placement_events_inline
            .iter()
            .flatten()
            .chain(self.placement_events.iter())
            .copied()
    }

    pub(super) fn record_placement_event(&mut self, unit: HandleId, kind: PlacementEventKind) {
        let Some(game_time) = self.timer_game_time().filter(|time| time.is_finite()) else {
            self.placement_events_incomplete = true;
            return;
        };
        if self.placement_event_history().count() >= self.inner.max_handles {
            self.placement_events_incomplete = true;
            return;
        }
        let event = PlacementEvent {
            unit,
            game_time,
            kind,
        };
        if let Some(slot) = self
            .placement_events_inline
            .iter_mut()
            .find(|slot| slot.is_none())
        {
            *slot = Some(event);
            return;
        }
        let reserve = if self.placement_events.is_empty() {
            self.inner.max_handles.min(8)
        } else {
            1
        };
        if self.placement_events.try_reserve(reserve).is_err() {
            self.placement_events_incomplete = true;
            return;
        }
        self.placement_events.push(event);
    }
}
