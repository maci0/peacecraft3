//! Original assets stay local. This sweep proves shipped-byte table derivation,
//! not native gameplay behavior or World Editor display parity.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pcx_objects::editor::SlkId;
use pcx_objects::editor::defaults::{Behavior, DefaultTables, Family, Refusal, Unresolved};

#[test]
#[ignore = "requires both local installs; set PEACECRAFT3_LEGACY_DIR and optionally PEACECRAFT3_INSTALL"]
fn sweep_default_tables_both_installs() {
    let compatibility = std::env::var_os("PEACECRAFT3_INSTALL")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../wc3/game/app"));
    let legacy =
        PathBuf::from(std::env::var_os("PEACECRAFT3_LEGACY_DIR").expect(
            "set PEACECRAFT3_LEGACY_DIR; a missing original install is not a successful skip",
        ));
    for install in [compatibility, legacy] {
        assert!(
            install.is_dir(),
            "original install required: {}",
            install.display()
        );
        let tables = DefaultTables::open(
            &install,
            "edition/version unverified; archive members fingerprinted independently",
        )
        .unwrap_or_else(|error| panic!("{}: {error}", install.display()));
        println!(
            "install={} identity={}",
            tables.identity().install,
            tables.identity().edition_version
        );
        let slk_count = SlkId::ALL
            .iter()
            .filter(|&&id| tables.editor().table(id).is_some())
            .count();
        let profile_count = tables.assets().len() - slk_count;
        let mut total_rows = 0;
        let mut total_cells = 0;
        let mut total_resolved = 0;
        let mut total_refused = 0;
        let mut group_refusals = 0;
        for &family in Family::ALL {
            let mut rows = 0;
            let mut cells = 0;
            let mut profiles = 0;
            let mut resolved = 0;
            let mut refusals: BTreeMap<&str, usize> = BTreeMap::new();
            let mut groups: BTreeMap<String, usize> = BTreeMap::new();
            for rawcode in tables.rawcodes(family) {
                rows += 1;
                let object = tables.object(family, rawcode).unwrap();
                for cell in object.cells() {
                    cells += 1;
                    let pcx_objects::editor::defaults::Location::Slk { row, column } =
                        &cell.provenance.location
                    else {
                        unreachable!()
                    };
                    let id = family
                        .tables()
                        .iter()
                        .find(|id| id.path() == cell.provenance.asset.member)
                        .unwrap();
                    assert_eq!(
                        Some(cell.text),
                        tables.editor().table(*id).unwrap().by_name(*row, column)
                    );
                    assert_eq!(cell.provenance.identity, tables.identity());
                    assert!(!cell.provenance.asset.archive.is_empty());
                }
                for key in object.profile_keys() {
                    profiles += object.profile(key).len();
                }
                if family == Family::Unit {
                    match object.cell(SlkId::UnitData, "race") {
                        Ok(race) => *groups.entry(race.text.to_string()).or_default() += 1,
                        Err(refusal) => {
                            group_refusals += 1;
                            println!("group_refusal={refusal}");
                        }
                    }
                }
                for def in tables
                    .editor()
                    .fields()
                    .iter()
                    .filter(|def| def.from == family.metadata())
                {
                    match object.field(def.fourcc) {
                        Ok(field) => {
                            resolved += 1;
                            assert_eq!(field.definition.fourcc, def.fourcc);
                            assert_eq!(
                                field.metadata.attribute("ID").unwrap().text.as_bytes(),
                                def.fourcc
                            );
                        }
                        Err(refusal) => {
                            let kind = match refusal {
                                Refusal::MissingCell { .. } => "missing-cell",
                                Refusal::MalformedValue { .. } => "malformed-value",
                                Refusal::Metadata { .. } => "metadata",
                                Refusal::Mapping { .. } => "unverified-mapping",
                                Refusal::AmbiguousProfile { .. } => "conflicting-profile",
                                Refusal::ProfileIndex { .. } => "missing-profile-slot",
                                Refusal::NativeEvidence { .. } => "native-evidence",
                            };
                            *refusals.entry(kind).or_default() += 1;
                        }
                    }
                }
                if family == Family::Ability {
                    object
                        .level_count()
                        .unwrap()
                        .typed(pcx_objects::editor::FieldKind::Int)
                        .unwrap();
                    for (level, columns) in object.level_columns() {
                        for column in columns {
                            let stem = column.trim_end_matches(|ch: char| ch.is_ascii_digit());
                            // Sparse columns remain explicit; padding is never inherited.
                            if let Ok(cell) = object.level_cell(stem, level) {
                                assert_eq!(cell, object.cell(SlkId::AbilityData, &column).unwrap());
                            }
                        }
                    }
                }
            }
            println!(
                "family={family:?} rows={rows} slk_cells={cells} profile_assignments={profiles} resolved_fields={resolved} refusals={refusals:?}"
            );
            if !groups.is_empty() {
                println!("authored_race_groups={groups:?}");
            }
            assert!(rows > 0, "original fixture lacks {family:?}");
            total_rows += rows;
            total_cells += cells;
            total_resolved += resolved;
            total_refused += refusals.values().sum::<usize>();
        }
        let mut entry_issues: BTreeMap<&str, usize> = BTreeMap::new();
        for issue in tables.unresolved() {
            let kind = match issue {
                Unresolved::Identity => "identity",
                Unresolved::Asset { .. } => "asset",
                Unresolved::ProfileLine { .. } => "profile-line",
                Unresolved::Row { .. } => "slk-row",
                Unresolved::Object { .. } => "object",
            };
            *entry_issues.entry(kind).or_default() += 1;
            println!("unresolved={issue}");
        }
        println!("entry_issues={entry_issues:?} group_refusals={group_refusals}");
        let behaviors = [
            Behavior::EngineDefaults,
            Behavior::SelectorApplicability,
            Behavior::TechTreeEvaluation,
            Behavior::AvailabilityEvaluation,
            Behavior::RandomUnitSelection,
            Behavior::RandomItemSelection,
            Behavior::EditorStatParity,
        ];
        for behavior in behaviors {
            println!("native_refusal={}", tables.behavior(behavior));
        }
        println!(
            "summary slk_tables={slk_count} profile_assets={profile_count} object_rows={total_rows} slk_cells={total_cells} resolved_fields={total_resolved} field_refusals={total_refused} unresolved={} native_refusals={}",
            tables.unresolved().len(),
            behaviors.len()
        );
        assert_eq!(
            slk_count,
            SlkId::ALL.len(),
            "fixture lacks required shipped tables"
        );
    }
}
