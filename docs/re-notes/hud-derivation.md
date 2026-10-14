# HUD frame derivation

Bounded source-leaf module `crates/peacecraft3/src/hud.rs`.

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
- Production integration and Bevy spawning are out of scope for this slice.
- Original console FDF traversal has not yet been connected to a live asset;
  synthetic fixtures exercise parser and derivation behavior.
- Native HUD behavior beyond anchor/extent derivation is not established by
  parser or layout data.
