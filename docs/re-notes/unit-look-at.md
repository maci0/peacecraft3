# Scripted unit look-at

## Evidence

Selected client: Legacy 1.29.2.9231 (`Warcraft III.exe`, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`).
Static objdump reads, not runtime captures:

- Primary `SetUnitLookAt` registration at `0x49d6c6` selects `0x4ab650`;
  the broad stub registrar uses `0xcbda30`. The primary handler resolves both
  units, resolves the supplied bone name through the model, supplies the selected
  bones to `0x967570`, and passes the target and three offsets to `0x69ddd0`.
- `0x69ddd0` retains a target reference at unit `+0x4d0`, stores the offsets at
  `+0x4c4..+0x4cc`, and installs a callback. Callback `0x6970a0` reads the target
  position and adds the three offsets component by component. Offsets therefore
  use world axes, rather than rotating with target facing.
- Primary `ResetUnitLookAt` registration at `0x49d6f1` selects `0x4a55e0`.
  It supplies an empty bone set and calls the unit reset helper `0x69bc90`.
- Unchanged Human03 calls `SetUnitLookAt(udg_Arthas, "bone_head", udg_Captain,
  0, 0, 80)` at script line 2606. The script supplies both the bone name and
  offsets; none is a production constant. Later calls change target and reset.

The supplied `wc3/*.exe.c` IDA exports were inspected earlier: they are launcher
exports that load Game.dll, not these selected engine bodies. Their addresses
and edition are not substituted for the selected executable.

## Recreation

Unit admission validates the declarations and their source. A set call validates
live unit handles, finite offsets, an admitted model and one matching authored
BONE name before changing the retained request. Missing or ambiguous bones refuse
with model context. Reset restores the ordinary sampled animation. Removing a
target clears references from remaining units.

The campaign actor resolves the target's current world position each frame using
the existing terrain and authored move-height transform, then adds the script's
world offsets. It converts that point through the looking actor's inverse world
transform. The existing skeletal composition is corrected about the selected
bone's animated pivot; descendants inherit the same correction and other branches
keep their sampled pose. Mesh skinning and lighting consume that corrected
skeleton. Other shared rig callers supply no aim. Bone-attached effect rendering
is not qualified by this change.

Immediate minimal rotation of the bone's current +X axis is a recreation choice.
Native bone-axis details, angular limits, blending/slew timing, exact target
height semantics and case matching remain unqualified. Multi-name/wildcard bone
selection is unsupported and refuses rather than inventing a selection. Synthetic
pose tests establish geometry, not rendered or original-client parity.

## Validation

Worktree-private target, stable Rust, `CARGO_BUILD_JOBS=1 --locked --offline`:

- `cargo test -p pcx-play`: 1,181 passed, 0 failed, 46 ignored. The campaign
  common/BJ/script/placement fixtures were explicitly supplied from local files.
- `cargo test -p pcx-gfx --lib mdx_look_at`: 2 passed. Tests check fixed pivots,
  descendants, actual skinned vertices, unchanged roots and malformed-input
  refusal. Host tests check retained offsets, atomic refusal, reset without
  animation restart and target removal.
- Strict Clippy for play, graphics and the app, all targets with `test-fixtures`:
  exit 0. This compiles the native app consumers and their fixtures.
- Selected-install startup/order/profile and patrol gates: 3 passed.
- `cargo check -p peacecraft3 --lib --bins --target wasm32-unknown-unknown`:
  exit 0, covering the play/graphics libraries and browser app consumers.

The ignored local longer-run diagnostic uses unchanged Human03 (SHA-256
`a62e36a1825a2edb0e2259d36db0c44fb50cb877a457014ed568168045559b82`),
seed 1, and the same initial player move as the preceding movement audit. At tick
257 it records Arthas handle 1430, model `units\human\Arthas\Arthas.mdx`, looking
at handle 744 with the script's `bone_head` and `[0,0,80]`. Execution next refuses
`IssueImmediateOrder` through `IssueImmediateOrderBJ` at Blizzard.j:7817, tick
980. The diagnostic exits 101 at that separate frontier; it does not finish its
4,096-tick window. No cinematic-skip input is sent. The temporary test source
was moved back to ignored scratch after the run. No whole-map, rendered or
original-client aiming parity claim follows from this headless result.
