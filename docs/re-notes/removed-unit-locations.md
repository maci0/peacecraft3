# Removed-unit location reads

Target: selected Legacy `Warcraft III.exe` 1.29.2.9231, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.

## Original-client observations

Two local diagnostic maps derived from Human01 ran under the private Docker/Wine
reference harness. They preserved standard initialization and omitted placed units
and authored trigger initialization. They are diagnostic inputs, not unchanged-map
acceptance. Each checkpoint wrote its own file; no conclusion depends on missing
screen text. A normal Space press dismissed the chapter card in each run.

Both runs returned the live location immediately after `RemoveUnit` in the same
callback at printed game time 5.000. Run A returned null from a timer callback one
game second later. Run B armed a zero-delay timer immediately after removal: its
callback ran at printed game time 5.063 and returned null. Three later timer
callbacks also returned null. This bounds the observed transition between the
synchronous removal call and the next timer callback; it does not identify the
engine's exact reclamation phase or qualify arbitrary event ordering.

Run A also observed `GetUnitLoc(null)` returning null, null `GetLocationX/Y`
printing 0.000, and `RemoveLocation(null)` returning through the completion mark.

Receipts remain local under the `perf/cpu-hotpaths` checkout's
`.scratch/location-probe/result.json` and `boundary/result.json`; those directories
retain the maps, patched scripts, build checks and all 26 checkpoint files. Map
hashes are `8b43747554fc3a720265e98ae4267b32a8bfeb5b24dbfda7a72819d01550d659`
and `7e4b7cc508c8bf29809a7bcbf48ed895c944957f2a20a7a602ce9d17bd22781d`.

## Static evidence

The selected client's primary `GetUnitLoc` handler at `0x4977d0` calls the unit
resolver at `0x483940`. A null resolution returns null without allocating a
location; the live path copies the position and calls `0x4a3100`.
`GetLocationX/Y` at `0x493a50`/`0x493a70` resolve through `0x4823e0` and return
zero for null. `RemoveLocation` (`0x4a51a0`) and `MoveLocation` (`0x4a32f0`) check
for null before dereferencing the location. The latter no-op is established
statically; it was not included in the runtime probes.

## Recreation boundary

The tombstone retains the last accepted position and removal clock inline.
`GetUnitLoc` can allocate a snapshot while that clock is still current, then
returns null after clock advance. This is a recreation boundary consistent with
the observations above, not proof of identical native object lifetime. Other
removed-unit consumers retain their previous behavior. There is no new map,
allocation at removal, per-step cleanup pass or timeout constant.

Null location operations retain argument-count, type, finite-coordinate and
handle-family validation. A live location snapshot retains its own lifetime.
The case without an admitted game clock has no transient-location window.

The post-intro Human01 callback failure motivated this change. Exact callback
identity and unchanged rendered continuation require separate verification;
these diagnostic maps alone do not establish that the campaign now continues.

## Checks on the implementation

The private core suite passes 1,164 tests (33 ignored), strict all-target Clippy,
the WASM library check and the native app build. Synthetic regression checks
cover synchronous retention, retirement on clock advance without allocating a
null location, independent snapshot lifetime, wrong-family rejection and malformed
null-operation arguments.

The selected Human01 escape gate now injects the two authored footman destination
orders after the real intro-cancel trigger restores control. Over 256 further ticks,
the first footman's real EnterRect callback finishes and disables itself without
the old location refusal. The second callback remains enabled: its unit did not
reach the region in this bounded run. This is a focused callback regression with
explicit injected orders, not acceptance of both routes or campaign completion.

## Rendered continuation

A fresh native run of clean code commit `c6c396c` followed Main → Single Player →
Campaign → Human → Chapter One and skipped the intro through Escape. The rendered
world and HUD remained live beyond the former 10-second stop. F7 captures show
23.719, 59.719 and 99.859 seconds; a physical terrain right click moved selected
Arthas from the group to the cursor destination. The run log contains no
`GetUnitLoc`/wrong-family refusal. Its new-game seed was 10311066; this is separate
from the selected VM regression's controlled seed 1.

The binary SHA-256 is
`304945b65af5dfbe4ef7bfc358c3c20eb82ec922b88b1ae3e0961b98ee9f9f2f`.
The reviewed local receipt is `.scratch/menu-entry/location-next-run/run/reviewed-result.json`
in `review/menu-entry-20261022`; the owned app and display were stopped. The final
all-target Clippy check and native-app WASM check also pass with the added gate.

Sky illumination and cinematic-border inset diagnostics remain, as do HUD/quest
presentation gaps. This bounded entry/control observation establishes neither
mission completion nor original-client rendering parity. Browser campaign entry
for the supplied 1.27 ISO remains unsupported.
