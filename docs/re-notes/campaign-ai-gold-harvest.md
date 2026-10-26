# Campaign gold harvesting

`HarvestGold(integer town, integer peons)` must assign real workers to a real
mine, preserve its resource amount, and credit gold when workers return to a
depot. Accepting the call or crediting a bank immediately would omit observable
behavior. The campaign call now uses the existing harvesting runtime and
canonical units, mines, depots, movement, resource counters and death events.

## Source evidence

The selected 1.29.2.9231 executable SHA-256 is
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Registration `0xab63cd` binds `HarvestGold` to `0xab57b0`. The handler returns
for nonpositive worker counts; otherwise it calls `0xabf110` with the town,
count and gold selector. That helper checks the town index and associated
resource/depot references before reusing prior assignments or assigning more
workers. It is separate from the harvest-cycle rollover.

Retained REA/Ghidra observations:

- Registration: `ev_226ec85bd4e23ba7632813733bd5fb85dde3aa83a600e2453afb63e7f2e756dd`.
- Handler: `ev_99374b22a4f8376f55468ec5d5c5fa87a6c07796ead58dc560b00b64289f0189`.
- Assignment helper: `ev_b2985462f74e8a36aa78ad548d1d9021e162bc1addd189b1e0b532ccbd084443`.

These are bounded static reads, not execution or a requirement to copy the
native linked lists. The owned analysis session is closed. The selected
`Custom_V0\Units\AbilityData.slk` gives the mine's `Agld` row; its SHA-256 is
`ddc360800784f808219bbc2518b16b0f12413082aada2755cc00944adcc31cec`.
The selected `Units\AbilityMetaData.slk` SHA-256 is
`ddff3e0e247928f647a54671e9d2929bf69e73c4ed8ae90c9ffe1eb70e12f986`.
Metadata and the selected editor strings identify `Gld2`/`DataB` as mining
duration, `Gld3`/`DataC` as mining capacity, and `Har3`/`Ahar.DataC` as worker
gold capacity. Mine resources, worker cargo, and simultaneous mining capacity
are distinct values.

`GoldMineRules` now reads the selected ability's duration and worker capacity
through the existing metadata/typed-override path. Custom ability aliases
resolve through their base records. Missing, ambiguous, malformed, nonpositive
or incorrectly typed inputs refuse. It supplies no fallback timing or capacity
and supplies the accepted mining task. Synthetic tests discriminate nonstock
values and overrides; an explicit original-table test reads the locally
selected source files.

## Matched original-client observation

Two stopped, isolated Wine reference runs used the same Human02-derived map,
authored worker/mine/town-hall positions, seed and diagnostic timing. One AI
requested `HarvestGold(0,1)`, the other `HarvestGold(0,0)`. Both ran `InitAI`,
`CreateCaptains` and `ClearHarvestAI` first; a resource-bank handshake qualified
the return from the harvest call before sampling began. Both launched with
`-classic`; `VersionGet` was not sampled, so no edition readback is claimed.

Both map/AI scripts parsed and resolved, all packed members round-tripped, and
normalized sources differed only in requested count and diagnostic identity.
The collector validated all 106 files per arm, finite values, canonical
identities, unchanged separate-owner control, acknowledgment ordering, 100
consecutive samples and a terminal record equal to the last sample. Its
synthetic tests reject altered identities, wrappers, counts, chronology,
non-finite values and inconsistent terminal/control records.

| Observation | One worker | Zero workers |
| --- | --- | --- |
| Worker order/motion | harvest, mine entry, depot return and repeated work | idle and stationary |
| First observed hidden interval | 6.813 through 7.563 seconds | none |
| First observed mine debit | 10 at 7.813 seconds, when the worker was visible again | none |
| First observed bank/gathered credit | 10/10 at 10.813 seconds | none |
| End at 30.063 seconds | gold 40, gathered gold 40, mine 19,960 | gold 0, gathered gold 0, mine 20,000 |

These are sampled observations, not constants for production timing, range or
transfer size. The unordered separate-owner control stayed unchanged in both
arms. Positive/zero map SHA-256 values are respectively
`9e1049ef4da1bdbe9a2de00ac3d9b8ed4fd5b59180040b79e715ecd48c1a5728` and
`33df67c3fd65d2a09517e2dba20b7cbd96a6875dcff6e5b8b87cd42a0a5a013c`.
The stopped result SHA-256 values are
`18045ab4afe6707156207fdfde923fa416b2a0751f26adf40fa69259e3b6effe` and
`714887c28ebac056d40fd2f906e3b5e68111aab8025dc7810c945405e04b21d5`.
All source files, probes, raw records and receipts remain local under ignored
`.scratch/harvest-gold/`.

## Final-load observation

A separate stopped RoC run retained the same AI and started the mine with seven
gold. At 7.813 seconds the worker emerged, mine resources and life became zero,
and one registered death event fired. At 10.813 seconds the depot credited seven
gold and gathered gold; the worker remained visible and idle through 30.063
seconds. The separate-owner control stayed unchanged. This qualifies a partial
final load and the mine death event, without turning these sample times into
game rules. `VersionGet` reported RoC in this run.

The map SHA-256 is
`956e6bbc43f284b1cfd845cfeb0129548a03ffc05b83e7288c87e108dbfa03e9`;
the stopped result SHA-256 is
`cd722f5366e252dbeb9ce5055548a15e1910fbe966cdb17d2b873992e4ce89d9`.
The AI bytes match the earlier positive arm. Raw records and receipts remain
local under ignored `.scratch/harvest-gold/exhaustion/`.

## Runtime behavior and limits

The source-bound AI handler checks owned declarations and canonical active
players before assignment. Nonpositive counts and out-of-range town indices
preserve the selected handler's no-op behavior. Positive assignments reuse
current/prior canonical worker tasks, preflight new routes and capacity, and
preserve other players' assignments. Creation order supplies town slots and
nearest compatible mine selection is a recreation choice; native internal
lists are not copied.

An `Ahar` worker travels to an `Agld` mine, waits for its source-derived capacity,
hides during its source-derived mining duration, withdraws integer cargo on
exit, and credits both bank and gathered gold at an `Artn` gold depot. Mine
exhaustion uses the existing unit death/event path. Normal combat steering does
not overwrite mining; accepted retreat cancels the task while preserving cargo.
Units without harvest capability reject a gold order without mutation.
Stop and manual return preserve a visible worker's cargo, and orders during
mining return false after validating their arguments. Gold credits reuse the
existing prepared resource-counter/event commit.

Synthetic regressions cover nonstock mining duration/capacity and cargo,
capacity queues, partial depletion and one death callback, committed bank-event
readback, manual return without duplicate credit, retreat, two-player assignment
isolation, rollover reuse and whole-assignment budget refusal. These tests
exercise the existing runtime rather than only the source reader.

Other race-specific harvest abilities, external destruction of an occupied
mine, cross-resource carried loads and several mid-mining effect/ownership
transitions remain unsupported or unqualified. This does not establish native
Windows presentation or a complete campaign economy.

Combined verification passes 1,724 core tests, 55 app-library and 638 app-binary
tests, the published two-program session, selected-image policy decoding,
Human01 activation and victory Continue, strict native/WASM Clippy, formatting
and the default desktop build.
The unchanged Human02 opening-minute test now passes `HarvestGold` and refuses
`HarvestWood` at 8.312500 seconds. Its complete failure is retained in
`.scratch/harvest-gold/consumer-final-gates-r3/frontier.log`; no chapter-completion
claim follows from this advance.

## Hidden accepted mine and depot continuation (2026-10-26)

The short Human02 outro diagnostic stopped at host clock 3.640625 because the
occupied-mine exit treated `shown == false` as removal/death. Its source action
hid the playable-rectangle units; those canonical records remained alive. This
was an authored-scene diagnostic, not mission victory evidence.

The first local hide probe never reached occupancy: it transplanted Human02
positions into different terrain. Its 245 raw records and timeout refusal remain
under `.scratch/gold-world-hide-probe/arm-a/`; they establish no hide behavior.
Corrected arms preserve the exact full map, destructables, player-nine setup and
acknowledged `HarvestGold(0,1)` from the earlier successful gold probe. The
selected executable is the same pinned Legacy image identified above; launch
used `-classic`. These are stopped, isolated and muted original-client runs.

Arm B observed actual mine occupancy before calling public `ShowUnit(false)` on
the mine, depot and worker. At logical 2.750 the worker emerged, its return order
was active and the mine had lost cargo while both structures were still hidden.
The structures were restored at 3.750; this arm alone did not qualify depositing
into a hidden depot. Arm B2 changes only the diagnostic hide window to fifteen
seconds. Before restoration at 16.750, both structures were still hidden, mine
resources had fallen from 20,000 to 19,980, and bank/gathered gold had increased
from zero to twenty. The worker was visible again with a harvest order. Thus
accepted mining, exit/reveal, return, credit and another cycle continued while
the accepted mine and depot were hidden, despite the explicit worker hide.
These sampled amounts and times are observations, not production rule constants.

B and B2 collectors accepted respectively 83 and 135 strict records. Map hashes
are `cafdb9f01d7d0b898195f6cf1fd9e4efa62a6ac1ed132b68196d48ce3da30fb5`
and `1e79a89dc08c6bf286d62c0860cdb16500f79288a3221548ba1e836e0087cc6a`.
Raw captures, source-table provenance, member round trips and results remain
local in `.scratch/gold-world-hide-probe/`. The fifteen-second window belongs
only to the diagnostic controller.

The runtime now distinguishes retained accepted relationships from acquisition.
A live occupied mine can exit while hidden, and the existing gold task can
return to and deposit into its retained hidden, live, owned gold-accepting depot.
Subsequent cycles keep that task's accepted live mine. New mine/depot selection
still excludes hidden candidates. Ownership, life, resource and capability checks
remain; there is no new resource store, suspension rule, automatic credit or
visibility override marker. Removed/dead occupied mines still refuse before
exit or debit, with both worker and mine identities in the error. External
occupied-mine destruction remains unqualified, as do independent hide ownership
outside the observed occupied phase and other resource/race-specific behavior.

Synthetic regressions use nonstock cargo and mining duration to exercise repeated
hidden-world deposits, explicit worker reveal, restoration without duplicate
credit, hidden replacement exclusion after depot removal/owner transfer, and
atomic removed/dead occupied-mine refusals. Existing new-acquisition and
capability-refusal tests remain in the focused gold suite. Parser/synthetic
checks do not establish rendered parity or completion of the full chapter.
