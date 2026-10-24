# Campaign gold harvesting

`HarvestGold(integer town, integer peons)` must assign real workers to a real
mine, preserve its resource amount, and credit gold when workers return to a
depot. Accepting the call or crediting a bank immediately would omit observable
behavior. The campaign call remains unsupported while that lifecycle is being
connected to the existing harvesting runtime.

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
and does not yet start a mining task. Synthetic tests discriminate nonstock
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

Town/worker selection with several candidates, queues, edited mining fields,
interruptions, depleted mines and other race-specific harvest abilities still
need implementation or qualification. This does not establish unchanged
Human02 execution, native Windows presentation, or a complete campaign economy.

Reader verification passes 1,713 core tests, the explicit locally selected
original-table test, strict native/WASM Clippy, formatting, and the default
desktop build. These checks validate the source reader; they do not qualify
the pending assignment or work-loop consumer.
