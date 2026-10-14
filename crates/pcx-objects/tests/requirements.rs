#![allow(dead_code)]
#[path = "../src/requirements.rs"]
mod requirements;

use pcx_objects::editor::defaults::DefaultTables;
use pcx_objects::editor::defaults::Family;
use requirements::RequirementsCatalog;
use std::path::PathBuf;

#[test]
#[ignore = "requires original installs; set PEACECRAFT3_LEGACY_DIR"]
fn original_tech_requirement_catalog_gate() {
    let legacy = PathBuf::from(std::env::var_os("PEACECRAFT3_LEGACY_DIR").unwrap_or_default());
    assert!(
        legacy.is_dir(),
        "original install required: {}",
        legacy.display()
    );
    let tables = DefaultTables::open(&legacy, "edition/version unverified").expect("open");
    let fields: &[[u8; 4]] = &[
        *b"ureq", *b"urq1", *b"urq2", *b"urq3", *b"urq4", *b"urq5", *b"urq6", *b"urq7", *b"urq8",
        *b"urqc", *b"urqa",
    ];
    let catalog = RequirementsCatalog::new(&tables, fields);
    assert!(!catalog.is_empty());
    let item_entries = catalog.for_family(Family::Item);
    assert!(!item_entries.is_empty());
    println!("entries={}", catalog.len());
}
