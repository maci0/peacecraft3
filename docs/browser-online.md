# Browser online sessions

The first transport is an authoritative WebSocket server. Browsers send the existing `pcx-net` binary command format; the host runs `pcx-sim` and sends its world snapshots. No game archives, movies or textures are transmitted. WebSocket's browser API supports binary `ArrayBuffer` messages directly ([MDN](https://developer.mozilla.org/en-US/docs/Web/API/WebSocket/binaryType)).

WebRTC P2P is not the first transport: it still needs an application signaling channel, ICE configuration and often a TURN relay when peers cannot connect directly ([WebRTC peer connections](https://webrtc.org/getting-started/peer-connections), [TURN](https://webrtc.org/getting-started/turn-server)). Keeping one server as authority fits the existing simulation and avoids treating a peer's claimed world as trusted state.

## Run locally

```sh
cargo run --locked -p pcx-net --features session-server --example session_server
python3 -m http.server 8931 --bind 127.0.0.1 --directory web
```

The host defaults to `ws://127.0.0.1:8932`. In a browser page served from the second command:

```js
const { PeacecraftSession } = await import('/network.js');
const session = new PeacecraftSession('ws://127.0.0.1:8932');
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

## What is working, and what remains

This is a real browser-to-Rust command and state path, **not a completed Warcraft multiplayer match**. The current `pcx-sim` only supports selection, movement and stop. The local host initializes one unit per slot to exercise those existing rules. Combat, economy, map initialization, original lobby behavior, matchmaking and the game's input/render adapter are not wired into this session. `peacecraft3` currently has no gameplay caller of `pcx-net` or `pcx-sim`; `network.js` is a module for that adapter, not an Online Play button that pretends a match exists.

This host is for local development. There are no accounts, isolated rooms, persistent matches or TLS termination. Reconnect tokens are random browser-generated bearer tokens; protect them and use `wss://` once public hosting is implemented. Sixteen player slots remain reserved for the host's lifetime. Its synchronous, time-limited handshake can delay ticks and must be replaced with asynchronous admission before public hosting. No public service has been deployed.

## Verification

Core regression tests exercise same-tick commands from both players, identity spoofing, horizon and coordinate rejection, duplicate batches, and disconnect/resync without rewinding. A browser check uses two Chromium pages, issues opposing movement orders through the binary codec, compares exact authoritative snapshots at the same tick, forges another player's stop packet and checks rejection, then disconnects/reconnects and verifies shared authority again.

With `playwright-core` installed separately:

```sh
node crates/pcx-net/examples/check_browser_session.mjs /absolute/path/to/playwright-core/index.mjs
```

The script expects both local servers above. Chromium defaults to `/usr/bin/chromium`; set `CHROMIUM` for a different executable. Successful output records the player slots and ticks compared. It proves the current movement simulation and transport, not unimplemented original game rules or UI.
