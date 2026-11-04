# Campaign minimap

The in-game minimap is not the map-selection preview. `ConsoleUI.fdf` supplies
the surrounding art, but does not declare the minimap rectangle.

## Native evidence

The selected Legacy 1.29.2.9231 image, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`,
provides the following static joins:

- GameUI creates a separate frame at `5e7d93`, stores it at its `+45c` field,
  and applies all points of the logical root through `873750` at `5e7dc9`.
- The minimap constructor call at `5e7e7e` installs `CMinimap`'s vtable.
  The resulting `+400` object receives a bottom-left anchor relative to that
  frame's bottom-left, then a top-right anchor relative to the same point.
  The four floating operands are read from the two `873a10` call sites;
  no measured rectangle is substituted. The viewport is read from the existing
  logical width/height operands at `640ea8` and `640e96`.
- The constructor's primary vtable binds the image method at `636db0`.
  Its `637683` path names the map image and calls `71ba80`. That loader
  replaces the extension, tries the source extension table in order, and stops
  on success. The selected table tries TGA before BLP. This differs from
  `pcx-map::Map::minimap_image`'s BLP-first inspection accessor; that accessor
  is not a source-qualified runtime preference.

`pcx_play::minimap_layout::Rules` reads these anchors, viewport and image names
from the supplied image. It rejects unsupported image identities, truncated
instructions/data, changed call/receiver/anchor relationships, invalid extents
and unsupported extension records. The application read budget is independent
of the source's game values. Synthetic fixtures vary geometry and extension
order; the explicit ignored local-image gate decodes the actual selected image.
The retained local mapped-window receipt is
`.scratch/campaign-minimap-source/windows.json`, SHA-256
`14017be57ada71840ec38c64534cea977014e50f3e35f20850462b2533ab6ed0`.

Validation on this reader: 1,679 core tests pass (53 explicit ignored gates),
the selected-image decode passes separately, and strict native all-targets /
WASM library Clippy and formatting pass. The native decoder test uses
`PCX_PLAYER_RESOURCE_IMAGE`; original image bytes remain local.

## Terrain image projection

`campaign_minimap.rs` now mounts the admitted map's terrain image. It checks the
published context and receipt identities, checks the retained map bytes against
the context's SHA-256, and extracts the source-selected member through the
bounded checked MPQ path. Present malformed art refuses rather than silently
trying another member. Decode bytes and the render device's actual texture
dimensions are preflighted before decoding/upload. Resources
retire with the script-session witness and are retained during camera suspension.
The already-installed SHA-256 dependency is now also used by the production app.

The additional static quad trace distinguishes the outer image from the inset
overlay clamp: `6378d0..637970` constructs four vertices from the outer frame's
width/height, and the draw method submits those vertices from `+1e8` at
`640277`. The inset `+830` rectangle from `636e5f..636eba` is separately used by
the overlay clamp at `62f887..62f8c6`. The mapped-window receipt is
`.scratch/campaign-minimap-source/quad-windows.json`, SHA-256
`29baa0a5fcb739c048dce31c2596e90bc4397f743e0bafa3d98b42910f1e3aaf`.
The recreation preserves the outer quad through the shared fractional-image
renderer and reads dynamic sampler modes from the native flag and backend
tables. See the raster correction below for scope and remaining gaps.

The hit surface is independent of the terrain image. It remains present when
fog hides the image or image decoding refuses. Presses consume the existing HUD
input gate and do not fall through to ground selection/orders. Cinematic
interface suppression and camera suspension still hide the entire control.

The minimap now projects circular sight from canonical live unit positions and
source `UnitBalance/sight` or `nsight`, using the existing game clock's day/night
classification. Explicit shared-vision alliances and running radius modifiers
feed the same snapshot. Before-unit and after-unit modifier phases retain their
creation order. Hidden, dead and removed units do not provide sight. No second
unit store or guessed sight radius is introduced.

A bounded presentation raster retains explored cells for the current viewer.
Movement leaves explored terrain fogged; current sight reveals it. Fog and mask
switches are independent. The raster fits the source-derived expanded camera rectangle into the
actual map image, preserving aspect ratio and north-up orientation. It begins
updating while the cinematic camera hides the control and retires with the
session; changing viewers resets its history. With both switches false, the
authored image alpha is restored even when sight admission is unavailable.

This is a house visibility implementation, not a copy of native grid internals.
It still lacks terrain/tree sight occlusion and sight-changing upgrades/effects.
Presentation-frame exploration sampling, native grid/rounding behavior,
modifier masking behavior and blend/filter parity need original-client
comparison. World terrain/actor fog and public visibility queries do not yet
consume this raster. Hero, resource and neutral icons and unit-icon targeting
remain unfinished (see Image drawing below). The renderer keeps an explicit missing-occlusion
diagnostic rather than claiming completed visibility fidelity.

### Camera navigation

Left clicks and held-left-button dragging now move the existing canonical
camera target. Pointer positions use Bevy's computed node-relative coordinates;
the inverse of the sight raster's mapping converts them into world positions.
Padding and dragging past the surface clamp to the map extent. Both minimap
and arrow-key input then apply the same executed camera bounds and cancel the
same running pan. No separate camera or copied map dimensions are introduced.

Releasing the button stops tracking. Control/interface locks, quest/script
dialogs, an inactive view, a retired session or a departed local player prevent
camera or order changes. Session retirement clears pending presses and drags.

Right clicks issue point Move orders through the existing standard-order
consumer. Armed Move, Attack and Patrol use a left click as their point target;
right clicking an armed command cancels without issuing a replacement move.
Unit-targeted spells cancel as on bare ground; targeting unit icons remains
unimplemented. Orders recheck the live selection and local ownership, derive
their IDs from the admitted command policy, and enter the existing player
journal only after native acceptance. Point orders do not pan the camera.
Hotkey/source preparation runs before minimap input, which consumes the pointer
before world picking. A bounded, fallible press queue preserves multiple presses
in one frame instead of replacing them with the last event. This queue is input
collection, not Shift waypoint queuing; movement/formation semantics remain
those of the existing unit router.

This uses the source-derived camera rectangle with a floating image fit; native
grid sampling, image-fit rounding and padding behavior still need comparison. It does not implement
unit-icon targets.

### Fog alpha source join

A further selected-image static trace joins the fog alpha inputs without
assigning a guessed darkening palette. `CMinimap`'s constructor reads
`FogOfWar/FogColorBlackMaskAlpha`, `FogColorDarkMaskAlpha`,
`FogColorFogMaskAlpha` and `FogColorVisibleAlpha` through `4304c0`. The stores at
`620419`, `620430`, `620447` and `620455` populate `+9cc`, `+9d0`, `+9d4` and
`+9d8`, respectively. Both retained base and expansion `UI\\MiscData.txt`
members contain these fields and have the same member SHA-256,
`d097d3cbb4a03719a97591900b45ab42fdb847385819c3d5e7dd7b47d7d9b516`.
Their numeric contents remain local; a renderer must read the admitted profile.

The texture updater `658ac0` calls `657340` at `658cd9`. The latter selects
from those fields, samples two visibility grids through a shared mask and
writes the fourth byte of the minimap CPU image. It also considers frame alpha
and averages changing horizontal samples. This establishes alpha input and
update relationships, not rendered blend parity or a complete visibility model.
The current renderer reads the normal black-mask, fogged and visible fields from
the receipt-matched profile. The alternate dark-mask mode is not implemented.
An opaque backing uses the minimap updater's own packed clear pixel, decoded
from the paired immediate writes at `658bae` and `658bb5`. This prevents masked
pixels exposing the world through transparent console art. The UI backing is a
recreation choice; the clear pixel is source-derived, not a fitted black color.

Complete static dossiers remain ignored under
`.scratch/campaign-minimap-source/`. The constructor evidence is
`ev_3eca7ff35417fcc6a5c906f4aeec2c14652928d930fe870afe09a72358be4b33`,
the updater evidence is
`ev_a2f06a059f46e1064a9e51f3006b8f522ec8aaefb5c91f55c338828a197e470b`,
and the fog function evidence is
`ev_ea4d3d461bd6787d1f8e77c056880b87c9d74df4230a5703ef45ed2a109c8331`.
These are static observations on the image identified above, not a client run.

## Runtime evidence and limits

The current suites pass 69 object tests, 1,682 core tests, 55 app-library tests
and 631 app-binary tests. Synthetic fixtures vary day/night sight, alliances,
modifier states/order, opacity and clear-pixel values. Movement, exploration,
switch combinations, canonical removal, budgets and refusal without history
mutation are covered. The selected-image minimap decoder passes separately.
Strict native all-targets and WASM library/binary Clippy, the default desktop
build and formatting pass.

The explicit selected-Human01 camera test also passes. It changes the camera
through the minimap consumer, checks the shared raster inverse and executed
bounds, holds and releases a drag, then verifies control/UI locks, a quest modal,
camera suspension and a stale session cannot move it. The same selected-map
gate checks two owned units versus a foreign selected unit, accepted-only
journaling, armed Move/Attack/Patrol, cancellation without a pointer coordinate,
and multiple point-order presses in one update. A separate unit test
cancels a held drag with no cursor position; projection tests cover nonsquare
maps, padding, out-of-surface coordinates and non-finite refusal. This is
application behavior evidence, not an original-client camera comparison.

A normal desktop-window XTest replay now enters Human01 from the launcher and
menu, selects the hero, clicks the minimap, drags it, releases, and moves the
pointer away. Captures show distinct terrain regions after click and drag;
camera-target diagnostics cease after release. The balanced-input receipt is
`.scratch/minimap-camera-native/receipt-launcher.json`, SHA-256
`bf55c0bd0036f4d9fa1f96664c5e763e0844ec8325330818d516ef698412d9c9`.
It pins the source/build/content, owned process/window and 1280x1024/96-DPI
display. The default executable SHA-256 is
`267008c26136dfc7b3d35d2db374a96035828109d8d85b1820b0b882791b38d0`;
the tested binary is preserved locally in that directory. Software Vulkan was
used, audio was disabled and no fixed seed or original-client parity is claimed.
The private application/display were stopped afterward. Failed launcher and
pre-input replay-validation attempts remain separate; no game input was sent in
those attempts.

A subsequent desktop replay in `.scratch/minimap-orders-native/` reaches the
same chapter, selects the hero, sends a right-click minimap Move and then an
armed Attack click. The installed command-string members agree on the Attack
hotkey; their bytes and hashes remain local in `hotkey-source.json`. The replay
uses that derived key. The application records one accepted player command for
each press and no minimap camera requests; captures show unit movement while
the terrain view remains fixed. This verifies application dispatch/motion, not
original-client arrival, formation or timing parity.

The balanced-input receipt SHA-256 is
`4b7e92759904787602db34a421689c360b7d0a3dffb76b28e872e07c72ec9d80`;
the tested default binary is preserved with SHA-256
`7ebae3e0d2ab0d170c1f82b8f50f6bff77f2abae04b0fc458b7bc3caf1bc12fc`.
This run also uses software Vulkan at 1280x1024/96 DPI, disabled audio and no
fixed seed. Its private application and display were stopped after collection.

The production offscreen Vulkan run in
`.scratch/campaign-minimap-render/vision-2/` follows mounted menu and mission
widgets into Human01, then skips the cinematic. At this unchanged checkpoint,
130 units exist, both fog switches remain true and `war3mapMap.blp` is shown with
black unexplored terrain and an owned-unit sight area. The gameplay capture's
SHA-256 is `f03c15bfea71c05cc733fc86b3f0a505c2f00a1e2e35af6d185883df476aa2fe`;
`scope.json` is `60f73a6cfc1a9899b8e322fe47f8c7bbb488ce8a1a6c973cb16ab443d82a3f2c`.
The **strict unchanged rendering gate still exits 101** for known sky
illumination and missing minimap sight occlusion. It was not weakened.

After that checkpoint, the diagnostic separately executes `FogEnable(false)`
and `FogMaskEnable(false)`. That labelled mutation shows the fully revealed image
with no minimap error; its PNG SHA-256 is
`a5532945661960fcfa7620c9385b5d8a3c761239f017d0462aa5115f865fc1b6`.
It is not evidence of unchanged campaign visibility. Earlier captures remain
preserved, including `vision-1`, which exposed the transparent-backing bug fixed
before the final recapture.

A separate native-window XTest replay of the input implementation, before the
final texture-dimension preflight, enters Human01, selects the hero,
right-clicks the fog-hidden minimap and then right-clicks ordinary ground. The
minimap refusal is logged; the hero remains at the selected position after that
press and visibly moves after the ground order. Balanced inputs, nine captures,
window/PID ownership, source/build/content fingerprints and calibrated
1280x1024/96-DPI display identity are retained in
`.scratch/campaign-minimap-native/receipt-launcher.json`, SHA-256
`431925cbaff039c250f316035edfc45cb0e5af09b70930761850ad214976542f`.
That replay's default binary SHA-256 is
`77ef21a661990647be29e17c8f04a888aa38fcefa05b5a48a984f4e521815fcc`.
The tested executable is preserved locally as `peacecraft3-tested` in that
receipt directory. That replay predates the current sight projection and does
not validate its visibility behavior. Pointer isolation is unchanged. This
replay used software Vulkan and
disabled audio. Failed display-setup logs
are preserved separately; the app and private display were stopped afterward.
Neither run establishes original-client pixel parity, fog fidelity, audio or a
completed campaign.

## Image drawing: border, markers, outline

Static trace of the selected image (same SHA-256 as above). The client keeps a 256 by 256
CPU image (`mov esi,0x100` at `658afd`, row stride shift at `658dc6`) and writes into it in
this order inside the updater `658ac0`: terrain copy or clear, creep camps (`6568a0`), icons
(`659220(0)`) and unit markers (`65ba90`), the fog alpha (`657340`), icons again
(`659220(1)`), then the border lines. The outline is a separate quad pass.
`pcx_play::minimap_draw_layout::Draw` reads the items marked implemented below from the
image, refuses on any changed instruction, and has an explicit local-image gate
(`supplied_client_draw_operands_are_admitted`, `PCX_PLAYER_RESOURCE_IMAGE`).

### Map-edge border (implemented)

- The constructor reads `Minimap/MinimapColorBorder` (`620473..62049d`, packed A,R,G,B
  parser `430390`) into `+0x9e0`; the updater loads it at `658d59`. `UI\MiscData.txt`
  holds `MinimapColorBorder=255,60,60,70` for this install. The reader takes section and key
  from the string operands, not from application text.
- The loader (`637295..63753b`) stores the width and height fractions
  `w/max(w,h)`, `h/max(w,h)` at `+0x218`, `+0x21c` and the paddings
  `((1 - fraction) * 256) >> 1` at `+0x220` (x), `+0x224` (y). The conversion helper
  `a56bf0` is called for the float to integer step; its rounding mode is not read, the
  clone truncates.
- The lines are written after the fog alpha, over the whole image, opaque (the colour's
  own alpha), only when the fractions differ and the paddings differ by more than the
  immediate at `658d4f` (6). A taller map gets four columns `255 - padx`, `254 - padx`,
  `padx`, `padx - 1`; a wider map gets four rows `pady - 1`, `pady`, `256 - pady`,
  `257 - pady`. The pairs are not mirror images; the decoder keeps them as read.
- This accounts for the two 2-pixel (60,60,70) lines in the stock capture (rows 758 and
  919 at 1280x960). With the 256 texel image scaled to the 222 pixel frame, the measured
  161 pixel spacing implies a padding of about 36 texels, consistent with the formula.
  Not a pixel comparison.
- The clone derives the integer paddings from its own fit (`Raster::pads`: slack
  truncated, halved), which equals the client's formula for a 256 image. Other image sizes
  keep the 1 to 2 pixel line widths and are a recreation choice.

### Unit markers (implemented: size and class; colour was already source-decoded)

- Marker pass `65ba90` fills a square block of the owner colour, no sprite and no alpha
  scaling. The block half is `1 << (class + 1)` (`65c0f5`, `65c0fd..65c107`), the side
  twice that, so class 0 is 4 pixels and class 1 is 8 pixels (counted stores of the colour in the unrolled paths: 64 for class 1 and 16 for class 0,
  each including the two common stores at `65c1f5` and `65c1f7`). At the
  222 pixel frame that is about 3.5 and 7 screen pixels; the 7 by 7 white squares in the
  stock capture are class 1.
- The class is the result of `696b50(unit, 1)`: 1 when unit flag bit `0x10000` is set,
  except 0 when `flags & 0x8000080 == 0x80`. Bit `0x10000` is set from a type record lookup
  (`5cc9f0`, at `66f06e..66f07d` and `680bf2..680c04`). The clone uses `UnitBalance/isbldg`
  as that predicate (the unit-type table declares `isbldg` beside it at `6b4407`). The
  identity of the bit with `isbldg` and the `0x80` exception are not established.
- Placement: `floor((x - minx) * scale) + padx - half`, rows measured from the bottom
  padding; the block is clamped to `0..256 - side` horizontally and to
  `255 - pady - side` vertically (`65c0f5..65c1e0`). The clone floors the image pixel of the
  position and applies the same clamps with its own float padding.
- Colour: the table at `+0x248` is built at `65bb97..65bd47`. The local slot always gets
  `FogColorPlayer` (`65bbf0..65bbf6`; white in this install's `MiscData.txt`). For every
  other slot the build branches on the CMinimap flag `+0x824` (`65bbfb`): nonzero reads
  the player-colour table through `5c60d0` (`65bc04..65bc14`), zero reads
  `FogColorAlly` or `FogColorEnemy` by the alliance bit test `478da0` (`65bc19..65bcd7`).
  Slots 24 to 27 take `5c60d0` for a nonzero flag and `+0x9c4` (the creep colour) for zero
  (`65bcf5..65bd47`). The skin key `MinimapSprite` (`MiniMapUnitIcon.mdl`) is not named
  anywhere in this client, so the sprite path was removed.
- Initial flag (2026-10-27): the constructor stores `+0x824 = (option 0x38 != 0)`
  (`6208be..6208d1`: `cmp [option],1; sbb ecx,ecx; inc ecx`), so the default option
  `1` starts the minimap on player colours with a white local player. Only the
  `SetAllyColorFilterState` path (`4a6e70` -> dispatcher `42ab70` case `0x38` ->
  `42b082` -> `5ea740`) maps the mode with `652e30`: modes 1 and 2 store 0 (ally
  colours), any other value stores 1. `ally_color::Rules::decode` reads these relations
  (store, constructor, table branch, both mode calls) and refuses on any difference;
  the host now keeps a separate minimap flag, initially `initial == 0` and set by
  `filters_minimap(value)` on each native call. Before this, the clone applied the mode
  mapping to the start value and drew teal allies and red enemies at map start.
- Not established: the stock capture shows only white squares in the base area, where
  the clone drew three small teal squares and one larger teal block near the gold mine
  (the owners were not read from that run; Prologue02 places three `Player(3)` peons about 450 units from
  the mine). With player colours those become the owners' palette colours, not white;
  whether the stock draws them under its ping glow is not settled. The per-unit skip tests at
  `65be68..65bec5` (`696b00`, flags `+0x164 & 0x40000`, `+0x48c & 0x200`,
  `+0x168 & 1`, `5ca4d0`) are not decoded; the clone only applies `hideOnMinimap`.
- Fog gate: the per-unit visibility call (`65bf1c..65bf63`, vtable `+0x100` with a mask
  word) runs only when the fog or mask flags (`[ebp-0x40]`, from the two flags at
  `65bb4c..65bb62`) are set, matching the clone's gate. The mask word depends on the
  size class (`4` for class 0, `7` for class 1 when a viewer exists), so buildings use a
  wider visibility mask than units; the clone skips every unit that is not currently
  visible. The mask bit meanings are not read.
- A global flag (`113e1e0`) and a float from the object at `113b6f4+0x170` switch some
  markers to `FogColorEnemy` (`65c0e8..65c0f2`); unread.

### Camera outline (implemented, geometry partly recreated)

- Draw method `63fd30`: the four near-to-far frustum edges are clipped to the plane at the
  camera target's height (`63ff50..640003`, minimum separation 0.01), the bounding box of
  the four points is measured, its half extents are transformed to the minimap and drawn
  around the mapped camera target (`6401c0..640226`). Clipped to the control
  (`6406ba..640843`) and submitted as a line loop with texture `+0x76c`; colour is the
  control alpha shifted up, or `0xffffff` (`640676..640680`). The decoder returns that
  RGB; the clone draws a one-pixel border node of that colour.
- Recreation choices: corners come from the camera's viewport rays, not the client's
  frustum corners; a ray that does not run down through the plane hides the outline where
  the client keeps the far corner; the plane height is taken from the optical axis at the
  camera target (equal to the target height plus the Z offset, the client reads one value
  from `5e06d0` whose offset handling is not read); the line is a UI border, one screen
  pixel wide, as in the stock capture.
- Open: the stock outline measured 32 by 19 pixels; a paired capture at the same camera
  fields would compare it. No such comparison has been made.

### Not implemented, evidence recorded

- Hero icon: heroes (unit type id starting with an uppercase letter and `5ca470` false,
  `65d460`) are not drawn as squares. Their position goes to a per-owner list (`65c034..65c0a0`)
  drawn at `640357..64063e` as a textured quad from `MinimapHeroTexture`
  (`UI\Minimap\minimap-hero.blp`, loaded at `637651`), tinted by the same colour table,
  alpha pulsing from `+0x500`, enemy hits redrawn in `FogColorEnemy`. The quad size is
  `min(256/(W*2^-7), 256/(H*2^-7)) * 0.00390... * 0.5` built at `6379ab..637a5d` from
  extents read through the profile reader `4304c0` (`6370c0..637200`, the camera margin
  keys); those inputs are not decoded. The clone still draws a class 0 square for heroes.
- Neutral buildings and Circle of Power still need their special-icon path.
  Resource icons now use the source-derived CPU blit described below. The native
  special pass at `659220` loads `MinimapResourceTexture`,
  `...EntangledResource...`, `...HauntedResource...`, `MinimapNeutralTexture`
  and `MinimapCopTexture` (`6376ae..63777e`).
  Selection includes resource bit `0x20000`, neutral-structure bit `0x40000`,
  and 'ncop'/'ncp2'/'ncp3'. Mine variants come from `+0x168` bits 7 and 8.
  The earlier claim that `ngol`'s `Agld,Avul` excludes the resource blit was
  incorrect: the lookup accepts registered base types, including `Agld` under
  `Agmb`. The visible mine icon's complete runtime path remains to be qualified;
  the earlier preference for a model quad is withdrawn. See the audit below.
- Creep camps: models `MinimapCreepCamp`/`MinimapCreepCampTough`, thresholds
  `MinimapMiddleCampThreshold=10`, `MinimapToughCampThreshold=20`, scale and colours
  (`620473..620545`, stored `+0x9e4..+0x9fc`), gated by the creep button flag `+0x81c`.
  Pings use `MinimapIndicator`, `MinimapWaypoint`. Not read further.
- Sight shape: stock edges follow terrain occlusion, the clone's are circular (above).

## Stock capture comparison (not parity)

Measured on 1280x960 captures (stock `matched-skip-control.png`, clone `selected-unit.png`),
minimap area x 0..300, y 720..960. The captures are not proven to be the same game time.

- Stock draws 7 pixel white squares (class 1 above) and no small unit squares in view; the
  clone drew 3 by 5 pixel sprites and now draws 4 and 8 texel blocks.
- Stock icons not reproduced: gold mine ball, hero glow (above).
- Stock outline: 1 pixel white rectangle; the clone now draws one from the camera fields.
- Stock border lines: now drawn from `MinimapColorBorder`.
- Terrain button: terrain off zeroes the colour planes before the fog alpha, as the
  updater does (`0x658ac0`). See [campaign-hud-panels.md](campaign-hud-panels.md).
- Marker colour mode: `FogColorPlayer`, `FogColorAlly`, `FogColorEnemy` land in
  `+0x9a8`, `+0x9ac`, `+0x9b0`; `ally_color` selects among them, starting on player colours
  (see "Initial flag" above).

## Camera rectangle correction (2026-11-03)

Static shipped-code evidence on the same SHA-256-qualified Legacy image shows
that the minimap rectangle is not the full terrain extent. The camera getter
at `5e0700..5e0758` returns the four stored corner pairs. The minimap reads those
at `636f9c`, takes its native corner-pair minima/maxima at `63705f..6370b7`, then
expands them using profile values read at `6370d4`, `637105`, `637132` and
`637163`. Arithmetic destinations join the keys to left, right, bottom and top.
The scale global read at `63706c` joins the initializer at `40a860..40a872`.
The ensuing fit at `63724c..6372b8` divides each extent by their maximum and
centers the remaining slack. These are static observations, not runtime probes.
Bounded disassemblies and their hashes remain local in
`.scratch/minimap-mapping-20261103/native-evidence.json`.

`minimap_layout::Rules` now decodes the profile references, native getter field
ordering and scale initializer. `world_bounds` joins them to the admitted
`CameraState`/`CameraMargins` source identity, section, keys and scale digest.
It preserves the native corner-pair arithmetic and rejects unsupported source
relationships, missing bounds and invalid rectangles. No margin value, map
extent or screenshot offset is copied into code. The fog raster, marker drawing
and hit-testing, camera outline, camera navigation and Alt-signal conversion use
the same source-bound rectangle. The existing shared visibility ledger retains
its full world extent; its classifications are sampled through this minimap fit.

The explicit CPU-only selected-map receipt confirms the old full-world rectangle
and expanded camera rectangle differ for RoC Prologue02. Those values and source
identity remain local in `.scratch/minimap-mapping-derived-20261103`. Synthetic
checks vary geometry, profile references and scale, preserve native corner-pair
behavior and reject malformed relationships/invalid inputs. Three rule tests
pass (one explicit local-image ignore); nineteen app checks pass (two explicit
local-input ignores), and the selected-chapter camera/order/control replay passes
separately. Strict native/WASM Clippy and both application builds pass.

The isolated, muted production capture in
`.scratch/pixel-minimap-mapping-20261103` passes. Against the unchanged whole-frame
original reference, differences fall from 988,203 to 986,747 / 1,228,800, and MAE
from 12.52657958984375 to 12.392502983940972. Exactly 7,209 renderer pixels change,
within [16,754,237,923]. All thirteen clocks hold, repeated selected frames match
byte-for-byte and all 2,640 upkeep diagnostic pixels remain exact. The full-frame
overlay, diff, side-by-side and five README captures were visually reviewed.

This corrects the mapping rectangle; it does not establish native fog-grid
interpolation/rounding, occlusion, minimap texture filters or fractional quad
placement, special icons, remembered-marker behavior or line rasterization.
Original tick, animation phase and RNG identity remain unqualified.

## Dynamic sampler and image quad correction (2026-11-03)

Static evidence on the same admitted image joins dynamic texture creation at
`63786e..6378a8` to `61dfb0`. The five source arguments clear the constructor's
low flag fields; the first argument, its mask at `61dfe4`, and the backend mask
at `7def4a` determine the sampler record. This is a different path from the
`CSimpleTexture` file-loader flags used by adjacent filter buttons. Creation
uses the already-qualified `7d11f0`/`7df940`/`7d4590`/`7d2bf0` chain that retains
flags at texture+20. The backend's `7def50`, `7def61` and `7def74` operands supply
the min/mag/mip table addresses. U/V addressing comes from the flag shifts,
masks and table operands at `7def8f..7defbd`. The selected record is point/point,
no mip filtering, and clamp U/V. Values are read locally, not copied into code.
Bounded disassembly hashes are retained in
`.scratch/minimap-raster-20261103/native-evidence.json`; this is static evidence,
not an original-client sampler probe.

`minimap_layout::Rules` now retains these modes. The shared texture-record
reader is reused by file-image and dynamic-image rules. Unsupported flag paths,
filter/mip combinations or address modes refuse. The application uses the
existing source-image sampler conversion and retains explicit U/V addressing.
Synthetic source fixtures vary the selector and table records independently;
app checks verify distinct min/mag and U/V values reach the decoded image and
reject unsupported modes.

The native outer quad at `6378d0..637970`, submitted at `640277..64028c`, is
already bound to the source control's dimensions. Its image and clear backing
now use `SourceImageRect`, retaining those fractional bounds through layout and
the shared D3D9 pixel-center conversion. Picking stays on the logical surface.
The outline is a sibling under that surface, keeping the image a leaf; native
outline rasterization is still unqualified.

Four minimap rule tests pass (one explicit original-input ignore); all 68
layout checks pass (11 explicit input ignores), including shared-table callers.
Nineteen minimap app checks pass (two explicit original-input ignores). The
CPU-only selected-source receipt in `.scratch/minimap-raster-source-20261103`
records the actual sampler operands. Strict native/WASM Clippy and both app
builds pass. No dependency was added.

The isolated muted production capture in `.scratch/pixel-minimap-raster-20261103`
passes, including an assertion that the actual minimap uses one fractional image
leaf. The unchanged original whole-frame reference now differs at 984,904 /
1,228,800 pixels, down from 986,747. MAE falls from 12.392502983940972 to
12.3603662109375. Exactly 4,887 recreation pixels change in [15,728,238,949]. All
13 capture clocks hold; repeated selected frames are byte-identical and all
2,640 upkeep diagnostic pixels remain exact. Full-frame overlay, diff,
side-by-side and all five README captures were reviewed and provenance refreshed.

Native fog-grid interpolation/rounding, terrain/tree occlusion, special icons,
remembered-marker behavior and outline rasterization remain open. The original
reference still has no qualified tick/animation/RNG identity. These measurements
are diagnostic evidence, not pixel-parity acceptance or screenshot-fit inputs.

## Minimap native audit in progress (2026-11-03)

The same SHA-qualified Legacy image has now been decompiled through the image
initializer (`636db0`), draw method (`63fd30`), CPU texture updater (`658ac0`),
special-icon pass (`659220`), unit-marker pass (`65ba90`) and fog passes
(`6568a0`, `657340`). The constructor entry is `61f930`; `620000` is an interior
address, not a valid standalone function entry. Independent pseudocode is a
cross-check, not proof of function boundaries, inferred types or rendered parity.
Local outputs and input copies remain ignored under
`.scratch/minimap-native-audit-20261103/`.

Shipped-byte evidence corrects the resource classification inference above:

- `670635` calls the type-id getter at `b626a0`, returning `Agmb`. The call to
  `6d6570` at `67063d` enables base-type matching. A successful lookup sets unit
  `+164` bit `0x20000` at `670646`.
- The registration at `b62bf0..b62c07` passes `Agld`, `Agmb` and its type record
  to `7f6360`. The lookup at `6d6570` compares the concrete id and, when enabled,
  calls `7f55f0` for the registered type relationship. Comparing the literal
  ability list against `Agmb` alone therefore misses gold-mine inheritance.
- The special-icon CPU loop combines each channel with the texture alpha as a
  bit mask, rather than using ordinary alpha interpolation. Resource variants
  also run the native colour transform before that mask. The generic square
  approximation does not implement this behavior.

The local resource receipt retains exact bytes and disassembly, with input
SHA-256, in `resource-classification.json`. These are static observations;
resource visibility, remembered icons, complete fog behavior, hero/camp drawing
and exact full-frame parity remain unqualified. No renderer change or new
screenshot result is claimed by this audit.

## Visible resource icons and solid marker writes (2026-11-03)

The draw reader now consumes the three resource texture keys from the initializer,
ability-class getter operands (`b62690`, `b941a0`, `b263d0`), blit loop counters,
row stride, centre offsets and clamp operands. Synthetic fixtures vary the keys,
classes and dimensions. Luminance coefficients are read from the multiply operands
and SIB scales in `62d0c0`; tint rounding and shifts come from `63a850`. The
selected source yields RGB weights 77/151/28 and the native multiply/add/shift
transform. These values are not application defaults.

The renderer resolves those keys from the selected skin and uses the shared
native image-loader extension order. Decoded dimensions must match the blit;
missing, ambiguous or unsupported input refuses with context. Resource class
selection reads the same admitted ability tables and map overlay as the host's
ability behavior. Visible normal, entangled and haunted resources draw their
textures before ordinary markers; variants use the owner/role colour transform.
The alpha byte masks destination/source bits, including the visible alpha plane.
Synthetic tests distinguish that operation from interpolated alpha blending.

Ordinary CPU marker blocks now write solid pixels. The old fractional-area
coverage was an unqualified reconstruction layered ahead of the actual UI sampler
and has been removed. Their projected coordinates now truncate from the lower
map origin before row inversion, with whole-pixel padding; the same projection
and source resource dimensions reach hit testing. Native fixed-point conversion
edge cases, resource visibility/remembering outside the currently visible pass,
hero/neutral/camp icons, fog sampling, occlusion and outline rasterization remain
open. This is not complete minimap or full-screen parity.

## Native fog-grid scan qualification (2026-11-03)

Two private Wine/Xvfb/null-sink sessions supplied read-only CMinimap observations.
The second was paused through its Game Menu; its CPU buffer repeated byte-for-byte
before retention. Both containers were stopped and verified exited. Input hashes,
source executable identity, object/grid records and acquisition receipts stay
local in `.scratch/minimap-native-audit-20261103/` and the two private run folders.
This is Wine runtime evidence, not native Windows evidence or a tick match to the
published reference frame.

The marker transform uses an edge-minus-one span after padding and operates on
terrain-cell coordinates. It is distinct from the continuous hero-quad transform.
The source helpers `6540e0`/`7ffbe0` derive a pathing-rectangle centre and shift it
to the terrain-cell grid. Ordinary/resource markers now use the source float-bit
conversion, bias, shift and edge-minus-one scales. The retained snapshot gate
checks both scale values bit-for-bit. The world-position adapter agrees with 38
paired original map placements in a third read-only session; nine wildlife
positions differ after live movement, so that comparison does not qualify moving
unit bounds or synchronized ticks.

For fog, `657340` scans the map region with a fixed-point accumulator, takes
nearest source rows and averages each changing horizontal alpha with its previous
source-cell alpha. The previous value starts at zero on every row. It preserves
base-image alpha outside the written rectangle. The shift width, unit, subtraction
operands, row/column edge operands, initial zero and averaging instruction are
validated/read from the supplied executable. The synthetic executable fixture
uses another shift width.

A separate reconstruction using the observed visibility/exploration grids and
base image matches **all 65,536 CPU alpha bytes**, with no excluded pixels. The
Rust renderer's explicit ignored original-input gate independently reproduces
that full plane, including source-derived border stores. Synthetic fixtures cover
horizontal transitions, untouched base pixels, failed samples and deterministic
work-budget refusal without replacing accepted alpha. These establish the scan
for the retained inputs. The live adapter samples the shared ledger at source
cell centres; its sight/occlusion data, alternate dark-mask mode, partial frame
alpha and other map/input cases remain unqualified. Parser checks alone do not
establish those behaviors.

The ordinary marker pass no longer forces creep-owned units into the building
size class. Their authored structure class is preserved; the separate native
creep-camp model overlay remains open.

## Hero image pass and remaining sight trace (2026-11-03)

Heroes now draw through a separate textured UI pass after the CPU map and before
its camera outline. Skin lookup and extension order, texture filter/address
records, UV coordinates, quad size, projection operands and triangle-wave alpha
come from the selected executable/assets. Authored mip levels remain available;
other non-mipmapped UI callers preserve their existing restriction. Hit testing
uses the same hero quad. Synthetic fixtures use alternate geometry and phase
operands. The explicit original-input test checks the retained native quad half
and all four matrix values bit-for-bit; parser/geometry checks do not prove the
complete rendered pass, enemy repass or native animation phase.

The current screenshot has 976,058 differing full-frame RGB pixels out of
1,228,800, with mean absolute channel error 11.714644911024305. No pixels are
excluded, aligned or resized. The selected recreation repeat is byte-identical.
This is improvement over the previous 976,183 / 11.755233289930555 baseline, not
minimap or screen parity. Fog contours, special neutral/camp overlays, remembered
visibility, native unit flags and outline rasterization remain open.

Further native-code tracing separates grid production from image sampling:
`4c61c0` admits unit sight, `4c62b0` stamps it, `4c3fc0` caches radius-specific
`FogNode` graphs, and `4c26d0` constructs their dependencies. The span path
`4c3e70` expands octant samples generated by the integer recurrence at `4c4e40`;
`4c51f0` clips/stamps a row into current/explored planes. The main stamp propagates
blocker state through paired predecessor nodes and compares encoded heights.
These are static-code findings, not a qualified replacement for the shared
ledger's current circle/ray sampling. The native grid constructor, height/blocker
inputs, source radius quantization and graph ordering still need independent
runtime checks before changing that shared gameplay state.

All three original-client sessions used private Docker/Wine/Xvfb and a null audio
sink, with no network, host display/audio sockets or GPU devices. They were
stopped and verified exited. Original bytes, decompilation and read-only records
remain in ignored local evidence folders.

A third-session resource diagnostic reconstructs the original mine's source-grid
origin and BLP mask blit. Of its 256 CPU patch pixels, 228 match; the remaining
28 are white pixels from later marker overlays. This supports the existing
resource placement without a screenshot-fitted offset, but does not qualify the
complete overlay ordering or final UI filtering. Pillow's BLP reader supplied
an all-zero alpha plane for this input; the diagnostic uses the project's BLP
decoder instead. This diagnostic does not exclude any pixels from the reported
full-screen comparison.

## Blocked-source preservation and sight graphs (2026-11-04)

The shared vision stamp no longer writes `Fogged` when a sight ray is blocked.
The native unit routine `4c62b0` selects identity mask stores for a blocked graph
node, preserving earlier sources and exploration. The former recreation write
could erase another unit's clear sight; it also presented a newly blocked,
unexplored cell as fogged rather than masked. A regression test fails before
the fix and passes afterward. It covers both source orders, removal of the clear
source with retained exploration, and fresh unexplored cells. All 24 vision
checks pass. The existing height/ray blocker calculation remains a recreation;
this change qualifies the blocked-write behavior, not the blocker decision.

A fourth paused original-client session retains eleven radius-specific span
and `FogNode` tables plus the height plane. It used the same isolated private
display/null-audio setup and was stopped and verified exited. A source-operand
reconstruction matches all 209 span rows. A geometric reconstruction matches
all 3,315 node coordinates and their predecessor-coordinate pairs, including
asymmetric boundary omissions. Node order is not asserted identical. Radius
conversion follows `4c3bf0`: multiply by the map scale, then use the shared
`9a1d70` add-bias/floor rounder. These are selected-client findings, not a
hard-coded radius list.

The initial offline comparison mislabeled the two retained fog planes and reported
26 missing cells. The corrected interpretation is: FogMap `+0x2c` stores the
complement of explored history; `+0x30` stores current visibility. The acquisition
filenames `minimap-current-grid.bin` and `minimap-explored-grid.bin` retain their
legacy labels, respectively, and must not be interpreted by their names. The
corrected graph propagation matches all 1,209 currently visible cells with zero
missing or extra cells in three retained snapshots. This uses native positions,
radii and heights; it does not establish tick identity with the published frame.

An independent asset reconstruction matches all 12,288 retained height cells.
It uses W3E corner layers, live automatic/script-created destructables and units,
and their catalog `occH` fields. Automatically created doodad flags are honored;
script-only tree records are not counted twice. No native height plane or cached
graph is imported into the game. Optional support/deck heights, flying sight and
wide-footprint occlusion remain unqualified.

Two production captures match all five published states and hold all 26 clock
receipts. Exactly 387 recreation pixels change, within the minimap. Full-screen
RGB differences fall by 61 to 975,997 of 1,228,800; mean absolute channel error
falls to 11.714486762152777. The selected repeat is byte-identical. Upkeep and
13 panel text rectangles remain exact. Strict native/WASM Clippy, both builds,
formatting and whitespace checks pass. Full minimap/screen parity, native tick
identity, special overlays, outlines and building shadows remain open.

## Native sight lattice integration (2026-11-04)

Campaign startup now supplies a selected-image sight policy to the canonical
vision ledger. Lattice pitch, nearest-vertex bias, height shift, radius rounding,
integer span recurrence and ownership gates are decoded from the reviewed image.
Live heights are derived from W3E and catalog/map overrides; native dumps are
comparison inputs only. Ground sight follows the graph's predecessor OR rule and
asymmetric boundary omissions. Circle/rectangle modifiers keep their separate
radius rounding and row/end-point rules. Rebuilds stage changes before committing,
retain exploration and skip unchanged source/height plans.

The movement table at `691a80` feeds the type's `+1a8` field at `67ce77`.
Construction compares that value at `66f055` and sets the `+164` sight flag;
`4c62df..4c6302` selects full spans while the `+40c` suppression count is clear.
The matching authored movement token is read from that table, rather than copied
as an enum value. On this branch the previously calculated eye height does not
participate in visibility. The recreation therefore uses inclusive full spans
for ordinary flying sources. Grounded-flying suppression, optional support/deck
height and wide occluders are still unqualified; positive wide occluders refuse
instead of receiving an invented footprint.

The explicit local-input Rust test compares every retained span and propagation
through each retained native graph against synthetic blockers. This qualifies
geometry and blocker accumulation, not live source state, rendered parity or
native node ordering. Production capture validation is recorded separately below.


Production validation: two isolated muted offscreen runs pass and match all five
published states byte-for-byte; all 26 acquisition clocks hold and selected
repeats match. Each production height lattice equals all 12,288 retained native
height cells. The exact whole-frame comparison improves from 975,997 to 974,859
of 1,228,800 differing pixels, with mean channel error falling from
11.714486762152777 to 11.652511393229167. Only 2,511 recreation pixels change,
inside minimap bounds `[108,760]..[236,884]`. Upkeep and 13 panel text diagnostic
regions remain exact. Full difference, mask, overlay and side-by-side images are
retained locally. Remaining fog edges/history, overlays and camera-outline pixels
are still counted; this is not a pixel-perfect or native-tick acceptance.

Strict native all-target/WASM Clippy, native/WASM builds, formatting and whitespace
checks pass. No dependency was added.


A third/fourth capture also retains the production viewer ledger and its source
list locally. It has 1,242 visible cells against the separately paused native
observation's 1,209: no missing cells and 33 extras. Production tower sources
read authored sight 1,600 and select graph radius 13; all four retained native
towers have cached radius 12. Substituting radius 13 only for those four towers
in the offline reconstruction reproduces the production ledger exactly, including
all 33 extras. This isolates the residual to live sight-value/radius selection,
not the retained height plane or graph propagation. `67c490` obtains the active
value through `69e130` before calling `4c3bf0` and storing `+214`; the recreation's
base day/night reader does not yet model that live value path. No tower-specific
radius or screenshot correction is installed.

Exploration is a separate residual: production has 1,268 explored cells against
1,241 in that retained native snapshot, with six missing and the same 33 extras.
Native sample tick identity is unqualified. The local CPU-image comparison has
304 differing alpha bytes; it is a separate-time diagnostic, not a replacement
for the complete rendered-frame comparison. Four production runs match all five
published states and hold their 52 acquisition clocks. Both later source/ledger
diagnostics leave published screenshot bytes unchanged.


## Live sight endpoints and JPEG precision (2026-11-04)

The four tower-radius differences above were not authored overrides. Read-only
acquisition finds profile sight 1600 (`44c80000`), but the completed live property
is 1599.9998779296875 (`44c7ffff`). The active getter chain is
`67c490 -> 69e130 -> 800d80`; `4c3bf0` converts that property to the cached radius.
The profile reader/setter retains the authored value exactly. Day/night changes
at `6985e0`/`6987b0` pass their difference through `680530` and numeric division
`9a02d0`. The duration is initialized at `4015c0`; both division operands at
`680557`/`68055c` reference that same global. The decoder reads and validates these
instructions instead of embedding a duration or tower-specific correction.

Native numeric division, multiplication and addition leave a rounding residue
when the rate is integrated over the completed transition. The recreation now
retains that property on each unit, including hidden units, and stages updates
with the vision rebuild. Synthetic tests vary the duration and reciprocal data,
exercise signed residuals and reject malformed operands. The explicit supplied-
input test compares completed live properties to profile-derived results.
Intermediate transition timing is still a recreation shortcut, not qualified
native behavior; completed endpoints do not establish tick identity.

A separate read-only CMinimap acquisition retains the original base and CPU
images from the same local Prologue02 map. The base repeats byte-for-byte while
paused. Its BLP1 JPEG stream supplies its own dimensions, quantization tables and
coefficients. Existing JPEG conversion/channel order was correct, but upstream
jpeg-decoder's full-size 12-bit transform changed 2,172 RGB pixels by one shade;
its portable path still changed 568. Native code around `9d5c19..9d5d18` contains
the standard 13-bit integer transform constants. These are JPEG algorithm
invariants, not copied game content or measured asset values. The corresponding
standard algorithm is documented in
[libjpeg-turbo jidctint.c](https://github.com/libjpeg-turbo/libjpeg-turbo/blob/main/src/jidctint.c).

The existing Rust jpeg-decoder 0.3.2 is now vendored with a 13-bit full-size
transform and signed rounding. BLP enables its portable path so architecture
specific 12-bit dispatch cannot change results. Reduced-size transforms retain
upstream behavior. This adds no native imaging dependency. Synthetic transform,
saturation and bounded/malformed BLP checks remain in place. The explicit local
`native_minimap` test compares every RGBA pixel of the supplied BLP with a supplied
native BGRA buffer: all 65,536 pixels now match exactly. This proves that base
image for this selected client/map, not every JPEG or final rendered minimap.
Portable decoding replaces SIMD dispatch; startup decoding cost has not been
benchmarked, and no elapsed-time performance acceptance is claimed.

Local evidence is under `.scratch/minimap-sight-live-20261104`,
`.scratch/minimap-sight-native-map-20261104` and
`.scratch/minimap-texture-native-20261104`. Both reference clients were isolated
on private Wine/Xvfb/null audio, with no host display/audio/GPU or network; stopped
status is retained. Source executable SHA-256 is
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`;
map SHA-256 is
`ffa1ac3550b708a802b50ae02de54e52ef62c3dc17114e258998d5f93fe98f20`.
Local decompilation of `680530` and `63fd30`, instruction windows and runtime
receipts support the findings above. Original bytes/dumps remain ignored locally.

Production rendering in `.scratch/minimap-jpeg-after-20261104` and
`.scratch/minimap-jpeg-verified-20261104` matches five published states in two
independent runs, with all 26 acquisition clocks held. All 1,209 current visibility
cells and 12,288 height cells match the separately paused native observation.
Exploration has 1,235 cells against native 1,258: 23 missing, no extras. History
and intermediate transition timing must be qualified before interpreting that
residual as a spatial calculation error. The native CPU texture from the separate
texture acquisition differs in 16 RGB pixels (one marker area) and 162 alpha
bytes; its tick/camera state differs and this is diagnostic only.

The old retained reference was cleaned before this work. Against the newly
acquired 1280×960 reference (SHA-256
`2a0bffe246848512d274a01270991f2d77517b73e7dded92e39d69859f72bd92`),
the fresh baseline has 974,765 differing pixels; sight endpoints reduce that to
974,655, and JPEG precision to 972,999. Mean channel error changes from
11.614192979600695 to 11.609236924913194. The entire frame is compared with no
resizing, registration, tolerance or excluded pixels; full diff, mask, overlay
and side-by-side are retained. JPEG loading also changes other textures, so this
iteration changes 72,554 recreation pixels across the frame. Native tick/RNG/
animation identity, explored history, marker memory and camera-outline geometry/
rasterization remain open. This is not pixel-perfect minimap acceptance.

Final checks for this iteration: 2,499 gameplay library tests pass (98 explicit
original-input tests ignored); all six sight tests, including the supplied native
property gate, pass. Decoder/BLP tests and the supplied base-image gate pass, as
do both decoder documentation examples. Strict native all-target and WASM Clippy,
native application build, WASM gameplay/BLP builds and the full WASM application
build pass with warnings denied. Formatting, whitespace and screenshot/source
provenance hashes pass. Live decoder timing and native Windows rendering were not
measured; historical span/graph fixtures cleaned before this iteration were not
reacquired or rerun.


## Finite outline frustum and aliased line coverage (2026-11-04)

The previous outline's infinite ray/plane intersections did not reproduce the
selected client's finite frustum operation. `63ff50..640003` retains far corners
unless the near/far heights bracket the target plane with sufficient separation.
On that branch the source adds `near + (near - far) * phase`; substituting the
usual intersection sign changes its extents. The minimum separation is now read
as a finite positive double through the operand at `63ffab`, rather than copied
into a constant. Synthetic operands vary it and reject nonfinite values.

The first finite-frustum candidate narrowed the outline but increased its height;
its full-frame difference count regressed to 973,009 and it was rejected. Native
read-only acquisition and the source setup path `617ea0 -> 973ea0 -> 973f70`
qualified the camera inputs. A separate rendering issue was then isolated:
outline Update ran before Bevy's camera projection refresh. The campaign camera
recreates its projection component with an initial aspect, which the camera
systems later normalize to the actual viewport. Outline generation now runs in
PostUpdate after `CameraUpdateSystems` and before UI preparation. It therefore
uses the finite frustum at the aspect rendered in that frame. The resulting
extents agree with the retained native quad to within 0.00005 display pixels in
the selected observation. Native terrain target smoothing remains unqualified.

The UI border also differed from the client's aliased line coverage. The outline
now draws four bounded pixel spans using integer pixel centers and directional
diamond exits. The source clipped-edge flags at `640763..640a96` suppress sides
outside the control, rather than inventing a border along the clipping boundary.
Source-authored frame/viewport relationships determine positions; pixel spans
are expressed relative to the HUD target so world internal-resolution changes
do not change the screen stroke width. No screenshot dimensions or offsets are
installed as asset values. Synthetic tests cover direction, degenerate input,
source clipping, and suppression at each map boundary.

[Microsoft's D3D9 rasterization rules](https://learn.microsoft.com/en-us/windows/win32/direct3d9/rasterization-rules)
corroborate integer pixel centers and the GDI-style aliased line contract;
[the documented diamond-exit model](https://learn.microsoft.com/en-us/windows/win32/direct3d11/d3d10-graphics-programming-guide-rasterizer-stage-rules)
provides algorithm context. These API references do not establish native Windows
hardware output. Qualification here uses the retained selected-client Wine
frame, not an assumption that every GPU has identical tie behavior.

Local evidence and the failed candidates remain under
`.scratch/minimap-outline-trace-20261104` and the adjacent outline capture folders.
The new original-client run used private Xvfb/null audio, no host display/audio/
GPU mounts and no network. Its reader only used process-memory queries/reads;
source image and map identities are unchanged. Both isolation and stopped-state
receipts are retained; original bytes and dump contents remain local.

The selected production capture now reproduces all 97 outline pixels in the
retained native frame; its white-pixel mask has no differences in the diagnostic
outline box. This is a selected-frame result, not full minimap or every-camera
acceptance. Only 141 recreation pixels change, all inside the outline's screen
bounds. The exact whole-frame RGB comparison improves from 972,999 to 972,883
of 1,228,800 differing pixels, with mean channel error decreasing from
11.609236924913194 to 11.583758409288194. It uses the unchanged native reference
SHA-256 `2a0bffe246848512d274a01270991f2d77517b73e7dded92e39d69859f72bd92`;
there is no alignment, resizing, tolerance or excluded pixel. Full diff, mask,
overlay and side-by-side are retained. Exploration history, marker memory,
source-height smoothing, other resolutions/line ties, shadows and native tick/
RNG/animation identity still require qualification.

Final validation: 2,499 gameplay library tests and 939 application tests pass
(98 and 115 explicit original-input/GPU tests ignored respectively). Seven draw-
policy checks including the selected-client operand gate pass. Strict native
all-target/WASM Clippy, native/full-WASM application builds, formatting and
whitespace checks pass. Two isolated production captures match all five published
states and hold all 26 acquisition clocks. Viewer grids, height grids and the
CPU minimap bytes retain their previous hashes. The four fixed-size line spans
bound per-frame work; no elapsed-time/FPS performance gate is claimed.
Screenshot/source provenance is refreshed.
