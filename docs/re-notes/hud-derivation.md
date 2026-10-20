# HUD frame derivation

Bounded derivation in `crates/peacecraft3/src/hud.rs`, with resource planning in
`hud_assets.rs` and Bevy mounting in `hud_render.rs`.

Inputs:
- parsed FDF from `crates/peacecraft3/src/fdf.rs`
- caller-supplied authored root frame names
- caller-supplied viewport in frame units

Output:
- `HudLayout`: roots, viewport, and derived `HudFrame` records in visitation order
- each `HudFrame` retains name, kind, inheritance, parent/children, authored
  records, resolved `SetPoint` (including inheritance), resolved rectangle,
  width/height where authored, and dependency edges

Dependencies are represented as `HudDependency`:
- `Template`
- `Parent`
- `Anchor`
- `Label`
- `Child`

No measured constants, race mappings, command actions, or native role assignments
are invented by the module. Provenance (archive, edition, version) remains the
caller's responsibility for the supplied FDF and root names.

Checked derivation policy:
- Missing templates and inheritance cycles are errors even when an overriding
  dimension would otherwise hide the broken relationship.
- Missing label dependencies are errors; enclosing parents are also traversed
  and validated when their rectangles are used.
- Duplicate interpreted records, negative/non-finite extents, non-finite anchor
  offsets, and conflicting anchor modes are refused with frame/property context.
- An unanchored root may use the caller's finite positive viewport. A child
  needs authored/inherited dimensions or `SetAllPoints`; it cannot silently take
  the viewport as its missing size.
- Multiple anchors and `INHERITS WITHCHILDREN` remain explicitly unsupported.
  No template-child cloning or native precedence is assumed.

These are checked import restrictions, not evidence of native rejection policy.
Ten HUD tests and the existing FDF suite pass with warnings denied. A source-leaf
WASM run separately checks six malformed refusals and one valid layout. The
private baseline/review evidence remains in `.scratch/catjam-supervision/`.

Unresolved:
- `campaign_hud.rs` produces the policy for the published campaign session:
  `ConsoleUI` from `UI\FrameDef\UI\ConsoleUI.fdf`, `UI\war3skins.txt` with the
  skin section matched case-insensitively to the local player's executed
  `RACE_PREF_<NAME>` constant, then `[Default]`, and the `Texture` files those
  resolve to (`hud_assets::texture_members`, `.blp` decoration). An unset race
  refuses. Human01 mounts the Human tiles. Other console frames (resource bar,
  upper buttons, minimap, portrait, command card) are not mounted.
- The world is drawn behind the whole console, so transparent console texels
  show terrain. Whether the original clips the world viewport above the console
  is unmeasured.
- Native HUD behavior beyond anchor/extent derivation is not established by
  parser or layout data.

## Resource planning and mounting

`HudSubmit` accepts explicit source-qualified inputs. The admitted-map path uses
`HudSourcePolicy`: named FDF, skin, image and font reads through the resolver for
the exact published receipt `Arc`, after `MapRequestSet::Deliver`. A successful
outcome alone does not identify a parsed context. Revision, outcome identity,
edition/version and receipt ownership are checked before reading. A changed
policy remounts; unchanged accepted or refused input is not read again.

Caller limits bound input files, decoded image buffers, retained payloads and
the input-plus-decoded staging sum. These are payload limits, not a bound on
parser, font backend, metadata or total GPU memory. Images are header-checked
before decoding; font registration precedes font asset insertion. A failed
candidate leaves the mounted tree and its handles intact. A successful candidate
replaces both. Frame placement stays relative to its parent; shadow offsets use
the caller's logical surface size. Text does not interpret menu access-key runs.

Texture records retain authored order, dimensions, anchors and `TexCoord` ranges.
Descending ranges use image flips. `BLEND` and `ALPHAKEY` use the ordinary image
path supported by the pinned viewer reconstruction; no colour key or alpha
cutoff is invented. `ADD`, unsupported records, missing bindings, ambiguous skin
keys and `FIXEDSIZE` text refuse the candidate. Treating v=0 as the decoded top
row remains an interpretation awaiting a native texel comparison.

Synthetic checks cover ownership, changed viewport, unchanged-frame read counts,
font limits before copying, malformed fonts, image failure and mounted-tree
rollback. These checks establish resource handling, not native texture sampling,
text rasterization, rendered composition or a playable chapter.

A local Vulkan readback on a Radeon RX 7900 XTX passes 48 channel checks for
normal and descending texture ranges, image orientation and decoded transparent
pixels. Explicit compatibility and Legacy ConsoleUI inputs also pass the mount
gate. The Legacy input selects the base FDF, expansion-locale skin and named
base/expansion textures explicitly; this does not establish native archive
precedence. Source hashes and bounded read receipts remain in local evidence.
These are resource and renderer checks, not a native screenshot comparison.
