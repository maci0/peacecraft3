use super::{Entry, ObjectHost};
use crate::ai_placement::{
    ExclusionError, KinematicWindow, PlacementEvent, PlacementEventKind, beyond_birth_cast_reach,
    beyond_cast_reach,
};
use pcx_script::HandleId;
use std::num::{NonZeroU32, NonZeroU64};

#[derive(Clone, Copy)]
pub(super) struct PackedEvent {
    time: f64,
    tagged_unit: NonZeroU32,
    payload: u32,
}

impl PackedEvent {
    fn pack(event: PlacementEvent) -> Option<Self> {
        let token = u32::try_from(event.unit.token()).ok()?;
        if token > u32::MAX >> 3 {
            return None;
        }
        let (tag, payload) = match event.kind {
            PlacementEventKind::PauseUnit(value) => (0, u32::from(value)),
            PlacementEventKind::BuffApplied(value) => (1, u32::from_ne_bytes(value)),
            PlacementEventKind::BuffRemoved(value) => (2, u32::from_ne_bytes(value)),
            PlacementEventKind::Relocated => (3, 0),
            PlacementEventKind::OwnerRegistration(value) => (4, value),
        };
        Some(Self {
            time: event.game_time,
            tagged_unit: NonZeroU32::new(token | (tag << 29))?,
            payload,
        })
    }

    fn unpack(self) -> PlacementEvent {
        let word = self.tagged_unit.get();
        let unit = HandleId::new(NonZeroU64::new(u64::from(word & (u32::MAX >> 3))).unwrap());
        let kind = match word >> 29 {
            0 => PlacementEventKind::PauseUnit(self.payload != 0),
            1 => PlacementEventKind::BuffApplied(self.payload.to_ne_bytes()),
            2 => PlacementEventKind::BuffRemoved(self.payload.to_ne_bytes()),
            3 => PlacementEventKind::Relocated,
            4 => PlacementEventKind::OwnerRegistration(self.payload),
            _ => unreachable!("packed placement event tag"),
        };
        PlacementEvent {
            unit,
            game_time: self.time,
            kind,
        }
    }
}

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

    /// The spatial cleanup interval, divided with this session's numeric policy.
    pub(crate) fn spatial_cleanup_interval(
        &self,
        rules: &crate::spatial_pool::SpatialRules,
    ) -> Result<f32, String> {
        rules.cleanup().interval(
            self.numeric
                .as_ref()
                .ok_or("script numeric policy is missing")?,
        )
    }

    /// Set the clock for a diagnostic snapshot without replaying timer callbacks.
    #[cfg(feature = "test-fixtures")]
    pub fn diagnostic_set_snapshot_time(&mut self, at: f64) -> Result<(), super::Error> {
        self.set_timer_game_time(at)
    }

    /// Read the spatial cleanup interval with this session's numeric policy.
    #[cfg(feature = "test-fixtures")]
    pub fn diagnostic_spatial_cleanup_interval(
        &self,
        image: &[u8],
        rules: &crate::ai_placement::Rules,
    ) -> Result<f32, String> {
        let numeric = self
            .numeric
            .as_ref()
            .ok_or("script numeric policy is missing")?;
        crate::native_path_hierarchy::spatial_chains::Chains::reviewed_cleanup_interval(
            image, rules, numeric,
        )
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

    /// Bound a live source against a target using their retained creation state.
    /// The caller must supply qualified speed caps and account for non-script displacement.
    pub fn ai_birth_reach_bound(
        &self,
        source: HandleId,
        target: HandleId,
        maximum_speeds: [f32; 2],
        range: f32,
    ) -> Result<bool, ExclusionError> {
        let units = [source, target];
        let mut births = [crate::playercolor_host::UnitBirth {
            game_time: None,
            position: None,
        }; 2];
        let mut radii = [0.; 2];
        let mut positions = [None; 2];
        for (index, unit) in units.into_iter().enumerate() {
            let record = self
                .executed_unit(unit)
                .map_err(|_| ExclusionError::Unavailable {
                    unit,
                    source_name: "live unit record",
                })?;
            if record.definition_revision != 0 {
                return Err(ExclusionError::UnknownMovement {
                    unit,
                    source_name: "unit form history may change collision and movement bounds",
                });
            }
            births[index] = self
                .unit_birth(unit)
                .map_err(|_| ExclusionError::Unavailable {
                    unit,
                    source_name: "unit creation history",
                })?;
            radii[index] = record
                .collision_radius
                .map_err(|source_name| ExclusionError::Unavailable { unit, source_name })?;
            positions[index] = record.position;
            if self
                .placement_events(unit)?
                .any(|event| matches!(event.kind, PlacementEventKind::Relocated))
            {
                return Err(ExclusionError::UnknownMovement {
                    unit,
                    source_name: "scripted relocation invalidates the movement bound",
                });
            }
        }
        let now = self.timer_game_time().ok_or(ExclusionError::Unavailable {
            unit: target,
            source_name: "game clock",
        })?;
        let fail = |source_name| ExclusionError::Unavailable {
            unit: target,
            source_name,
        };
        if beyond_birth_cast_reach(births, now, maximum_speeds, range, radii).map_err(fail)? {
            return Ok(true);
        }
        let [Some(source_position), Some(target_position)] = positions else {
            return Err(fail("live unit position"));
        };
        let start = births[0]
            .game_time
            .expect("validated creation clock")
            .max(births[1].game_time.expect("validated creation clock"));
        let elapsed = (now - start).next_up();
        beyond_cast_reach(
            Some(KinematicWindow {
                origin: source_position,
                maximum_speed: maximum_speeds[0],
                elapsed,
            }),
            Some(KinematicWindow {
                origin: target_position,
                maximum_speed: maximum_speeds[1],
                elapsed,
            }),
            range,
            radii,
        )
        .map_err(fail)
    }

    /// Derive speed bounds from the live units and admitted gameplay limits.
    /// The caller must still account for non-script displacement.
    pub fn ai_live_birth_reach_bound(
        &self,
        source: HandleId,
        target: HandleId,
        range: f32,
    ) -> Result<bool, ExclusionError> {
        let limits = self
            .unit_activity
            .as_ref()
            .and_then(|state| state.move_speed_limits())
            .ok_or(ExclusionError::Unavailable {
                unit: target,
                source_name: "movement speed limits",
            })?;
        let mut speeds = [0.; 2];
        for (index, unit) in [source, target].into_iter().enumerate() {
            let record = self
                .executed_unit(unit)
                .map_err(|_| ExclusionError::Unavailable {
                    unit,
                    source_name: "live unit record",
                })?;
            if record.activity.is_none() {
                return Err(ExclusionError::Unavailable {
                    unit,
                    source_name: "unit movement state",
                });
            }
            let authored =
                super::unit_activity::balance_real(&record.definition, "spd").map_err(|_| {
                    ExclusionError::Unavailable {
                        unit,
                        source_name: "unit movement speed",
                    }
                })?;
            speeds[index] = authored.max(limits.max_unit).max(limits.max_building);
        }
        self.ai_birth_reach_bound(source, target, speeds, range)
    }

    fn placement_event_history(&self) -> impl Iterator<Item = PlacementEvent> + '_ {
        self.placement_events_inline
            .iter()
            .flatten()
            .map(|event| event.unpack())
            .chain(self.placement_events.iter().copied())
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
        if self.placement_events.is_empty()
            && let Some(packed) = PackedEvent::pack(event)
            && let Some(slot) = self
                .placement_events_inline
                .iter_mut()
                .find(|slot| slot.is_none())
        {
            *slot = Some(packed);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packed_events_preserve_payload_time_and_handle() {
        let unit = HandleId::new(NonZeroU64::new(u64::from(u32::MAX >> 3)).unwrap());
        for kind in [
            PlacementEventKind::PauseUnit(false),
            PlacementEventKind::PauseUnit(true),
            PlacementEventKind::BuffApplied([0, 128, 255, 7]),
            PlacementEventKind::BuffRemoved([255, 0, 4, 1]),
            PlacementEventKind::Relocated,
            PlacementEventKind::OwnerRegistration(u32::MAX),
        ] {
            let event = PlacementEvent {
                unit,
                game_time: 12.345678901234,
                kind,
            };
            assert_eq!(PackedEvent::pack(event).unwrap().unpack(), event);
        }
        let too_large = HandleId::new(NonZeroU64::new(u64::from(u32::MAX >> 3) + 1).unwrap());
        assert!(
            PackedEvent::pack(PlacementEvent {
                unit: too_large,
                game_time: 0.0,
                kind: PlacementEventKind::Relocated,
            })
            .is_none()
        );
    }
}
