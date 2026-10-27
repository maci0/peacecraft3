# Renderer screenshots

Fresh 0.7.0 captures from PeaceCraft III on 27 October 2026, using locally supplied classic assets. The production renderer ran headless at 1280×960 with offscreen Vulkan and audio disabled. These are rendered demonstrations, not original-client captures or pixel-parity evidence.

- `main-menu.png`: Reign of Chaos main menu.
- `chapter-selection.png`: campaign and chapter selection.
- `campaign.png`: selected Thrall in Prologue02, with the unit panel and command card.
- `research-menu.png`: source-derived hero skill menu and readiness states.
- `quest-log.png`: populated quest dialog with the Done button hovered.

The dedicated HUD capture test passes, including selection, skill-menu states, quest dialog interaction and upper-button states. This does not qualify the separate full campaign/item diagnostic or establish native pixel parity. Quest border appearance, row/status binding, scrollbar presentation, world rendering and effect icons still have fidelity gaps. The underlying quest notification is visible behind the dialog in this capture.

Image hashes and capture provenance are retained in `provenance.json`. Original archives, profiles, probe outputs and other generated game contents remain local.
