# Main menu → Campaign entry (native)

Reconnects the original `Main → SinglePlayer → Campaign` screen to the produced
campaign launch path. The work is native-only (`native_campaign` is
`cfg(not(target_arch = "wasm32"))`); the browser has no `CampaignStrings`
chapter caller and none is claimed here.

## What this slice does

- Reads the selected install's `UI\CampaignStrings.txt` /
  `UI\CampaignStrings_exp.txt` through `parse_campaign_index`, which now keeps the
  authored `CampaignList` order, each campaign's `DefaultOpen` availability
  (validated strictly: absent = closed, `0`/`1` accepted, anything else is an
  error) and `Background` skin key, plus the mission rows.
- Preserves the campaign/mission grouping instead of flattening campaign
  metadata (the fallback launcher card still gets a flat chapter view derived
  from the same parse).
- Binds the real `CampaignMenu` screen: campaign buttons (left) and, once a
  campaign is selected, its mission buttons (right). Selecting a campaign never
  launches; selecting a mission enqueues the shared
  `Configuration → MapRequest → campaign startup` path that already
  publishes the production session.
- Uses the shared `GlueButton` renderer for the buttons, so the authored
  `CampaignButtonTemplate` backdrop/pushed/disabled/mouse-over art and offsets
  come from source. Labels are styled from an authored FDF text frame
  (`MissionNameHeader`) with the source skin font.

## Source-derived choices

- Button geometry: `CampaignButtonTemplate` `Width`/`Height` read at runtime
  (`fdf.rect`), not a copied constant. Non-positive geometry refuses the panel.
- Button art/state: `glue_menu::button` + `glue_menu::Item`/`spawn`.
- Label face/size/colour: `render_style_with_parent` on the authored
  `StandardButtonTextTemplate` profile (the shared button-label style: `MasterFont`,
  `0.013`, `JUSTIFYCENTER`/`JUSTIFYMIDDLE`, `FIXEDSIZE`), resolved through the
  skin table and loaded from the install. `CampaignButtonTemplate` is a bare
  `TEXTBUTTON` with no `ButtonText` child, so there is no per-row authored text
  frame; no explicit class default/inheritance binding between
  `CampaignButtonTemplate`'s label and `StandardButtonTextTemplate` was found, so
  choosing the shared button-label profile is a **recreation/inference** (its
  style bytes are sourced and authored, but the relationship is not established
  merely by template existence); the `MissionNameHeader` screen header was not
  used as a fallback. Label **width** is derived from the authored
  `CampaignButtonTemplate` `Width` minus the authored
  `StandardCampaignButtonBackdropTemplate` `BackdropBackgroundInsets`, and the
  label honours the authored `FontJustificationH`/`V` and wraps within that width
  at the authored size — long mission names stay fully readable without spilling
  into adjacent columns or being clipped/truncated.

## Recreation choices (no authored position exists)

The FDF declares `CampaignSelectFrame`/`MissionSelectFrame` as containers with no
size or anchors (they resolve to the full frame), so the list **placement** is a
recreation policy: campaigns column at `4%` left, missions at `45%` left, both
starting `30%` from the top, stacking by the template geometry with a `25%`
height gap. These offsets are labelled recreation and are the only invented
values; nothing about item size, art or text is invented.

Mission-list **viewport scrolling** is not sourced: the FDF exposes no list
region, scrollbar art or scroll anchors. Deep mission lists remain fully
reachable, so the missions column is rendered inside a bounded, clipped viewport
whose rows scroll with wheel and keyboard-focus navigation, reusing the shared
Bevy `Overflow::scroll_y`/`MouseWheel` input pattern used by the settings lists and
keeping the focused row in view. This viewport is a labelled recreation geometry,
and no scrollbar art or asset values are invented.

## Open fidelity questions

- Difficulty: the `DifficultySelect` popup has no authored `MenuItem` choice in
  the FDF; it currently renders the generic `Unavailable` refusal. A
  source-qualified difficulty model is deferred, not invented.
- Campaign backgrounds (`Background` skin), fog/cursor/ambient keys are parsed
  and kept but not applied to the screen yet.
- Campaign unlock persistence (`DefaultOpen` is initial availability only;
  completion-based unlocks are not modelled). Initially closed campaigns are
  disabled in the list and refused at selection/mission activation.
- Mission rows are exposed without mission-specific unlock filtering; this is
  an entry-path implementation, not qualified original progression UI.
- Exact list coordinates and scroll behaviour are not sourced from the client;
  the mission viewport is a labelled recreation geometry (see above).
- Edition `_V0`/`_V1` data variants remain unselected (unchanged).

## Verification

Dynamic rows do not carry `ScreenPosition`: the generic FDF layout pass resolves
that component against the template origin, which would collapse every row onto
one position. Container and label nodes ignore picking so the button receives
the completed pointer click. A synthetic spawn test checks both boundaries.

- Unit tests (synthetic fixtures, no install required):
  - `campaign_parse_preserves_order_and_default_open_and_mission_rows`
  - `malformed_default_open_is_refused_instead_of_treated_closed`
  - `mission_row_background_key_does_not_match_prefix_fields`
  - `selecting_a_campaign_never_launches`
  - `button_inner_width_reflects_the_authored_backdrop_inset`
  - `fill_mode_row_art_fills_and_adds_no_interactive_components`
  - `mission_scroll_reveals_the_focused_row_and_clamps_wheel`
  - `pcx-objects` `campaign_index` suite (8 tests)
- Ignored install-gated integration: `selected_install_chapter_activation_publishes_the_production_session`
  (requires `PCX_NATIVE_CHAPTER_INSTALL` / `_MAP` / `_ADDON`).

Worktree-private checks (`CARGO_BUILD_JOBS=1`, `--locked --offline`):

| Check | Result |
|---|---|
| `cargo test -p pcx-objects --lib campaign_index` | 8 passed |
| `cargo test -p peacecraft3 --bin peacecraft3` | 479 passed, 45 ignored |
| `native_campaign::tests -- --include-ignored`, selected Legacy install / Human01 / RoC | 6 passed |
| `cargo clippy -p pcx-objects -p peacecraft3 --all-targets -- -D warnings` | exit 0 |
| Native `peacecraft3` binary build | exit 0 |
| WASM `pcx-objects` library and `peacecraft3` binary checks | exit 0 |
| `rustfmt --check` on the four changed Rust files | exit 0 |

## Physical functional entry observation (2026-10-22)

The corrected checkpoint's native executable has SHA-256
`36241f13d74e7847d8f4feb332f7df45862c74cbdfc888515e28e7cb0f11c811`.
It ran on an owned Xvfb display with software Vulkan, fresh isolated profile
storage, and the selected Legacy client `1.29.2.9231` (executable SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`).

Pointer/key input followed configuration RoC selection, original Main menu,
Single Player, Campaign, Human, Chapter One. It did not use the fallback
launcher's chapter buttons. The unchanged `Maps\Campaign\Human01.w3m` opened
with terrain, units and its opening cinematic; normal Escape input restored
the gameplay world, HUD, inventory and command card. The source-file hashes
remained unchanged through the run. Screenshots, input log, complete app log,
binary/source identities and reviewed receipt remain local under
`.scratch/menu-entry/native-run/` in the review checkout.

The initial review run exposed a second consumer treating the dynamic buttons
as unimplemented generic menu actions. `CampaignAction` now marks their actual
owner and excludes them from generic navigation. The regression checks marked
and unmarked controls; the corrected physical run shows no false notice.

This is functional entry evidence, not continued playable-campaign acceptance.
At game time 10 seconds, F7 and the full log show `GetUnitLoc` failing with
`handle identifies another family`. The native already has an implementation;
the evaluator's generic "not implemented" prefix must not be mistaken for its
underlying cause. Removed-unit handling and the triggering callback need
investigation. HUD clipping, scene/material fidelity, audio refusals, mission
completion and original-client parity remain unqualified. Software-renderer
timing is not a performance gate. Both owned app/display processes were stopped.
