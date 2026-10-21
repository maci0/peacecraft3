//! Explicit editor-trigger source: a map's `war3map.wtg` (with its `war3map.wct`)
//! lowered to JASS and compiled as the map script.
//!
//! This is a caller-selected source. Admission imports the compiled `war3map.j`
//! (`map_scripts_import::import_map_scripts`) and never reads the trigger graph;
//! the two sources are not merged.

use crate::editor_object_globals::{self, MapRecords, ObjectGlobalError};
use crate::scripts::{AssetIdentity, MapScripts, ScriptAsset};
use pcx_map::{Map, MapMemberError, Regions};
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
    #[error("war3map.w3r: {0}")]
    Regions(#[from] pcx_map::RegionError),
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
    #[error("binding object globals: {0}")]
    ObjectGlobals(#[from] ObjectGlobalError),
    #[error("compiling the lowered triggers: {0}")]
    Compile(String),
}

/// Parses, lowers and compiles one map's editor triggers. `catalog` is the
/// TriggerData of the same edition and version as `identity`. This is the
/// editor-source path without object-global binding: an object-global reference
/// (`gg_unit_*`, `gg_rct_*`, `gg_dest_*`) that the map records cannot bind is
/// refused by name at compile time.
pub fn compile_editor_triggers(
    identity: AssetIdentity,
    common: ScriptAsset<'_>,
    blizzard: ScriptAsset<'_>,
    wtg: &[u8],
    wct: &[u8],
    catalog: &Catalog,
) -> Result<MapScripts, EditorTriggerError> {
    compile_editor_triggers_with_objects(
        identity,
        common,
        blizzard,
        wtg,
        wct,
        catalog,
        MapRecords::default(),
    )
}

/// Parses, lowers and compiles one map's editor triggers, binding the object
/// globals the graph references from the map's placement records. `catalog` is
/// the TriggerData of the same edition and version as `identity`. A reference
/// the supplied records cannot derive is refused by name.
///
/// Only regions are bound with runtime assignment: the generated script declares
/// each `gg_rct_*` global and initializes it with `Rect(...)` from the authored
/// `war3map.w3r` bounds in `InitGlobals`. Unit and destructable object globals
/// require their creation/assignment to be generated; until that initializer is
/// implemented, a graph that references one is refused rather than left as a
/// null handle.
pub fn compile_editor_triggers_with_objects(
    identity: AssetIdentity,
    common: ScriptAsset<'_>,
    blizzard: ScriptAsset<'_>,
    wtg: &[u8],
    wct: &[u8],
    catalog: &Catalog,
    records: MapRecords<'_>,
) -> Result<MapScripts, EditorTriggerError> {
    let arity = catalog
        .arity()
        .map_err(|e| EditorTriggerError::Wtg(e.to_string()))?;
    let wtg = Wtg::parse(wtg, &arity).map_err(|e| EditorTriggerError::Wtg(e.to_string()))?;
    let wct = Wct::parse(wct).map_err(|e| EditorTriggerError::Wct(e.to_string()))?;
    let jass = lower(&wtg, &wct, catalog)?;
    let bindings = editor_object_globals::bind(&jass, &records)?;
    for binding in &bindings.declarations {
        if binding.keyword != "rect" {
            return Err(EditorTriggerError::ObjectGlobals(
                editor_object_globals::ObjectGlobalError::MissingInitialization {
                    kind: binding.keyword,
                    name: binding.name.clone(),
                },
            ));
        }
    }
    let declarations = editor_object_globals::render_declarations(&bindings);
    let jass = editor_object_globals::merge_declarations(&jass, &declarations);
    let region_init = match records.regions {
        Some(regions) => editor_object_globals::render_region_init(&bindings, regions)?,
        None => String::new(),
    };
    let jass = editor_object_globals::merge_region_init(&jass, &region_init)?;
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
/// `war3map.wtg`, `war3map.wct` and optional `war3map.w3r`, through the checked member reader with the
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
    let regions = map
        .extract_checked(source_label, "war3map.w3r", budget)?
        .map(|bytes| Regions::parse(&bytes))
        .transpose()?;
    Ok(compile_editor_triggers_with_objects(
        identity,
        common,
        blizzard,
        &wtg,
        &wct,
        catalog,
        MapRecords {
            regions: regions.as_ref(),
            ..Default::default()
        },
    )?)
}

#[cfg(test)]
#[path = "../tests/support/editor_fixture.rs"]
pub(crate) mod editor_fixture;
