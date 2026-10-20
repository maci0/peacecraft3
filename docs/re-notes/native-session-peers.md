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
`session_server --print-identity`.

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
being playable. `session_server --map FILE` prints those reasons to stderr and
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
- CLI conformance with clispec v0.3 is not claimed. The spec needs a `schema` command,
  a declared top-level `clispec` version, a JSON format flag, declared exit codes and
  outcomes, and an error envelope on stderr. `--print-identity` is one plain-text data line
  on stdout, and diagnostics go to stderr. Adopting the spec is a separate decision.
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
