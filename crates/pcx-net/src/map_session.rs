//! Refusal of maps that cannot run as a playable session yet.
//!
//! A playable map session needs the production startup: `war3map.j` and creation
//! triggers, the authored random tables under the selected RNG policy, the player
//! roster, and source unit movement. None of that is wired into a native host.
//! Placement records alone are not a playable map, so this module reports why a map
//! is unsupported and never builds a substitute world.

use pcx_map::{Map, RandomUnit};
use pcx_mpq::checked::ReadBudget;

const SCRIPT_MEMBER: &str = "war3map.j";
const MAP_READ_BUDGET: ReadBudget = ReadBudget {
    max_archive_bytes: 64 << 20,
    max_archive_table_bytes: 8 << 20,
    max_raw_packed_bytes: 16 << 20,
    max_decoded_bytes: 16 << 20,
};

#[derive(Debug, thiserror::Error)]
pub enum AssessError {
    #[error("map: {0}")]
    Map(String),
    #[error("units: {0}")]
    Units(String),
}

/// A reason the map cannot run. Listed in the order the refusal prints them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    /// Random-selection placements: the unit is chosen at creation from authored tables.
    RandomSelection { count: usize },
    /// `war3map.j` creates units and runs triggers; this host does not execute it.
    MapScript,
    /// Owners are raw control words. Player slots come from the startup roster.
    OwnerRoster,
    /// Movement would use the harness rule, not the source unit movement.
    HarnessMovement,
}

impl std::fmt::Display for Reason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Reason::RandomSelection { count } => write!(
                f,
                "{count} random-selection placements; authored random tables and the selected RNG policy are not decoded"
            ),
            Reason::MapScript => write!(
                f,
                "{SCRIPT_MEMBER} is present; map script initialization and creation triggers are not run"
            ),
            Reason::OwnerRoster => write!(
                f,
                "owner-to-player mapping is not sourced from the startup roster"
            ),
            Reason::HarnessMovement => write!(
                f,
                "unit movement would be the harness rule, not source unit movement"
            ),
        }
    }
}

/// Why `map_bytes` cannot run as a session. The list is never empty for a map this host
/// supports today, because the roster and source movement are always missing.
pub fn assess(map_bytes: &[u8], label: &str) -> Result<Vec<Reason>, AssessError> {
    let map = Map::parse(map_bytes).map_err(|error| AssessError::Map(error.to_string()))?;
    let units = map
        .units()
        .map_err(|error| AssessError::Units(error.to_string()))?;
    let count = units
        .instances
        .iter()
        .filter(|placement| placement.random != RandomUnit::None)
        .count();
    let script = map
        .extract_checked(label, SCRIPT_MEMBER, MAP_READ_BUDGET)
        .map_err(|error| AssessError::Map(error.to_string()))?
        .is_some();

    let mut reasons = Vec::new();
    if count > 0 {
        reasons.push(Reason::RandomSelection { count });
    }
    if script {
        reasons.push(Reason::MapScript);
    }
    reasons.push(Reason::OwnerRoster);
    reasons.push(Reason::HarnessMovement);
    Ok(reasons)
}
