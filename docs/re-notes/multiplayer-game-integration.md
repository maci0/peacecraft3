# Multiplayer integration plan

## Existing boundary

`pcx-net::session::Session` owns the authoritative movement server, binds a peer
to a player, merges per-player input batches, and sends world snapshots. The
native `session_server` binary and `web/network.js::PeacecraftSession` already
exercise two browser clients. Admission is now nonblocking and bounded. This is
a movement transport, not a running original map or completed multiplayer match.

The application already routes `MultiPlayerButton` and `LocalAreaNetworkButton`
through `Navigation::action` to `Screen::Multiplayer`, which loads the authored
`LocalMultiplayerJoin.fdf`. Preserve that route and its Cancel/Back behavior.
Attach connection ownership to entering/leaving that screen; do not replace the
route with a second launcher control. Repeated activation must not create a
second connection, and callbacks from a cancelled connection must not restore it.
No game-session caller is currently attached to this route.

`pcx-play::prepare_map_context` is local admission: it parses the selected map,
resolves caller-selected object fields and compiles the supplied scripts. Its
`ParsedMapContext` retains provenance, input budgets, an `ImportedWorld` and an
`Arc<MapScripts>`. It does not establish network readiness or execute map startup.
Its public `SourceDigest` values use FNV, and nonempty `AssetIdentity` labels do
not authenticate content. `pcx_sim::world::ImportedWorld` is distinct from the
movement simulation's `pcx_sim::World`; their records cannot be substituted.

## Proposed next implementation

Keep the authoritative Rust host and existing command codec over WebSocket/WSS.
The static demo needs a separately hosted match service; it cannot run the native
host itself. Add one prematch agreement state around the existing `Session`
instead of creating another transport or map parser.

Before accepting gameplay commands, both sides must agree on a canonical match
manifest: protocol and simulation compatibility, selected edition, admitted map,
common/Blizzard scripts, effective catalog/rules, source-derived slot policy,
host-selected seed and a new match epoch. Produce content fingerprints while
those exact local inputs are available. Use cryptographic hashes and an explicit
canonical encoding; existing FNV receipts and machine-local path strings are not
an adequate agreement format. Matching hashes detect disagreement, not remote
attestation or proof that a client is honest.

Every participant imports their own original files locally. Transmit only the
manifest's identifiers/fingerprints, lobby control, commands and authoritative
runtime state. Do not send ISO, MPQ, map, script, catalog or other asset files,
nor dumps of their authored contents. A peer with missing or mismatched local
inputs remains unready with an actionable reason.

Changing an agreed map, edition, content fingerprint, seed or epoch invalidates
readiness and queued commands before a new start. Reconnect credentials must be
scoped to the match and assigned slot. Joining or resynchronizing a client must
never rewind another player's authority. Derive playable slots and object
bindings from the admitted map, rather than copying the example host's fixed
slot policy or generating substitute units.

Map initialization and the input/render adapter remain a separate dependency.
Reuse the admitted scripts and records; preserve unsupported native and stat
resolution errors. A compiled program is not a playable map. Do not mark a match
ready while its startup or required world bindings are unsupported.

## State and verification gates

Current snapshots encode tick, next unit ID, and each movement unit's ID, owner,
position and optional target. They do not carry authoritative RNG state, VM
globals, handles, timers, triggers, combat or economy. Existing client history
does not make those omitted states restorable. Extend authoritative state and its
checkpoint tests alongside the corresponding gameplay features before claiming
full-match rollback or reconnect. Native Warcraft LAN/Battle.net interoperability
is a separate unverified protocol target.

Movement-client rollback records the accepted authoritative base with its
retained RNG frame before replay. Recording an earlier tick discards the newer
history tail, so replay replaces the abandoned trajectory in chronological order
without duplicate ticks evicting still-replayable frames. A synthetic server,
wire encode/decode and the actual `Client` exercise two successive corrections
with four- and eight-frame limits, checking every retained world, selection,
movement target, replay count and history bound. These are deterministic harness
checks, not original-game movement or protocol observations.

The positional divergence report accepts the snapshot format's full `i32`
coordinate range. Axis distances use `abs_diff` and widen to `u64` before adding;
the aggregate saturates at `u64::MAX`. A valid extreme-coordinate snapshot must
reach normal Client correction rather than panic during diagnostic arithmetic.
The wire regression also checks identity-based matching with reversed unit
record order. This sets no original map extent or movement rule.

The agreement slice needs paired synthetic-client tests for commands before
readiness; every manifest mismatch; invalidation after ready; stale callbacks;
wrong-match reconnect; and cancellation without unrelated menu mutation. Retain
the existing same-tick input, spoof, duplicate, horizon, disconnect and resync
checks. Capture browser traffic to verify that only approved control/state
messages cross the connection, with no original-file uploads. Use deterministic
work bounds for fairness tests rather than wall-clock/FPS thresholds.

Public deployment additionally needs explicit Origin policy, TLS, room isolation,
admission/rate limits and bounded credential/slot lifetime. These are open, and
the local development host is not a public service.
