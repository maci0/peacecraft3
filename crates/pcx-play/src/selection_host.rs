//! The local player's unit selection, shared by scripts and player input.
//! Only the local player has a selection; other players read as empty.
//! `SyncSelections` completes at once in this single-machine session.
//!
//! Selection UI gates (`EnableSelect`, `EnableDragSelect`, `EnablePreSelect`) and
//! per-unit selectability (`BlzIsUnitSelectable`, `BlzUnitDisableSelection`, the
//! `Aloc` locust effect) are admitted here. The exe strings exist
//! (`campaign-box-selection.md`); signatures are read from the script's own
//! native declarations, so a map without them keeps the enabled defaults.
use super::{Entry, Error, Failure, ObjectHost};
use pcx_script::{HandleId, ScriptCall, Value};

const NATIVES: [(&str, &[&str], &str, bool); 5] = [
    ("ClearSelection", &[], "nothing", false),
    ("SelectUnit", &["unit", "boolean"], "nothing", false),
    ("SyncSelections", &[], "nothing", false),
    ("IsUnitSelected", &["unit", "player"], "boolean", true),
    (
        "GroupEnumUnitsSelected",
        &["group", "player", "boolexpr"],
        "nothing",
        false,
    ),
];

/// Locust ability: units carrying it at a positive level are not input-pickable.
/// The rawcode is a shipped ability identifier, not a measured selection value.
const LOCUST: [u8; 4] = *b"Aloc";

fn has_locust(record: &super::ExecutedUnit) -> bool {
    record
        .abilities
        .as_ref()
        .is_some_and(|abilities| abilities.level(LOCUST) > 0)
}

impl ObjectHost {
    pub fn with_selection(mut self) -> Result<Self, Error> {
        for (name, args, result, constant) in NATIVES {
            if self.inner.program.natives().get(name).is_some() {
                Self::contract_in(&self.inner, name, args, result, constant, None, false)?;
            }
        }
        // `EnableSelect`/`EnableDragSelect`/`EnablePreSelect` ship as
        // `(boolean flag, boolean fade)`; some maps declare the single-flag form.
        // Admit either boolean-only shape instead of substituting a signature.
        for name in ["EnableSelect", "EnableDragSelect", "EnablePreSelect"] {
            if self.inner.program.natives().get(name).is_none() {
                continue;
            }
            if Self::contract_in(
                &self.inner,
                name,
                &["boolean", "boolean"],
                "nothing",
                false,
                None,
                false,
            )
            .is_err()
            {
                Self::contract_in(
                    &self.inner,
                    name,
                    &["boolean"],
                    "nothing",
                    false,
                    None,
                    false,
                )?;
            }
        }
        for (name, args, result, constant, family) in [
            (
                "BlzIsUnitSelectable",
                &["unit"][..],
                "boolean",
                false,
                Some("unit"),
            ),
            (
                "BlzUnitDisableSelection",
                &["unit", "boolean"][..],
                "nothing",
                false,
                Some("unit"),
            ),
        ] {
            if self.inner.program.natives().get(name).is_some() {
                Self::contract_in(&self.inner, name, args, result, constant, family, true)?;
            }
        }
        self.selection = Some(Vec::new());
        Ok(self)
    }

    /// Selected units in selection order.
    pub fn selection(&self) -> &[HandleId] {
        self.selection.as_deref().unwrap_or(&[])
    }

    /// Script gates; `None` (never executed) is the client's default: enabled.
    pub fn select_allowed(&self) -> bool {
        self.select_enabled.unwrap_or(true)
    }

    pub fn drag_select_allowed(&self) -> bool {
        self.drag_select_enabled.unwrap_or(true)
    }

    pub fn preselect_allowed(&self) -> bool {
        self.preselect_enabled.unwrap_or(true)
    }

    /// Per-unit input selectability: an explicit `BlzUnitDisableSelection`
    /// refusal wins, then the `Aloc` locust effect (inference, exe flag bits
    /// unread). Scripts can still force-select via `SelectUnit`; input picks
    /// (`box_candidate` and the click pick) honour this.
    pub fn is_unit_selectable(&self, id: HandleId) -> bool {
        if self
            .unit_selectable
            .get(&id)
            .is_some_and(|&allowed| !allowed)
        {
            return false;
        }
        match self.inner.entry(id) {
            Ok(Entry::Unit { record }) => !has_locust(record),
            _ => false,
        }
    }

    /// The local player's drag-box view of `id`: `None` for a unit that is not live,
    /// shown and selectable. Visibility to the local player and screen position
    /// are the caller's. Unselectable units never enter a box pick, the same way
    /// the down handler tests its per-unit flag before selecting (`0x5fd820`).
    pub fn box_candidate(
        &self,
        id: HandleId,
    ) -> Result<Option<crate::selection_box::Candidate>, String> {
        let record = self.executed_unit(id).map_err(|e| e.to_string())?;
        if !record.shown || record.combat.as_ref().is_some_and(|c| c.dead) {
            return Ok(None);
        }
        if !self.is_unit_selectable(id) {
            return Ok(None);
        }
        let local = self
            .local_player_settings()
            .ok_or("box selection has no local player")?;
        let owner = self
            .player_settings(record.owner)
            .map_err(|e| e.to_string())?;
        Ok(Some(crate::selection_box::Candidate {
            id,
            own: owner.identity == local.identity,
            structure: super::unit_types::structure(&record.definition)?,
            hero: record.abilities.as_ref().is_some_and(|a| a.hero.is_some()),
            priority: super::subgroup_host::priority(&record.definition).unwrap_or(0),
            kind: record.definition.rawcode,
        }))
    }

    /// Player input: replace or extend the local selection with live, shown units.
    pub fn select_units(&mut self, units: &[HandleId], replace: bool) {
        let live: Vec<HandleId> = units
            .iter()
            .copied()
            .filter(|&u| {
                matches!(self.inner.entry(u), Ok(Entry::Unit { record })
                    if record.shown && record.combat.as_ref().is_none_or(|c| !c.dead))
            })
            .collect();
        self.active_subgroup = None;
        let Some(selection) = &mut self.selection else {
            return;
        };
        if replace {
            selection.clear();
        }
        for unit in live {
            if !selection.contains(&unit) {
                selection.push(unit);
            }
        }
    }

    fn local_player_handle(&self, player: &Value) -> bool {
        let Value::Handle(id) = player else {
            return false;
        };
        matches!(self.inner.entry(*id), Ok(Entry::Player { number })
            if Some(*number) == self.local_player_number())
    }

    pub(super) fn begin_selection_call(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Result<Option<ScriptCall>, Error> {
        if name != "GroupEnumUnitsSelected" || self.selection.is_none() {
            return Ok(None);
        }
        let [group, player, filter] = args else {
            return Err(self.inner.error(name, Failure::Signature));
        };
        let group = self.inner.id(self.group_index(group, name)?, name)?;
        let candidates = if self.local_player_handle(player) {
            self.selection().to_vec()
        } else {
            Vec::new()
        };
        let predicate = match filter {
            Value::Null => None,
            Value::Handle(id) => Some(match self.inner.entry(*id)? {
                Entry::Filter { .. } => self.filter_callback(*id)?.function,
                Entry::Condition { .. } => self.condition_callback(*id)?.function,
                _ => return Err(self.inner.error(name, Failure::WrongFamily)),
            }),
            _ => return Err(self.inner.error(name, Failure::WrongFamily)),
        };
        self.begin_unit_enumeration("GroupEnumUnitsSelected", group, candidates, predicate)
    }

    pub(super) fn selection_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        self.selection.as_ref()?;
        if !matches!(
            name,
            "ClearSelection"
                | "SelectUnit"
                | "SyncSelections"
                | "IsUnitSelected"
                | "EnableSelect"
                | "EnableDragSelect"
                | "EnablePreSelect"
                | "BlzIsUnitSelectable"
                | "BlzUnitDisableSelection"
        ) {
            return None;
        }
        self.inner.program.natives().get(name)?;
        Some(match (name, args) {
            ("ClearSelection", []) => {
                self.active_subgroup = None;
                self.selection.as_mut().expect("admitted").clear();
                Ok(Value::Null)
            }
            ("SyncSelections", []) => Ok(Value::Null),
            ("SelectUnit", [Value::Null, Value::Bool(_)]) => Ok(Value::Null),
            ("SelectUnit", [Value::Handle(unit), Value::Bool(true)]) => {
                self.select_units(&[*unit], false);
                Ok(Value::Null)
            }
            ("SelectUnit", [Value::Handle(unit), Value::Bool(false)]) => {
                self.selection
                    .as_mut()
                    .expect("admitted")
                    .retain(|u| u != unit);
                Ok(Value::Null)
            }
            ("IsUnitSelected", [unit, player]) => Ok(Value::Bool(
                self.local_player_handle(player)
                    && matches!(unit, Value::Handle(u) if self.selection().contains(u)),
            )),
            ("EnableSelect", [Value::Bool(flag)])
            | ("EnableSelect", [Value::Bool(flag), Value::Bool(_)]) => {
                self.select_enabled = Some(*flag);
                Ok(Value::Null)
            }
            ("EnableDragSelect", [Value::Bool(flag)])
            | ("EnableDragSelect", [Value::Bool(flag), Value::Bool(_)]) => {
                self.drag_select_enabled = Some(*flag);
                Ok(Value::Null)
            }
            ("EnablePreSelect", [Value::Bool(flag)])
            | ("EnablePreSelect", [Value::Bool(flag), Value::Bool(_)]) => {
                self.preselect_enabled = Some(*flag);
                Ok(Value::Null)
            }
            ("BlzIsUnitSelectable", [Value::Null]) => Ok(Value::Bool(false)),
            ("BlzIsUnitSelectable", [Value::Handle(unit)]) => {
                Ok(Value::Bool(self.is_unit_selectable(*unit)))
            }
            ("BlzUnitDisableSelection", [Value::Null, Value::Bool(_)]) => Ok(Value::Null),
            ("BlzUnitDisableSelection", [Value::Handle(unit), Value::Bool(disable)]) => {
                self.unit_selectable.insert(*unit, !disable);
                Ok(Value::Null)
            }
            _ => Err(self.inner.error(name, Failure::Signature)),
        })
    }
}
