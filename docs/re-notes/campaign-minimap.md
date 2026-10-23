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
input gate and report unsupported minimap camera/order projection; they do not
fall through to ground selection/orders. Cinematic interface suppression and
camera suspension still hide the entire control.

The existing fog host retains executed modifiers but explicitly does not
simulate visibility. Displaying the full map image through unexplored fog
would therefore be an incorrect completion of the blank minimap. Fog-enabled
rendering remains open: the terrain image is displayed only while both admitted
fog switches are explicitly false. Unknown settings and enabled fog refuse that
image projection. Standard Human01 has both switches true after cinematic skip,
so its minimap image is still hidden at that checkpoint. World fog rendering,
exploration history, sight blockers, markers, units, camera overlay, panning and
minimap orders remain unfinished.

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
The grid producers, fog-switch identity, world-to-minimap transform and retained
exploration history still need qualification before enabling the fogged image.

Complete static dossiers remain ignored under
`.scratch/campaign-minimap-source/`. The constructor evidence is
`ev_3eca7ff35417fcc6a5c906f4aeec2c14652928d930fe870afe09a72358be4b33`,
the updater evidence is
`ev_a2f06a059f46e1064a9e51f3006b8f522ec8aaefb5c91f55c338828a197e470b`,
and the fog function evidence is
`ev_ea4d3d461bd6787d1f8e77c056880b87c9d74df4230a5703ef45ed2a109c8331`.
These are static observations on the image identified above, not a client run.

## Runtime evidence and limits

All 626 app tests pass, including decoded synthetic pixels, source attempt order,
fog gating, pointer consumption and recursive session/asset retirement. Strict
native all-targets and WASM app Clippy, the default native build and formatting
pass. Core code is unchanged from the 1,679-test reader gate above.

The production offscreen diagnostic retains its unchanged menu-to-Human01
checkpoints, then separately executes `FogEnable(false)` and
`FogMaskEnable(false)` through the host and captures the image projection. This
mutation is labelled in `minimap-no-fog-diagnostic.json`; it is not evidence of
unchanged campaign visibility. The retained Vulkan run is
`.scratch/campaign-minimap-render/run-3/`: the decoded `war3mapMap.blp` is visible
inside the source-derived console rectangle, with no minimap error after the
explicit mutation. The image SHA-256 is
`edbb3eb2b3ac59e4f1238dcfc6543f7d984f111e0de67bf13849a07bcdb87e5c`.
The **strict unchanged rendering gate still exits 101** for the known sky
illumination diagnostic and unsupported minimap fog. It was not weakened.

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
receipt directory. The later dimension guard passes the app suite, native/WASM
checks, default native build and the hardware offscreen run above; that guard
did not change the input implementation. This replay used software Vulkan and
disabled audio. Failed display-setup logs
are preserved separately; the app and private display were stopped afterward.
Neither run establishes original-client pixel parity, fog fidelity, audio or a
completed campaign.
