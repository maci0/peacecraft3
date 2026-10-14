# Script execution and deterministic play harness

The harness has runnable synthetic scenarios and source-aware JASS chapter execution.
Neither establishes original campaign completion. PLAN M7/M9 gates remain open.

## Synthetic scenarios

Run the checked-in ordering fixture:

```sh
tools/playrun.sh tests/play/synthetic/ordering/same-tick.rpl /tmp/ordering.trace --runs 100 --max-work 2000
```

Add `--baseline /path/to/original.trace` to compare a canonical unit-state trace.
A comparison failure retains the candidate trace for diagnosis. Exit 0 means all
assertions, repeats and any supplied baseline comparison passed; 1 means an assertion,
work limit or baseline failed; 2 means invalid input or an I/O error. A trace supplied
as a baseline is not automatically evidence from the original executable.

`pcx-rpl 1` requires one each of `scenario`, `map`, `ticks`, `seed`, `asset-mode`,
and at least one `expect`. Names follow the existing trace identifier rules.
Coordinates are signed raw fixed-point integers in `pcx_sim::Fx`, not inferred game units.
`#` starts a comment; empty lines are ignored.

| Record | Meaning |
| --- | --- |
| `spawn <owner> <x> <y>` | Synthetic starting unit |
| `at <tick> <name> rng` | Draw from the shared harness RNG |
| `at <tick> <name> <player> move/select <x> <y>` | Existing simulation command |
| `at <tick> <name> <player> stop/nop` | Existing simulation command |
| `expect <tick> units <count>` | Post-step unit count |
| `expect <tick> unit <id> <owner> <x> <y> idle` | Exact post-step unit state |
| `expect <tick> unit <id> <owner> <x> <y> target <x> <y>` | Exact state and pending target |
| `expect <tick> timer <index> <name>` | Action-event ordering |
| `expect <tick> rng-draws <count>` | Draw count |
| `fix adopt/reject/defer <id>` | Existing trace fix-ledger entry |

Same-tick actions retain declaration order within each player. The simulation applies
players in player order. Unsupported commands such as attack/build/cast/train/research
report the scenario path, source line and command rather than becoming no-ops. Select
uses the current synthetic simulation's behavior; actual selection parity is unproved.

`--max-work` bounds the reproducible estimate
`runs * (ticks * (spawns + 1) + scheduled_actions)`. It is a harness work gate, not a
measurement of combat cost. No timing or FPS threshold decides pass/fail.

The runner checks that the 64-bit tick count fits the platform's `usize` and
`TickRecord` vector layout before creating the world. It reserves trace storage
fallibly: an unrepresentable layout returns `RunError::TraceCapacity`, and a
reservation failure returns `RunError::TraceAllocation`, both with the scenario
name and tick count. The CLI reports these as run failures before writing output.
This prevents 32-bit tick-count truncation and capacity-overflow panics; it is
not a work or memory budget. Use `--max-work` to bound an accepted scenario.

## Source-aware maps and chapters

`pcx_play::scripts::MapScripts::compile` takes actual bytes for `common.j`, `Blizzard.j`,
`war3map.j`, optional WTS, and explicit edition/version/source identity. It parses and
resolves one source set; there is no union of installs, copied native-signature table,
or substitute BJ library. The existing VM executes the supplied function bodies.
Source digests are reproducibility checksums, not release authentication.

`LocalizedHost` reads a WTS entry only when the VM uses it. Authored control codes and
newlines are preserved. Missing entries and invalid encoding stop the VM. Invalid
unused WTS entries do not prevent valid entries from executing. Unknown native calls
include the called name, signature, declaring source and call span. Prelude errors
render against their own source, not the map's line numbers.

`pcx_play::campaign::run_chapter` uses explicit entrypoints and scheduled calls.
`CampaignHost::advance_tick` must be implemented by the actual game host; there is no
default silently dropping timers. It returns due function IDs in the host's evidenced
order. The harness runs these callbacks, scheduled calls, then outcome predicates.
That ordering is the harness contract, not measured original event-loop parity.

Every chapter requires explicit dialogue, quest, victory and end-game boolean predicate
functions. These functions must query script or host state; unit counts or successful
parsing cannot satisfy the gates. A false/non-boolean result or native error fails the
chapter. `run_campaign` opens chapters in caller-supplied order, creates a fresh host/VM
for each, and stops before opening another chapter after failure. Campaign metadata
parsing and campaign-state transfer remain importer/game-host responsibilities.

`ChapterTrace::encode` emits a separate `pcx-script-trace 1`, retaining source identity,
source checksums, exact real bits, function returns, native Host observations and gate
checks. Native observations and completed script calls carry a common sequence so
interleaving is preserved. It does not contain original script bytes. Unit-state traces
retain their existing schema; the two are not interchangeable. `ChapterTrace::decode`
requires canonical encoding and validates value classes, tick bounds and contiguous
sequence numbers. `pcx-diff` accepts either schema, reports the first script event
with its native/function name, and rejects cross-schema comparisons. Exact real bits
are compared, including NaN payloads and signed zero.

For a host-free original-source execution probe:

```sh
cargo run --locked -p pcx-play --bin playrun-script -- \
  '<edition>' '<measured-version>' '<asset-source-identity>' \
  /local/common.j /local/Blizzard.j /local/war3map.j [/local/war3map.wts]
```

This executes globals/config/main through `NullHost`, so original maps are expected to
fail at their first unsupported game native. It does not assert campaign outcomes.
All files remain local. Do not check extracted originals into the repository.

The editor binding input is documented in
[the TriggerData catalog notes](re-notes/script-trigger-catalog.md). It preserves actual
ScriptName aliases and reports ambiguous/malformed metadata; it does not lower WTG.

## Verification and remaining gates

Synthetic tests exercise same-tick ordering, 100 independent byte-identical runs,
explicit state checks, WTS preservation, source diagnostics, failed initialization,
host-observed outcomes, native/script trace interleaving and fail-stop campaign ordering.
Capacity regressions cover oversized counts, unchanged normal traces and preservation
of an existing CLI output. The same runner tests execute on actual 32-bit release
WASM, including a tick count larger than `u32::MAX`.
The ignored original-source test requires explicit `PEACECRAFT3_SCRIPT_COMMON`,
`BLIZZARD`, `MAP`, `EDITION`, `VERSION`, and `SOURCE` environment variables, each prefixed
with `PEACECRAFT3_SCRIPT_`. Missing originals fail explicitly when requested.

Opaque Host-owned handle tokens now survive globals, arrays and arguments and support
identity/null comparisons. They do not define engine handle numbers, allocation,
recycling or lifetime. Native game Host implementations and those native handle rules
remain open, along with measured timers/triggers/event ordering, WTG/WCT lowering
from actual TriggerData, map/campaign importer and application wiring, combat/AI
execution, original runtime baselines, and stock-campaign outcomes.
DotA, Athena and Footman Frenzy have not passed. Parser or synthetic tests do not prove
native or rendered behavior.
