//! Local original gate: parses a Legacy 1.29 replay named by `PCX_W3G_SAMPLE`.
//!
//! Run with `PCX_W3G_SAMPLE=<path> cargo test -p pcx-net --test w3g_sample -- --ignored`.
//! The sample is never committed. The gate fails when the variable is unset.

use pcx_net::w3g::{Leave, Record, parse, write};

#[test]
#[ignore = "requires a local original replay named by PCX_W3G_SAMPLE"]
fn legacy_sample_matches_the_observed_structure() {
    let path =
        std::env::var_os("PCX_W3G_SAMPLE").expect("PCX_W3G_SAMPLE must name a local Legacy replay");
    let bytes = std::fs::read(path).expect("sample is readable");
    let replay = parse(&bytes).expect("sample parses");
    assert_eq!(replay.header.game_version, 29);
    assert_eq!(replay.header.build, 6060);
    assert_eq!(replay.header.length_ms, 73_625);
    assert_eq!(replay.header.decompressed_size, 10_570);
    assert_eq!(replay.header.block_fields.len(), 2);
    // Seen in the match: the map is Booty Bay and the host is replay1.
    assert_eq!(replay.startup.map_path, b"Maps/(2)BootyBay.w3m");
    assert_eq!(replay.startup.encoded_tail.len(), 20);
    assert_eq!(replay.startup.host.name, b"replay1");
    assert_eq!(replay.startup.game_name, b"Local Game");
    assert_eq!(replay.startup.slots.len(), 2);
    assert_eq!(replay.startup.random_seed, 0x04b3_ff52);
    let slots = replay
        .records
        .iter()
        .filter(|r| matches!(r, Record::TimeSlot { .. }))
        .count();
    let checks = replay
        .records
        .iter()
        .filter(|r| matches!(r, Record::Checksum(_)))
        .count();
    assert_eq!((slots, checks), (939, 939));
    assert_eq!(
        replay.records.last(),
        Some(&Record::Leave(Leave {
            reason: 12,
            player_id: 1,
            result: 7,
            trailer: 1,
        }))
    );
    // Recompression preserves content and recomputes block checksums.
    let written = write(&replay).expect("sample writes with its block fields present");
    let round = parse(&written).expect("written sample re-parses");
    let mut expected = replay;
    expected.header.block_fields = round.header.block_fields.clone();
    assert_eq!(round, expected);
}
