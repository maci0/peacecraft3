//! Original-input gate: the selected install's `Scripts\common.j` admits the score counters.
//! Run with `PEACECRAFT3_SCORE_INSTALL` (install directory). The digest is computed from the
//! extracted member and written with the install path to `PEACECRAFT3_SCORE_RECEIPT` when set.
//! The edition is not derived from the path; the receipt records it as unverified.

use pcx_play::playercolor_host::{EnumHost, Limits, ObjectHost};
use pcx_script::Prelude;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::Arc;

const COMMON_MEMBER: &str = "Scripts\\common.j";
const LABEL: &str = "selected install Scripts\\common.j";

#[test]
#[ignore = "requires PEACECRAFT3_SCORE_INSTALL (selected install directory)"]
fn selected_common_admits_player_score_counters() {
    let install = std::env::var("PEACECRAFT3_SCORE_INSTALL")
        .expect("set PEACECRAFT3_SCORE_INSTALL to the selected install directory");
    let mut archives = pcx_mpq::MpqSet::open_wc3(Path::new(&install)).unwrap();
    let bytes = archives.extract(COMMON_MEMBER).unwrap();
    let digest: String = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if let Some(receipt) = std::env::var_os("PEACECRAFT3_SCORE_RECEIPT") {
        let line = format!(
            "install={install}\nmember={COMMON_MEMBER}\nbytes={}\nsha256={digest}\nedition=unverified\n",
            bytes.len()
        );
        std::fs::write(receipt, line).unwrap();
    }

    let common = pcx_script::parse_script(std::str::from_utf8(&bytes).unwrap()).unwrap();
    let map = pcx_script::parse_script("").unwrap();
    let program = Arc::new(
        pcx_script::resolve_with(&map, &[Prelude::new(LABEL, &common)]).unwrap(),
    );
    let enums = EnumHost::checked_selected(
        program,
        "selected install map",
        &["ConvertPlayerScore"],
        Limits {
            max_handles: 64,
            admission_steps: 4096,
            expression_depth: 16,
            expression_work: 4096,
        },
    )
    .unwrap();
    let host = ObjectHost::checked(enums)
        .unwrap()
        .with_player_scores()
        .unwrap();
    let snapshot = host.player_score_snapshot(0).unwrap();
    assert_eq!(snapshot.counters.len(), 25);
    assert!(snapshot.counters.iter().all(|c| c.value.is_none()));
    assert!(
        snapshot
            .counters
            .iter()
            .any(|c| c.name == "PLAYER_SCORE_UNITS_KILLED")
    );
}
