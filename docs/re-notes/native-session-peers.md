# Native session peers

## Scope

Native peers use the same WebSocket binary protocol as `web/network.js`: a
reconnect token, a `{"player":N,"tickMs":M}` greeting, binary snapshots and
`Input` datagrams. The wire format, tick lead (`INPUT_LEAD_TICKS = 4`, as in
`network.js`), per-tick command limit and identity rules are unchanged. This
is a project protocol. Original Legacy clients do not use it.

The host loop moved from `examples/session_server.rs` into `pcx_net::host`
(feature `session-server`, non-wasm), so tests can run it on a thread. The
example is now a thin wrapper, and the CLI is unchanged.

## Code

- `crates/pcx-net/src/host.rs`: `Host::bind`, `Host::poll(now)`, `local_world`.
  The admission state machine stays in `src/host/admission.rs`.
- `crates/pcx-net/src/session_client.rs`: `SessionClient` (`connect`, `submit`,
  `send_datagram`, `resync`, `receive`). It reads the greeting strictly and
  refuses tokens that are not 32 hex digits before opening a socket.
- `crates/pcx-net/tests/local_peers.rs`: four tests over real loopback sockets.

## World and maps

`local_world` is the existing harness world: one unit per slot at fixed
offsets. It does not load a map. Admitted pathing (`pcx_sim::admit_movement`)
is not installed, so these runs do not exercise map movement. The world is not
a decoded original map, and nothing here is a measured game value.

## Checks

- Two peers receive identical snapshots at every tick they share. Each unit's
  per-tick step is `0..=MOVE_SPEED`, the sim's clamp, and both units reach their
  targets exactly.
- A forged `player` in an `Input` is refused with `peer is not assigned to that
  player`, and the other slot receives no order.
- Dropping a peer stops its unit at the next tick and the unit stays frozen.
  Reconnecting with the same token reclaims the slot, and the first snapshot does
  not rewind.
- Each wait is capped at 400 snapshots and every socket has a 5 s timeout.

Mutation check: removing the disconnect stop (`stops_pending = false` in
`Session::leave`) fails `disconnect_stops_the_departed_units_order_and_reconnect_reclaims_the_slot`
with `condition not reached`. The restored tree passed three repeated runs.

## Not covered

- The native peer does not predict locally. `pcx_net::Client` requires its
  predicted tick to match the next step, which an ahead-of-authority submission
  cannot satisfy. Prediction for native peers is open.
- No WASM build of the session path was run here. `session_client` and `host`
  are compiled out for `wasm32`, and `network.js` is unchanged.
- The Chromium check (`examples/check_browser_session.mjs`) was not rerun.
- Game input, lobby, map initialization and original BNCS/W3GS negotiation are
  outside this path. See `docs/browser-online.md`.
