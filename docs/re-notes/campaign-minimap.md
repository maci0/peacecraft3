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

## Still unimplemented

This reader does not mount a minimap or establish rendered parity. A runtime
consumer must use the exact admitted map bytes and published receipt identity,
bound extraction/decode allocations, and retire its resources with the session.
It must also qualify image orientation/crop, world bounds, texture sampling,
fog, unit/marker/camera projection and pointer actions. Parser tests do not
establish any of those behaviors.

The exported rectangle is the outer frame. The image method also constructs an
inset rectangle around `636e5f..636eba`; its relationship to the image quad and
world projection has not yet been admitted. A renderer must not assume the
outer anchors alone describe the image's visible texel bounds.

The existing fog host retains executed modifiers but explicitly does not
simulate visibility. Displaying the full map image through unexplored fog
would therefore be an incorrect completion of the blank minimap. Fog-enabled
rendering remains open rather than being replaced with an unmasked image.
