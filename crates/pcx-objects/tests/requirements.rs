//! Explicit local asset gate; field roles and native prerequisite rules are not inferred.
use pcx_objects::editor::defaults::DefaultTables;
use pcx_objects::requirements::RequirementsCatalog;
use std::path::PathBuf;

#[test]
#[ignore = "requires PEACECRAFT3_LEGACY_DIR and PEACECRAFT3_REQUIREMENT_FIELDS (comma-separated fourccs)"]
fn original_selected_field_catalog_gate() {
    let install = PathBuf::from(
        std::env::var_os("PEACECRAFT3_LEGACY_DIR").expect("set explicit install directory"),
    );
    assert!(
        install.is_dir(),
        "original install required: {}",
        install.display()
    );
    let selected =
        std::env::var("PEACECRAFT3_REQUIREMENT_FIELDS").expect("select exact metadata fourccs");
    let fields = selected
        .split(',')
        .map(|field| {
            field
                .trim()
                .as_bytes()
                .try_into()
                .expect("each selected fourcc must contain exactly four bytes")
        })
        .collect::<Vec<[u8; 4]>>();
    let tables = DefaultTables::open(
        &install,
        "edition/version unverified; archive members independently fingerprinted",
    )
    .expect("open explicit install");
    let catalog = RequirementsCatalog::new(&tables, &fields)
        .expect("every selection must have supplied metadata");
    let mut mapped_count = 0;
    let mut refused_count = 0;
    for entry in catalog.entries() {
        match &entry.outcome {
            Ok(mapped) => {
                mapped_count += 1;
                assert_eq!(mapped.definition.fourcc, entry.field.fourcc);
                assert_eq!(
                    mapped.metadata.attribute("ID").unwrap().text.as_bytes(),
                    entry.field.fourcc
                );
                assert_eq!(mapped.cell.provenance.identity, tables.identity());
                assert_eq!(
                    tables.assets().get(&mapped.cell.provenance.asset.member),
                    Some(mapped.cell.provenance.asset)
                );
                assert!(!mapped.cell.provenance.asset.archive.is_empty());
                let direct = tables
                    .object(entry.family, entry.rawcode)
                    .unwrap()
                    .field(entry.field.fourcc)
                    .unwrap();
                assert_eq!(mapped.cell, direct.cell);
                assert_eq!(mapped.value, direct.value);
            }
            Err(_) => refused_count += 1,
        }
    }
    assert!(
        mapped_count > 0,
        "selection produced no successful authored joins"
    );
    println!(
        "mapped={mapped_count} refused={refused_count} source_issues={}",
        tables.unresolved().len()
    );
}
