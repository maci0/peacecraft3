# Console texture blocks

The shared application FDF parser now preserves `Texture { ... }` blocks on
their enclosing frame, in authored order. Previously each closing brace popped
a frame: the first texture closed `ConsoleUI`, and later records lost their
owner. Texture records now have their own block state and never set the frame's
width, height or anchor. Unsupported block kinds and textures without an owning
frame return contextual errors.

The existing `Fdf::parse` and `parse_into` entry points remain the callers' parser.
No texture path, dimensions, UV rectangle, color or anchor is copied into
production constants. Texture interpretation, HUD composition and rendering
remain separate work.

## Selected original inputs

Each archive was selected explicitly; these observations establish no archive
priority or release identity through directory adjacency. Temporary extracts
were removed after parsing.

| Source | Member | Bytes | SHA-256 | Parsed graph |
| --- | --- | ---: | --- | --- |
| compat `wc3/game/app/war3.mpq` | `UI\FrameDef\UI\ConsoleUI.fdf` | 1963 | `76a3208eef2cbfa8e77f3926c8df035b3c9e7338d30271ef6c24627ed09cfcdd` | one frame, nine texture blocks |
| caller-selected Legacy `War3.mpq` | same explicit member | 2770 | `155757357f40abf6f5ae46da7ebe148af2eb7b1c59b6293ff4e5aa9458e61559` | one frame, thirteen texture blocks |

These are distinct supplied artifacts. Both pass the tracked parser; neither
texture count is a production assumption.

## Validation and limits

All 13 parser tests pass in actual WASI execution, including texture/frame
property isolation, frame ancestry, the record after a texture closes, inherited
menu layout and unknown-block refusal. The full application suite passes 4
library, 323 application and 84 integration tests, with 15 explicit application
ignores. All-target/all-feature scoped Clippy with warnings denied, the WASM
application compile check and formatting pass.

This establishes parsing and compatibility, not a rendered HUD, native texture
sampling or pixel-perfect output. The HUD placement adapter remains private and
is not called by the running game.
