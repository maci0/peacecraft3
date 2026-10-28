//! Fixed-work simulation CPU workloads: script front end, script VM, and the Human03 headless
//! campaign. Each mode prints one JSON line with the allocation counters of its measured
//! region and an output digest. Instruction counts come from `perf stat` on the whole process;
//! the `*-baseline` modes run only the setup of their sibling, so a difference isolates the
//! measured work. Elapsed time is report-only.
//!
//! Real-asset modes read the selected local install and map named by
//! `PCX_SIM_BENCH_INSTALL` and `PCX_SIM_BENCH_MAP` (defaults: none). No asset content is
//! embedded, printed or written.
use pcx_play::headless_startup::start;
use pcx_script::{Host, Program, Value, Vm};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    fmt::Write as _,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    time::Instant,
};

struct CountingAllocator;
static MEASURING: AtomicBool = AtomicBool::new(false);
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static REALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static ALLOCATED_BYTES: AtomicUsize = AtomicUsize::new(0);

fn record(bytes: usize, realloc: bool) {
    if MEASURING.load(Ordering::Relaxed) {
        let counter = if realloc {
            &REALLOCATIONS
        } else {
            &ALLOCATIONS
        };
        counter.fetch_add(1, Ordering::Relaxed);
        ALLOCATED_BYTES.fetch_add(bytes, Ordering::Relaxed);
    }
}
// GlobalAlloc is required to count actual allocations.
// SAFETY: System receives unchanged layouts and valid pointers from GlobalAlloc callers;
// counters use only atomics and cannot allocate, unwind or affect returned pointers.
#[allow(unsafe_code)]
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record(layout.size(), false);
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record(layout.size(), false);
        unsafe { System.alloc_zeroed(layout) }
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

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_5242;
const FNV_PRIME: u64 = 0x0100_0000_01b3;
const HUMAN03_SEED: i32 = 1;
const HUMAN03_DEFAULT_TICKS: u64 = 4096;
const MAX_SCRIPT_BYTES: u64 = 16 << 20;
const SYNTHETIC_ROUNDS: i32 = 40;

/// Synthetic script exercising calls, loops, arrays, string concatenation and native dispatch.
const SYNTHETIC_SCRIPT: &str = "\
native HostAdd takes integer a, integer b returns integer
native HostName takes integer a returns string
native HostScale takes real a, integer b returns real
globals
    integer array data
    integer total = 0
    string label = \"\"
endglobals
function Fib takes integer n returns integer
    if n < 2 then
        return n
    endif
    return Fib(n - 1) + Fib(n - 2)
endfunction
function Fill takes nothing returns nothing
    local integer i = 0
    loop
        exitwhen i >= 256
        set data[i] = HostAdd(i, 3) * 2
        set i = i + 1
    endloop
endfunction
function Sum takes nothing returns integer
    local integer i = 0
    local integer sum = 0
    loop
        exitwhen i >= 256
        set sum = sum + data[i]
        set i = i + 1
    endloop
    return sum
endfunction
function Names takes nothing returns string
    local integer i = 0
    local string text = \"\"
    loop
        exitwhen i >= 32
        set text = text + HostName(i)
        set i = i + 1
    endloop
    return text
endfunction
function Reals takes nothing returns real
    local integer i = 0
    local real r = 0.0
    loop
        exitwhen i >= 128
        set r = r + HostScale(1.5, i)
        set i = i + 1
    endloop
    return r
endfunction
function Round takes nothing returns nothing
    call Fill()
    set total = total + Sum() + Fib(12)
    set label = Names()
    if Reals() > 0.0 then
        set total = total + 1
    endif
endfunction
";

struct SyntheticHost;

impl Host for SyntheticHost {
    fn native(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
        match (name, args) {
            ("HostAdd", [Value::Int(a), Value::Int(b)]) => Ok(Value::Int(a + b)),
            ("HostName", [Value::Int(a)]) => Ok(Value::Str(format!("n{a};"))),
            ("HostScale", [Value::Real(a), Value::Int(b)]) => Ok(Value::Real(a * *b as f32)),
            _ => Err(format!("unexpected native {name}")),
        }
    }
}

fn fnv(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(FNV_PRIME);
    }
}

struct Region {
    start: Instant,
}

impl Region {
    fn begin() -> Self {
        ALLOCATIONS.store(0, Ordering::Relaxed);
        REALLOCATIONS.store(0, Ordering::Relaxed);
        ALLOCATED_BYTES.store(0, Ordering::Relaxed);
        MEASURING.store(true, Ordering::Relaxed);
        Self {
            start: Instant::now(),
        }
    }
}

struct Report {
    workload: &'static str,
    params: String,
    digest: u64,
    allocations: usize,
    reallocations: usize,
    bytes: usize,
    elapsed_ns: u128,
}

fn finish(region: Region, workload: &'static str, params: String, digest: u64) {
    let elapsed_ns = region.start.elapsed().as_nanos();
    MEASURING.store(false, Ordering::Relaxed);
    let report = Report {
        workload,
        params,
        digest,
        allocations: ALLOCATIONS.load(Ordering::Relaxed),
        reallocations: REALLOCATIONS.load(Ordering::Relaxed),
        bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
        elapsed_ns,
    };
    println!(
        "{{\"workload\":\"{}\",{},\"allocations\":{},\"reallocations\":{},\"allocated_bytes\":{},\"digest\":\"{:016x}\",\"elapsed_ns_report_only\":{}}}",
        report.workload,
        report.params,
        report.allocations,
        report.reallocations,
        report.bytes,
        report.digest,
        report.elapsed_ns
    );
}

fn env_path(name: &str) -> Result<PathBuf, String> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .ok_or_else(|| format!("set {name} to the local selected install or map"))
}

/// The three scripts the campaign admission parses: `common.j`, `Blizzard.j`, the map's script.
fn real_scripts(install: &Path, map_path: &Path) -> Result<[Vec<u8>; 3], String> {
    use pcx_assets::{Addon, AssetMode, Behaviour, Build, GameMode};
    use pcx_play::campaign_data_set::{CampaignRecipe, ImageRules};
    let mode = GameMode {
        build: Build::V1_29b,
        addon: Addon::NoAddon,
        assets: AssetMode::Vanilla,
        behaviour: Behaviour::Vanilla,
    };
    let map_bytes = std::fs::read(map_path).map_err(|e| format!("{}: {e}", map_path.display()))?;
    let exe_path = install.join("Warcraft III.exe");
    let image = std::fs::read(&exe_path).map_err(|e| format!("{}: {e}", exe_path.display()))?;
    let rules = ImageRules::read(
        &image,
        pcx_play::campaign_inputs::client_identity(&image, &exe_path.display().to_string()),
    )
    .map_err(|e| e.to_string())?;
    let recipe = CampaignRecipe::pin(install, mode, rules, &image, None, "VERSION_REIGN_OF_CHAOS")
        .map_err(|e| e.to_string())?;
    let budget = pcx_mpq::checked::ReadBudget {
        max_archive_bytes: 64 << 20,
        max_archive_table_bytes: 8 << 20,
        max_raw_packed_bytes: 16 << 20,
        max_decoded_bytes: 16 << 20,
    };
    let label = map_path.display().to_string();
    let reader = recipe
        .open_map(&map_bytes, &label, budget)
        .map_err(|e| e.to_string())?;
    let read = |member: &str| {
        reader
            .resolver
            .resolve_bounded(member, MAX_SCRIPT_BYTES)
            .map(|asset| asset.bytes)
            .map_err(|e| format!("{member}: {e}"))
    };
    let map = pcx_map::Map::parse(&map_bytes).map_err(|e| e.to_string())?;
    let map_script = map
        .extract_checked(&label, "war3map.j", budget)
        .map_err(|e| e.to_string())?
        .ok_or("the map has no war3map.j")?;
    Ok([
        read("Scripts\\common.j")?,
        read("Scripts\\Blizzard.j")?,
        map_script,
    ])
}

fn utf8(bytes: &[u8]) -> Result<&str, String> {
    std::str::from_utf8(bytes).map_err(|e| e.to_string())
}

fn script_front_end(stage: &str) -> Result<(), String> {
    let install = env_path("PCX_SIM_BENCH_INSTALL")?;
    let map = env_path("PCX_SIM_BENCH_MAP")?;
    let sources = real_scripts(&install, &map)?;
    let bytes: usize = sources.iter().map(Vec::len).sum();
    let region = Region::begin();
    let mut digest = FNV_OFFSET;
    fnv(&mut digest, &bytes.to_le_bytes());
    if stage != "load" {
        let common = pcx_script::parse(utf8(&sources[0])?).map_err(|e| e.to_string())?;
        let blizzard = pcx_script::parse(utf8(&sources[1])?).map_err(|e| e.to_string())?;
        let map_script = pcx_script::parse(utf8(&sources[2])?).map_err(|e| e.to_string())?;
        fnv(&mut digest, &common.hash().to_le_bytes());
        fnv(&mut digest, &blizzard.hash().to_le_bytes());
        fnv(&mut digest, &map_script.hash().to_le_bytes());
        if stage == "resolve" {
            let prelude = [
                pcx_script::Prelude::new("common.j", &common),
                pcx_script::Prelude::new("Blizzard.j", &blizzard),
            ];
            let program =
                pcx_script::resolve_with(&map_script, &prelude).map_err(|e| e.to_string())?;
            let mut text = String::new();
            for (name, count) in program.natives_called() {
                let _ = write!(text, "{name}:{count};");
            }
            fnv(&mut digest, text.as_bytes());
        }
    }
    finish(
        region,
        "sim-script-front-end-v1",
        format!("\"stage\":\"{stage}\",\"source_bytes\":{bytes}"),
        digest,
    );
    Ok(())
}

fn synthetic_program() -> Result<Program, String> {
    let script = pcx_script::parse(SYNTHETIC_SCRIPT).map_err(|e| e.to_string())?;
    pcx_script::resolve(&script).map_err(|e| e.to_string())
}

fn script_vm(run: bool) -> Result<(), String> {
    let program = synthetic_program()?;
    let mut vm = Vm::new(&program, SyntheticHost);
    let region = Region::begin();
    let mut digest = FNV_OFFSET;
    if run {
        vm.init_globals().map_err(|e| e.to_string())?;
        for _ in 0..SYNTHETIC_ROUNDS {
            vm.call("Round", &[]).map_err(|e| e.to_string())?;
        }
        fnv(&mut digest, format!("{:?}", vm.global("total")).as_bytes());
        fnv(&mut digest, format!("{:?}", vm.global("label")).as_bytes());
    }
    finish(
        region,
        "sim-script-vm-v1",
        format!("\"rounds\":{},\"executed\":{run}", SYNTHETIC_ROUNDS),
        digest,
    );
    Ok(())
}

fn human03(ticks: u64) -> Result<(), String> {
    use pcx_assets::{Addon, AssetMode, Behaviour, Build, GameMode};
    let install = env_path("PCX_SIM_BENCH_INSTALL")?;
    let map_path = env_path("PCX_SIM_BENCH_MAP")?;
    let bytes = std::fs::read(&map_path).map_err(|e| format!("{}: {e}", map_path.display()))?;
    let mode = GameMode {
        build: Build::V1_29b,
        addon: Addon::NoAddon,
        assets: AssetMode::Vanilla,
        behaviour: Behaviour::Vanilla,
    };
    let region = Region::begin();
    let mut startup = start(
        &install,
        mode,
        &map_path.display().to_string(),
        &bytes,
        HUMAN03_SEED,
        None,
    )
    .map_err(|e| e.to_string())?;
    // The application runtime samples shared vision once before its first tick.
    startup
        .session
        .sample_admitted_vision()
        .map_err(|e| e.to_string())?;
    // One tick per call, so each tick's vision sample statistics and revisions can be read.
    let (mut vision_cell_visits, mut ticks_with_unit_change, mut ticks_with_vision_change) =
        (0usize, 0usize, 0usize);
    let revisions = |startup: &mut pcx_play::headless_startup::Startup| {
        let host = startup.session.host().ok_or("no published host")?;
        Ok::<_, String>((
            host.objects().unit_revision(),
            host.objects().vision_revision(),
        ))
    };
    let mut last = revisions(&mut startup)?;
    for _ in 0..ticks {
        startup.step_clock(1).map_err(|e| e.to_string())?;
        let host = startup.session.host().ok_or("no published host")?;
        if let Some(stats) = host.objects().vision_sample_stats() {
            vision_cell_visits += stats.cell_visits;
        }
        let now = revisions(&mut startup)?;
        ticks_with_unit_change += usize::from(now.0 != last.0);
        ticks_with_vision_change += usize::from(now.1 != last.1);
        last = now;
    }
    let mut digest = FNV_OFFSET;
    let mut units = 0usize;
    let host = startup.session.host().ok_or("no published host")?;
    let objects = host.objects();
    for (id, unit) in objects.executed_units() {
        let [x, y] = unit.position.unwrap_or([unit.x, unit.y]);
        let line = format!(
            "{id:?}|{}|{:?}|{}|{}|{:?}",
            unit.executed_owner,
            unit.definition.rawcode,
            x.to_bits(),
            y.to_bits(),
            unit.vitals
        );
        fnv(&mut digest, line.as_bytes());
        units += 1;
    }
    if let Some(time) = objects.timer_game_time() {
        fnv(&mut digest, &time.to_bits().to_le_bytes());
    }
    let random = objects
        .script_random_state()
        .map(|state| format!("{state:?}"));
    fnv(&mut digest, random.as_deref().unwrap_or("none").as_bytes());
    let random_present = random.is_some();
    finish(
        region,
        "sim-human03-v1",
        format!(
            "\"seed\":{HUMAN03_SEED},\"ticks\":{ticks},\"tick_reached\":{},\"units\":{units},\"random_state\":{random_present},\"vision_cell_visits\":{vision_cell_visits},\"ticks_with_unit_change\":{ticks_with_unit_change},\"ticks_with_vision_change\":{ticks_with_vision_change}",
            startup.tick()
        ),
        digest,
    );
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["script-front-end", stage @ ("load" | "parse" | "resolve")] => script_front_end(stage),
        ["script-vm", "baseline"] => script_vm(false),
        ["script-vm", "run"] => script_vm(true),
        ["human03"] => human03(HUMAN03_DEFAULT_TICKS),
        ["human03", ticks] => ticks.parse().map_err(|e| format!("ticks: {e}")).and_then(human03),
        _ => Err("usage: sim_bench script-front-end load|parse|resolve | script-vm baseline|run | human03 [ticks]".into()),
    };
    if let Err(error) = result {
        eprintln!("sim_bench: {error}");
        std::process::exit(1);
    }
}
