//! Explicit current research answers, independent of retained setter inputs.
use super::{Error, Failure, ObjectHost, ParsedMapContext, TechLimitBudget};
use pcx_script::Value;
use std::sync::Arc;
/// One caller-qualified boolean answer; neither a level nor a category rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResearchObservation {
    pub player_number: i32,
    pub techid: i32,
    pub specific_only: bool,
    pub researched: bool,
}
fn key(row: &ResearchObservation) -> (i32, i32, bool) {
    (row.player_number, row.techid, row.specific_only)
}
const GET_RESEARCH: &str = "GetPlayerTechResearched";
const GET_COUNT: &str = "GetPlayerTechCount";
pub(super) struct UpgradeResearch {
    pub(super) rules: crate::upgrade_research::Rules,
    pub(super) active: bool,
    pub(super) count_admitted: bool,
}
impl ObjectHost {
    pub(in super::super) fn upgrade_requirement_met(
        &self,
        owner: pcx_script::HandleId,
        techid: i32,
        required: Option<i32>,
    ) -> Result<bool, Error> {
        let fail = |reason| self.inner.error(GET_RESEARCH, Failure::PlayerState(reason));
        let player = self.player_settings(owner)?;
        let model = self
            .players
            .as_ref()
            .and_then(|players| players.upgrade_research.as_ref())
            .filter(|model| model.active)
            .ok_or_else(|| fail("current upgrade model is not admitted"))?;
        if model.rules.maximum(techid).is_none() {
            return Err(fail("prerequisite is not an admitted upgrade"));
        }
        let super::TechRead::Level(level) = self.stored_tech_level(player.number, techid) else {
            return Err(fail("prerequisite level is unavailable"));
        };
        match required {
            Some(required) if required > 0 => Ok(level >= required),
            Some(_) => Err(fail("prerequisite level must be positive")),
            None => Ok(model.rules.researched(level)),
        }
    }

    /// Admit fresh upgrade counters before any research assignment. Unit/item
    /// technology counts and upgrade gameplay/stat effects remain separate.
    pub fn admit_upgrade_research(
        &mut self,
        context: &Arc<ParsedMapContext>,
        rules: crate::upgrade_research::Rules,
    ) -> Result<(), Error> {
        let fail = |reason| self.inner.error(GET_RESEARCH, reason);
        let players = self
            .players
            .as_ref()
            .ok_or_else(|| fail(Failure::PlayerState("players not admitted")))?;
        if players.current_research.is_some()
            || players.upgrade_research.is_some()
            || players.rows.iter().any(|p| !p.tech_researched.is_empty())
        {
            return Err(fail(Failure::PlayerState(
                "upgrade counters require a fresh research state",
            )));
        }
        if rules.catalog.install != context.sources.catalog.source
            || rules.catalog.edition_version
                != format!(
                    "{}; {}",
                    context.sources.catalog.edition, context.sources.catalog.version
                )
        {
            return Err(fail(Failure::Source));
        }
        let budget = TechLimitBudget {
            max_records: players
                .research
                .as_ref()
                .ok_or_else(|| fail(Failure::PlayerState("research setters not admitted")))?
                .max_records,
        };
        let count_admitted = self.inner.program.natives().get(GET_COUNT).is_some();
        if count_admitted {
            Self::contract(
                &self.inner,
                GET_COUNT,
                &["player", "integer", "boolean"],
                "integer",
                true,
                Some("player"),
            )?;
            if self.inner.program.natives().get(GET_COUNT).unwrap().source
                != self
                    .inner
                    .program
                    .natives()
                    .get("Player")
                    .ok_or_else(|| fail(Failure::Signature))?
                    .source
            {
                return Err(self.inner.error(GET_COUNT, Failure::Source));
            }
        }
        // Reuse the existing complete getter/setter/context/source validation.
        self.admit_current_research(context, Vec::new(), budget)?;
        let players = self.players.as_mut().expect("validated research context");
        players.current_research = None;
        players.upgrade_research = Some(UpgradeResearch {
            rules,
            active: true,
            count_admitted,
        });
        Ok(())
    }
    /// Replace current answers on the exact admitted player context, without
    /// minting players or interpreting stored setter levels. The record bound
    /// counts distinct `(player number, techid, specificonly)` queries.
    ///
    /// Context identity does not authenticate the observations. The caller must
    /// refresh/invalidate external changes. An accepted research setter retires
    /// all answers for that player, since category/equivalence rules are unknown.
    /// Failed admission and failed setters preserve the previous answers.
    pub fn admit_current_research(
        &mut self,
        context: &Arc<ParsedMapContext>,
        mut entries: Vec<ResearchObservation>,
        budget: TechLimitBudget,
    ) -> Result<(), Error> {
        let fail = |reason| self.inner.error(GET_RESEARCH, reason);
        if self
            .player_context()
            .is_none_or(|selected| !Arc::ptr_eq(selected, context))
        {
            return Err(fail(Failure::PlayerState(
                "research map context is not admitted",
            )));
        }
        if entries.len() > budget.max_records {
            return Err(fail(Failure::Budget("current research")));
        }
        std::alloc::Layout::array::<ResearchObservation>(budget.max_records)
            .map_err(|_| fail(Failure::Capacity))?;
        let source = &self
            .inner
            .program
            .natives()
            .get("Player")
            .ok_or_else(|| fail(Failure::Signature))?
            .source;
        for (name, parameters, result) in [
            (
                GET_RESEARCH,
                &["player", "integer", "boolean"][..],
                "boolean",
            ),
            (
                "SetPlayerTechResearched",
                &["player", "integer", "integer"][..],
                "nothing",
            ),
        ] {
            Self::contract(&self.inner, name, parameters, result, true, Some("player"))?;
        }
        for name in [GET_RESEARCH, "SetPlayerTechResearched", "CreateTimer"] {
            if &self
                .inner
                .program
                .natives()
                .get(name)
                .ok_or_else(|| fail(Failure::Signature))?
                .source
                != source
            {
                return Err(self.inner.error(name, Failure::Source));
            }
        }
        let players = self.players.as_ref().expect("checked player context");
        if players.upgrade_research.is_some() {
            return Err(fail(Failure::PlayerState(
                "upgrade model and observations cannot be mixed",
            )));
        }
        for entry in &entries {
            if usize::try_from(entry.player_number)
                .ok()
                .is_none_or(|n| n >= players.slots)
            {
                return Err(fail(Failure::OutsideDomain(entry.player_number)));
            }
        }
        entries.sort_unstable_by_key(key);
        if entries
            .windows(2)
            .any(|pair| key(&pair[0]) == key(&pair[1]))
        {
            return Err(fail(Failure::PlayerState(
                "duplicate current research query",
            )));
        }
        self.players
            .as_mut()
            .expect("checked player context")
            .current_research = Some(entries);
        Ok(())
    }
    /// Retire all current answers without reverting to retained setter levels.
    pub fn invalidate_current_research(&mut self) {
        if let Some(model) = self
            .players
            .as_mut()
            .and_then(|p| p.upgrade_research.as_mut())
        {
            model.active = false;
        }
        if let Some(entries) = self
            .players
            .as_mut()
            .and_then(|p| p.current_research.as_mut())
        {
            entries.clear();
        }
    }
    pub(super) fn current_research_query(
        &self,
        name: &str,
        args: &[Value],
    ) -> Result<Value, Error> {
        let fail = |reason| self.inner.error(name, reason);
        let [player, Value::Int(techid), Value::Bool(specific)] = args else {
            return Err(fail(Failure::PlayerArguments));
        };
        let at = self.player_index(player, name)?;
        let players = self.players.as_ref().expect("checked player identity");
        if let Some(model) = &players.upgrade_research {
            if !model.active {
                return Err(fail(Failure::PlayerState(
                    "upgrade research is invalidated",
                )));
            }
            let level = players.rows[at]
                .research_levels
                .iter()
                .find(|(id, _)| id == techid)
                .map_or(model.rules.initial, |(_, level)| *level);
            return Ok(if name == GET_COUNT {
                Value::Int(level)
            } else {
                Value::Bool(model.rules.researched(level))
            });
        }
        let entries = players
            .current_research
            .as_ref()
            .ok_or_else(|| fail(Failure::PlayerState("current research is not admitted")))?;
        entries
            .binary_search_by_key(&(players.rows[at].number, *techid, *specific), key)
            .map(|index| Value::Bool(entries[index].researched))
            .map_err(|_| fail(Failure::PlayerState("current research query is unset")))
    }
}
