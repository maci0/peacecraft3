# Original seeded RNG probe

Current implementation: [explicit seeded host and factory](#explicit-seeded-production-host-2026-10-18). Human01 initial-state qualification and full consumer parity remain open.

Two fresh isolated Wine runs loaded the same private, intentionally modified
map and emitted identical measured results. This establishes a working seed
experiment and output channel; it does not establish recreation RNG parity.

The selected executable is `Warcraft III.exe`, VERSIONINFO `1.29.2.9231`,
SHA-256 `3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Each run uses its own copied installation, Wine prefix, profile and framebuffer
at 1280×960 and 96 DPI. Wine/software Mesa is not native Windows ground truth.
The selected source map's release is not inferred from the adjacent executable.

## Experiment and observations

The local map preparation preserves every retained original member except its
script. Four reversible script edits add self-authored probe functions and call
them before the original `main` setup. The fresh unsigned archive omits optional
attributes/signature metadata rather than presenting stale checksums as valid.
All 15 retained members were independently re-extracted; the original map and
installation remain unchanged. Script resolution and five synthetic preparation
tests pass. This modified experiment is not unmodified-map acceptance.

Each measured block calls the declared `SetRandomSeed`, captures eight ordered
`GetRandomInt` results, two `GetRandomReal` results, then two more integers.
Caller-selected seeds and bounds are synthetic experiment inputs, not defaults.
Four blocks use seed A, A, B, A. Results are stored before formatting/export.

Both actual runs show:

- All ten integer results match across the three A blocks.
- The B block differs from A; returning to A restores its measured sequence.
- The four records match across both fresh runs, including formatted real text.
- The generated output file is byte-identical: 759 bytes, SHA-256
  `633a85c6c282e14f99ae29f163e893fd52fd9cbd3c6ede67f5f775ebe7eba17e`.
- The records also appear visibly in the running original game. Captures were
  inspected; no generated preload script was executed to read the results.

The output was absent before each launch. Each run produced exactly one matching
file under its private Documents/Warcraft III/CustomMapData directory. Original
UI navigation, launch/source/map digests, generated records and captures remain
local under `.scratch/original-game-vm/rng-probe-{1,2}/`. The preparation and
cross-run receipts are in `.scratch/original-rng-map-probe-20261016/`.
No original map, script, generated value sequence or capture is distributed.

## What remains open

`R2S` provides formatted real observations, not bit-exact floating-point results.
This experiment neither identifies the generator nor observes all consuming
subsystems. It controls one serial block after entering `main`, not every random
choice made during configuration or loading. Equal probe output does not prove
equal world state, draw order, simulation clocks or animation phase.

The recreation's JASS gameplay Host still lacks these random natives. Its
synthetic simulation and menu-particle RNGs are separate implementation choices.
The paired UI replay therefore continues to reject a non-null seed instead of
claiming a launcher flag or a shared applied seed. The next step is an evidenced
recreation RNG contract and an actual paired map execution checkpoint. RE-Q6 and
RE-Q11 remain open; two fresh runs do not meet the 100-run baseline gate.

## Selected primary generator: static and retained-output checks

A later root review binds the three JASS random names to their registration
sites in the same SHA-256-qualified executable above. The candidate handlers
are `GetRandomInt` at VA `0x494e40`, `GetRandomReal` at `0x494e80` and
`SetRandomSeed` at `0x4a99a0`. These addresses describe this input image only;
they are not production lookup constants or runtime observations. The second
registration family points all three names at one common stub and is not used
as evidence of their gameplay implementation.

The integer and real wrappers both call the primary draw helper at `0x480dd0`.
The seed wrapper calls `0x9892a0`, takes one primary draw, then transfers to a
separate routine at `0x69a0d0`. That last routine initializes additional state;
it has not been reconstructed as a complete set of consumers. The integer
wrapper has an equal-bound return before the draw. The real wrapper also has a
pre-draw near-equal branch and calls custom integer-based arithmetic helpers.
Ordinary host floating-point interpolation is therefore not established as a
bit-exact substitute.

A bounded offline interpreter executes the selected seed and primary-helper
instructions directly against the selected image's table bytes. It does not
copy the table or seed-mixing values into a production implementation. Three
self-authored checks cover multiplication/high-word handling, rotation and the
stack/return contract. The serial probe inputs come from the retained private
experiment configuration. All **80 integer observations in eight blocks across
the two original runs match**, including the two integers after the real calls.
Only the two caller-selected seeds are covered.
Those post-real checks support the inferred shared primary-stream consumption
for these nondegenerate bounds. They do not compare the numerical real outputs.
The interpreter refuses unknown instructions and bounds execution work. This is
offline selected-code emulation against earlier original output, not a new
original-client run or validation across all seeds and argument shapes.

The retained unchanged Human01 execution at `c2bf777` reaches one attempted
`GetRandomReal`, map span `186558..186584`. Its recorded JASS prefix has no
successful random native and no `SetRandomSeed` call before that frontier. This
is script-level ordering only; it does not expose engine-internal random users
before globals, `config` or `main`. The original initial generator state remains
unknown. Reusing the separate synthetic `pcx_sim::Rng` would not answer that gap.

The complete local records, selected-input fingerprints, decoder windows,
interpreter checks and startup-prefix receipt are in
`.scratch/script-rng-root-20261017/`. Generic literal-reference searches hit their
explicit caps; no complete alias/caller graph is claimed. No original payload,
value sequence or extracted table is distributed.

The next production acceptance needs a caller-qualified initial state, the real
arithmetic and degeneracy rules, secondary-state effects and a checked ordered
consumer. Native all-consumer behavior, paired recreation execution, the
100-consecutive baseline and crossplay parity remain open. `GetRandomReal` stays
unsupported on the production Host; no guessed seed or algorithm was installed.

## Selected real-wrapper arithmetic follow-up

The same fingerprinted image's real wrapper has now been followed through its
constant initializer, integer arithmetic helpers and primary draw. A bounded
offline interpreter executes those selected instructions rather than replacing
them with an assumed float formula. The initializer is read from the image;
its runtime constant is not taken from an uninitialized file-backed location.
The interpreter accepts only its implemented instruction subset, bounds work
and call depth, restricts memory writes and refuses unknown instructions or
unmapped reads. Six self-authored checks cover arithmetic, stack/return,
rotation, signed comparison and zero-input bit scanning.

The complete serial wrapper check preserves all 80 integer checkpoints and
matches all **16 formatted real observations** across the retained two runs and
eight seed blocks. Original real output is still only `R2S` text. Locally derived
result bits are emulation results, not newly captured original float bits.
Only the two original caller-selected seeds and their probe bounds are covered
by this runtime-observation comparison.

A separate check derives its bounds from the qualified Human01 frontier receipt
and uses 64 synthetic seeds. Ordinary round-to-nearest `f32` multiplication
followed by addition differs from the selected wrapper in **32 cases**. The
reference uses the same source-derived random fraction. This is a selected-code
counterexample to substituting ordinary float interpolation; it is not a new
original-client observation or proof for every bound/seed shape.

Eleven additional synthetic checks exercise equal integer/real bounds,
adjacent real bounds, the loaded tolerance boundary, and interpreter refusal
for unknown instructions, unbounded loops and unmapped reads. On these selected
paths equal/near-equal bounds preserve the primary state, while the tolerance
boundary consumes a draw. NaNs, infinities, denormals, all signed-zero cases and
native exceptional-input behavior remain outside this evidence.

The selected primary-owner constructor prefix forwards a caller argument to
the seed helper. That static path supplies no justified constant for Human01's
initial state. Its later engine consumption and secondary streams still need
an explicit caller-qualified checkpoint. The production Host therefore still
refuses `GetRandomReal`; neither a guessed seed nor the synthetic simulation RNG
has been substituted.

Private source/input fingerprints, complete decoder windows, preserved checks
and their actual exit receipts are in
`.scratch/script-rng-real-root-20261017/`. These results extend the selected-code
model. They do not close bit-exact original real outputs, paired recreation
execution, full campaign behavior or the 100-consecutive original baseline.
No new original-client, rendered gameplay, public-site or production build was
run for this documentation-only update.

## Explicit seeded production host (2026-10-18)

`ObjectHost::with_script_random` now admits the three exact, non-constant
`common.j` declarations together. The startup factory accepts an optional
`RandomInput` containing a shared source policy and an explicit secondary-state
allocation ceiling. Omission keeps those natives unsupported. Admission supplies
no initial seed: an executed `SetRandomSeed` creates the primary state, discards
the source-prescribed warmup draw, and initializes a retained secondary block.
An ordinary draw before that call refuses. Equal integer bounds and real bounds
strictly inside the loaded tolerance preserve the source's early return.

The policy reader uses twelve caller-reviewed code windows in one complete,
fingerprinted local PE. It derives the packing relations, lane periods, index
adjustments, rotations, indexed table span, real mask/base, initializer operand,
loaded tolerance and secondary count. It checks initialized readable/executable
ranges, selection budgets, window identity and overlap, shared primary-pointer
references, direct-call targets, and agreement between the standalone and
inlined secondary draw. Arithmetic-window checks recognize critical shapes;
the caller must still establish their reviewed roles. A digest establishes byte
identity, not semantic proof or automatic discovery of another executable.
No original address, table or output sequence is a production lookup constant.

The portable real helper preserves the selected integer arithmetic rather than
host round-to-nearest interpolation. Its admitted domain is finite normal
operands and signed zero, with unsupported intermediate/result classes refusing.
NaNs, infinities and denormals have no invented fallback. Refused calls leave the
retained state unchanged. Reversed integer bounds keep the original lower
argument as the anchor, and full signed-span reduction retains the selected
wrapping behavior and draw consumption. These are compatibility behaviors,
not a newly specified general-purpose random API.

The retained secondary block is a recreation-owned representation. Offline
comparison executes the selected loop with a synthetic non-null owner pointer;
it does not establish original owner availability or connect secondary streams
to game consumers. State belongs to the actual object host, survives `config`
into `main`, and is isolated from replacement candidates. A failed replacement
preserves the published VM/state and its witness; a successful replacement
retires it and starts without an inherited seed. No handle arena is added.

Validation runs 443 unit cases on native and actual WASI, with six unrelated
explicit local-asset gates ignored. Four explicit local RNG gates run on both:
1,815 selected-code arithmetic cases; a twelve-role profile with refusal
mutants; 2,842 host state/result checkpoints across seven synthetic seeds; and
all 80 integer plus 16 formatted real observations from the retained two
original runs. The local checkpoint inputs, including the synthetic secondary
owner setup, remain private. Formatted observations do not establish original
real bits. Native map loading passes 103 cases, and the explicit factory gate
passes separately. Strict native/browser all-target Clippy and formatting pass.

Actual Chromium also executes four publication/replacement cases from the Cargo
WebAssembly factory artifact with the local selected image. This is execution of
the production factory, not a rendered-game or public-site package comparison.
Private commands, exit receipts, source hashes and reference provenance are in
`.scratch/script-rng-implementation-root-20261018/`.

Human01's first random call still has no qualified initial engine state or
preceding engine-consumer history. This change therefore does not silently seed
that unchanged map. Automatic source-profile selection, secondary game users,
exceptional native real behavior, original bit-exact real observations, ordered
all-consumer playback, paired gameplay and the 100-consecutive original baseline
remain open. No new original runtime run was needed for this implementation.

## Unmodified Human01 startup checkpoints (2026-10-18)

A bounded debugger observation of the selected 1.29b executable now captures
four real-return bit patterns and their primary pre/post states while the
unchanged Human01 map loads. The first three calls have the argument pair from
the retained recreation frontier. Their bridge backtraces are retained, but an
argument match alone does not identify an exact runtime JASS source span. The
checkpoint image still shows the chapter loading screen; this is not a rendered
gameplay or chapter-completion result.

The frontend primary state matches the constructor's source-derived seed
packing, without the script seed setter's warmup. Starting the selected map
replaces the owner twice, then an engine caller invokes the primary seed helper.
Static decoding of that observed caller shows a branch between a loaded seed
and an instruction immediate, followed by secondary initialization. The observed
run takes the loaded-seed path. No JASS seed setter or integer wrapper is recorded
before these four real calls. That does not mean the engine has made no draws.

After the reseed, 37 primary draw writes precede the first recorded real entry.
Executing the selected seed/draw instructions reproduces those primary states,
then each captured real result and post-state exactly. The writer stops occur
inside the draw helper, so their saved stack words are not function-entry
arguments or caller return addresses. The consuming engine calls still need
qualification. In particular, 37 is an observed history for this run, not a
universal warmup count or a production initialization rule.

The existing production `ObjectHost` checkpoint test also passes on native and
actual WASI with this private history. Its reference retains the previous 2,842
selected-code checkpoints and adds 41 replay rows: a synthetic explicit seed
call sharing the first observed primary draw, 36 synthetic integer advances
checked against the remaining observed draw states, and the four native real
results and post-states. These advances reproduce primary state; they do not
claim that the original called the integer wrapper. Replay secondary setup is
synthetic and does not establish the original secondary owners or state.

The debugger fixture first checks pointer arguments, state writes, returned
register bits and clean detach. Runtime observations verify the complete image
identity, loaded image span and selected entry bytes before arming traps. Earlier
failed attempts, commands, bounded logs, unchanged-map provenance, source windows,
captures and native/WASI exit receipts remain local in
`.scratch/script-rng-initial-state-root-20261018/`. The successful observation
detaches cleanly and its isolated container is deliberately stopped and preserved.
It runs under Wine/Xvfb/software Mesa and is not native Windows ground truth;
debugger intrusion and incomplete thread/watch coverage remain limitations.

This closes the absence of original real bits for these four checkpoints only.
It changes no production initialization behavior and commits no original seed,
state tuple, table, output sequence or asset. Engine seed provenance, complete
consumer ordering, secondary consumers, repeated unmodified runs, native Windows
comparison, paired gameplay and the 100-consecutive original baseline remain
open. The public site was not rebuilt.

## Startup consumer identities (2026-10-18)

A second fresh, unchanged Human01 run records the actual return-address and
argument slots from the draw helper's frame. Every captured draw call is checked
against its direct-call bytes in the selected image, and one backtrace is retained
per distinct caller. This run has a different engine seed from the preceding run.
Its four real results and primary states also match the selected-code reference
and the existing production-host replay on native and actual WASI. There are now
eight raw real checkpoints across these two runs, not eight repeated observations
of one seeded sequence.

The 37 draws before the first recorded real entry split into two caller paths:

- Nineteen come from the player-slot race-resolution loop. Its preference and
  resolved-race fields match the registered `SetPlayerRacePreference` and
  `GetPlayerRace` wrappers, and its random branch matches the explicit local
  `common.j` random-race flag. Observed slot indices are retained privately.
- Eighteen use a fraction/range helper reached through the registered
  `CreateUnit` wrapper. The captured callback and virtual-method chain reaches
  a method whose selected-image RTTI identifies `CAbilityWander`. The method has
  a conditional early return; its caller also tests unit/component state before
  reaching it. The retained recreation prefix executes 155 `CreateUnit` calls,
  so one draw per created unit would not reproduce this observed history.

Static analysis also follows the engine seed's local accumulator: initialization
to zero, conditional combination with decoded per-slot values, and rotation before
the later primary reseed. The semantic origin of those values remains unresolved.
Neither a captured seed nor the 37-draw total becomes a production default.

Private evidence, RTTI relations, bounded source windows, unchanged-map hashes,
caller frames, captures and successful native/WASI replay receipts are in
`.scratch/script-rng-consumer-order-root-20261018/`. The debugger detaches cleanly;
the isolated container is stopped and preserved. These remain intrusive Wine
observations, with no native Windows, complete thread coverage or gameplay claim.
The next integration needs the source-bound race inputs and the executed units'
ability associations and branch conditions. A generic startup skip or an
unconditional `CreateUnit` consumption rule would discard those relationships.
