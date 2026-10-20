# Browser online sessions

The first transport is an authoritative WebSocket server. Browsers send the existing `pcx-net` binary command format; the host runs `pcx-sim` and sends its world snapshots. No game archives, movies or textures are transmitted. WebSocket's browser API supports binary `ArrayBuffer` messages directly ([MDN](https://developer.mozilla.org/en-US/docs/Web/API/WebSocket/binaryType)).

WebRTC P2P is not the first transport: it still needs an application signaling channel, ICE configuration and often a TURN relay when peers cannot connect directly ([WebRTC peer connections](https://webrtc.org/getting-started/peer-connections), [TURN](https://webrtc.org/getting-started/turn-server)). Keeping one server as authority fits the existing simulation and avoids treating a peer's claimed world as trusted state.

## Run locally

```sh
cargo run --locked -p pcx-net --features session-server --example session_server
python3 -m http.server 8931 --bind 127.0.0.1 --directory web
# The identity a page must expect, from an independent process (same source, same seed):
PCX_SESSION_IDENTITY=$(cargo run -q --locked -p pcx-net --features session-server --example session_server -- --print-identity | awk '{print $3}')
```

The host defaults to `ws://127.0.0.1:8932`. In a browser page served from the second command:

```js
const { PeacecraftSession } = await import('/network.js');
const session = new PeacecraftSession('ws://127.0.0.1:8932', identity); // identity: the expected 64-hex value
// The page shows no state and sends nothing until the host announces this identity.
session.addEventListener('world', ({ detail }) => console.log(detail));
session.addEventListener('rejected', ({ detail }) => console.error(detail));
session.connect();
// After the first world event: coordinates use pcx-sim's 16-bit fixed-point scale.
session.commands([{ kind: 'move', x: 10 * 65536, y: 0 }]);
session.commands([{ kind: 'stop' }]);
session.resync();
session.disconnect();
// Reuse the same object/token to reclaim this player's slot and get current authority.
session.connect();
```

The server binds each connection to its player slot. A packet cannot choose another player. Per-tick batches merge every player's commands before `Server::accept`, preventing one player's order from replacing another's. Commands are limited to 32 per player per tick, 1 KiB packets and the existing 120-tick horizon. Coordinate bounds prevent overflowing the simulation's fixed-point subtraction. Duplicate batches are rejected; submit multiple commands in one batch. The server advances the existing movement rules every 25 ms, with no new orders when a player is silent. Disconnect drops that player's queued orders and issues stop. Resync sends the current snapshot; it never rewinds the shared server for a client.

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

Disconnect is an authority action: at the next server tick it stops every unit
owned by that player without changing selection. A new connection's accepted
orders run after that cleanup. This keeps unselected units from continuing an
old order after their owner disconnects.

## What is working, and what remains

This is a real browser-to-Rust command and state path, **not a completed Warcraft multiplayer match**. The current `pcx-sim` only supports selection, movement and stop. The host runs the harness world (one unit per slot). `--map FILE` refuses the map with its reasons, because map startup is not wired (`docs/re-notes/native-session-peers.md`). Combat, economy, original lobby behavior, matchmaking and the game's input/render adapter are not wired into this session. `peacecraft3` currently has no gameplay caller of `pcx-net` or `pcx-sim`; `network.js` is a module for that adapter, not an Online Play button that pretends a match exists.

This host is for local development. There are no accounts, isolated rooms, persistent matches, Origin restrictions or TLS termination. Reconnect tokens are random browser-generated bearer tokens; protect them and use `wss://` once public hosting is implemented. Sixteen player slots remain reserved for the host's lifetime. No public service has been deployed.

TCP streams are nonblocking before HTTP upgrade. Each pending upgrade gets at most eight I/O calls and 4 KiB of I/O per loop, with a five-second deadline. Pending and upgraded connections share the connection cap; rejecting excess connections does not skip session service. These are development-host resource policies, not original game limits. Completed WebSocket traffic retains the existing packet and per-peer work limits. This prevents blocking admission from stopping the movement clock; it does not establish public-service hardening or a full match-ready barrier.

## Verification

Core regression tests exercise same-tick commands from both players, identity spoofing, horizon and coordinate rejection, duplicate batches, and disconnect/resync without rewinding. A browser check uses two Chromium pages, issues opposing movement orders through the binary codec, compares exact authoritative snapshots at the same tick, forges another player's stop packet and checks rejection, then disconnects/reconnects and verifies shared authority again.

Seven admission tests exercise partial reads/writes, deterministic per-poll work ceilings, deadline expiry, invalid upgrades and unrestricted I/O after completion. They preserve Tungstenite's refusal of bytes after the HTTP request. The two-browser check also passed with an additional incomplete TCP upgrade held open. No original assets were served or transmitted in that run. Native tests and strict native/WASM compilation passed; the native TCP host does not run in WASM.

With `playwright-core` installed separately, and `PCX_SESSION_IDENTITY` set as above:

```sh
node crates/pcx-net/examples/check_browser_session.mjs /absolute/path/to/playwright-core/index.mjs
node crates/pcx-net/examples/check_browser_identity.mjs /absolute/path/to/playwright-core/index.mjs
```

The identity check runs a page that expects a different identity. It must be refused with no binary frame sent and no state shown, while the counterpart page joins.

The script expects both local servers above. Chromium defaults to `/usr/bin/chromium`; set `CHROMIUM` for a different executable. Successful output records the player slots and ticks compared. It proves the current movement simulation and transport, not unimplemented original game rules or UI.

Native peers run the same protocol over loopback sockets: `cargo test -p pcx-net --features session-server --test local_peers`. Map files are refused by `cargo test -p pcx-net --features session-server --test map_peers -- --ignored` with the variables named in its source. Details and remaining gaps are in `docs/re-notes/native-session-peers.md`.

## Selection regression checks

The integrated native simulation/network suite passes 115 tests; actual WASI
execution passes 102 (desktop transport tests are omitted there). Three Node
tests cover five Rust-generated accepted packets, nine malformed packets and a
control that demonstrates selection loss if the suffix is dropped. The existing
playtest suite and strict native/WASM compilation remain green.

The fresh two-browser session now also forces a deselected snapshot, verifies
that an empty selection cannot move its unit, reselects it and observes movement
again. Both pages receive identical authority, reconnect and resync successfully;
spoofed input is rejected. The run records four local code GETs without request
bodies, three expected local WebSocket connections and zero page errors. No game
assets are loaded by this check. Test-owned servers stop after the run.

This verifies the existing movement session and wire format. The running game
still needs its gameplay input/render and lobby integration; native selection
behavior and complete multiplayer matches remain open.
