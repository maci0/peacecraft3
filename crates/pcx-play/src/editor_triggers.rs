//! Explicit editor-trigger source: a map's `war3map.wtg` (with its `war3map.wct`)
//! lowered to JASS and compiled as the map script.
//!
//! This is a caller-selected source. Admission imports the compiled `war3map.j`
//! (`map_scripts_import::import_map_scripts`) and never reads the trigger graph;
//! the two sources are not merged.

use crate::scripts::{AssetIdentity, MapScripts, ScriptAsset};
use pcx_map::{Map, MapMemberError};
use pcx_mpq::checked::ReadBudget;
use pcx_trig::catalog::Catalog;
use pcx_trig::lower::{LowerError, lower};
use pcx_trig::{Wct, Wtg};

/// Label of the lowered source, retained in diagnostics and provenance.
pub const LOWERED_LABEL: &str = "lowered war3map.wtg";

#[derive(Debug, thiserror::Error)]
pub enum EditorMapError {
    #[error(transparent)]
    Member(#[from] MapMemberError),
    #[error("map has no {0}")]
    MissingMember(&'static str),
    #[error(transparent)]
    Triggers(#[from] EditorTriggerError),
}

#[derive(Debug, thiserror::Error)]
pub enum EditorTriggerError {
    #[error("war3map.wtg: {0}")]
    Wtg(String),
    #[error("war3map.wct: {0}")]
    Wct(String),
    #[error("lowering: {0}")]
    Lower(#[from] LowerError),
    #[error("compiling the lowered triggers: {0}")]
    Compile(String),
}

/// Parses, lowers and compiles one map's editor triggers. `catalog` is the
/// TriggerData of the same edition and version as `identity`.
pub fn compile_editor_triggers(
    identity: AssetIdentity,
    common: ScriptAsset<'_>,
    blizzard: ScriptAsset<'_>,
    wtg: &[u8],
    wct: &[u8],
    catalog: &Catalog,
) -> Result<MapScripts, EditorTriggerError> {
    let arity = catalog
        .arity()
        .map_err(|e| EditorTriggerError::Wtg(e.to_string()))?;
    let wtg = Wtg::parse(wtg, &arity).map_err(|e| EditorTriggerError::Wtg(e.to_string()))?;
    let wct = Wct::parse(wct).map_err(|e| EditorTriggerError::Wct(e.to_string()))?;
    let jass = lower(&wtg, &wct, catalog)?;
    MapScripts::compile(
        identity,
        common,
        blizzard,
        ScriptAsset {
            label: LOWERED_LABEL,
            bytes: jass.as_bytes(),
        },
        None,
    )
    .map_err(EditorTriggerError::Compile)
}

/// Compiles the editor triggers of an admitted map archive. Reads only
/// `war3map.wtg` and `war3map.wct`, through the checked member reader with the
/// caller's budget; it never reads `war3map.j`.
pub fn compile_editor_map_scripts(
    map: &Map<'_>,
    source_label: &str,
    identity: AssetIdentity,
    common: ScriptAsset<'_>,
    blizzard: ScriptAsset<'_>,
    catalog: &Catalog,
    budget: ReadBudget,
) -> Result<MapScripts, EditorMapError> {
    let member = |name: &'static str| {
        map.extract_checked(source_label, name, budget)?
            .ok_or(EditorMapError::MissingMember(name))
    };
    let wtg = member("war3map.wtg")?;
    let wct = member("war3map.wct")?;
    Ok(compile_editor_triggers(
        identity, common, blizzard, &wtg, &wct, catalog,
    )?)
}

#[cfg(test)]
#[path = "../tests/support/editor_fixture.rs"]
pub(crate) mod editor_fixture;
