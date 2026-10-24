# Campaign wood harvesting

`HarvestWood(integer town, integer peons)` assigns canonical workers to the
existing lumber work loop. Worker assignment, work, carried lumber and resource
credit share the same world as explicit map orders and gold harvesting.

## Source evidence

The selected executable SHA-256 is
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Registration `0xab63e1` binds `HarvestWood`, `(II)V`, handler `0xab5810` and
registrar `0x8ccc80`. The handler checks the owned AI context, returns for
nonpositive counts and calls the same `0xabf110` town-assignment helper as
`HarvestGold`, with the wood selector. The existing helper supplies the checked
town bound; no new town-count constant is introduced.

Retained REA/Ghidra byte observations:

- Registration: `ev_062eed86b6fb3d505da8a544055ea04037f7c3f7665f27c7f1e5df371917ce08`.
- Handler: `ev_35c79c0888ab0ddeaf15d597c96e8ce0a9bdcc1eb27642ee108619ad9450ed36`.
- NUL-terminated name: `ev_5cc74faea980569cfaf5e6c2dbc4576a0605a8c61f195379590fff2a0f36507e`.

All reads were complete. These are static observations, not runtime or a
requirement to reproduce native data structures. The owned analysis session is
closed. `GatheringPolicy` checks these windows, the shared signature, context
resolution, count branches, selector and helper joins before admission.

## Matched original-client observation

Two stopped, isolated Wine reference runs used the same Human02-derived harvest
fixture. Worker, town-hall, tree and separate-owner control setup came from the
retained tree-damage r4 stimulus. Its experimental `Ahar` damage override stayed
byte-identical in both arms. Both AI scripts called `InitAI`, `CreateCaptains`,
`SetSlowChopping(false)` and `ClearHarvestAI`; only the requested worker count
and diagnostic identity differed. Both map scripts used the same diagnostic
seed. A gold-bank handshake qualified the completed AI request before sampling.
`VersionGet` reported RoC in both runs.

Both maps and AI scripts parsed and resolved; all packed members round-tripped.
The bounded collector checked all 106 records per arm, source/staged hashes,
wrappers, finite values, identity, ordering, monotonic tree/bank values,
unchanged control and a terminal record equal to the last of 100 samples.

| Observation | One worker | Zero workers |
| --- | --- | --- |
| First tree damage | 7 at 5.563 seconds | none |
| First depot credit | lumber/gathered lumber 10 at 9.813 seconds | none |
| Initial work order | `smart`, then return; `harvest` after first deposit | idle |
| At 30.063 seconds | lumber/gathered 30, tree life 0, returning | lumber/gathered 0, tree life 50, stationary |

These sampled values are evidence, not production constants. The separate-owner
worker and tree remained unchanged. The positive/zero maps have SHA-256 values
`89350e56e4490d740e2d88731b32d2a8ef0ac366a18282046cf979565723c7b9` and
`2a3eaa746ed9166ef5168684baf0bbfae8cd9ccec8e5ef239f3afca0e67850f0`.
Stopped result SHA-256 values are respectively
`ede2fe17bb9a399c095d0b8f8342149511fa784a0e0209155ae3fd9335d60cd1` and
`4944d660a0b5dfbbfbd3c91233d5b040ff28710a76f59afa7f6dd4b20d142db6`.
Original files, generated probes, raw records and receipts remain local under
ignored `.scratch/harvest-wood/`.

## Shared-world implementation

Owned-source ABI and active-player checks precede assignment. Gold and wood
share bounded candidate selection, town slots, current/previous assignment
masks and task publication. Previously assigned compatible tasks retain their
motion, work clock and cargo; current gold assignments are not stolen for wood.
New tasks prepare all routes and work animations and reserve capacity before
publishing any worker or assignment flag. Both resource calls use the same
creation-ordered gold-returning structures for town slots, while wood can return
to a separate lumber depot. A task retains its canonical assigned town across
deposits and rollover; a lumber mill does not create an extra town slot. This
town mapping and nearest compatible tree/depot selection are explicit recreation
choices, not native multi-town enumeration proof.

The initial AI wood leg reports the source-registry `smart` order. Subsequent
work reports `harvest`; depot return uses the existing return order. Explicit
harvest orders retain their existing reporting. The work loop continues to read
`Ahar`, the tree-targeting weapon, upgrades, depot `Artn` flags and the active
owner's slow-chopping policy. It does not credit resources at assignment time.

Other race-specific harvest abilities, native town reordering, exact native
worker-selection priority and initial AI wake cadence remain unqualified. This observation does not prove
full chapter completion, rendered parity or native Windows presentation.

Combined verification passes 1,730 core tests, 55 app-library and 638 app-binary
tests, the published two-program session, selected-image policy decoding,
the explicit locally selected original-table test, Human01 startup and victory
Continue, strict native/WASM Clippy, formatting and the default desktop build. The unchanged Human02
opening-minute test passes `HarvestWood` and now refuses `AddDefenders` at
8.312500 seconds. The full failure is retained in
`.scratch/harvest-wood/gates-final/frontier.log`; chapter completion is still
unproved.
