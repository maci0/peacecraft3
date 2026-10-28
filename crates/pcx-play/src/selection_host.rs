//! The local player's unit selection, shared by scripts and player input.
//! Only the local player has a selection; other players read as empty.
//! `SyncSelections` completes at once in this single-machine session.
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

impl ObjectHost {
    pub fn with_selection(mut self) -> Result<Self, Error> {
        for (name, args, result, constant) in NATIVES {
            if self.inner.program.natives().get(name).is_some() {
                Self::contract_in(&self.inner, name, args, result, constant, None, false)?;
            }
        }
        self.selection = Some(Vec::new());
        Ok(self)
    }

    /// Selected units in selection order.
    pub fn selection(&self) -> &[HandleId] {
        self.selection.as_deref().unwrap_or(&[])
    }

    /// The local player's drag-box view of `id`: `None` for a unit that is not live and
    /// shown. Visibility to the local player and screen position are the caller's.
    pub fn box_candidate(
        &self,
        id: HandleId,
    ) -> Result<Option<crate::selection_box::Candidate>, String> {
        let record = self.executed_unit(id).map_err(|e| e.to_string())?;
        if !record.shown || record.combat.as_ref().is_some_and(|c| c.dead) {
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
            "ClearSelection" | "SelectUnit" | "SyncSelections" | "IsUnitSelected"
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
            _ => Err(self.inner.error(name, Failure::Signature)),
        })
    }
}
