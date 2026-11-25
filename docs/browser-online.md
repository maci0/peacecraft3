# Browser online sessions

The first transport is an authoritative WebSocket server. Browsers send the existing `pcx-net` binary command format; the host runs `pcx-sim` and sends its world snapshots. No game archives, movies or textures are transmitted. WebSocket's browser API supports binary `ArrayBuffer` messages directly ([MDN](https://developer.mozilla.org/en-US/docs/Web/API/WebSocket/binaryType)).

WebRTC P2P is not the first transport: it still needs an application signaling channel, ICE configuration and often a TURN relay when peers cannot connect directly ([WebRTC peer connections](https://webrtc.org/getting-started/peer-connections), [TURN](https://webrtc.org/getting-started/turn-server)). Keeping one server as authority fits the existing simulation and avoids treating a peer's claimed world as trusted state.

## Run locally

```sh
# Terminal 1
cargo run --locked -p pcx-net --features session-server --bin session_server

# Terminal 2
python3 -m http.server 8931 --bind 127.0.0.1 --directory web

# Terminal 3: derive the expected identity from an independent process.
export PCX_SESSION_IDENTITY=$(cargo run -q --locked -p pcx-net --features session-server --bin session_server -- identity | awk '{print $3}')
```

The host defaults to `ws://127.0.0.1:8932`. Open
`http://127.0.0.1:8931/network.js` and run this snippet in the browser console.
Replace the identity placeholder with the exported 64-hex value.

```js
const { PeacecraftSession } = await import('/network.js');
const session = new PeacecraftSession('ws://127.0.0.1:8932', '<expected identity>');
session.addEventListener('rejected', ({ detail }) => console.error(detail));
const ownedUnit = world => world.units.find(unit => unit.owner === session.player);
session.addEventListener('world', ({ detail }) => {
  const start = ownedUnit(detail);
  // Coordinates have 16 fixed-point fractional bits.
  session.commands([{ kind: 'move', x: start.x + 10 * 65536, y: start.y }]);
}, { once: true });
session.addEventListener('world', ({ detail }) => console.log(ownedUnit(detail)?.x));
session.connect();
```

After observing movement, use `session.commands([{ kind: 'stop' }])` or
`session.resync()`. After calling `disconnect()`, wait for the `disconnected` event. Reconnect
with the same session object to retain its token.

The server binds each connection to its player slot. A packet cannot choose another player. Per-tick batches merge every player's commands before `Server::accept`, preventing one player's order from replacing another's. Commands are limited to 32 per player per tick, 1 KiB packets and the existing 120-tick horizon. The server rejects coordinates outside its admitted bounds. Duplicate batches are rejected; submit multiple commands in one batch. The server advances the existing movement rules every 25 ms, with no new orders when a player is silent. Resync sends the current snapshot; it never rewinds the shared server for a client.

## Selection and snapshots

`Select` now replaces the sending player's selection. `Move` and `Stop` affect
that selection; units outside it retain their previous movement orders. The
existing strict fixed-point selection boundary is unchanged. Spawned harness
units start selected to preserve move-before-select behavior; this is a project
simulation convention, not a measured original-game selection default.

World snapshots preserve selection. An all-selected world keeps the original
version-0 bytes. A world with any deselected unit appends ASCII `SEL`, version
`1`, then one byte (`0` or `1`) per unit in the existing order. Rust and browser
decoders reject unknown versions, malformed flags, truncated records and extra
bytes. Older decoders reject the extension rather than silently restoring the
wrong selection. Browser unit objects always expose a boolean `selected`.

The Rust prediction client replays unacknowledged commands when an older
snapshot arrives, even if positions match. Same-tick authority replaces the
complete predicted world without applying inputs twice. Existing history,
acknowledgement and replay-work limits still decide when correction must snap.

Disconnect discards that player’s queued orders. At the next server tick it stops every unit
owned by that player without changing selection. A new connection's accepted
orders run after that cleanup. This keeps unselected units from continuing an
old order after their owner disconnects.

## What is working, and what remains

This is a real browser-to-Rust command and state path, **not a completed Warcraft multiplayer match**. The current `pcx-sim` only supports selection, movement and stop. The host runs the harness world (one unit per slot). `check-map --map FILE` reports why the map is refused, because map startup is not wired (see [native peers](re-notes/native-session-peers.md)). Combat, economy, original lobby behavior, matchmaking and the game's input/render adapter are not wired into this session. `network.js` is the harness client. The main application uses simulation
object data separately and still needs an online gameplay adapter.

This host is for local development. There are no accounts, isolated rooms, persistent matches, Origin restrictions or TLS termination. Reconnect tokens are random browser-generated bearer tokens; protect them and use `wss://` once public hosting is implemented. Sixteen player slots remain reserved for the host's lifetime. No public service has been deployed.

TCP streams are nonblocking before HTTP upgrade. Each pending upgrade gets at most eight I/O calls and 4 KiB of I/O per loop, with a five-second deadline. Pending and upgraded connections share the connection cap; rejecting excess connections does not skip session service. These are development-host resource policies, not original game limits. Completed WebSocket traffic retains the existing packet and per-peer work limits. This prevents blocking admission from stopping the movement clock; it does not establish public-service hardening or a full match-ready barrier.

## Verification

Core regression tests exercise same-tick commands from both players, identity spoofing, horizon and coordinate rejection, duplicate batches, and disconnect/resync without rewinding. A browser check uses two Chromium pages, issues opposing movement orders through the binary codec, compares exact authoritative snapshots at the same tick, forges another player's stop packet and checks rejection, then disconnects/reconnects and verifies shared authority again.

Seven admission tests exercise partial reads/writes, deterministic per-poll work ceilings, deadline expiry, invalid upgrades and unrestricted I/O after completion. They preserve Tungstenite's refusal of bytes after the HTTP request. The two-browser check also passed with an additional incomplete TCP upgrade held open. No original assets were served or transmitted in that run. Native tests and strict native/WASM compilation passed; the native TCP host does not run in WASM.

Verify the isolated display and null audio settings before running these
headless checks. Install `playwright-core` separately and export
`PCX_SESSION_IDENTITY` as above.

```sh
node crates/pcx-net/examples/check_browser_session.mjs /absolute/path/to/playwright-core/index.mjs
node crates/pcx-net/examples/check_browser_identity.mjs /absolute/path/to/playwright-core/index.mjs
```

The identity check runs a page that expects a different identity. It must be refused with no binary frame sent and no state shown, while the counterpart page joins.

The script expects both local servers above. Chromium defaults to `/usr/bin/chromium`; set `CHROMIUM` for a different executable. Successful output records the player slots and ticks compared. It proves the current movement simulation and transport, not unimplemented original game rules or UI.

Native peers run the same protocol over loopback sockets: `cargo test -p pcx-net --features session-server --test local_peers`. Map files are refused by `cargo test -p pcx-net --features session-server --test map_peers -- --ignored` with the variables named in its source. See the [native session guide](re-notes/native-session-peers.md) for details and remaining gaps.

## Recorded selection regression checks

The recorded native simulation/network suite passed 115 tests. Its WASI
run passed 102 (desktop transport tests are omitted there). Three Node
tests cover five Rust-generated accepted packets, nine malformed packets and a
control that demonstrates selection loss if the suffix is dropped. The existing
playtest suite and strict native/WASM compilation remain green.

The two-browser run also forced a deselected snapshot, verifies
that an empty selection cannot move its unit, reselects it and observes movement
again. Both pages receive identical authority, reconnect and resync successfully;
spoofed input is rejected. The run records four local code GETs without request
bodies, three expected local WebSocket connections and zero page errors. No game
assets are loaded by this check. Test-owned servers stop after the run.

This verifies the existing movement session and wire format. The running game
still needs its gameplay input/render and lobby integration; native selection
behavior and complete multiplayer matches remain open.
