# Renderer screenshots

Fresh 0.6.0 captures from PeaceCraft III on 26 October 2026, using locally supplied classic assets. The production renderer ran headless at 1280×960 with offscreen Vulkan and audio disabled. These are rendered demonstrations, not original-client captures or pixel-parity evidence.

- `main-menu.png`: Reign of Chaos main menu.
- `chapter-selection.png`: campaign and chapter selection.
- `campaign.png`: Prologue02 after the authored cinematic returns control, showing the world, minimap and command card.

The local capture diagnostic reached campaign control, then stopped because this chapter had no visible ground item required by its later item test. It also reported open unit-panel label, doodad model and sight-occlusion limitations. The full capture diagnostic is not recorded as passing.

Image hashes and capture provenance are retained in `provenance.json`. Original archives, profiles, probe outputs and other generated game contents remain local.
