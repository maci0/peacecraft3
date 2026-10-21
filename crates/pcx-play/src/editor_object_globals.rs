//! Bind the editor object globals that a lowered trigger graph references.
//!
//! The World Editor writes object globals for placed map objects into the compiled
//! `war3map.j` alongside its trigger graph:
//!
//! - `gg_unit_<type>_<creation>` for placed units, where `<type>` is the unit's
//!   four-character rawcode and `<creation>` is the record's authored creation
//!   number, zero-padded to four digits;
//! - `gg_rct_<name>` for regions, where `<name>` is the authored region name with
//!   every non-alphanumeric character replaced by `_`;
//! - `gg_dest_<type>_<creation>` for destructable doodad placements, with the
//!   same `type`/`creation` shape as units.
//!
//! The naming rules are not guessed. They were derived by matching every
//! `gg_unit_*`, `gg_rct_*` and `gg_dest_*` declaration in the compiled
//! `war3map.j` of all 44 shipped 1.29b campaign maps against the records in that
//! map's `war3mapUnits.doo`, `war3map.w3r` and `war3map.doo` (see
//! `tests/editor_object_globals_differential.rs`). Every declared unit global
//! matched a record via `gg_unit_<type>_<creation:04>`, and every declared region
//! global matched a record via the sanitization above, with no residual on any
//! map. The shipped corpus is ASCII, so the sanitizer's exact behavior on
//! non-ASCII region names is not established.
//!
//! This module binds exactly the object globals a lowered script names, resolving
//! each against the supplied records. A name that cannot be derived from the
//! supplied records, or that the records identify ambiguously (duplicate
//! `type`/`creation` or colliding sanitized region names), is refused by name.
//! Sounds (`gg_snd_*`) and camera setups (`gg_cam_*`) come from other map files,
//! not the unit/doodad/region records, and are refused as out of scope.

use pcx_map::{Doodads, Regions, Units};
use pcx_script::lex as lex_script;
use std::collections::{BTreeMap, BTreeSet};

/// Unit object-global prefix.
pub const UNIT_PREFIX: &str = "gg_unit_";
/// Region object-global prefix.
pub const RECT_PREFIX: &str = "gg_rct_";
/// Destructable doodad object-global prefix.
pub const DEST_PREFIX: &str = "gg_dest_";
/// Trigger object-global prefix, declared by the lowering itself.
const TRIGGER_PREFIX: &str = "gg_trg_";
/// The object-global prefixes this binding can derive from map records.
const BOUND_PREFIXES: [&str; 3] = [UNIT_PREFIX, RECT_PREFIX, DEST_PREFIX];

/// A name that is an object-global identifier (starts with `gg_` and is not a
/// trigger).

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ObjectGlobalError {
    #[error("object global {name} cannot be bound: no {kind} record derives it")]
    Unknown { kind: &'static str, name: String },
    #[error("object global {name} cannot be bound: {kind} identity is ambiguous ({reason})")]
    Ambiguous {
        kind: &'static str,
        name: String,
        reason: String,
    },
    #[error("object global {name} cannot be bound from the unit/doodad/region records")]
    Unsupported { name: String },
    #[error("object global {name} cannot be initialized: {kind} creation is not generated yet")]
    MissingInitialization { kind: &'static str, name: String },
    #[error("region {name} has nonfinite bounds")]
    InvalidBounds { name: String },
}

fn unknown(kind: &'static str, name: &str) -> ObjectGlobalError {
    ObjectGlobalError::Unknown {
        kind,
        name: name.into(),
    }
}

/// The canonical global name for a placed unit or destructable doodad record.
pub fn object_global_name(prefix: &str, type_id: &[u8; 4], creation: u32) -> String {
    format!("{prefix}{}_{creation:04}", String::from_utf8_lossy(type_id))
}

/// The editor's identifier sanitization: every character outside `[A-Za-z0-9]`
/// becomes `_`, trailing whitespace is dropped. This mirrors the trigger-name
/// rule `pcx_trig::lower` applies, which the editor also applies to region
/// names. Non-ASCII behavior is not established by the ASCII shipped corpus.
pub fn sanitize_identifier(name: &str) -> String {
    name.trim_end()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

/// The canonical global name for a region record.
pub fn region_global_name(name: &[u8]) -> Option<String> {
    let sanitized = sanitize_identifier(&String::from_utf8_lossy(name));
    if sanitized.is_empty() {
        None
    } else {
        Some(format!("{RECT_PREFIX}{sanitized}"))
    }
}

/// One resolved object-global binding.
#[derive(Debug, Clone, PartialEq)]
pub struct Binding {
    /// The declaration keyword the editor uses for this kind (`unit`, `rect`,
    /// `destructable`).
    pub keyword: &'static str,
    /// The canonical global name.
    pub name: String,
}

/// The object-global bindings derived for one lowered script.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Bindings {
    /// Declarations in first-reference order.
    pub declarations: Vec<Binding>,
}

/// The map placement records an object-global binding can derive names from. All
/// three are optional; a binding still verifies the kinds whose records are
/// supplied, and refuses references it cannot resolve.
#[derive(Default, Clone, Copy)]
pub struct MapRecords<'a> {
    /// Placed-unit records (`war3mapUnits.doo`).
    pub units: Option<&'a Units>,
    /// Region records (`war3map.w3r`).
    pub regions: Option<&'a Regions>,
    /// Destructable doodad records (`war3map.doo`).
    pub doodads: Option<&'a Doodads>,
}

#[derive(Default)]
struct Records {
    units: BTreeMap<(String, u32), usize>,
    regions: BTreeMap<String, usize>,
    doodads: BTreeMap<(String, u32), usize>,
}

impl Records {
    fn new(source: &MapRecords<'_>) -> Self {
        let mut records = Records::default();
        if let Some(units) = source.units {
            for u in &units.instances {
                let key = (
                    String::from_utf8_lossy(&u.type_id).into_owned(),
                    u.creation_number,
                );
                *records.units.entry(key).or_insert(0) += 1;
            }
        }
        if let Some(regions) = source.regions {
            for r in &regions.regions {
                let sanitized = sanitize_identifier(&String::from_utf8_lossy(&r.name));
                if !sanitized.is_empty() {
                    *records.regions.entry(sanitized).or_insert(0) += 1;
                }
            }
        }
        if let Some(doodads) = source.doodads {
            for d in doodads.instances() {
                let key = (
                    String::from_utf8_lossy(&d.type_id).into_owned(),
                    d.creation_number,
                );
                *records.doodads.entry(key).or_insert(0) += 1;
            }
        }
        records
    }

    fn unique_identity(
        &self,
        counts: &BTreeMap<(String, u32), usize>,
        key: &(String, u32),
    ) -> bool {
        counts.get(key).copied().unwrap_or(0) == 1
    }
}

/// Resolve and bind the object-global identifiers `script` references against the
/// supplied map records. Identifiers are read from the JASS lexer, so string
/// literals and comments cannot be mistaken for references.
pub fn bind(script: &str, records: &MapRecords<'_>) -> Result<Bindings, ObjectGlobalError> {
    let records = Records::new(records);
    let mut bindings = Bindings::default();
    let mut seen = BTreeSet::new();
    let lexed = lex_script(script).map_err(|e| ObjectGlobalError::Unsupported {
        name: format!("lowered script does not lex: {e}"),
    })?;
    for token in &lexed.tokens {
        let Some(ident) = token.kind.ident() else {
            continue;
        };
        let name = ident.as_str();
        if !name.starts_with("gg_") {
            continue;
        }
        if name.starts_with(TRIGGER_PREFIX) {
            continue;
        }
        if !BOUND_PREFIXES.iter().any(|p| name.starts_with(p)) {
            return Err(ObjectGlobalError::Unsupported {
                name: name.to_string(),
            });
        }
        if !seen.insert(name.to_string()) {
            continue;
        }
        let binding = resolve(&records, name)?;
        bindings.declarations.push(binding);
    }
    Ok(bindings)
}

fn resolve(records: &Records, name: &str) -> Result<Binding, ObjectGlobalError> {
    if let Some(rest) = name.strip_prefix(UNIT_PREFIX) {
        let (type_id, creation) = split_type_creation(rest).ok_or_else(|| unknown("unit", name))?;
        let key = (type_id, creation);
        if records.units.contains_key(&key) {
            if !records.unique_identity(&records.units, &key) {
                return Err(ObjectGlobalError::Ambiguous {
                    kind: "unit",
                    name: name.into(),
                    reason: "duplicate type/creation records".into(),
                });
            }
            Ok(Binding {
                keyword: "unit",
                name: name.into(),
            })
        } else {
            Err(unknown("unit", name))
        }
    } else if let Some(rest) = name.strip_prefix(DEST_PREFIX) {
        let (type_id, creation) =
            split_type_creation(rest).ok_or_else(|| unknown("destructable doodad", name))?;
        let key = (type_id, creation);
        if records.doodads.contains_key(&key) {
            if !records.unique_identity(&records.doodads, &key) {
                return Err(ObjectGlobalError::Ambiguous {
                    kind: "destructable doodad",
                    name: name.into(),
                    reason: "duplicate type/creation records".into(),
                });
            }
            Ok(Binding {
                keyword: "destructable",
                name: name.into(),
            })
        } else {
            Err(unknown("destructable doodad", name))
        }
    } else if let Some(region) = name.strip_prefix(RECT_PREFIX) {
        let count = records.regions.get(region).copied().unwrap_or(0);
        if count == 0 {
            return Err(unknown("region", name));
        }
        if count > 1 {
            return Err(ObjectGlobalError::Ambiguous {
                kind: "region",
                name: name.into(),
                reason: format!("more than one record sanitizes to {region:?}"),
            });
        }
        Ok(Binding {
            keyword: "rect",
            name: name.into(),
        })
    } else {
        Err(ObjectGlobalError::Unsupported { name: name.into() })
    }
}

/// The call that binds a region global at runtime from its record bounds.
fn region_assignment(binding: &Binding, bounds: [f32; 4]) -> Option<String> {
    if binding.keyword == "rect" {
        Some(format!(
            "    set {} = Rect({}, {}, {}, {})\n",
            binding.name,
            fmt_real(bounds[0]),
            fmt_real(bounds[1]),
            fmt_real(bounds[2]),
            fmt_real(bounds[3]),
        ))
    } else {
        None
    }
}

fn fmt_real(value: f32) -> String {
    let text = value.to_string();
    if text.contains('.') {
        text
    } else {
        format!("{text}.0")
    }
}

/// Split a `gg_<type>_<creation>` suffix into its type id and creation number.
fn split_type_creation(suffix: &str) -> Option<(String, u32)> {
    let (type_id, creation) = suffix.rsplit_once('_')?;
    if type_id.is_empty() || !creation.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let creation = creation.parse::<u32>().ok()?;
    Some((type_id.to_string(), creation))
}

/// Emit the JASS declaration block for a set of bindings.
pub fn render_declarations(bindings: &Bindings) -> String {
    let mut out = String::new();
    for binding in &bindings.declarations {
        out.push_str(&format!("    {} {}\n", binding.keyword, binding.name));
    }
    out
}

/// Emit the `set gg_rct_X = Rect(...)` lines for region bindings, matching the
/// authored `war3map.w3r` bounds. These run in `InitGlobals` so the region
/// globals hold real rects, not null handles.
pub fn render_region_init(
    bindings: &Bindings,
    regions: &Regions,
) -> Result<String, ObjectGlobalError> {
    let mut source = BTreeMap::new();
    for record in &regions.regions {
        if let Some(name) = region_global_name(&record.name)
            && source.insert(name.clone(), record).is_some()
        {
            return Err(ObjectGlobalError::Ambiguous {
                kind: "region",
                name,
                reason: "duplicate sanitized name".into(),
            });
        }
    }
    let mut out = String::new();
    for binding in &bindings.declarations {
        if binding.keyword != "rect" {
            continue;
        }
        let record = source
            .get(&binding.name)
            .ok_or_else(|| unknown("region", &binding.name))?;
        let bounds = match record.bounds {
            pcx_map::RegionBounds::Float(values) => values,
            pcx_map::RegionBounds::Integer(values) => values.map(|v| v as f32),
        };
        if bounds.iter().any(|value| !value.is_finite()) {
            return Err(ObjectGlobalError::InvalidBounds {
                name: binding.name.clone(),
            });
        }
        if let Some(line) = region_assignment(binding, bounds) {
            out.push_str(&line);
        }
    }
    Ok(out)
}

/// Merge object-global declarations into the lowered script's `globals` block.
/// If the lowered script has no `globals` block, one is prepended.
pub fn merge_declarations(script: &str, declarations: &str) -> String {
    if declarations.is_empty() {
        return script.to_string();
    }
    if let Some(at) = script.find("globals\n") {
        let after = at + "globals\n".len();
        let mut out = String::with_capacity(script.len() + declarations.len());
        out.push_str(&script[..after]);
        out.push_str(declarations);
        out.push_str(&script[after..]);
        out
    } else {
        format!("globals\n{declarations}endglobals\n\n{script}")
    }
}

/// Inject `set gg_rct_X = Rect(...)` lines into the lowered script's
/// `InitGlobals` body before user-global assignments.
pub fn merge_region_init(script: &str, init: &str) -> Result<String, ObjectGlobalError> {
    if init.is_empty() {
        return Ok(script.to_string());
    }
    const MARKER: &str = "function InitGlobals takes nothing returns nothing\n";
    let missing = || ObjectGlobalError::MissingInitialization {
        kind: "region",
        name: "InitGlobals".into(),
    };
    let at = script.find(MARKER).ok_or_else(missing)?;
    let body_start = at + MARKER.len();
    let rest = &script[body_start..];
    if !rest.starts_with("endfunction\n") && !rest.contains("\nendfunction\n") {
        return Err(missing());
    }
    let mut out = String::with_capacity(script.len() + init.len());
    out.push_str(&script[..body_start]);
    out.push_str(init);
    out.push_str(rest);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit_fixture(type_id: &[u8; 4], creation: u32) -> Vec<u8> {
        let mut data = b"W3do".to_vec();
        for w in [8u32, 11, 1] {
            data.extend(w.to_le_bytes());
        }
        data.extend(type_id);
        data.extend(0u32.to_le_bytes());
        for v in [1f32, 2., 3.] {
            data.extend(v.to_bits().to_le_bytes());
        }
        data.extend(0f32.to_bits().to_le_bytes());
        for v in [1f32, 1., 1.] {
            data.extend(v.to_bits().to_le_bytes());
        }
        data.push(2);
        data.extend(0u16.to_le_bytes());
        for w in [0u32, 0, 0] {
            data.extend(w.to_le_bytes());
        }
        data.extend(u32::MAX.to_le_bytes());
        data.extend(0u32.to_le_bytes());
        data.extend(0u32.to_le_bytes());
        data.extend(0f32.to_bits().to_le_bytes());
        data.extend(0u32.to_le_bytes());
        for _ in 0..3 {
            data.extend(0u32.to_le_bytes());
        }
        data.extend(0u32.to_le_bytes());
        data.extend(0u32.to_le_bytes());
        data.extend(u32::MAX.to_le_bytes());
        for _ in 0..2 {
            data.extend(0u32.to_le_bytes());
        }
        data.extend(creation.to_le_bytes());
        data
    }

    fn region_fixture(name: &str) -> Vec<u8> {
        let mut data = Vec::new();
        for w in [4u32, 1] {
            data.extend(w.to_le_bytes());
        }
        for v in [10f32, 20., 30., 40.] {
            data.extend(v.to_bits().to_le_bytes());
        }
        data.extend(name.as_bytes());
        data.push(0);
        data.extend(0u32.to_le_bytes());
        data.extend(b"RAIN");
        data.extend(b"ambience\0");
        data
    }

    #[test]
    fn unit_reference_resolves_against_a_record() {
        let units = Units::parse(&unit_fixture(b"h001", 1)).unwrap();
        let record = MapRecords {
            units: Some(&units),
            ..Default::default()
        };
        let bindings = bind("call UseUnit(gg_unit_h001_0001)", &record).unwrap();
        assert_eq!(bindings.declarations.len(), 1);
        assert_eq!(bindings.declarations[0].keyword, "unit");
        assert_eq!(bindings.declarations[0].name, "gg_unit_h001_0001");
    }

    #[test]
    fn unknown_unit_reference_is_refused_by_name() {
        let units = Units::parse(&unit_fixture(b"h001", 1)).unwrap();
        let record = MapRecords {
            units: Some(&units),
            ..Default::default()
        };
        let error = bind("call UseUnit(gg_unit_h001_0002)", &record).unwrap_err();
        assert!(error.to_string().contains("gg_unit_h001_0002"));
    }

    #[test]
    fn duplicate_unit_identity_is_refused_as_ambiguous() {
        let mut bytes = unit_fixture(b"h001", 1);
        // Append a second identical record (count = 2). Header: magic(4) +
        // version(4) + subversion(4) + count(4).
        let header = 16;
        bytes[12..16].copy_from_slice(&2u32.to_le_bytes());
        bytes.extend_from_slice(&unit_fixture(b"h001", 1)[header..]);
        let units = Units::parse(&bytes).unwrap();
        let record = MapRecords {
            units: Some(&units),
            ..Default::default()
        };
        let error = bind("call UseUnit(gg_unit_h001_0001)", &record).unwrap_err();
        assert!(error.to_string().contains("ambiguous"), "{error}");
    }

    #[test]
    fn region_reference_resolves_from_the_region_record() {
        let regions = Regions::parse(&region_fixture("Start Area")).unwrap();
        let record = MapRecords {
            regions: Some(&regions),
            ..Default::default()
        };
        let bindings = bind("call Teleport(gg_rct_Start_Area)", &record).unwrap();
        assert_eq!(bindings.declarations[0].name, "gg_rct_Start_Area");
        assert_eq!(bindings.declarations[0].keyword, "rect");
    }

    #[test]
    fn string_literals_and_comments_are_not_taken_as_references() {
        let script = "// gg_unit_h001_0001 in a comment\ncall Display(\"gg_unit_h001_0001\")\ncall UseUnit(gg_unit_h001_0001)\n";
        let units = Units::parse(&unit_fixture(b"h001", 1)).unwrap();
        let record = MapRecords {
            units: Some(&units),
            ..Default::default()
        };
        let bindings = bind(script, &record).unwrap();
        assert_eq!(bindings.declarations.len(), 1);
        assert_eq!(bindings.declarations[0].name, "gg_unit_h001_0001");
    }

    #[test]
    fn sound_and_camera_globals_are_refused_as_out_of_scope() {
        let error = bind("call PlaySound(gg_snd_Wind)", &MapRecords::default()).unwrap_err();
        assert!(error.to_string().contains("gg_snd_Wind"));
        let error = bind("call SetCamera(gg_cam_Close)", &MapRecords::default()).unwrap_err();
        assert!(error.to_string().contains("gg_cam_Close"));
    }

    #[test]
    fn region_init_emits_the_authored_bounds() {
        let regions = Regions::parse(&region_fixture("Area")).unwrap();
        let bindings = Bindings {
            declarations: vec![Binding {
                keyword: "rect",
                name: "gg_rct_Area".into(),
            }],
        };
        let init = render_region_init(&bindings, &regions).unwrap();
        assert!(
            init.contains("set gg_rct_Area = Rect(10.0, 20.0, 30.0, 40.0)\n"),
            "{init}"
        );
    }

    #[test]
    fn merge_inserts_declarations_into_the_existing_globals_block() {
        let script = "globals\n    trigger gg_trg_X\nendglobals\n\nfunction main takes nothing returns nothing\nendfunction\n";
        let merged = merge_declarations(script, "    unit gg_unit_h001_0001\n");
        assert!(
            merged
                .contains("globals\n    unit gg_unit_h001_0001\n    trigger gg_trg_X\nendglobals")
        );
    }

    #[test]
    fn merge_prepends_a_globals_block_when_absent() {
        let script = "function main takes nothing returns nothing\nendfunction\n";
        let merged = merge_declarations(script, "    unit gg_unit_h001_0001\n");
        assert!(
            merged.starts_with("globals\n    unit gg_unit_h001_0001\nendglobals\n\nfunction main")
        );
    }

    #[test]
    fn region_init_is_injected_before_initglobals_endfunction() {
        let script = "function InitGlobals takes nothing returns nothing\n    set udg_Count = 1\nendfunction\n";
        let merged = merge_region_init(
            script,
            "    set gg_rct_Area = Rect(10.0, 20.0, 30.0, 40.0)\n",
        )
        .unwrap();
        assert!(
            merged.contains(
                "set gg_rct_Area = Rect(10.0, 20.0, 30.0, 40.0)\n    set udg_Count = 1\nendfunction"
            ),
            "{merged}"
        );
    }

    #[test]
    fn region_initialization_handles_an_empty_body_and_refuses_missing_body() {
        let init = "    set gg_rct_Area = Rect(1.0, 2.0, 3.0, 4.0)\n";
        let script = "function InitGlobals takes nothing returns nothing\nendfunction\n";
        assert!(merge_region_init(script, init).unwrap().contains(init));
        assert!(merge_region_init("// no initializer\n", init).is_err());
    }

    #[test]
    fn nonfinite_region_bounds_are_refused_before_emitting_jass() {
        let mut regions = Regions::parse(&region_fixture("Area")).unwrap();
        regions.regions[0].bounds = pcx_map::RegionBounds::Float([f32::NAN, 0.0, 1.0, 1.0]);
        let bindings = bind(
            "call Use(gg_rct_Area)",
            &MapRecords {
                regions: Some(&regions),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(matches!(
            render_region_init(&bindings, &regions),
            Err(ObjectGlobalError::InvalidBounds { .. })
        ));
    }
}
