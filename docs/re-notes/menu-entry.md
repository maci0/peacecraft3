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
- Label face/size/colour: `render_style_with_parent` on the authored text frame,
  resolved through the skin table and loaded from the install.

## Recreation choices (no authored position exists)

The FDF declares `CampaignSelectFrame`/`MissionSelectFrame` as containers with no
size or anchors (they resolve to the full frame), so the list **placement** is a
recreation policy: campaigns column at `4%` left, missions at `45%` left, both
starting `30%` from the top, stacking by the template geometry with a `25%`
height gap. These offsets are labelled recreation and are the only invented
values; nothing about item size, art or text is invented.

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
- Exact list coordinates and scroll behaviour are not sourced from the client.
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
  - `pcx-objects` `campaign_index` suite (8 tests)
- Ignored install-gated integration: `selected_install_chapter_activation_publishes_the_production_session`
  (requires `PCX_NATIVE_CHAPTER_INSTALL` / `_MAP` / `_ADDON`).

Build/check receipt:

```
cargo check -p pcx-objects -p peacecraft3 --tests
cargo test -p pcx-objects --lib campaign_index
cargo test -p peacecraft3 --bin peacecraft3 native_campaign::tests
```
