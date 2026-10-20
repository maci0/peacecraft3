# Native and browser session peers

## Status

The session host runs the harness world only. A real map is refused with its
reasons, because the production startup that would make it playable is not
wired to this host. Peers verify the host's identity before they show state or
send a command.

## Wire and identity

Peers speak the binary protocol in `web/network.js`: a reconnect token, a text
greeting, binary snapshots, and `Input` datagrams. The greeting is required to
be `{"player":N,"tickMs":M,"session":"<64 lowercase hex>"}`. A greeting without
a valid `session` field is refused.

The identity is `pcx_net::host::session_identity(world, seed)`: SHA-256 over the
domain label, the seed and the encoded initial world. A peer derives it from its
own inputs and compares it with the greeting before play:

- Native: `SessionClient::connect(address, token, &expected)` returns
  `ClientError::Identity` on a mismatch and sends nothing.
- Browser: `new PeacecraftSession(url, expectedIdentity, token)`. A mismatch
  dispatches `rejected` with `{reason: 'identity', expected, host}` and closes the
  socket with code 4001. Snapshots are ignored until verification, and `commands()`
  and `resync()` throw until then.

The expected value must come from an independent counterpart, not from the host
under test. For the harness world, that counterpart is
`session_server identity`.

## Scheduling and latency

The host advances once per `TICK_MS` (25 ms, real time). A tick applies the orders
that arrived for it. A tick with no order applies no new order, so missing input
does not stall the host. A peer submits for the newest snapshot tick plus
`INPUT_LEAD_TICKS` (4, as in `network.js`). An order for a tick the host has already
run is refused with the sim's `CannotRewind` text. An order more than
`MAX_INPUT_LEAD` (120) ticks ahead is refused.

Native and browser peers do not predict. Displayed state is the host's snapshot,
delayed by the lead, the round trip and up to one tick. The protocol is correct
without prediction.

## Work bounds

These are operating bounds, named and explicit:

- `MESSAGES_PER_PEER_PER_POLL` (32) reads per peer per host pass, handshake
  budgets, and a 5 s join deadline.
- `MAX_SESSION_UNITS` (1024). A snapshot is at most 23 bytes per unit, which fits
  the host's 64 KiB write buffer.
- `pcx_sim` bounds each unit step to `MAX_SEGMENT_CHECKS` (512) cell checks.

The harness movement rule (0.1 world units per tick) is the harness's own rule.
It is not a game value.

## Map refusal

`pcx_net::map_session::assess(map_bytes, label)` lists what keeps a map from
being playable. `session_server check-map --map FILE` prints those reasons to stderr and
exits 3, without listening. The reasons are:

- `random-selection`: random placements. The unit is chosen at creation from
  authored random tables under the selected RNG policy. Neither is decoded here.
- `map-script`: `war3map.j` is present. Its initialization and creation triggers
  are not run here.
- `owner-roster`: owners are raw control words. Player slots come from the
  startup roster, which this host does not read.
- `harness-movement`: source unit movement is not implemented. Movement would be
  the harness rule.

The last two reasons apply to every map, so a map is never reported playable.

Melee maps often place only start locations and let the map script create
units. A placement-only census therefore misses units and does not show a
playable map. Nothing here invents starting units.

## Production startup required

Playable maps need the existing production startup to run headlessly and feed
this host. The chain lives in the Bevy app today:

- `peacecraft3/src/campaign_prepare.rs` calls
  `peacecraft3::campaign_inputs::prepare_new_game(context, bindings, reader,
  image, identity, seed, expansion)`. It needs `MapBindings` from
  `peacecraft3/src/map_requests.rs`, and that module depends on Bevy.
- `pcx_play::map_startup_host::assemble(context, MapStartupInputs)` builds the
  `MapStartupHost`, and `pcx_play::playercolor_host::ObjectHost` holds the unit
  arena.
- `peacecraft3/src/map_script_session.rs` drives `war3map.j` and creation
  triggers. The timer, unit-activity and callback paths are there too.

Publishing the startup needs a Bevy-free entry point in `pcx-play` or a
non-Bevy `peacecraft3` module. That entry point must take the parsed map context,
the source reader and the selected executable, and return the object arena. The
session host would then read units, owners and movement from that arena. Owners
come from the startup roster, and random placements from the authored tables
with the selected RNG policy. Building and running that path needs a root build
slot, so it is not verified here.

## Tests

- `tests/local_peers.rs` (feature `session-server`): loopback peers on the harness
  world. Shared authority, forged-identity refusal, disconnect stop, reconnect
  without rewind, and identity refusal at the greeting.
- `tests/map_peers.rs`: `bytes_that_are_not_a_map_are_an_assessment_error` runs
  always. `real_maps_are_refused_...` is `#[ignore]` and needs `PCX_SESSION_MAP`
  and `PCX_SESSION_OTHER_MAP`.
- `examples/check_browser_session.mjs` (run by hand with playwright-core):
  two browser peers with the counterpart identity in `PCX_SESSION_IDENTITY`.
- `examples/check_browser_identity.mjs` (run by hand): a page expecting another
  identity sees a `rejected` event with `reason: 'identity'`, sends zero binary
  frames, and cannot call `commands()` or `resync()`. The counterpart page joins,
  and its order moves only its own unit.

## Not covered

- Playable maps (see the production startup above).
- Owner-to-player mapping and random-placement decoding.
- The original's unit speed and collision. Harness movement is not game movement.
- CLI surface, per `AGENTS.md` (The CLI Spec v0.3 candidate, pinned at
  `tools/original-game-reference/schema/clispec-v0.3.json`, hash in its `CHECKSUMS.txt`):
  `session_server` is a binary (`src/bin/session_server.rs`). It has the commands
  `schema`, `identity`, `check-map` and `serve`, and a global `--format json|text`
  (default text, declared in `output`). Errors are `usage` (2), `runtime_failed` (1)
  and `unsupported_map` (3). In JSON mode each error is one envelope line on stderr.
  Legacy `--print-identity` and `--map FILE` remain aliases. `serve` keeps its text
  listening line on stdout.
  Checked: `tests/cli_process.rs` (12 pass, 1 gated real-map test) and
  `tools/session-server/test_schema.py` (3 pass, `schema` validated with jsonschema
  against the pinned bytes). Full CLI Spec conformance is not claimed. The spec's
  conformance tooling has not been run.
- Game input, lobby, combat, economy, and the original Battle.net and W3GS paths.

## Extraction for the production adapter (branch-local)

Startup preparation moved out of the Bevy crate into `pcx-play`, with no change to
its logic:

- `crates/pcx-play/src/campaign_inputs.rs` (from `peacecraft3/src/campaign_inputs.rs`):
  `prepare_new_game` and `prepare`, which return `MapStartupInputs`. Its map
  inputs are the new `MapSources { map_label, map_bytes, catalog_identity,
  catalog }`, which replaces `MapBindings`.
- `crates/pcx-play/src/campaign_assets.rs` (from `peacecraft3/src/campaign_assets.rs`):
  identities, day/night, file-sound and unit-model sources. Visibility is now `pub`.
- Callers: `peacecraft3/src/campaign_prepare.rs` is the adapter. Its `map_sources`
  builds `MapSources` from `MapBindings`. `campaign_startup.rs` and
  `campaign_placement.rs` now name `pcx_play::campaign_assets`.

Verified: `cargo check -p pcx-play --all-targets --offline` exits 0. The
`peacecraft3` crate has not been compiled: it needs the Bevy build, which is
waiting for a root build slot.

Still Bevy-coupled, and needed for a headless startup: `map_requests::prepare` and
`MapBindings` (binding and receipts), `native_campaign::configuration` (the install
`Resolver` and script identities), `map_script_session` (`war3map.j` and creation
triggers), and the assembly in `campaign_startup`.

## Production authority (branch-local)

`pcx_net::production::ProductionAuthority` drives the admitted map's startup from peer
batches. `pcx_net::host::Host` is generic over `SessionAuthority`, and the harness `Session`
remains its default.

Rules implemented:

- **Admission.** A peer joins only as a human slot of the map's own roster
  (`context.world.info.human_players()`). Reconnect tokens take roster slots in ascending
  order. A second human, a non-roster slot, or a bound slot is refused.
- **Packets.** `parse_packet` checks size, identity (the packet's player must be the
  peer's slot), command count and coordinate bounds. Tick checks belong to the authority:
  a tick already run is refused, one more than `MAX_INPUT_LEAD` ahead is refused, and a
  duplicate for a peer and tick is refused.
- **Gating before orders.** A batch is refused, with no native call, when the game is paused
  (`game_paused`), when user control is off (`presentation().control_enabled == Some(false)`,
  the application's rule), when the player owns no movable unit, or when it carries `Select`
  or `Stop`. The gate runs before any `IssuePointOrder`, so a script's own order calls are
  not the admission check.
- **Apply once.** A batch applies at the clock tick it names, then the clock steps one
  tick. Refusals go to a bounded log with the number dropped.
- **Fail-stop.** An order native that returns anything but `true` ends the authority rather
  than continuing from a partly applied batch.
- **Export.** A snapshot maps each executed unit to a wire unit with checked handle, owner and
  coordinate values. It reads `selection()` for the selected flag and `activity.destination()`
  for the move target.
- **Identity.** `startup.admission_digest()`: the executable, map, both scripts, catalog
  source, seed, mode and roster. Unit snapshots are not the identity.

Scope and limits:

- `Move` orders every owned movable unit, through the same `IssuePointOrder` native the
  application uses. The application orders its selected units, and the wire carries no
  selection, so this divergence stays until selection is on the wire.
- Coordinates become `f32` world units for orders and come back as wire fixed point, so a
  round trip is exact only to the `f32` precision of the position.
- Snapshot equality compares executed-unit identity, owner and current position. It does not
  establish VM globals, RNG streams or script-level parity.

Observations, from `production_authority` and `pcx-play` probes run during this change:

- **Human01 is paused for the window that was observed.** User control is off from tick 0 to
  tick 2315. At tick 2315 control is on, and the game is paused. The pause did not end
  within 131,072 ticks (about 34 minutes of game time). Player orders are refused by the
  gate for that whole window, so accepted orders can't be shown on this map yet. The
  application ends the cinematic with an end-cinematic event from Escape. That input is not on
  the wire.
- **Route and destination.** A direct `IssuePointOrder` east of unit 777 before any step gives
  a one-waypoint route to the target, and the unit moves about 4.2 units per tick. A control
  run with no player order has unit 777 and unit 780 each holding a multi-waypoint route
  after the first step (unit 777 ends at `(-3952, 2896)`), and unit 805 already holds one
  before any step. Those routes come from script orders in the first clock steps. The
  north-west detour is script-authored in this map. The authority's batches in those probes
  were refused by the control gate at tick 0, which is why the ordered and control runs
  matched. Direct natives bypass that gate.
- **Human03** stops at startup on `SetPlayerAbilityAvailable`, which is not implemented.

Gaps, in order of effect: the end-cinematic input is not on the wire, so Human01 cannot
reach an accepted order; the roster policy refuses maps with more than one human slot
(`campaign_inputs::prepare_new_game`), so a two-human source-map session needs a policy
decision; selection and stop orders need the wire to carry them; and `SetPlayerAbilityAvailable`
is missing.
