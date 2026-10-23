# Nearby lumber order

Human02 initialization orders `udg_Lumber01` and `udg_Lumber02` to
`autoharvestlumber`; these refer to the authored player-nine peasants
`gg_unit_hpea_0045` and `gg_unit_hpea_0044`. The shipped TriggerData label is
`Harvest Nearby Lumber`. That label establishes an immediate worker order;
it does not establish an ability autocast toggle or a harvesting algorithm.

A controlled Human02-derived map ran on the reviewed Legacy 1.29.2.9231
executable, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Map SHA-256:
`4080d1e37859378f5ca25c20385f9a19d4b23faa3ad6d18d253b3014b78bab41`.
The staged map and script matched the build receipt. Terrain and map overrides
were retained, standard initialization ran, and authored units and initialization
triggers were omitted. The probe created those two peasants from their authored
creation statements; only the first received the diagnostic order.

All 16 records arrived. `VersionGet` reported `VERSION_REIGN_OF_CHAOS` under
the explicit `-classic` launch. Observations:

| Checkpoint | Ordered peasant | Unordered peasant |
| --- | --- | --- |
| 5.000 before order | order 0; position (-3538.000, -4775.750) | order 0; position (-3862.250, -4799.250) |
| 5.000 first and repeated order | both returned true; order 852018 | no order issued |
| 6.000 | order 852018; position (-3537.386, -4798.449) | order 0; unchanged position |
| 10.000 | order 852018; same position as 6.000 | order 0; unchanged position |

Player-nine gold and lumber were both zero before the order and at 10.000.
The numeric current order above is an observed diagnostic value, not a constant
to copy into dispatch; production order identifiers come from the admitted
client registry. The two peasants use different authored positions, so they are
not a matched test of local tree accessibility.

This qualifies order acceptance and changed order/motion, not tree selection,
work animation, damage, collection, carrying capacity, return behavior or a full
resource cycle. An inert accepted handler would lose observable behavior.
The existing unsupported boundary remains until that behavior is modeled.
Receipts, markers and launch arguments stay local in ignored
`.scratch/auto-harvest-probe`. The Wine/software reference does not qualify
native Windows or rendered parity.

## Controlled tree and deposit cycle

A separate diagnostic used the same reviewed executable with `-classic` and
reported `VERSION_REIGN_OF_CHAOS`. Map SHA-256:
`1304832fd9ad8260511a1ccd991c55ecf483530cfcdc182c580635e2ea10c486`.
Its original Human02 member set was retained except for the replacement script.
Before measurement, the script removed existing destructables in the test
region, created an owned town hall from the authored creation statement, and
created a worker/tree pair plus a separate-owner unordered control pair. It
configured a two-way passive alliance between the test owners and reset both
lumber banks to zero. These are diagnostic stimuli, not unchanged-campaign
acceptance.

The controlled tree was requested at (-3600, -4800) and read back at
(-3584, -4800), with maximum/current life 50. The ordered worker began at
(-3600, -4700); the town hall read back at (-3776, -4032), owned by player nine.
The order returned true. Sampling used the actual game clock, with a requested
0.1-second timer period and a bound of 1200 samples.

All 155 samples arrived through the completion checkpoint at 20.500:

| First observed checkpoint | Ordered worker/tree/bank |
| --- | --- |
| 5.000 | tree life 50, order 852018, lumber zero |
| 5.500 | tree life 49, worker still at its starting point |
| 15.438 | tree life 40 after ten one-point decrements |
| 16.000 | order changed to 852017; tree life 40, lumber zero |
| 18.500 | lumber became 10 while tree life remained 40; order was 852018 again; worker at (-3661.882, -4254.364) |

The control worker retained order zero and its initial position; its tree
remained at life 50 and its owner's lumber stayed zero throughout. This observes
a complete first harvest/deposit cycle while the measured tree remains alive,
contradicting a kill-required reward model. The measurements do not directly
expose a carried-lumber counter or prove arbitrary upgrade/partial-load rules.
The order numbers are diagnostic values; production resolves their identities
through the selected client registry.

The shipped ability metadata supplies independent field meanings: `Har1` maps
`Ahar.DataA` to `Damage to Tree`, `Har2` maps `DataB` to `Lumber Capacity`, and
`Har3` maps `DataC` to `Gold Capacity`. These labels alone do not specify transfer
timing. Exact work animation phase, nearest-tree/depot selection, interaction
distance and the intervals between sampled checkpoints remain separate fidelity
questions. Local source hashes, all records and the recovered launch receipt
are in ignored `.scratch/harvest-cycle-probe-v2`. The launch wrapper timed out
while its existing container continued initialization; no duplicate was started.

## Partial cargo and explicit return

Three separate Human02-derived diagnostic maps used the same controlled
worker, tree, depot and unordered control as the cycle above. Each ran on the
same reviewed executable with `-classic`, and each reported RoC. At game time
10.000 the tree had lost five life points, both resource counters remained
zero, and the worker had not moved. The script then issued Stop, repeated
`autoharvestlumber`, or moved toward the depot. At 10.250 it issued
`returnresources`. All submitted calls returned true.

| Intervening order | Effective order immediately afterward | First observed deposit | Lumber / gathered lumber |
| --- | --- | --- | --- |
| Stop | 0 | 12.813 | 5 / 5 |
| Repeated nearby harvest | 852018 | 12.813 | 5 / 5 |
| Move | 851986 | 12.625 | 5 / 5 |

Each explicit return reported current order 852020 immediately after the call.
At deposit the worker's order was zero; the tree still had life 45/50. The
unordered worker, its tree and its owner's bank remained unchanged. This
qualifies retained partial cargo across these three interruptions and a manual
return that deposits and ends idle. It does not directly expose the cargo
counter, qualify other interruptions, or establish death/removal behavior.
The earlier automatic capacity return instead reported 852017 and resumed
harvesting after deposit.

An independent decode of the selected image's order-registry initializer
(`0xb11550..0xb12c75`) distinguishes the names:

| Registry name | Selected image identifier |
| --- | ---: |
| `harvest` | 852018 |
| `autoharvestlumber` | 852022 |
| `resumeharvesting` | 852017 |
| `returnresources` | 852020 |

Thus the submitted nearby-harvest command and the effective work order differ;
automatic and explicit return also differ. These are evidence values, not
production constants. Dispatch and current-order reporting must resolve their
separate identities through the admitted registry.

The stop, reissue and move map SHA-256 values respectively are
`02c03fb6f69f6258d68ea73184d800e9384ca5a562de1679a22188aee1a41785`,
`bd493185bcdab36869c90d2660961f2d93248442a692f5d59b4aa2cf4bc629eb`,
and `faa0e20355dd162af73af256581aa1d4cae6c1d0a0f5a8a246b697c22cad4320`.
All script parse/resolve and packed-member byte checks passed. The bounded
collector validated all phase records and 98, 98 and 96 periodic samples;
its six synthetic positive/refusal tests passed. Raw markers, hashes, scripts,
launch receipts and results remain local in ignored
`.scratch/partial-lumber-{stop,reissue,move}-probe/`; the registry receipt is
`.scratch/harvest-order-source.json`. These diagnostic runs do not qualify
rendered presentation, native Windows, or unchanged Human02 startup.

## Upgrade during an active load

A separate RoC run changed `Rhlh` research while the same worker was already
chopping. At game time 10.000 the tree had life 45/50, the worker retained its
effective harvest order, both own counters were zero, and own research read
zero. `SetPlayerTechResearched` changed the readback to one in that same
checkpoint without reissuing the order. The first observed deposit at 24.000
credited 15 lumber and 15 gathered lumber; the tree had life 35/50 and the
worker had resumed harvesting. The unordered different-owner control retained
its position/order, tree life and zero counters throughout.

The independently extracted `Custom_V0` rows give `Ahar.DataB1` capacity 10
and `Rhlh` effect `rlum` with base 5, modifier 5 and maximum level 2. Metadata
identifies the capacity and lumber-effect fields. The observed level-one
deposit matches base capacity plus the effect base, rather than additionally
applying its modifier at level one. This qualifies a capacity increase during
an already active partial load in this configuration. It does not establish
all upgrade formulas, reductions or other map-edited fields. Production values
must come from the selected tables and applicable overlays.

### First-level capacity selector boundary

Matched `Rhlh` runs distinguish the `gba1` (`base1`) selector at applied levels
one and two. `Ahar.DataB1` was 10, the `rlum` modifier stayed 5, the maximum
level was 2, and the edited run set only the `rlum` base (`gba1`) to 9.
First-deposit lumber was:

| level | unedited | `gba1 = 9`, `[0,0]` | `gba1 = 9`, `[1,0]` |
| --- | --- | --- | --- |
| 1 | 15 | 19 | 15 |
| 2 | 20 | 24 | 20 |

At the first credit the tree life was 35/31/35 at level one and 30/26/30 at
level two. The neutral `[0,0]` base edit therefore persists at level two,
while a first selector word of `1` was ineffective at both tested levels for
this `base1` field.

The runtime admits a scalar `base*`/`mod*` override only with selectors
`[0,0]`, and refuses unqualified or other selectors contextually rather than
guessing a level. For this `Rhlh` effect it derives the value kind from the
mapped `UpgradeMetaData` row (`gba1` is `unreal`), requires the override wire
type to match, and requires a finite integral value. This evidence covers only
`Rhlh`'s `rlum` base edit to 9 through level two; it does not establish selector
behavior for other fields, higher levels or partial-load amounts.
Raw runs and the selector audit remain local under ignored
`.scratch/w3q-level2-runs/` and `.scratch/w3q-original-runs/`.

Map SHA-256 is
`e8cc4e6fd9b405665332f4c2ebd577701f2f26a9d1694abef17a7d938928a56f`;
script SHA-256 is
`e5940c37d24c6450556e86e11a315b1138f6c7839f18e84f2967c5edc8419352`.
Only the original Human02 script member changed. Parse/resolve, all packed
member byte checks and five collector tests passed. All 96 marker files
validated, including 84 periodic checkpoints. Raw markers and exact selected
archive/member hashes remain local in `.scratch/harvest-active-upgrade-probe/`.
This is a diagnostic resource-cycle observation, not unchanged chapter or
presentation acceptance.

## Edited tree damage and capacity boundary

A separate diagnostic on the same reviewed client and RoC selection changed
only `Ahar`'s `Har1` damage field to the experimental integer 7. Its W3A
field and Data pointer were checked against the selected metadata. Capacity
remained the selected value 10 and `Rhlh` research read zero throughout.
The effective override is witnessed by tree life 50 → 43 at 5.500 seconds,
then 43 → 36 at 6.625. The first observed deposit at 9.750 credited 10 lumber
and 10 gathered lumber. The separate-owner control retained its position,
zero order, tree life 50 and zero bank/gathered counters.

Thus the final swing in this case still damages the tree by the full edited
amount even though only three units of carry capacity remain. The observation
supports the recreation's separate full tree hit and capacity-limited cargo
update. It does not establish all edited damage amounts, fractional amounts,
tree-depletion yields or upgrade combinations. Existing synthetic capacity-hit
tests also distinguish full hits from clipping tree damage to remaining cargo.

All 77 marker files validated, including 67 periodic checkpoints. Map SHA-256:
`f2e3938686947684209168c48ce41e5624fdfa362183bff8b69052ac80bac9d3`;
script SHA-256:
`a2b53a98b4de3a85e0e67d360ec74c5063b19a81eb6c96c7625fc0a7af818fda`.
The complete ability-file framing passed `pcx_objects::ObjectFile::parse`,
and all packed members round-tripped. The collector's complete positive,
unqualified and malformed synthetic cases passed. Raw markers, selected source
hashes and the stopped reference run remain local under
`.scratch/harvest-tree-damage-probe/r4/` and run `hdamage-261023s`.
Earlier drafts had incomplete ability-file framing or collector defects;
the exited r3 run supplied no gameplay qualification and remains preserved.

## Model presentation evidence

The selected `UnitUI.slk` binds `hpea.file` to
`units\human\Peasant\Peasant`. The corresponding binary model has SHA-256
`34fd116662fd076e538bd9c8ea65895ed72a0e593536b84d3994012575842b90`.
An offline read through `pcx-mdl` retains its exact sequence intervals, loop
flags and EVTS tracks in ignored `.scratch/mdx-peasant-probe/peasant.json`.
The parser already exposes `Mdlx::event_nodes`; no new binary parser is needed.

The model contains distinct ordinary, Gold and Lumber variants of Attack,
Walk and Stand Work. Its `SNDXAREP` event occurs within the three Stand Work
intervals. This establishes authored event placement, not the worker order
that selects a sequence. In particular, the presence of a Lumber suffix does
not establish that an empty worker moving toward a tree uses that variant.
Order-to-sequence selection, carry-state transitions and sound playback remain
unqualified; the measured tree damage/deposit cycle does not qualify those
presentation details.

The retained SoundInfo rows resolve `SNDXAREP` through its `AREP` payload:
`UI\SoundInfo\AnimLookups.slk` maps `AnimSoundEvent` to the Repair label,
and `AnimSounds.slk` resolves that label to the repair sound files. The same
chain resolves `DPES` to PeasantDeath. Thus the Stand Work event is sourced
Repair audio; its presence is not evidence that harvesting uses that sequence.
The weapon's `AxeMediumChop` binding has a separate Wood combat-sound row in
`UnitCombatSounds.slk`. Playback selection and timing still need a consumer
and qualification. FPT, SPL and SPN payloads were not found in the inspected
SoundInfo members; that bounded search does not establish that their mappings
are absent from all shipped data.

Exact archive/member identities, member hashes and matching parsed rows stay
local in `.scratch/mdx-peasant-probe/peasant-sound-receipt.json`, SHA-256
`cba4a485881d9aa5139db9a152bff49c861f6bab9a3d419e093292c1e234795b`.

## Placed-tree life byte

The runtime host admits placed trees from `war3map.doo` as destructable handles
with shared life pools (`admit_placed_destructables`, `PlacedDestructable`); the
collision raster and the handle life are updated together on damage and death.
An implementation that scanned only script-created destructables would miss the
campaign's lumber order, so the runtime scans both admitted placed and
script-created destructables.

A controlled RoC run used five original Human02 `LTlt` records, preserving
their positions, variations, flags, per-axis scales and unique creation IDs,
and varying only the stored life byte. There were no script-created trees.
All 12 conditional markers validated, with one matching placed handle per
rectangle and max life 50 from the selected destructable row:

| Stored life byte | Observed current life |
|---|---:|
| 0 | 0 |
| 25 | 12.5 |
| 50 | 25 |
| 100 | 50 |
| 255 | 50 |

This supports a percentage interpretation through 100, retaining a dead placed
object at zero life. The 255 observation does not distinguish saturation from
a special full-life sentinel; intermediate bytes above 100 were not measured.
Maximum HP remains a source-table/typed-map-override value, not a copied 50.

Map SHA-256 is
`94d6c9b87054b07962201bc963622eedd5b68ba3855954964ea21112fb69f5b2`;
script SHA-256 is
`b8f3da220b4a18185c8e2036d0b1199a4c1e8fe102184623481e68eff237daa1`;
diagnostic doo SHA-256 is
`df25a3b9e0cd9a20ce41972dd5154b6d71997ae1635494a8809cec2c31dd0f97`.
Receipts remain local at the repository root's
`.scratch/placed-tree-life-probe/`. This is life-state evidence, not a completed
harvesting implementation or a rendered comparison. Tree death must update
derived pathing without erasing overlapping objects or terrain restrictions.

## Runtime model

`harvest_runtime` installs only when the selected order registry resolves all
four authored identities: `autoharvestlumber` starts a worker, `harvest` is
reported while chopping, `resumeharvesting` is reported while an automatic
capacity return runs, and `returnresources` is reported during an explicit
return. Unit activity must also admit a movement order. The runtime keeps one
worker record per unit; tree, depot, timing, damage, capacity and reach come
from the map-selected unit definition, `Ahar`, the worker's tree weapon slot,
`Artn` and the destructable catalog. Missing capability, an unusable tree set,
a missing depot or malformed authored values produce a contextual error; the
runtime does not substitute stock numbers.

Selection is bounded by `Ahar.Area` and deterministic: nearest candidate wins,
and equal squared distance breaks on the lower handle. Tree reach is
`Ahar.Rng`; the tree weapon's own range does not extend it. Depot reach is the
depot's admitted collision radius plus `Ahar.Rng`; it is not a copied world
distance.

Work timing requires the authored tree weapon's `cool`, `dmgpt` and `Backsw`.
A unit without an admitted tree-targeting weapon is refused before tree
selection; `Ahar.Dur` is not a timing fallback. Each swing applies
`Ahar.DataA` damage to the tree and adds the same amount to carried lumber,
capped at `Ahar.DataB`. A dead tree with at least one whole lumber carried is
deposited even when below capacity. Sub-unit carry is not deposited; fractional
damage/yield behavior remains unqualified. The edited-damage observation above
qualifies a full final hit past the remaining capacity for its tested values;
depositing incomplete carry on tree death remains a recreation choice.
The integer damage cell is validated as a signed 32-bit value before conversion
to the runtime float; fractional or out-of-range source text refuses with its
resolved ability alias and column instead of creating fractional cargo.

The worker's selected `upgradeList` admits its authored upgrades. For each
upgrade with an applied player level, the runtime resolves the active object,
including a map `war3map.w3q` custom upgrade and its base, then reads
`UpgradeData` effect slots. The active `rlum` (lumber capacity) contributes
`base + mod * (level - 1)` to `Ahar.DataB`. The applied level is re-read on
every tick, so a research completed mid-cycle changes capacity before the next
swing. A preserved dynamic-refresh observation is in ignored
`.scratch/harvest-active-upgrade-probe/result.json`: a worker already chopping
with partial cargo received a mid-cycle `Rhlh` level without a harvest reissue,
and its first deposit and tree damage reflected the selected upgrade. An
inactive upgrade leaves capacity unchanged; a malformed effect, an
unsupported selector, or a level beyond the authored maximum is a contextual
refusal. Combat, armor and movement effects are not harvest inputs and remain
unit-runtime responsibilities.

Deposits prepare the player's lumber bank and `PLAYER_STATE_LUMBER_GATHERED`
rows before writing either value or firing player-state events. Both prepared
rows are written before their events so a callback observes a coherent bank and
counter. Stop, hold-position and non-harvest orders cancel the worker record but
keep its partial carry attached to the unit. Reissuing `autoharvestlumber`
resumes with that carry; an explicit `returnresources` routes it to a qualified
depot, deposits the preserved amount and ends idle, while an automatic capacity
return reports `resumeharvesting` and resumes harvesting after the deposit. A
worker with no remaining reachable tree stops searching. Unit death currently
retains cargo, while unit removal discards it and retires the worker record.
Cargo-retention evidence covers only Stop, Move and reissue, so death retention
and removal discard are unqualified recreation choices.
Hiding currently retires the active worker while retaining its partial cargo,
so a later reveal and explicit return cannot silently lose already earned
lumber. A synthetic hide/reveal/return regression covers that invariant; native
hide/resume order and cargo behavior remain unqualified.

A synthetic complete chop/deposit cycle registers both player-state events and
executes their actions in the same script VM. Both callbacks see the complete
bank and gathered credit, in bank-then-gathered delivery order, and the next tick
does not duplicate the events. This verifies the engine-to-script delivery seam;
it does not establish original-client callback timing.

Tree deaths caused by an engine-tick swing reserve and push their registered
death events into the delivery queue at the tick boundary; they do not run
synchronously inside `SetLife` as the destructable native evidence indicates
for direct mutation. Nested `TriggerExecute` behavior across that boundary and
exact nested native ordering remain unqualified here. The work phase enters
`attack` as a recreation presentation choice; no native order-to-animation
selection or sequence start was observed to establish that choice, and the
runtime refuses a unit whose admitted model lacks `attack`. Work and carry
variant selection, animation phase and harvest audio fidelity are unqualified;
dedicated carry animation selection is not implemented by this runtime.
