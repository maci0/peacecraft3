# Player score counters

Status: no counter is derived. `GetPlayerScore` refuses every selected
`PLAYER_SCORE_*` constant, and the snapshot reports every value as `None`
(not derived). No counter is zero-filled.

## Declaration and provenance

- `common.j` (evidence copy `.scratch/map-import-root-original/common.j`, not
  re-verified against the selected install in this note): `type playerscore extends
  handle`, `constant native ConvertPlayerScore takes integer i returns playerscore`,
  `constant native GetPlayerScore takes player whichPlayer, playerscore
  whichPlayerScore returns integer`, and 25 constants `PLAYER_SCORE_UNITS_TRAINED`
  (`ConvertPlayerScore(0)`) through `PLAYER_SCORE_TOTAL` (`ConvertPlayerScore(24)`).
- Production admission reads these constants from the program at startup
  (`map_startup_host.rs`, presence-gated on the `GetPlayerScore` native like the
  `Location` gate). Ordinals come from the declarations, not from a table in code.
- Not verified here: that the evidence copy is the selected `common.j`. The
  original-input admission gate (selected install, `Scripts\common.j` through the
  production resolver) is not written yet.

## Native evidence (selected executable)

Executable `Warcraft III.exe`, SHA256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`, read with
`objdump` (static, no Ghidra).

- Name string `GetPlayerScore` at VA `0xe734b0`. Registration `push 0xe734b0` at
  `0x49e71c`, call `0x8ccc80` with function `0x494710`.
- Wrapper `0x494710`: resolves the player handle with `0x482770` (null returns 0).
  The score index `[ebp+0xc]` must be below `0x19` (25); otherwise the wrapper
  returns 0. The original therefore reports 0 for invalid input. This port refuses.
- Jump table `0x4948e0` (25 entries) selects a getter per index. Index 0 reaches
  `0x4c11b0`, index 1 reaches `0x4c11a0`. Both load `[player+0x390]` and tail-jump
  into a list walk (`0x514cc0`, `0x514c80`) that sums a field at `+0x2c` over
  nodes whose `+0x30` has bit `0x8` set.

### Static writer and caller trace (objdump, no Ghidra)

- The getters' list walk at `0x514c80` (index 1) and `0x514cc0` (index 0) uses
  byte-offset links inside a container at `[player+0x390]`: the walk starts at the
  container count `[+0x10]`, reads base `[+0x8]`, follows `[node+0x4]`, sums `[node+0x2c]`
  only where `[node+0x30]` has bit `0x8`.
- Records are inserted by `0x513470`. It decodes a handle to an object with `0x95c860`,
  stores that object at `[node+0x14]`, and sets flags `0x2`, `0x4` or `0x8` on
  predicates `0x6e0580`, `0x5cc9f0` and `0x696d50`. The predicates test membership in
  globals `0x113e8d8` and `0x113d410`; their meaning is not yet named.
- Writers of `+0x390` (`mov [reg+0x390], …`) include constant stores at
  `0x9b4f33`-`0x9b55d3`, which look like object constructors. Not traced to a caller.
- Unidentified: which writer stores `+0x2c`. The kill path that would set it is not
  located. The name of each predicate global is the next Ghidra target.
- Index 0 (`PLAYER_SCORE_UNITS_TRAINED`, `ConvertPlayerScore(0)`) walks the same
  container and sums `[node+0x1c]`, not `+0x2c`, over flag `0x8` nodes (`0x514cc0`).
  So the two counters read two different per-record fields.
- `0x43fb90` writes `+0x2c` as a stream cursor (`add [edi+0x2c],esi` after a read),
  so it is not a score writer and is excluded.
- Switch targets (jump table `0x4948e0`, index: handler), from `objdump`: 0 `0x494748`
  (getter `0x4c11b0`), 1 `0x494757` (`0x4c11a0`), 2 `0x494766` (`0x4c1010`), 3 `0x494775`
  (`0x4c1000`), 5 `0x4947a0` (`0x4bde40`), 6 `0x4947af` (`0x4bde50`), 7 `0x4947be`
  (`0x4bdba0`), 8 `0x4947cd` (`0x4be2d0`), 9 `0x4947dc` (`0x4bf090`), 10 `0x4947eb`
  (`0x4bd560`), 11 `0x494805` (`0x4bd560`), 12 `0x494814` (`0x4bd540`), 13 `0x494823`
  (`0x4bd520`), 14 `0x494832` (`0x4bd5d0`), 15 `0x494841` (`0x4bd5c0`), 16 `0x494850`
  (`0x4bf010`), 17 `0x49485f` (`0x4bf050`), 18 `0x49486e` (`0x4bf030`), 19 `0x49487d`
  (`0x4bf080`), 20 `0x49488c` (`0x4bf070`), 21 `0x49489b` (`0x4c1190`), 23 `0x4948bb`
  (`0x4c0030`). Indices 4, 22 and 24 were not resolved by the first-call scan.
- The `0x4c...` getters for indices 5 to 23 were not traced to their record fields.

## Not established

- What the `+0x2c` node field is and which event writes it. Candidate write sites
  exist in the text but are not identified. Kill credit, score formulas, totals,
  and every other counter are unverified.
- Therefore no kill-credit rule (heroes, structures, neutral, allied, self kills,
  double deaths, missile owner change, script `KillUnit`) is implemented. Each needs
  native evidence before a counter is supported.
- Rendered score rows. The score screen is owned by the score UI.

## Supported and unsupported (current)

| Counter | Ordinal source | Status |
|---|---|---|
| all 25 `PLAYER_SCORE_*` | `common.j` constants | refused; snapshot `None` |

## API

- `ObjectHost::with_player_scores()`: validates `GetPlayerScore` and its
  `ConvertPlayerScore` domain; reads each `playerscore` constant.
- `ObjectHost::player_score_snapshot(number) -> Option<PlayerScoreSnapshot>`:
  `None` before admission. `PlayerScoreSnapshot { player, counters:
  Vec<PlayerScoreCounter { name, ordinal, value: Option<i32> }> }`.
  `value` is `None` for every counter until a source is established.

## Next evidence

Root: a Ghidra slot is needed to identify the `+0x2c` writer and the owning kill
path, then to confirm each counter against a recorded native run.
