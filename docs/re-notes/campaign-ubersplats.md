# Campaign ubersplats

Ground decals under buildings (orc Great Hall, gold mine, and so on). Code:
`crates/pcx-play/src/uber_splat.rs` (catalog and lifecycle),
`crates/peacecraft3/src/campaign_uber_splats.rs` (renderer).

## Data source

- A unit names its splat through UnitUI `uberSplat`, a FOURCC. In the Legacy
  1.29 `Units\unitUI.slk` (War3x.mpq) the column is `uberSplat` and, for example,
  the `ogre` (Great Hall) and `ofrt` rows both carry `OLAR`; `-` is the authored
  empty marker.
- `Splats\UberSplatData.slk` is resolved through the selected map's sources and
  its identity is recorded with the catalog. Row key is `Name`.
- Columns read: `Dir`, `file` (texture `Dir\file.blp`), `BlendMode`, `Scale`,
  `BirthTime`, `Decay`, `StartR/G/B/A`, `MiddleR/G/B/A`, `EndR/G/B/A`.
- Columns not read: `comment`, `PauseTime`, `Sound`, `version`, `InBeta`.
- Stock `OLAR`: Scale 240, BirthTime 1, PauseTime 0, Decay 2, Start alpha 0,
  Middle alpha 255, End alpha 0 (all RGB 255). `NGOL`: Scale 180, same timing.

## Validation

- Colour channels, `BirthTime`, `Decay` and `Scale` are finite, non-negative
  reals kept as authored; `Scale` must also be non-zero. No upper bound is
  enforced by the parser: shipped `HFS1` writes `MiddleA` 256 and `EndR` 256,
  and a byte parse refused the whole table.
- A bad cell refuses only that row's splat. The error surfaces, with the splat
  name, when a unit that names it is drawn; other splats keep working.
- A row whose `Name` is missing or not four graphic bytes refuses the table,
  since no unit can be matched to it. A duplicate `Name` refuses that name.
- A table missing a required column header, over budget, or with a bad identity
  is refused whole.

## Lifecycle (game clock)

Time source is `ObjectHost::timer_game_time()`. The shade is the authored colour
interpolated linearly in authored channel units:

| Situation | Shade |
|---|---|
| Unit present when play begins | `Middle` |
| Unit first seen alive later | `Start` to `Middle` over `BirthTime` |
| Unit alive | `Middle` |
| Unit dead or gone from the host | `Middle` (or the mid-fade colour) to `End` over `Decay`, then removed |
| Unit hidden by script (`ShowUnit` false, not dead) | not drawn, clock unchanged |

"Play begins" is the game clock at the first frame the renderer observes the
session; units first seen alive at that clock are treated as already there. A
`BirthTime` of 0 or `Decay` of 0 is an immediate cut. A unit that dies during its
birth fade decays from the colour it had.

The host records no unit creation time on units without a model and no death
time that survives decay, so birth and end times are the game clock at the first
frame that sees the change (accurate to one frame). A unit seen only after it
died gets no splat.

## Recreation choices

- `Scale` is half the square's world width (square side `2 * Scale`).
- The square is clipped onto the admitted ground triangles with the selection
  circle projection, drawn unlit with alpha blending and a depth bias that keeps
  it in front of terrain and behind selection circles.
- Only `BlendMode` 0 is drawn; other values report an unsupported-mode error.
  Shipped mode 1 rows are `THND`, `NDGS`, `HFS1` and `HFS2`.
- Colour channels clamp to 0..=255 at the render boundary only.
- The initial-presence rule and the first-sighting clock above are recreation
  choices, not observed original behavior.
- Birth, hold and decay use linear interpolation of all four channels.

## Evidence and open questions

Shipped-byte evidence is the two tables above. No native-code or runtime
observation of splat timing was made.

- `PauseTime`: unit-bound rows (`OLAR`, `NGOL`, the other `*UberSplat` rows) all
  author 0, while the `Death*Building` rows author 5 with `Decay` 20 and spell
  rows such as `HFS1` author 0.1. That fits a timed-splat hold at `Middle` but
  does not show what a pause means for a splat tied to a living unit (hold
  while alive, or a delay after death). It is not applied. The two readings
  agree for every stock unit-bound row.
- Whether map-preplaced buildings fade in at the start of play in the original
  client is not observed.
- Whether `RemoveUnit` on a building decays its splat the same way as death is
  not observed; both are treated alike.
- How the original treats a channel above 255 (`HFS1` only, a spell effect) is
  not established; the clamp is a placeholder.
- Fading the RGB channels, not only alpha, and linear easing are unverified.
- What blend mode 1 means (`NDGS` DemonGateSplat is one such row) is not established, so it is not implemented.
- No pixel or timing comparison with the stock client was made; the GPU fixture
  captures are run separately.
