//! Explicit local gate for the upgrade effect slots read by `pcx-play`: the stock `UpgradeData`
//! rows, the `UpgradeMetaData` slot layout and the `UpgradeEffectMetaData` effect ids.
//! Run with `PEACECRAFT3_LEGACY_DIR` set to the selected install directory:
//! `cargo test -p pcx-play --test upgrade_effects_original -- --ignored`
use pcx_objects::editor::Overlay;
use pcx_objects::editor::defaults::{DefaultTables, Family};
use pcx_play::upgrade_effects::{Effect, Table};
use std::path::PathBuf;

/// Legacy resource version of the selected executable.
const EDITION_VERSION: &str = "1.29.2.9231";
/// Effect ids the stock rows use that `UpgradeEffectMetaData` does not define.
const UNDEFINED_STOCK_EFFECTS: [[u8; 4]; 2] = [*b"rarm", *b"rauv"];

fn table() -> (DefaultTables, Table, Vec<i32>) {
    let install = PathBuf::from(
        std::env::var_os("PEACECRAFT3_LEGACY_DIR")
            .expect("set PEACECRAFT3_LEGACY_DIR to the Legacy install directory"),
    );
    let tables = DefaultTables::open(&install, EDITION_VERSION).expect("install tables");
    let ids: Vec<i32> = tables
        .rawcodes(Family::Upgrade)
        .map(|alias| i32::from_be_bytes(alias.as_bytes().try_into().expect("four bytes")))
        .collect();
    let effects = Table::read(&tables, &Overlay::default(), &ids).expect("stock effects");
    (tables, effects, ids)
}

#[test]
#[ignore = "requires PEACECRAFT3_LEGACY_DIR (the selected Warcraft III Legacy install directory)"]
fn sorcery_training_slots_come_from_the_install() {
    let (_, effects, _) = table();
    let slots = effects.slots(i32::from_be_bytes(*b"Rhst"));
    let seen: Vec<_> = slots
        .iter()
        .map(|s| (s.number, s.effect, s.base, s.modifier))
        .collect();
    assert_eq!(
        seen,
        [
            (1, *b"rmnx", 100.0, 100.0),
            (2, *b"rmnr", 0.325, 0.325),
            (3, *b"rhpx", 40.0, 40.0),
            (4, *b"ratd", 0.0, 0.0),
        ]
    );
    assert_eq!(slots[0].value(2), 200.0);
}

#[test]
#[ignore = "requires PEACECRAFT3_LEGACY_DIR (the selected Warcraft III Legacy install directory)"]
fn every_stock_effect_id_is_defined_by_the_metadata_or_listed_as_undefined() {
    let (_, effects, ids) = table();
    let mut undefined = std::collections::BTreeSet::new();
    let mut applied = std::collections::BTreeSet::new();
    for id in ids {
        for slot in effects.slots(id) {
            if !effects.is_defined(&slot.effect) {
                undefined.insert(slot.effect);
            }
            if matches!(
                effects.classify(&slot.effect),
                Effect::MaxMana | Effect::MaxLife
            ) {
                applied.insert(slot.effect);
            }
        }
    }
    assert_eq!(
        undefined.into_iter().collect::<Vec<_>>(),
        UNDEFINED_STOCK_EFFECTS
    );
    assert_eq!(
        applied.into_iter().collect::<Vec<_>>(),
        [*b"rhpx", *b"rmnx"]
    );
}
