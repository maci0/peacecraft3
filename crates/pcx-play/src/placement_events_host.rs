use super::ObjectHost;
use crate::ai_placement::{ExclusionError, PlacementEvent, PlacementEventKind};
use pcx_script::HandleId;

impl ObjectHost {
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
