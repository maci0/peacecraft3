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
The recreation uses relative UI placement for that outer quad. House clamp /
linear sampling remains an implementation choice awaiting a paired native render.

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
switches are independent. The raster fits the admitted placement extent into the
actual map image, preserving aspect ratio and north-up orientation. It begins
updating while the cinematic camera hides the control and retires with the
session; changing viewers resets its history. With both switches false, the
authored image alpha is restored even when sight admission is unavailable.

This is a house visibility implementation, not a copy of native grid internals.
It still lacks terrain/tree sight occlusion and sight-changing upgrades/effects.
Presentation-frame exploration sampling, exact camera-margin projection,
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

This uses the current house map-image projection; exact native camera-margin
mapping and padding behavior still need comparison. It does not implement
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
- Gold mine, entangled and haunted mine, neutral building, Circle of Power: 16 by 16
  masked blits in `659220`, textures `MinimapResourceTexture`, `...EntangledResource...`,
  `...HauntedResource...`, `MinimapNeutralTexture`, `MinimapCopTexture` (`6376ae..63777e`).
  Selection: bit `0x20000` from `6d6570(0x41676d62)` ('Agmb', absent from `ngol`'s
  `Agld,Avul`, so the mine path is not the one the stock capture shows), bit `0x40000` for
  structures owned by slot 27 with a `+0x454 & 0x100` type flag, or 'ncop'/'ncp2'/'ncp3'.
  Mine variants come from `+0x168` bits 7 and 8. The yellow ball in the stock capture is
  more likely the `MinimapResource` model quad (`UI\Minimap\MiniMap-Goldmine.mdl`) than
  this blit; unresolved.
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
