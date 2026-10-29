//! Live inputs for the campaign structure placement search.

use super::ObjectHost;
use crate::ai_placement_reach::{LinkEvidence, Reach};
use pcx_script::HandleId;

impl ObjectHost {
    /// Compare resource candidates with the placement reach budget as a hypothesis.
    /// This is not the town-association search: it does not clear mine rectangles
    /// or use the town selector's work limit and registered-town list.
    #[cfg(feature = "test-fixtures")]
    pub fn diagnostic_ai_town_hypothesis(
        &self,
        owner: HandleId,
        worker: HandleId,
    ) -> Result<[f32; 2], String> {
        let player = self
            .player_settings(owner)
            .map_err(|error| error.to_string())?;
        let index = player
            .start_location
            .ok_or("AI start location is missing")?;
        let start = self
            .map_config()
            .ok_or("AI map configuration is missing")?
            .start_locations
            .iter()
            .find(|location| location.index == index)
            .map(|location| [location.x, location.y])
            .ok_or("AI start coordinates are missing")?;
        let placement = self
            .unit_placement
            .as_ref()
            .ok_or("AI placement cache is missing")?;
        let pool = placement.spatial().ok_or("AI spatial pool is missing")?;
        let rules = pool
            .placement_rules()
            .ok_or("AI placement rules are missing")?;
        let reach = Reach::from_placement(placement, worker, LinkEvidence::Absent)?;
        let mut selected: Option<(u32, [f32; 2])> = None;
        for (_, record) in self.executed_units() {
            if record.resource_amount.is_none()
                || !record.shown
                || record.combat.as_ref().is_some_and(|combat| combat.dead)
            {
                continue;
            }
            let point = record.position.ok_or("AI mine position is missing")?;
            let (distance, _) = reach.query(point, start)?;
            if let Some(distance) = distance
                && selected.is_none_or(|(old, _)| distance < old)
            {
                selected = Some((distance, point));
            }
        }
        let reference = self
            .ai_town_reference(owner)
            .map_err(|error| error.to_string())?
            .map(|unit| {
                self.executed_unit(unit)
                    .map_err(|error| error.to_string())
                    .and_then(|record| {
                        record
                            .position
                            .ok_or("AI town reference position is missing".to_string())
                    })
            })
            .transpose()?;
        rules.town_center(start, selected.map(|(_, point)| point), reference)
    }
}
