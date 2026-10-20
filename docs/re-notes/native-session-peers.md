# Native session peers and admitted maps

## Wire and scheduling

Native and browser peers use the binary protocol in `web/network.js`: a reconnect
token, a text greeting, binary snapshots and `Input` datagrams. The host
advances once per `TICK_MS` (25 ms of real time), and a tick applies whatever
orders arrived for it. A tick with no order applies no new order. Missing
commands are not a stall. A peer submits for the newest snapshot tick plus
`INPUT_LEAD_TICKS` (4, as in `network.js`), so an order arriving one or two
ticks late still lands. An order for a tick the host has already run is
refused with the sim's `CannotRewind` text. Orders more than
`MAX_INPUT_LEAD` (120) ticks ahead are refused.

The greeting is `{"player":N,"tickMs":M}` for the harness world. On a map host
it also carries `"session":"<64 lowercase hex>"`, the `SessionIdentity`. Browser
clients ignore the extra field.

Native peers do not predict locally. `pcx_net::Client` replays its own inputs
only when its predicted tick matches the next sim step, which an
ahead-of-authority submission cannot satisfy. Each peer's displayed state is
therefore the host's snapshot, delayed by the lead, the network round trip and
the 25 ms tick. The protocol remains correct without prediction. Adding
prediction is a separate change.

## Work bounds

- Each host pass is bounded: per-peer reads (`MESSAGES_PER_PEER_PER_POLL`),
  handshake budgets, and a join deadline.
- Units per session are capped at `MAX_SESSION_UNITS` (1024). A snapshot is at
  most 23 bytes per unit, which fits the host's 64 KiB write buffer.
- Movement checks per unit per tick are bounded by `pcx_sim` (`MAX_SEGMENT_CHECKS`
  = 512 cell checks per step clamp).

## Map admission

`pcx_net::map_session::admit(install, map_bytes, label, seed)` produces the
world, the identity and a report. Every input is an explicit file:

- The executable is `<install>/Warcraft III.exe`. The movement mask comes from
  `pcx_play::campaign_client::placement_rules`, which accepts only the reviewed
  Legacy 1.29.2.9231 image (SHA-256 `3f2ed012…570eed`). Other builds are refused.
- The catalog reads the unit and item metadata from the install's MPQ archives
  through `pcx_mpq::Install`, with the map's `war3map.w3u` and `war3map.w3t`
  overlays.
- The import is `ImportedWorld::from_map_aligned`. Terrain origin comes from
  `war3map.w3e`. The movement profile is that origin, a terrain tile of 128 world
  units (a W3E format invariant), and the reviewed ground mask. The pathing
  raster is admitted with `admit_aligned_movement`.
- Positions are world units scaled by `FX_ONE` and rounded to the nearest unit.
  Values outside ±16000 units are refused.

`SessionIdentity` is SHA-256 over the executable digest, the map digest, the
movement profile, the pathing raster, the seed, the excluded-record counts and
the initial world encoding. Two admissions agree exactly when all of these agree.

### Record policy

| Record | Handling | Counted as |
|---|---|---|
| `sloc` rawcode, no unit definition | Left out. Per `docs/re-notes/world-import.md`, `sloc` is the start-location marker. The name is the only evidence. | `start-location` |
| Only a random-selection word | Left out. The game picks the unit at creation, and that choice is not imported. | `random-selection` |
| Unit kind with empty, `_` or `none` `movetp` | Left out. Structures and resources do not move. | `immobile` |
| Other object kinds | Left out. | `non-unit` |
| Any other import problem | Refused, with the record ordinal. | none |

A unit owner must fit in `0..=255`. The owner is the raw control word, and
neutral or player remapping is not guessed. Peer slot N controls the units whose
owner is N. That is a project convention for this slice, not a measured
slot mapping.

Most installed melee maps admit with few or no mobile units, because most units
are random-selection placements. The census of `Maps/` in the Legacy install and
of the campaign maps is an observation. It is not an acceptance constant.

## Entry points

- `cargo run -p pcx-net --features session-server --example session_server -- [ADDRESS] [--install DIR --map FILE]`
  Without `--install` and `--map` the host runs the harness world with no identity.
  With both, it admits the map, prints `session identity <hex>` on stdout and an
  admission summary on stderr, then listens. Exit codes: 0 for help, 2 for usage
  errors, 1 for admission, bind or I/O failures.
- `pcx_net::host::Host::bind(address, world, seed, identity)`. `identity` is
  `None` only for the harness world.
- `pcx_net::session_client::SessionClient::connect_admitted(address, token,
  &identity)` refuses a host whose identity differs, before any command is sent.
  `connect` does not check identity.

## Tests

- `tests/local_peers.rs` (default in the `session-server` feature): loopback
  peers on the harness world. Covers shared authority, forged-identity refusal,
  disconnect stop and reconnect without rewind.
- `tests/map_peers.rs` (`#[ignore]`, needs `PCX_SESSION_INSTALL`,
  `PCX_SESSION_MAP`, `PCX_SESSION_OTHER_MAP`):
  - A peer admitting a different map or seed is refused before play.
  - Two peers admit the same map. Each peer's first snapshot matches its own
    admission replayed with no orders. Owner 1 stops at a pathing barrier and
    never enters a blocked cell. Owner 0 moves toward its open neighbour. Both peers
    hold identical authority at the same tick, and that authority equals a local
    replay of the accepted orders.

Gate used for the recorded run: `PCX_SESSION_INSTALL` is the Legacy install,
`PCX_SESSION_MAP` is `Human02Interlude.w3m` from the campaign set (owners 0 and 1
present), and `PCX_SESSION_OTHER_MAP` is `Maps/(2)Harrow.w3m`. The two-peer test
takes about 42 s, because the barrier is five cells away and the sim moves 0.1
world units per tick at real-time pace.

Mutation check: suppressing owner 1's orders in `Session::advance` makes the
two-peer test fail with `condition not reached`.

## Not covered

- Random-selection placements: their unit choice is not imported. Those records
  are left out and counted, so an admitted session is partial. Matching this
  needs the game's selection rule, which is unmeasured.
- Sim movement is the harness rule (0.1 world units per tick, fixed step). The
  original's unit speed and collision are not measured. Identical pathing blocks
  are not identical behavior.
- Browser peers do not admit maps, so they do not verify the identity. They
  observe and command the host's authority. The headless check passed against a
  map host (see `docs/browser-online.md`).
- No game input, lobby, combat, economy, or original Battle.net/W3GS path.
