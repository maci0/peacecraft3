//! Maps that the session host must refuse, until the production startup is wired.
//!
//! The gated test reads selected local map files. Run it with:
//!
//! PCX_SESSION_MAP=<map file> PCX_SESSION_OTHER_MAP=<map file> \
//!   cargo test -p pcx-net --features session-server --test map_peers -- --ignored
#![cfg(not(target_arch = "wasm32"))]

use std::path::PathBuf;

use pcx_net::map_session::{Reason, assess};

fn gate(name: &str) -> PathBuf {
    std::env::var_os(name)
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("set {name} to a selected local map file"))
}

#[test]
fn bytes_that_are_not_a_map_are_an_assessment_error() {
    assert!(assess(b"not a map archive", "bytes").is_err());
}

#[test]
#[ignore = "requires PCX_SESSION_MAP and PCX_SESSION_OTHER_MAP selected local map files"]
fn real_maps_are_refused_with_the_startup_reasons_that_are_still_missing() {
    for name in ["PCX_SESSION_MAP", "PCX_SESSION_OTHER_MAP"] {
        let path = gate(name);
        let bytes = std::fs::read(&path).expect("read map");
        let reasons = assess(&bytes, &path.display().to_string()).expect("assess map");
        assert!(
            reasons.contains(&Reason::OwnerRoster),
            "{name}: {reasons:?}"
        );
        assert_eq!(reasons.last(), Some(&Reason::HarnessMovement), "{name}");
    }
}
