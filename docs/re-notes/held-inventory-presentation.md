# Held inventory presentation

Sources: the selected Legacy `Warcraft III.exe`, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`, plus the
selected `UI\war3skins.txt`, `UI\MiscUI.txt`, `GlobalStrings.fdf` and font files.
Implementation: `crates/peacecraft3/src/campaign_inventory.rs` and
`crates/pcx-play/src/inventory_layout.rs`.

## Derived from the client

- Layout: `Rules::reviewed_legacy` admits the reviewed client image by digest and
  decodes slot, cover, label and tooltip operands from checked instructions.
- Cover: `cover_uv` is the normalized `[left, top, right, bottom]` rectangle read
  from the cover pointer. It is applied as `ImageNode.rect` over the decoded image
  size, so the crop is pixel-exact.
- Tooltip checkbox: the instruction at `0x6491aa` (`push imm8`) selects the
  descriptor index. `Defaults::discover` on the same image yields the entry with
  that index, and its fallback is the seed. Only the exact `0` or `1` bytes are
  admitted, matching the options preview. The production join uses the index and
  never the entry name.
- Saved preference: `Navigation.applied.toggles["TooltipsCheckBox"]` wins when
  present. The seed applies otherwise. The seed is a source-derived runtime value,
  not an invented default. Nothing writes that toggle today: the options control
  is a read-only preview, and the apply path in `menu_settings.rs` writes only
  Music and Sound toggles and controls with an input owner.
- Refusal: an unavailable seed (missing index, malformed fallback, failed
  discovery) shows the name only and records the reason in `last_error`.

## Load and failure behavior

- Fonts: every read and skin lookup precedes `Assets<Font>::add`. A refused load
  leaves no font asset.
- Images: a failed decode for a `(path, repeat)` key is memoized, up to 64 keys.
  Repeated frames return the memoized error without rereading the asset. Receipts
  are committed only when a decode succeeds.

## Verification

- `cargo test -p pcx-play --features test-fixtures --lib inventory_layout`:
  cover UV admission, slot and tooltip geometry, sampler admission. The
  `test-fixtures` feature exposes the synthetic `Rules` fixture to dependent
  crates. Within `pcx-play`, its unit tests build it under `cfg(test)`.
- `cargo test -p peacecraft3 --bin peacecraft3 campaign_inventory`: checkbox
  profile, saved-over-seed precedence, cover pixel mapping, and the ignored
  original gate.
- `cargo test -p peacecraft3 --features test-fixtures --bin peacecraft3
  campaign_inventory`: font transaction (two refusal cases and one admission
  case), failed-decode memoization, cover paint, and ECS hover tests. The ECS
  tests cover hovered held item, name plus seeded description, saved override,
  refused seed, despawn on unhover, and an empty slot. The feature enables the
  synthetic layout and the `CampaignCamera::active_for_test` constructor. Default
  native and WASM builds contain neither.
- Ignored original gates (`PCX_PLAYER_RESOURCE_IMAGE`):
  `supplied_client_inventory_layout_and_tooltip_operands_are_admitted` in
  `pcx-play` and `supplied_client_tooltip_index_joins_its_descriptor` in
  `peacecraft3`. The second verifies the index joins a descriptor whose name is
  `tooltips`, a check made only in the gate.

These checks cover parsing and ECS state. They do not establish rendered parity.

## Open

- Rendered comparison of the cover crop, slot art, label and tooltip against the
  original client.
- Item use, drag, drop and give refuse with a logged message. Their behavior is
  not implemented.
- A saved tooltip toggle needs a menu owner before it can be written.

## Native GUI verification (not yet run)

Private harness: `.scratch/gui-item-20261020/`.

- `compile_item_gui.py` builds `item-gui` from `.scratch/campaign-objective-20261020/item-gui.rs`. The closure comes from cargo's artifact messages, keyed by package ID, so duplicate crate versions do not collide. Direct externs come from `cargo metadata` resolution.
- `gui_item_verify.py BINARY RUN_DIR` launches the binary under `DISPLAY=:97` with `WAYLAND_DISPLAY` unset and `VK_DRIVER_FILES` set to the lavapipe ICD. It finds its own window with `xdotool search --pid`, and terminates only its own process group. Preferences go to a fresh `XDG_CONFIG_HOME`, so the user's settings are neither read nor written.
- The run right-clicks the fixture item at the reported screen point. It then hovers the held slot's rectangle, which comes from the `held slot` debug line. The rectangle is source-derived; the driver does not place slots by hand. Evidence is `evidence.json` and `hover.png`.
- The menu route (`MENU_ROUTE`) is inherited from `gui_normal.py`. It is unverified against this build.
- Debug lines added to `campaign_inventory.rs`: `inventory held slot` on paint, and `inventory tooltip` on projection. Both use `debug!`.

Not yet established: pickup and hover on a rendered frame, rendered slot icon and empty-capacity cover, tooltip text, pointer blocking from the world, and the Defeat presentation gap. The fresh binary waits for the integrator's build window.

## Native label bank (runtime finding)

- The inventory label key is a pushed string operand at VA `0x61ea25`, read from the
  selected Legacy `Warcraft III.exe`: `INVENTORY`. It is read by
  `.scratch/gui-item-20261020/read_pushed_string.py`, standard library only.
- The campaign loader initially read only `UI\FrameDef\GlobalStrings.fdf`. The
  RoC locale copy in `War3Local.mpq` has no `INVENTORY` record. The key is in
  `UI\FrameDef\InfoPanelStrings.fdf` (same archive, one `StringList`, line 119:
  `INVENTORY "Inventory"`). The file header makes identifiers unique across
  StringLists, so the lookup matches one record across the selected lists and
  refuses zero or two matches.
- Runtime evidence (`runs/route-2/gui.log`, 13:34:22Z): the Human01 map mounted
  (`source=Maps\Campaign\Human01.w3m`), the synthetic ledger was created
  (item 1608, screen 411.1, 349.4), and the log reported
  `campaign inventory: missing source inventory label`. The pixels in
  `runs/route-2/before-pickup.png` show the map and dialog with no inventory panel.
- Status: the label-bank change and two tests (unit and ignored original gate
  `supplied_client_label_bank_resolves_the_pushed_inventory_key`) are written but
  have not compiled. The shared `pcx-play` tree was mid-refactor with unresolved
  `unit_hero_host` imports and missing `unit_hero_bonus` methods, owned by another
  worker. No rebuild or gate result exists after the change.

## Private fixture and observer status (not built)

- The private fixture (`.scratch/campaign-objective-20261020/item-gui.rs`) places the
  synthetic ledger at the first frame where `PresentationState.control_enabled` is
  `Some(true)` and a hero with a position exists. It no longer waits for a fixed
  game clock. Its offset from the hero is unchanged.
- Each observed change prints `FIXTURE state clock=… control=… ui=… interface=…
  hero=… life=… exits=…`. The clock is `timer_game_time`. The hero is the first
  local-owned unit whose raw code starts uppercase, the existing fixture heuristic.
  Exit requests are read through the slice accessor `game_exit_requests`, not drained.
- Pickup and screen-point lines also print the clock.
- The state lines show whether control, UI, life, or exit requests change before a
  banner appears. They do not identify the authored Defeat dialog. A banner
  without a dialog is not dialog evidence.
- Not yet built or run. The build needs the Cargo slot, and the driver's pickup
  wait is 20 s.
- The observer also logs `paused` from `ObjectHost::game_paused()` and each dialog
  visible to the local player from `dialog_state()`: id, message, and buttons
  (id, text, hotkey, `quit_score_screen`). A `quit_score_screen` value marks an
  authored quit button. `quit_requests()` is read as a slice; the draining
  `take_dialog_quit_requests()` is not called. Not yet built or run.

## Authored Defeat Quit identification (private observer, not built)

- Human01's Defeat dialog is built by `CustomDefeatDialogBJ` in the selected
  Blizzard.j: `DialogCreate`, `DialogSetMessage`, then `DialogAddButton` with
  `GAMEOVER_RESTART`, optionally `GAMEOVER_REDUCE_DIFFICULTY`, `GAMEOVER_LOAD`, and
  `GAMEOVER_QUIT_MISSION`. The Quit button's trigger action is `CustomDefeatQuitBJ`
  (`PauseGame(false)`, `SetGameDifficulty`, `EndGame(true)`). It is an ordinary
  `DialogAddButton`, so `quit_score_screen` is `None` for it.
- Source match, read from the selected archives: text `GAMEOVER_QUIT_MISSION` in
  `GlobalStrings.fdf` (`"|CFFFFFFFFQ|Ruit Campaign"`), hotkey `GAMEOVER_QUIT_MISSION='Q'`
  in `UI_MiscUI.txt` (81). Checked by `check_source_strings.py`.
- Render position: the app spawns each dialog button with `Name("Script dialog button")`
  and a computed node, placed top to bottom in `dialog.buttons` order. The fixture
  reports each centre in physical pixels, matched only when the rendered and
  recorded counts agree.
- Not identified: the callback link (`TriggerRegisterDialogButtonEvent` trigger to
  `CustomDefeatQuitBJ`). The host stores that registration, but no public accessor
  exposes it, and trigger-action handles can't be enumerated. A read-only accessor in
  `pcx-play` would close this. That is a shared-source edit, so it waits for a
  coordinated window.
