use super::*;
use sha2::{Digest, Sha256};

fn origin() -> AssetIdentity {
    AssetIdentity {
        edition: "explicit gate selection".into(),
        version: "1.29.2.9231".into(),
        source: "synthetic".into(),
    }
}

#[test]
fn bytes_that_are_not_a_pe_image_refuse_without_a_capacity() {
    assert!(matches!(
        derive(b"MZ", origin()),
        Err(ShopTableError::Image(_))
    ));
}

/// Original-input gate: `PCX_WAR3_EXE` names the selected executable. Unset, it skips.
/// The clamp and cell count come from the code bytes, so a patched copy changes them.
#[test]
fn original_executable_yields_the_shop_capacity() {
    let Some(path) = std::env::var_os("PCX_WAR3_EXE") else {
        return;
    };
    let exe = std::fs::read(&path).expect("readable executable");
    let table = derive(&exe, origin()).expect("capacity in original executable");
    assert_eq!(
        table.item,
        SlotTable {
            slot_ceiling: 11,
            entries: 12,
            default_slots: 11
        }
    );
    assert_eq!(
        table.unit,
        SlotTable {
            slot_ceiling: 11,
            entries: 12,
            default_slots: 11
        }
    );
    assert_eq!(table.image_sha256, <[u8; 32]>::from(Sha256::digest(&exe)));
}

/// The default slot count is the immediate the component initializer stores into the count
/// field, so patching those stores changes it and nothing else.
#[test]
fn the_default_slot_count_is_read_from_the_initializer_store() {
    let Some(path) = std::env::var_os("PCX_WAR3_EXE") else {
        return;
    };
    let mut exe = std::fs::read(&path).expect("readable executable");
    let real = derive(&exe, origin()).expect("capacity");
    let store = [0xc7, 0x86, 0x80, 0x04, 0x00, 0x00, 0x0b, 0x00, 0x00, 0x00];
    let at: Vec<usize> = exe
        .windows(store.len())
        .enumerate()
        .filter(|(_, w)| *w == store)
        .map(|(at, _)| at)
        .collect();
    assert_eq!(at.len(), 2, "the item and the unit initializer");
    for at in at {
        exe[at + 6] = 0x07;
    }
    let patched = derive(&exe, origin()).expect("capacity after patch");
    assert_eq!(real.item.default_slots, 11);
    assert_eq!(
        (patched.item.default_slots, patched.unit.default_slots),
        (7, 7)
    );
    assert_eq!(patched.item.slot_ceiling, real.item.slot_ceiling);
}

#[test]
fn the_clamp_immediate_is_read_from_the_image_bytes() {
    let Some(path) = std::env::var_os("PCX_WAR3_EXE") else {
        return;
    };
    let mut exe = std::fs::read(&path).expect("readable executable");
    let real = derive(&exe, origin()).expect("capacity");
    let clamps: Vec<usize> = exe
        .windows(CLAMP.len())
        .enumerate()
        .filter(|(_, w)| {
            w[0] == CLAMP[0] && w[5..] == CLAMP[5..] && w[2..5] == [0, 0, 0] && w[1] == 0x0b
        })
        .map(|(at, _)| at)
        .collect();
    assert!(!clamps.is_empty(), "a clamp with ceiling 11 in the image");
    for at in clamps {
        exe[at + 1] = 0x0a;
    }
    let patched = derive(&exe, origin()).expect("capacity after patch");
    assert_eq!(real.item.slot_ceiling, 11);
    assert_eq!(
        (patched.item.slot_ceiling, patched.item.entries),
        (10, real.item.entries)
    );
}
