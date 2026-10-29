//! Live inputs for the campaign structure placement search.

use super::ObjectHost;
use crate::ai_placement_reach::{LinkEvidence, Reach};
use pcx_script::HandleId;

fn placement_text(definition: &pcx_sim::world::Definition, name: &str) -> Result<String, String> {
    let (_, field) = definition
        .column_field(name)
        .map_err(|error| format!("AI placement {name}: {error:?}"))?;
    match &field.value {
        Ok(pcx_sim::world::FieldValue::Cell(value)) => Ok(value.clone()),
        Ok(pcx_sim::world::FieldValue::Override(pcx_objects::Value::Str(value))) => {
            std::str::from_utf8(value)
                .map(str::to_owned)
                .map_err(|error| error.to_string())
        }
        Ok(pcx_sim::world::FieldValue::Override(pcx_objects::Value::Int(value))) => {
            Ok((*value as i32).to_string())
        }
        Ok(pcx_sim::world::FieldValue::Override(
            pcx_objects::Value::Real(value) | pcx_objects::Value::Unreal(value),
        )) if value.is_finite() => Ok(value.to_string()),
        _ => Err(format!("AI placement {name} value unavailable")),
    }
}

impl ObjectHost {
    pub(super) fn ai_structure_site(
        &mut self,
        player: i32,
        owner: HandleId,
        worker: HandleId,
        definition: &pcx_sim::world::Definition,
    ) -> Result<(Option<[f32; 2]>, usize), String> {
        use crate::ai_placement::{
            BuffType, ExclusionContext, ExclusionVisibility, Grid, MovementEvidence, exclude_unit,
        };
        use pcx_objects::editor::SlkId;
        if placement_text(definition, "buffType")?.eq_ignore_ascii_case("resource") {
            return Err("AI mine construction requires a resolved town mine target".into());
        }
        let center = self.ai_live_town_center(player, owner)?;
        #[cfg(feature = "test-fixtures")]
        if std::env::var_os("PCX_AI_PLACEMENT_TRACE").is_some() {
            eprintln!("AI placement town: player={player} center={center:?}");
            let traced = std::env::var("PCX_AI_TRACE_UNIT")
                .ok()
                .and_then(|value| value.parse::<u64>().ok());
            for (unit, record) in self.executed_units() {
                if (record.owner == owner && record.shown) || traced == Some(unit.token()) {
                    eprintln!(
                        "AI placement unit: {unit:?} owner={:?} kind={:?} position={:?} shown={} dead={}",
                        record.owner,
                        record.definition.rawcode,
                        record.position,
                        record.shown,
                        record.combat.as_ref().is_some_and(|combat| combat.dead)
                    );
                }
            }
        }
        self.ai_placement_effect_evidence(worker)?;
        let path = super::unit_positions::text(definition, SlkId::UnitData, "pathTex")
            .map_err(|error| error.to_string())?
            .to_owned();
        let masks = |name| -> Result<u8, String> {
            let value = super::unit_positions::text(definition, SlkId::UnitBalance, name)
                .map_err(|error| error.to_string())?;
            let mut mask = 0;
            for token in value.split(',').map(str::trim) {
                mask |= match token {
                    "" | "_" | "-" => 0,
                    "unbuildable" => 0x08,
                    _ => {
                        return Err(format!(
                            "AI placement {name} requirement {token:?} is not admitted"
                        ));
                    }
                };
            }
            Ok(mask)
        };
        let prevent = masks("preventPlace")?;
        let require = masks("requirePlace")?;
        let face = self
            .construction_facing()
            .map_err(|error| error.to_string())?;
        if !face.is_finite() || (face / 90.).fract() != 0. {
            return Err("AI placement building rotation is not admitted".into());
        }
        let quarter = (face / 90.).rem_euclid(4.) as u8;
        let search_radius = self
            .unit_placement
            .as_ref()
            .and_then(|placement| placement.spatial())
            .and_then(|pool| pool.placement_rules())
            .ok_or("AI placement rules missing")?
            .diagnostic_radius();
        let mut objects = Vec::new();
        let mut units = Vec::new();
        for (unit, record) in self.executed_units() {
            if !record.shown || record.combat.as_ref().is_some_and(|combat| combat.dead) {
                continue;
            }
            let point = record
                .position
                .ok_or_else(|| format!("AI placement unit {unit:?} position missing"))?;
            if super::unit_types::structure(&record.definition)
                .map_err(|error| error.to_string())?
            {
                if (point[0] - center[0]).hypot(point[1] - center[1]) > search_radius {
                    continue;
                }
                let object_path =
                    super::unit_positions::text(&record.definition, SlkId::UnitData, "pathTex")
                        .map_err(|error| error.to_string())?
                        .to_owned();
                let kind = placement_text(&record.definition, "buffType")?;
                let kind = match kind.as_str() {
                    "townhall" => BuffType::TownHall,
                    "resource" => BuffType::Resource,
                    "factory" => BuffType::Factory,
                    "buffer" => BuffType::Buffer,
                    _ => BuffType::Other,
                };
                let radius = if matches!(kind, BuffType::Other) {
                    0
                } else {
                    let radius = placement_text(&record.definition, "buffRadius")?
                        .parse::<f32>()
                        .map_err(|error| {
                            format!("AI placement object {unit:?} buffer radius: {error}")
                        })?;
                    if !radius.is_finite() || radius < 0. || radius >= i32::MAX as f32 {
                        return Err(format!(
                            "AI placement object {unit:?} has invalid buffer radius"
                        ));
                    }
                    radius as i32
                };
                objects.push((unit, point, object_path, kind, radius));
            } else if unit != worker {
                units.push((unit, record.owner, point));
            }
        }
        let placement = self
            .unit_placement
            .as_mut()
            .ok_or("AI placement cache missing")?;
        let target_texture = placement
            .native_texture(&path)
            .map_err(|error| error.to_string())?;
        let dimensions = [
            u32::from(target_texture.width),
            u32::from(target_texture.height),
        ];
        let pool = placement
            .spatial()
            .ok_or("AI placement spatial pool missing")?;
        let rules = pool
            .placement_rules()
            .ok_or("AI placement rules missing")?
            .clone();
        let bounds = placement.world_bounds();
        let reach = Reach::from_placement(placement, worker, LinkEvidence::Absent)?;
        let mut grid = Grid::sample(&rules, center, |point| {
            placement
                .diagnostic_sample(point)
                .map_err(|error| error.to_string())
        })?;
        for (unit, point, path, kind, radius) in objects {
            if (point[0] - center[0]).hypot(point[1] - center[1]) > rules.diagnostic_radius() {
                continue;
            }
            if matches!(kind, BuffType::Buffer) {
                return Err(format!(
                    "AI placement structure {unit:?} requires the buffer-count scoring branch"
                ));
            }
            if matches!(path.as_str(), "" | "_" | "-" | "none") {
                return Err(format!(
                    "AI placement structure {unit:?} has no texture rectangle"
                ));
            }
            let texture = placement
                .native_texture(&path)
                .map_err(|error| error.to_string())?;
            let mut size = [u32::from(texture.width), u32::from(texture.height)];
            if size[0] != size[1] {
                return Err(format!(
                    "AI placement structure {unit:?} has an unqualified non-square scoring texture"
                ));
            }
            if !quarter.is_multiple_of(2) {
                size.swap(0, 1);
            }
            let step = grid.diagnostic_step();
            for x in 0..size[0] {
                for y in 0..size[1] {
                    let coordinate = grid.diagnostic_object_indices([
                        point[0] + (x as f32 - (size[0] - 1) as f32 / 2.) * step,
                        point[1] + (y as f32 - (size[1] - 1) as f32 / 2.) * step,
                    ])?;
                    grid.score_rectangle([coordinate], kind, radius)?;
                }
            }
        }
        let host = std::cell::RefCell::new(self);
        let mut qualified = std::collections::BTreeSet::new();
        let rank = std::cell::Cell::new(0usize);
        let result = grid.search(
            |point| {
                rank.set(rank.get() + 1);
                let point = rules.snap(point, bounds, dimensions, quarter)?;
                #[cfg(feature = "test-fixtures")]
                if rank.get() <= 10 && std::env::var_os("PCX_AI_CANDIDATE_TRACE").is_some() {
                    eprintln!(
                        "AI candidate {}: point={point:?} score={:?}",
                        rank.get(),
                        grid.diagnostic_cell(point)
                    );
                }
                Ok((point, grid.diagnostic_indices(point)))
            },
            |point| {
                let (result, removals) = reach.query(center, point)?;
                #[cfg(feature = "test-fixtures")]
                if rank.get() <= 10 && std::env::var_os("PCX_AI_CANDIDATE_TRACE").is_some() {
                    eprintln!(
                        "AI candidate {}: reach={result:?} removals={removals}",
                        rank.get()
                    );
                }
                #[cfg(not(feature = "test-fixtures"))]
                let _ = removals;
                Ok(result.is_some())
            },
            |point| {
                let rectangle = rules.exclusion_rectangle(point, dimensions, quarter)?;
                let mut excluded = vec![worker];
                for &(unit, unit_owner, position) in &units {
                    if (0..2).any(|axis| {
                        position[axis] <= rectangle[0][axis] || position[axis] >= rectangle[1][axis]
                    }) {
                        continue;
                    }
                    let shared = host
                        .borrow()
                        .diagnostic_shared_control(unit_owner, owner)
                        .map_err(str::to_owned)?;
                    if !shared {
                        continue;
                    }
                    if !qualified.contains(&unit) {
                        host.borrow().ai_placement_effect_evidence(unit)?;
                        qualified.insert(unit);
                    }
                    if exclude_unit(
                        unit,
                        position,
                        rectangle,
                        ExclusionContext::AiPlacement,
                        shared,
                        ExclusionVisibility::NotChecked,
                        MovementEvidence::CountersZero,
                    )
                    .map_err(|error| error.to_string())?
                    {
                        excluded.push(unit);
                    }
                }
                let result = host
                    .borrow_mut()
                    .unit_placement
                    .as_mut()
                    .ok_or("AI placement cache missing")?
                    .diagnostic_construction_position_excluding(
                        point, face, &path, prevent, require, &excluded,
                    )
                    .map_err(|error| error.to_string())?;
                #[cfg(feature = "test-fixtures")]
                if rank.get() <= 10 && std::env::var_os("PCX_AI_CANDIDATE_TRACE").is_some() {
                    eprintln!(
                        "AI candidate {}: valid={result:?} excluded={excluded:?}",
                        rank.get()
                    );
                }
                Ok(result.is_some())
            },
        )?;
        Ok((result, rank.get()))
    }

    pub(super) fn ai_capture_town_center(
        &mut self,
        owner: HandleId,
        reference: &super::campaign_flee::TownReference,
    ) -> Result<[f32; 2], String> {
        self.ai_placement_source_evidence(reference.unit, true)?;
        let settings = self
            .player_settings(owner)
            .map_err(|error| error.to_string())?;
        let start_id = settings
            .start_location
            .ok_or("AI town start location missing")?;
        let start = self
            .map_config()
            .ok_or("AI town map configuration missing")?
            .start_locations
            .iter()
            .find(|location| location.index == start_id)
            .map(|location| [location.x, location.y])
            .ok_or("AI town start coordinates missing")?;
        let placement = self
            .unit_placement
            .as_ref()
            .ok_or("AI town placement missing")?;
        let pool = placement.spatial().ok_or("AI town spatial pool missing")?;
        if let Some(refusal) = pool.refusal() {
            return Err(format!("AI town spatial pool refused: {refusal:?}"));
        }
        let (threshold, limit, increment, merge_threshold) = pool
            .town_operands()
            .ok_or("AI town search operands missing")?;
        let minimum_work = limit.min(increment);
        if minimum_work == 0 {
            return Err("AI town guaranteed search work is zero".into());
        }
        let (_, (_, _, scale, _, _, _)) =
            pool.reach_inputs().ok_or("AI town reach inputs missing")?;
        let bounds = placement.world_bounds();
        let cell_size = placement.rules().cell_size() * scale as f32;
        let cell = |point: [f32; 2]| -> Result<[i32; 2], String> {
            if point.iter().any(|v| !v.is_finite()) || !cell_size.is_finite() || cell_size <= 0. {
                return Err("AI town coordinate is invalid".into());
            }
            Ok(std::array::from_fn(|axis| {
                ((point[axis] - bounds[0][axis]) / cell_size).floor() as i32
            }))
        };
        let lower_bound = |a: [f32; 2], b: [f32; 2]| -> Result<u64, String> {
            let a = cell(a)?;
            let b = cell(b)?;
            Ok(u64::from(scale)
                * (i64::from(a[0])
                    .abs_diff(i64::from(b[0]))
                    .max(i64::from(a[1]).abs_diff(i64::from(b[1])))))
        };
        for (index, (_, a)) in reference.mines.iter().enumerate() {
            for (_, b) in &reference.mines[index + 1..] {
                if lower_bound(*a, *b)? < u64::from(merge_threshold) {
                    return Err("AI town initial mine records can merge".into());
                }
            }
        }
        let candidate = |point| -> Result<Option<(HandleId, [f32; 2])>, String> {
            let mut result = None;
            for &(unit, mine) in &reference.mines {
                if lower_bound(mine, point)? < u64::from(threshold) {
                    if result.is_some() {
                        return Err("AI town has multiple reachable initial mine records".into());
                    }
                    result = Some((unit, mine));
                }
            }
            Ok(result)
        };
        let start_mine = candidate(start)?;
        let reference_mine = candidate(reference.position)?;
        if start_mine != reference_mine {
            return Err("AI town start and peon select different initial mine records".into());
        }
        let (mine, position) =
            start_mine.ok_or("AI town has no uniquely qualified initial mine")?;
        for (unit, record) in self.executed_units() {
            if !record.shown
                || record.combat.as_ref().is_some_and(|combat| combat.dead)
                || !super::unit_types::structure(&record.definition)
                    .map_err(|error| error.to_string())?
            {
                continue;
            }
            let hall = record
                .position
                .ok_or_else(|| format!("AI initial hall {unit:?} position missing"))?;
            let nearby = lower_bound(hall, start)? < u64::from(threshold)
                || lower_bound(hall, reference.position)? < u64::from(threshold)
                || lower_bound(hall, position)? < u64::from(merge_threshold);
            if nearby
                && placement_text(&record.definition, "buffType")?.eq_ignore_ascii_case("townhall")
            {
                return Err(format!(
                    "AI initial hall {unit:?} may change town association"
                ));
            }
        }
        let record = self
            .executed_unit(mine)
            .map_err(|error| error.to_string())?;
        let path = super::unit_positions::text(
            &record.definition,
            pcx_objects::editor::SlkId::UnitData,
            "pathTex",
        )
        .map_err(|error| error.to_string())?
        .to_owned();
        let face = record.face;
        let radius = record.collision_radius.map_err(str::to_owned)?;
        let placement = self
            .unit_placement
            .as_mut()
            .ok_or("AI town placement missing")?;
        let texture = placement
            .native_texture(&path)
            .map_err(|error| error.to_string())?;
        let mut dimensions = [usize::from(texture.width), usize::from(texture.height)];
        if !face.is_finite() || (face / 90.).fract() != 0. {
            return Err(format!(
                "AI town mine {mine:?} texture rotation is not admitted"
            ));
        }
        if (face / 90.).rem_euclid(2.) != 0. {
            dimensions.swap(0, 1);
        }
        let pool = placement.spatial().ok_or("AI town spatial pool missing")?;
        let rectangle = pool
            .texture_rectangle(mine)
            .ok_or_else(|| format!("AI town mine {mine:?} texture rectangle missing"))?;
        if [
            rectangle.upper[0] - rectangle.lower[0],
            rectangle.upper[1] - rectangle.lower[1],
        ] != dimensions
        {
            return Err(format!(
                "AI town mine {mine:?} texture rectangle has unqualified empty borders"
            ));
        }
        for axis in 0..2 {
            let native_axis = 1 - axis;
            let low = bounds[0][axis]
                + rectangle.lower[native_axis] as f32 * placement.rules().cell_size();
            let high = bounds[0][axis]
                + rectangle.upper[native_axis] as f32 * placement.rules().cell_size();
            if position[axis] - radius - placement.rules().cell_size() < low
                || position[axis] + radius + placement.rules().cell_size() > high
            {
                return Err(format!(
                    "AI town mine {mine:?} movement rectangle exceeds texture rectangle"
                ));
            }
        }
        let reference_rect = placement
            .movement_stamp_rectangle(reference.unit)
            .map_err(|error| error.to_string())?;
        let start_reach = Reach::from_rectangles(placement, &[rectangle], LinkEvidence::Absent)?;
        let selected = start_reach
            .query_with_limit(position, start, minimum_work)?
            .0;
        if selected.is_none_or(|distance| distance >= threshold) {
            return Err(format!(
                "AI town mine {mine:?} start reach exceeds the guaranteed work limit"
            ));
        }
        let reference_reach = Reach::from_rectangles(
            placement,
            &[rectangle, reference_rect],
            LinkEvidence::Absent,
        )?;
        let selected = reference_reach
            .query_with_limit(position, reference.position, minimum_work)?
            .0;
        if selected.is_none_or(|distance| distance >= threshold) {
            return Err(format!(
                "AI town mine {mine:?} peon reach exceeds the guaranteed work limit"
            ));
        }
        pool.placement_rules()
            .ok_or("AI town placement rules missing")?
            .town_center(start, Some(position), Some(reference.position))
    }

    pub(super) fn ai_live_town_center(
        &self,
        player: i32,
        owner: HandleId,
    ) -> Result<[f32; 2], String> {
        let reference = self
            .campaign_ai
            .town_reference(player)
            .ok_or("AI initial peon town reference was not captured")?;
        let unit = self.executed_unit(reference.unit).map_err(|error| {
            format!(
                "AI town reference {:?} no longer exists: {error}",
                reference.unit
            )
        })?;
        if unit.owner != owner
            || !unit.shown
            || unit.combat.as_ref().is_some_and(|state| state.dead)
        {
            return Err(format!(
                "AI town reference {:?} changed owner or eligibility",
                reference.unit
            ));
        }
        for (unit, record) in self.executed_units() {
            if record.owner == owner
                && record.shown
                && super::unit_types::structure(&record.definition)
                    .map_err(|error| error.to_string())?
            {
                let (_, field) = record
                    .definition
                    .column_field("buffType")
                    .map_err(|error| format!("AI town structure {unit:?} buff type: {error:?}"))?;
                let value = match &field.value {
                    Ok(pcx_sim::world::FieldValue::Cell(value)) => value.as_str(),
                    Ok(pcx_sim::world::FieldValue::Override(pcx_objects::Value::Str(value))) => {
                        std::str::from_utf8(value.as_slice()).map_err(|error| error.to_string())?
                    }
                    _ => return Err(format!("AI town structure {unit:?} buff type unavailable")),
                };
                if value.eq_ignore_ascii_case("townhall") {
                    return Err(format!(
                        "AI town structure {unit:?} changes the initial town record"
                    ));
                }
            }
        }
        let mines = self
            .executed_units()
            .filter(|(_, record)| {
                record.resource_amount.is_some()
                    && record.shown
                    && record.combat.as_ref().is_none_or(|combat| !combat.dead)
            })
            .map(|(unit, record)| record.position.map(|position| (unit, position)))
            .collect::<Option<Vec<_>>>()
            .ok_or("AI town mine position unavailable")?;
        if mines != reference.mines {
            return Err("AI town mine registration changed since captain initialization".into());
        }
        reference.center.clone()
    }

    #[cfg(feature = "test-fixtures")]
    pub fn diagnostic_ai_effect_evidence(&self, target: HandleId) -> Result<(), String> {
        self.ai_placement_effect_evidence(target)
    }

    #[cfg(feature = "test-fixtures")]
    pub fn diagnostic_ai_town_capture(&mut self, owner: HandleId) -> Result<[f32; 2], String> {
        let unit = self
            .ai_town_reference(owner)
            .map_err(|error| error.to_string())?
            .ok_or("AI initial town peon missing")?;
        let position = self
            .executed_unit(unit)
            .map_err(|error| error.to_string())?
            .position
            .ok_or("AI initial town peon position missing")?;
        let mines = self
            .executed_units()
            .filter(|(_, record)| {
                record.resource_amount.is_some()
                    && record.shown
                    && record.combat.as_ref().is_none_or(|combat| !combat.dead)
            })
            .map(|(unit, record)| record.position.map(|position| (unit, position)))
            .collect::<Option<Vec<_>>>()
            .ok_or("AI initial town mine position missing")?;
        self.ai_capture_town_center(
            owner,
            &super::campaign_flee::TownReference {
                unit,
                position,
                mines,
                center: Err("not evaluated".into()),
            },
        )
    }

    #[cfg(feature = "test-fixtures")]
    pub fn diagnostic_install_ai_town(
        &mut self,
        player: i32,
        owner: HandleId,
    ) -> Result<(), String> {
        let center = self.diagnostic_ai_town_capture(owner)?;
        let unit = self
            .ai_town_reference(owner)
            .map_err(|error| error.to_string())?
            .ok_or("AI initial town peon missing")?;
        let position = self
            .executed_unit(unit)
            .map_err(|error| error.to_string())?
            .position
            .ok_or("AI initial town peon position missing")?;
        let mines = self
            .executed_units()
            .filter(|(_, record)| {
                record.resource_amount.is_some()
                    && record.shown
                    && record.combat.as_ref().is_none_or(|combat| !combat.dead)
            })
            .map(|(unit, record)| record.position.map(|position| (unit, position)))
            .collect::<Option<Vec<_>>>()
            .ok_or("AI initial town mine position missing")?;
        if !self.campaign_ai.is_active(player) {
            self.campaign_ai.activate(player, None, None);
        }
        self.campaign_ai.set_town_reference(
            player,
            Some(super::campaign_flee::TownReference {
                unit,
                position,
                mines,
                center: Ok(center),
            }),
        );
        Ok(())
    }

    #[cfg(feature = "test-fixtures")]
    pub fn diagnostic_ai_structure_site(
        &mut self,
        player: i32,
        owner: HandleId,
        worker: HandleId,
        kind: [u8; 4],
    ) -> Result<(Option<[f32; 2]>, usize), String> {
        let definition = self
            .unit_catalog()
            .map_err(|error| error.to_string())?
            .resolve(kind)
            .map_err(|error| format!("AI placement catalog: {error:?}"))?;
        self.ai_structure_site(player, owner, worker, &definition)
    }

    /// Establish whether live abilities can add a reach link or a disabling effect.
    pub(super) fn ai_placement_effect_evidence(&self, target: HandleId) -> Result<(), String> {
        self.ai_placement_source_evidence(target, false)
    }

    fn ai_placement_source_evidence(
        &self,
        target: HandleId,
        warp_only: bool,
    ) -> Result<(), String> {
        let classes = self
            .unit_placement
            .as_ref()
            .and_then(|placement| placement.spatial())
            .ok_or("AI placement spatial pool is missing")?
            .placement_classes();
        if !warp_only {
            let target_record = self
                .executed_unit(target)
                .map_err(|error| error.to_string())?;
            if target_record.paused_since.is_some() || target_record.is_sleeping() {
                return Err(format!(
                    "AI placement unit {target:?} has an active pause or sleep"
                ));
            }
            for buff in self.unit_buffs(target).map_err(|error| error.to_string())? {
                let code = self.placement_buff_code(buff.rawcode())?;
                if classes[1..5].contains(&code) {
                    return Err(format!(
                        "AI placement unit {target:?} has active movement effect {:?}",
                        buff.rawcode()
                    ));
                }
            }
            drop(
                self.placement_events(target)
                    .map_err(|error| error.to_string())?,
            );
        }
        for (source, record) in self.executed_units() {
            let abilities = record.abilities.as_ref().ok_or_else(|| {
                format!("AI placement ability state unavailable for unit {source:?}")
            })?;
            for (alias, rank) in abilities.levels().filter(|(_, rank)| *rank > 0) {
                self.ai_placement_ability_effect(source, target, alias, rank, classes, warp_only)?;
            }
        }
        for (item, record) in self.executed_items() {
            let Some((holder, _)) = record.held_by else {
                continue;
            };
            let (_, field) = record
                .definition
                .column_field("abilList")
                .map_err(|_| format!("AI placement item {item:?} ability list is unavailable"))?;
            let text = match field.value.as_ref() {
                Ok(pcx_sim::world::FieldValue::Cell(value)) => value.as_str(),
                Ok(pcx_sim::world::FieldValue::Override(pcx_objects::Value::Str(value))) => {
                    std::str::from_utf8(value.as_slice()).map_err(|_| {
                        format!("AI placement item {item:?} ability list is not UTF-8")
                    })?
                }
                _ => {
                    return Err(format!(
                        "AI placement item {item:?} ability list is unavailable"
                    ));
                }
            };
            for token in text
                .split(',')
                .map(str::trim)
                .filter(|token| !matches!(*token, "" | "_" | "-"))
            {
                let alias: [u8; 4] = token.as_bytes().try_into().map_err(|_| {
                    format!("AI placement item {item:?} has malformed ability {token:?}")
                })?;
                self.ai_placement_ability_effect(holder, target, alias, 1, classes, warp_only)?;
            }
        }
        Ok(())
    }

    fn ai_placement_ability_effect(
        &self,
        source: HandleId,
        target: HandleId,
        alias: [u8; 4],
        rank: i32,
        classes: [[u8; 4]; 8],
        warp_only: bool,
    ) -> Result<(), String> {
        use pcx_objects::editor::FieldKind;
        let code = self.ability_data_text(alias, "code").map_err(|error| {
            format!("AI placement source {source:?} ability {alias:?}: {error}")
        })?;
        if code.as_bytes() == classes[0] {
            return Err(format!(
                "AI placement source {source:?} has Warp ability {alias:?}; auxiliary reach links are not admitted"
            ));
        }
        if warp_only {
            return Ok(());
        }
        let buffs = match self.hero_ability_level_cell(alias, "BuffID", rank, FieldKind::Token) {
            Ok(value) => value,
            Err(error)
                if error.contains(&format!("missing BuffID{rank}"))
                    || error.ends_with(&format!("no authored cell BuffID{rank}")) =>
            {
                String::new()
            }
            Err(error) => {
                return Err(format!(
                    "AI placement source {source:?} ability {alias:?}: {error}"
                ));
            }
        };
        let mut relevant = classes[5..].iter().any(|class| code.as_bytes() == class);
        for buff in buffs
            .split(',')
            .map(str::trim)
            .filter(|buff| !matches!(*buff, "" | "_" | "-"))
        {
            let raw: [u8; 4] = buff.as_bytes().try_into().map_err(|_| {
                format!("AI placement ability {alias:?} has malformed buff {buff:?}")
            })?;
            let class = self
                .placement_buff_code(raw)
                .map_err(|error| format!("AI placement source {source:?} buff {raw:?}: {error}"))?;
            relevant |= classes[1..5].contains(&class);
        }
        if relevant {
            let range = self
                .hero_ability_level_cell(alias, "Rng", rank, FieldKind::Unreal)
                .map_err(|error| {
                    format!("AI placement source {source:?} ability {alias:?} range: {error}")
                })?
                .parse::<f32>()
                .map_err(|error| {
                    format!("AI placement source {source:?} ability {alias:?} range: {error}")
                })?;
            let area = match self.hero_ability_level_cell(alias, "Area", rank, FieldKind::Unreal) {
                Ok(value) => value.parse::<f32>().map_err(|error| {
                    format!("AI placement source {source:?} ability {alias:?} area: {error}")
                })?,
                Err(error)
                    if error.contains(&format!("missing Area{rank}"))
                        || error.ends_with(&format!("no authored cell Area{rank}")) =>
                {
                    0.
                }
                Err(error) => {
                    return Err(format!(
                        "AI placement source {source:?} ability {alias:?} area: {error}"
                    ));
                }
            };
            if !range.is_finite() || range < 0. || !area.is_finite() || area < 0. {
                return Err(format!(
                    "AI placement source {source:?} ability {alias:?} has invalid reach"
                ));
            }
            let range = (range + area).next_up();
            if !range.is_finite() {
                return Err(format!(
                    "AI placement source {source:?} ability {alias:?} reach overflow"
                ));
            }
            if !self
                .ai_live_birth_reach_bound(source, target, range)
                .map_err(|error| error.to_string())?
            {
                return Err(format!(
                    "AI placement unit {target:?}: movement effect {alias:?} from unit {source:?} may have reached it"
                ));
            }
        }
        Ok(())
    }

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
