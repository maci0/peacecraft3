use super::*;

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
    assert_eq!((table.slot_ceiling, table.entries), (11, 12));
    assert_eq!(table.image_sha256, <[u8; 32]>::from(Sha256::digest(&exe)));
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
    assert_eq!(real.slot_ceiling, 11);
    assert_eq!((patched.slot_ceiling, patched.entries), (10, real.entries));
}
