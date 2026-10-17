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
impl ObjectHost {
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
        if let Some(entries) = self
            .players
            .as_mut()
            .and_then(|p| p.current_research.as_mut())
        {
            entries.clear();
        }
    }
    pub(super) fn current_research_query(&self, args: &[Value]) -> Result<Value, Error> {
        let fail = |reason| self.inner.error(GET_RESEARCH, reason);
        let [player, Value::Int(techid), Value::Bool(specific)] = args else {
            return Err(fail(Failure::PlayerArguments));
        };
        let at = self.player_index(player, GET_RESEARCH)?;
        let players = self.players.as_ref().expect("checked player identity");
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
