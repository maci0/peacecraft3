# Script weather areas

`AddWeatherEffect`, `EnableWeatherEffect` and `RemoveWeatherEffect` now use the
session's existing bounded handle arena. Their signatures and declaration source
are checked against the loaded script; `weathereffect` is a direct child of
`handle` in the selected `common.j`. Weather definitions come from the resolver's
selected `TerrainArt/Weather.slk`, preserving provenance and a digest. There is
no copied stock effect list or default particle profile.

The catalog retains texture, emission, motion, lifetime, atlas, head/tail,
start/mid/end appearance and ambient-sound fields. Missing headers, duplicate IDs,
malformed numeric/boolean fields, nonfinite values and exceeded input budgets
refuse with source context. An effect ID absent from that catalog is not replaced
with another weather type.

## Selected native evidence

REA/Ghidra 12.1.3 inspected the local selected executable with SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
The following are native-code observations, not rendered comparisons:

- Registration at `4a0a55` selects `489e10`. It resolves the rectangle and copies
  its four bound words before calling `703650`; weather therefore does not retain
  the mutable rectangle as its geometry.
- `763af0` looks up the requested weather definition, clamps bounds to its world
  extent and publishes a weather area. The host clips to the admitted terrain/
  pathing extent using the source-derived cell scale.
- `763b9f` initializes a constructor local; `763cb1` copies it to the same area
  member compared and written by `EnableWeatherEffect` at `764c90`. Runtime input
  preparation decodes that relationship and value from the selected image.
- `764be0` changes the matching area's enable state. `764850` removes its entry.
  Missing-area paths issue a warning and return, so repeated removal and enable
  after removal do not resurrect an effect or abort the script.

| Checked role | VA | Bytes | SHA-256 |
|---|---:|---:|---|
| Rectangle copy entry | `489e10` | 48 | `74457f89c1f9c4954f220faf0db941ac23e685914a73af3076e4b401e430dcdf` |
| Initial value | `763b9f` | 7 | `a4e7f9bfe55a6c4466d6cefcd09870bbf59bd14a535398cdb02251ad4cf1cb35` |
| Published member | `763cb1` | 6 | `180fbe83c2b7d28ea379e9748d5703054a689d34a3d31b823be955639b272bc6` |
| Enable compare/write | `764c90` | 10 | `f4337716a098e431041b7cdec2df210777aa48706586d93699c71167b7bf4b28` |

Synthetic tests exercise nonstock definitions, enabled defaults, copied/clipped
bounds, rectangle removal, shared identities, enable/remove, bad ABI/arguments,
unknown rows and capacity refusal. An explicit local-original test reads both
`PCX_WEATHER_SLK` and `PCX_WAR3_EXE`; it is ignored without those supplied files.

## Fidelity boundary

This slice exposes canonical weather state and removes the unsupported-native
startup failure. It does **not** draw rain/snow or play weather ambience yet.
Particle emission density, camera/terrain placement, blending and ambient audio
still need connected consumers and headless/muted reference comparisons. Do not
count these stateful natives as complete weather presentation.
