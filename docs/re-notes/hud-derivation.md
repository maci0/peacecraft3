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

Unresolved:
- Production integration and Bevy spawning are out of scope for this slice.
- Original console FDF traversal has not yet been connected to a live asset;
  synthetic fixtures exercise parser and derivation behavior.
- Native HUD behavior beyond anchor/extent derivation is not established by
  parser or layout data.
