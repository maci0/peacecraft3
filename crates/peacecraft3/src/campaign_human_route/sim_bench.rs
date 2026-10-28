//! Fixed-work counters for the headless route harness. With `PCX_SIM_BENCH` set, the Human04
//! route stops after its decision budget instead of failing and prints allocation counters (from
//! the test binary's counting allocator) and a state digest as JSON lines. Instruction counts
//! come from `perf stat` on the test binary; running with `PCX_HUMAN04_DECISIONS=0` isolates
//! startup. See docs/performance/sim-script.md.
use crate::counting_allocator;
use bevy::prelude::*;
use pcx_play::map_startup_host::MapStartupHost;
use peacecraft3::map_script_session::MapScriptSession;

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_5242;
const FNV_PRIME: u64 = 0x0100_0000_01b3;
const ENABLE: &str = "PCX_SIM_BENCH";

pub(super) fn enabled() -> bool {
    std::env::var_os(ENABLE).is_some()
}

/// Prints the process-lifetime allocation counters under `label`.
pub(super) fn counters(label: &str) {
    if enabled() {
        let totals = counting_allocator::totals();
        println!(
            "SIM_BENCH {{\"label\":\"{label}\",\"allocations\":{},\"reallocations\":{},\"allocated_bytes\":{}}}",
            totals.calls, totals.reallocations, totals.bytes
        );
    }
}

/// Prints the counters and a digest of the executed units, clock, source RNG and `last`
/// (the harness's own per-decision observation line) for the stopped route.
pub(super) fn finish(label: &str, app: &mut App, last: &str) {
    counters(label);
    let mut digest = FNV_OFFSET;
    let mut fnv = |bytes: &[u8]| {
        for byte in bytes {
            digest ^= u64::from(*byte);
            digest = digest.wrapping_mul(FNV_PRIME);
        }
    };
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let host = session.host().expect("published host");
    let objects = host.objects();
    let mut units = 0usize;
    for (id, unit) in objects.executed_units() {
        let [x, y] = unit.position.unwrap_or([unit.x, unit.y]);
        fnv(format!(
            "{id:?}|{}|{:?}|{}|{}|{:?}",
            unit.executed_owner,
            unit.definition.rawcode,
            x.to_bits(),
            y.to_bits(),
            unit.vitals
        )
        .as_bytes());
        units += 1;
    }
    if let Some(time) = objects.timer_game_time() {
        fnv(&time.to_bits().to_le_bytes());
    }
    let random = objects
        .script_random_state()
        .map(|state| format!("{state:?}"));
    fnv(random.as_deref().unwrap_or("none").as_bytes());
    fnv(last.as_bytes());
    println!(
        "SIM_BENCH {{\"label\":\"{label}-state\",\"units\":{units},\"random_state\":{},\"digest\":\"{digest:016x}\",\"last\":{last:?}}}",
        random.is_some()
    );
}
