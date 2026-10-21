//! Per-player overrides for the declared ability-availability native.
//!
//! `SetPlayerAbilityAvailable(player, abilid, avail)` records whether a rawcode
//! ability is available to a player. All abilities are available by default; an
//! explicit `avail=false` disables and `avail=true` re-enables. State is kept per
//! player identity (the player slot number resolved from a validated player
//! handle), distinct from unit-owned ability state and from the player tech/research
//! records in the player host. Rawcodes are retained as script integers.
use super::{Entry, Error, Failure, ObjectHost};
use pcx_script::{HandleId, Value};

const SET_PLAYER_ABILITY_AVAILABLE: &str = "SetPlayerAbilityAvailable";
/// `takes player whichPlayer, integer abilid, boolean avail returns nothing`.
const ABILITY_CONTRACT: (&str, &[&str], &str, bool) = (
    "SetPlayerAbilityAvailable",
    &["player", "integer", "boolean"],
    "nothing",
    false,
);

#[derive(Debug)]
pub(super) struct PlayerAbilityState {
    rows: std::collections::BTreeMap<(i32, i32), bool>,
    max_records: usize,
}

impl ObjectHost {
    /// Admit the declared `SetPlayerAbilityAvailable` native on the existing player
    /// identities. `max_records` bounds distinct stored records. Replacing a `(player,
    /// abilid)` override does not allocate a new record. Argument-shape refusals
    /// are this recreation's contract checks, not a claim about native exceptional
    /// inputs.
    pub fn with_player_abilities(mut self, max_records: usize) -> Result<Self, Error> {
        let fail = |reason| self.inner.error(SET_PLAYER_ABILITY_AVAILABLE, reason);
        if self.player_abilities.is_some() {
            return Err(fail(Failure::PlayerState("already admitted")));
        }
        ObjectHost::contract(
            &self.inner,
            ABILITY_CONTRACT.0,
            ABILITY_CONTRACT.1,
            ABILITY_CONTRACT.2,
            ABILITY_CONTRACT.3,
            Some("player"),
        )?;
        self.player_abilities = Some(PlayerAbilityState {
            rows: Default::default(),
            max_records,
        });
        Ok(self)
    }

    pub(super) fn player_ability_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if name != SET_PLAYER_ABILITY_AVAILABLE {
            return None;
        }
        self.player_abilities.as_ref()?;
        Some(self.set_player_ability_available(args))
    }

    fn set_player_ability_available(&mut self, args: &[Value]) -> Result<Value, Error> {
        let fail = |reason| self.inner.error(SET_PLAYER_ABILITY_AVAILABLE, reason);
        let [
            Value::Handle(player),
            Value::Int(abilid),
            Value::Bool(available),
        ] = args
        else {
            return Err(fail(Failure::PlayerArguments));
        };
        let number = self.player_number(*player, SET_PLAYER_ABILITY_AVAILABLE)?;
        let players = self
            .player_abilities
            .as_mut()
            .ok_or_else(|| fail(Failure::PlayerState("not admitted")))?;
        let key = (number, *abilid);
        if !players.rows.contains_key(&key) && players.rows.len() >= players.max_records {
            return Err(fail(Failure::Budget("player abilities")));
        }
        players.rows.insert(key, *available);
        Ok(Value::Null)
    }

    /// Whether the given player currently has the rawcode ability available.
    /// Unrecorded abilities are available; a recorded override returns its value.
    pub fn player_ability_available(&self, player: HandleId, abilid: i32) -> Result<bool, Error> {
        let fail = |reason| self.inner.error(SET_PLAYER_ABILITY_AVAILABLE, reason);
        let number = self.player_number(player, SET_PLAYER_ABILITY_AVAILABLE)?;
        let players = self
            .player_abilities
            .as_ref()
            .ok_or_else(|| fail(Failure::PlayerState("not admitted")))?;
        Ok(players.rows.get(&(number, abilid)).copied().unwrap_or(true))
    }

    /// Current owner-scoped availability; no override admission preserves the existing true policy.
    pub fn unit_ability_available(&self, unit: HandleId, ability: [u8; 4]) -> Result<bool, Error> {
        if self.player_abilities.is_none() {
            return Ok(true);
        }
        let owner = self.executed_unit(unit)?.owner;
        self.player_ability_available(owner, i32::from_be_bytes(ability))
    }

    fn player_number(&self, player: HandleId, name: &str) -> Result<i32, Error> {
        let fail = |reason| self.inner.error(name, reason);
        match self
            .inner
            .entry(player)
            .map_err(|e| self.inner.error(name, e.failure))?
        {
            Entry::Player { number } => Ok(*number),
            _ => Err(fail(Failure::WrongFamily)),
        }
    }
}
