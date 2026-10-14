//! Fixed-work simulation allocation gate; elapsed time is optional reporting only.
use pcx_sim::{Command, FX_ONE, Input, PlayerInput, Rng, World, step};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    time::Instant,
};

const UNITS: usize = 512;
const PLAYERS: usize = 4;
const TICKS: u64 = 2_000;
const SEED: u64 = 0x574333;
const EXPECTED_DIGEST: u64 = 0x867f_3f63_27d6_0281;

struct CountingAllocator;
static MEASURING: AtomicBool = AtomicBool::new(false);
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static REALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static ALLOCATED_BYTES: AtomicUsize = AtomicUsize::new(0);

fn record(bytes: usize, realloc: bool) {
    if MEASURING.load(Ordering::Relaxed) {
        if realloc {
            REALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        } else {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        ALLOCATED_BYTES.fetch_add(bytes, Ordering::Relaxed);
    }
}
// GlobalAlloc is required to count actual allocations in the deterministic gate.
// SAFETY: System receives unchanged layouts and valid pointers from GlobalAlloc callers;
// counters use only atomics and cannot allocate, unwind or affect returned pointers.
#[allow(unsafe_code)]
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record(layout.size(), false);
        unsafe { System.alloc(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, old: Layout, size: usize) -> *mut u8 {
        record(size, true);
        unsafe { System.realloc(ptr, old, size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

// Keep a function boundary for optional Callgrind collection of simulation work.
#[inline(never)]
pub fn pcx_perf_measure(world: &mut World, input: &mut Input, rng: &mut Rng) {
    for tick in 0..TICKS {
        input.tick = tick;
        for player in &mut input.commands {
            let x = (rng.next_u64() % 41) as i32 - 20;
            let y = (rng.next_u64() % 41) as i32 - 20;
            player.commands[0] = match (tick + player.player as u64) % 32 {
                0 => Command::Nop,
                1 => Command::Stop,
                _ => Command::Move {
                    x: x * FX_ONE,
                    y: y * FX_ONE,
                },
            };
        }
        step(world, input, rng);
    }
}

fn digest(world: &World) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    let mut add = |bytes: &[u8]| {
        for byte in bytes {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
    };
    add(&world.tick.to_le_bytes());
    add(&world.next_unit_id.to_le_bytes());
    for unit in &world.units {
        add(&unit.id.to_le_bytes());
        add(&[unit.owner]);
        add(&unit.x.to_le_bytes());
        add(&unit.y.to_le_bytes());
        match unit.move_target {
            Some((x, y)) => {
                add(&[1]);
                add(&x.to_le_bytes());
                add(&y.to_le_bytes());
            }
            None => add(&[0]),
        }
    }
    hash
}

fn main() {
    let timing = match std::env::args().nth(1).as_deref() {
        None => false,
        Some("--timing") => true,
        _ => {
            eprintln!("usage: peacecraft-perf [--timing]");
            std::process::exit(2);
        }
    };
    let mut world = World::empty();
    for i in 0..UNITS {
        world.spawn(
            (i % PLAYERS) as u8,
            ((i % 32) as i32 - 16) * FX_ONE,
            ((i / 32) as i32 - 8) * FX_ONE,
        );
    }
    let mut input = Input {
        tick: 0,
        commands: (0..PLAYERS)
            .map(|player| PlayerInput {
                player: player as u8,
                commands: vec![Command::Nop],
            })
            .collect(),
    };
    let mut rng = Rng::new(SEED);
    // World/input preparation, checksum and reporting are outside the allocation budget.
    let start = timing.then(Instant::now);
    MEASURING.store(true, Ordering::Relaxed);
    pcx_perf_measure(&mut world, &mut input, &mut rng);
    MEASURING.store(false, Ordering::Relaxed);
    let elapsed_ns = start.map(|start| start.elapsed().as_nanos());
    let allocations = ALLOCATIONS.load(Ordering::Relaxed);
    let reallocations = REALLOCATIONS.load(Ordering::Relaxed);
    let bytes = ALLOCATED_BYTES.load(Ordering::Relaxed);
    let digest = digest(&world);
    let max_bytes = TICKS as usize * PLAYERS * std::mem::size_of::<&PlayerInput>();
    println!(
        "{{\"workload\":\"sim-movement-v1\",\"seed\":{SEED},\"ticks\":{},\"units\":{},\"player_commands\":{},\"allocations\":{allocations},\"reallocations\":{reallocations},\"allocated_bytes\":{bytes},\"max_allocations\":{TICKS},\"max_allocated_bytes\":{max_bytes},\"state_digest\":\"{digest:016x}\",\"elapsed_ns_report_only\":{}}}",
        world.tick,
        world.units.len(),
        TICKS * PLAYERS as u64,
        elapsed_ns.map_or("null".into(), |value| value.to_string())
    );
    assert_eq!(world.tick, TICKS, "fixed workload did not complete");
    assert_eq!(world.units.len(), UNITS, "workload state changed");
    assert_eq!(
        digest, EXPECTED_DIGEST,
        "simulation result changed; review semantics before changing its baseline"
    );
    assert!(
        allocations <= TICKS as usize,
        "simulation allocated more than one temporary command-order array per tick"
    );
    assert_eq!(reallocations, 0, "simulation unexpectedly reallocated");
    assert!(
        bytes <= max_bytes,
        "simulation allocated more than its fixed command-order array budget"
    );
}
