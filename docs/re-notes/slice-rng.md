# Script RNG host: seed gate and call-site order

## Current correction (2026-11-13)

The historical inventory below predates the current caller log. `DrawLog` now
records admitted calls, and the host already connects the held spell-orb consumer
through `orb_swing_start`. Secondary real calls were incorrectly named
`SecondaryDie` in that log. They now use `SecondaryReal`; both secondary call
kinds record their selected channel's post-draw word as well as the primary word.
Primary calls carry no secondary word. This changes trace identity and inspection,
not generator arithmetic, seeds, or draw order. Secondary records identify the
call kind and channel, not the runtime unit/ability owner.

Fresh bounded GNU objdump inspection of the selected image confirms the
`0xc9f35d` call to `0x69a030`. The helper's valid-owner/channel path selects
`base + 4 + channel * 8` before invoking the arithmetic draw. The stream operand
continues to come from the admitted orb policy; no observed operand is copied
into production. This is native-code evidence, not runtime parity. Source identity
is the `1.29.2.9231` image SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
The existing detailed binding is in [passive-held-items.md](passive-held-items.md#the-proc-roll-observed).

`interleaved_secondary_real_and_die_retain_distinct_exact_state_records` checks
mixed script integer/real and secondary real/die calls, including two channels,
a one-sided die, failed channel/sides calls, unseeded refusal, and reseeding.
Each checkpoint compares the complete retained State (including lanes) with
explicit low-level draws and checks the exact caller/word log. The existing
`an_orb_without_chance_never_marks_and_rolls_once_per_swing_on_its_own_stream`
now replays the actual combat/orb trace and checks complete final state. These
are synthetic host-routing/lifecycle regressions, not independent evidence for
the low-level arithmetic or native tick parity.

Engine initialization remains blocked on the semantic origin of the per-slot
seed inputs and the complete owner/consumer lifecycle. The fresh
[initializer review](original-rng-control.md#enginescript-initializer-distinction-rechecked-2026-11-13)
confirms that the reviewed engine path seeds secondary state from the raw seed,
whereas the script setter uses its discarded primary result. No captured seed or
universal warmup is admitted. The preexisting selected-code and Wine evidence
below remains scoped to its recorded inputs and selected paths.

Independent native validation uses `/tmp/pc3-fidelity-sol61-03-target`, with
source compilation from this worktree and explicit execution of both named
regressions above: six host tests, eighteen numeric/profile tests and 113 orb
cases pass. One separate venom selected-install gate remains explicitly ignored
because it requires `PEACECRAFT3_INSTALL`. All runs verify private Xvfb and null
audio before launch; workspace compiler warnings are errors. Shared-target logs
are superseded because parallel worktrees can reuse stale workspace artifacts.
The durable local receipt is `/tmp/pc3-sol61-03-validation.json`, with the exact
commands and per-stage logs. The independent native library build, browser WASM
compile check and browser WASM library build also pass with warnings denied.
WASM runtime execution and a new original-client comparison were not run.

## Historical inventory (superseded where noted above)

`ObjectHost` script RNG in `crates/pcx-play/src/script_random_host.rs` does not draw before an executed `SetRandomSeed`. It also does not record the Human01 startup callers in `docs/re-notes/original-rng-control.md`. No seed, warmup count, or generator constant is added. Generator identity stays open. A matching Wine probe is not recreation parity.

## No draw before SetRandomSeed

`with_script_random` stores `state: None` (`script_random_host.rs` lines 51-55). State is created only by the `SetRandomSeed` arm, which calls `state::seeded` (lines 124-128). `GetRandomInt`, `GetRandomReal`, `draw_random_integer`, and `draw_random_secondary_die` do not call `seeded`.

`state::integer` returns `low` when `low == high` and does not read `state` (`crates/pcx-play/src/script_random/state.rs` lines 141-143). Any other pair returns `StateError::Uninitialized` while `state` is `None` (line 144), before `preview`. `state::real` returns `low` when the absolute difference is below `policy.real_tolerance`, without reading `state` (lines 187-189). A wider pair returns `Uninitialized` while `state` is `None` (line 190), before `preview`. `state::secondary_die` returns `Uninitialized` in that case (line 166). Those early returns do not create a generator.

`state::seeded` does call `Primary::preview` once (lines 122-123). That runs only after `SetRandomSeed`. The explicit-seeded-host section of `original-rng-control.md` names that single discarded draw as the script setter's warmup, not an engine initial seed. The Human01 startup section says the 37 primary writes before the first recorded real entry are one run's history, not a production initialization rule. This host does not install 37.

`failed_calls_do_not_consume_existing_seed_and_new_host_has_no_seed` in `script_random_host_tests.rs` checks `GetRandomInt(8, 8)` before a seed and `GetRandomInt(0, 1)` refusing with `explicit seed`. A fresh host has `script_random_state() == None`. The test does not call `GetRandomReal` before `SetRandomSeed`. A non-degenerate real uses the same `Uninitialized` path. Human01 has no JASS `SetRandomSeed` before its first `GetRandomReal` (map span `186558..186584`), so that call stays refused here.

## Call-site order is not in this file

`original-rng-control.md` ("Startup consumer identities") records one unchanged Human01 Wine run. The 37 draws before the first recorded real entry are 19 player-slot race-resolution draws and 18 `CAbilityWander` draws reached through `CreateUnit`. No JASS `SetRandomSeed` and no integer wrapper are recorded before those four real calls ("Unmodified Human01 startup checkpoints").

`script_random_host.rs` names `GetRandomInt`, `GetRandomReal`, `SetRandomSeed`, `draw_random_integer` (one draw as `GetRandomInt` makes), and `draw_random_secondary_die` (caller channel and sides, not the primary stream). It does not name the race resolver or `CAbilityWander`, and it stores no caller sequence.

The same note forbids installing that prefix: 37 is not a universal warmup, a generic startup skip or an unconditional `CreateUnit` draw would drop the race-preference and fresh-unit conditions, and the private adapter that reuses the script setter's first draw is not a production engine initializer ("Startup inputs and owned-factory boundary"). No order is both specified by that note and already present in this file, so none is recorded.

## Generator identity still missing

Not identified. Not selected here.

For one local `Warcraft III.exe` `1.29.2.9231`, SHA-256 `3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`, the note binds candidate handlers `GetRandomInt` at `0x494e40`, `GetRandomReal` at `0x494e80`, and `SetRandomSeed` at `0x4a99a0`, with primary draw helper `0x480dd0` and seed helper `0x9892a0` then `0x69a0d0`. Those addresses belong to that image. Offline checks matched 80 integer observations and 16 formatted real strings from two authored Wine runs, then raw real bits for four checkpoints on each of three fresh Human01 runs (12 checkpoints). The owned factory matches one of four required Human01 real results and stops at `TriggerRegisterUnitInRange` (Blizzard span `82885..82941`). Wine with software Mesa is not native Windows ground truth. `TRACKER.md` leaves RE-Q6 open.

Still missing:

- The semantic origin of the per-slot values mixed into the engine seed accumulator. The note leaves that origin unresolved. Neither a captured seed nor the 37-draw total is a production default. Human01 JASS does not supply `SetRandomSeed` before the first `GetRandomReal`.
- Whether every map takes the loaded-seed path seen on the retained Human01 runs. The static caller branches between a loaded seed and an instruction immediate. Only the loaded-seed path was observed.
- Consumers besides the 19 race rows and 18 wander entries in that startup prefix, including draws before globals, `config`, or `main`, and secondary-stream owners. `0x69a0d0` is not a reconstructed consumer set. The retained secondary block is recreation-owned. The offline comparison used a synthetic owner pointer.
- A captured mapping from each wander draw to a runtime handle. The `Awan` join finds 18 `CreateNeutralPassive` lists, a static count agreement, not that mapping. Fresh-unit flag and component conditions are not a rule in this host.
- Native Windows runs, unmodified runs beyond the three Wine captures, bit-exact reals outside those checkpoints (`R2S` text is not bits), and NaN, infinity, and denormal behavior. The 100-consecutive unchanged-map baseline is open.
- The four-result factory gate. One matching real is not call-site parity.

`pcx_sim::Rng` is a separate generator and is not used by this host.
