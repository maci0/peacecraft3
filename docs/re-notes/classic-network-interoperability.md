# Classic Battle.net and original-client interoperability

## Target and current boundary

Target the supplied Legacy Warcraft III 1.29.2.9231 client, recording its
executable digest for every original-client run. RoC and TFT availability must
come from the admitted installation. Earlier compatible client versions are
separate test profiles, not substitutes for the target.

The current `pcx-net` movement codec, project UDP discovery and browser
WebSocket session are PeaceCraft protocols. `bnet.rs` parses gateway data and
models session transitions; it is not a BNCS implementation. Original-client
crossplay, classic authentication and ten-minute mixed-client matches remain
unverified.

`classic_frame` now provides a separate bounded BNCS/W3GS header codec and
native TCP caller. Typed readers cover the selected authentication challenge
and W3GS join refusals, slot records, selected-map checks and opaque action subrecords. This is not
realm login or completed
W3GS lobby/gameplay support.

## Public references checked on 2026-10-16

| Reference | Role | Version boundary |
|---|---|---|
| [PvPGN-PRO](https://github.com/pvpgn/pvpgn-server) | Self-hosted classic Battle.net realm; BNCS handlers | README lists Warcraft III through 1.28.5 and notes client signature-verification modifications |
| [Eurobattle.net source list](https://eurobattle.net/page/open-source), [realm fork](https://github.com/eurobattle/pvpgn) | Community realm and published implementation | [Installation guide](https://eurobattle.net/page/wiki-main) specifies 1.26 and 1.28.5; no 1.29 compatibility inferred |
| [GHost++](https://github.com/ghostBot/ghostplusplus) | Game-host bot, not a realm; W3GS packet implementation | Reference code needs verification against the selected original build |
| [BNCSUtil](https://github.com/BNETDocs/bncsutil) | Classic authentication, key and version-check routines | Inspect supported algorithms and licensing before integration |
| [BNETDocs protocol headers](https://bnetdocs.org/document/16/protocol-headers) | Public protocol documentation | BNCS and W3GS framing are distinct; unrelated Storm UDP formats are not WC3 LAN discovery |

Pinned source review snapshots:

- PvPGN-PRO: `9cd173f4e02ba3d9f8f15a67ca308b5eb78723e4`.
- Eurobattle realm: `e198a6d3031a1703ff6840ea420098e030909eff`.
- GHost++: `6ba90f426e96f80fe013a468d1e07151a510adbe`.

[Open PvPGN proposal #501](https://github.com/pvpgn/pvpgn-server/pull/501), head
`b13ccfb337cb52e632fc19ad9eaeb812f556d084`, describes a 1.29.2 login path with
client-side changes to signature and password-proof handling. Its claimed
runtime success is upstream evidence, not a local result or unmodified-client
proof. Keep that profile distinct from native authentication and unmodified LAN
play. Do not copy its measured version/checksum values into application defaults;
derive build data from explicitly supplied files and the selected protocol.

## Protocol and simulation ownership

BNCS handles realm login, channels and game advertisements. A game connection
uses W3GS. In GHost++ `ghost/gameprotocol.cpp`, `RECEIVE_W3GS_SEARCHGAME` and
`SEND_W3GS_GAMEINFO` cover discovery, `SEND_W3GS_SLOTINFOJOIN` carries slots and
the random seed, `SEND_W3GS_MAPCHECK` describes the selected map, and
`RECEIVE_W3GS_OUTGOING_ACTION` / `SEND_W3GS_INCOMING_ACTION` carry ordered
actions. These are implementation references, not a measured 1.29 packet census.

Classic host relaying does not make an original client consume PeaceCraft
movement snapshots. Matching game rules, script execution, timers, random draws
and turn/checksum semantics are required for sustained crossplay. Keep current
project snapshot authority and classic interoperability as separate paths.

## Roadmap and test dependencies

`PLAN.md` §3.7 and `TRACKER.md` NET-C01 through NET-C09 define acceptance:
local realm, bounded codecs, both host directions, map/lobby/start negotiation,
full crossplay, realm client, lifecycle failures and comparison harness.

Use the existing local original-game reference harness for exact-build runs.
First establish original-to-original behavior, then compare each direction
with PeaceCraft. Record executable/map fingerprints, negotiated product/build,
slot/player identity, requested and observed seed evidence, action order and
checksum results. Equal requested seeds do not establish equal RNG streams.
Test mismatches and malformed packets before enabling a play control.

Use matching locally installed maps; no original asset transfer is needed.
Synthetic freely authored maps may test transfer separately. Keep credentials,
keys, original executables and asset bytes out of commits and public captures.
Timeouts, allocation/count caps and packet work must be explicit and tested.
Reconnect extensions such as GProxy require separate negotiation and evidence.

## Local reference realm

The local reference realm was built from the pinned PvPGN source above and
verified on 2026-10-16:

| Item | Observed value |
|---|---|
| Container / internal network | `pcx-classic-realm-20261016` |
| Image | `sha256:bb560435c5d51ca528326beb7a463fd00ced0e097483d4c0ef8a801e880fa207` |
| Debian bookworm-slim amd64 base | `sha256:a4672c0cb26fbdde88e38fa2dfb6c681942306680e41e4378b28770b6e79ee91` |
| Local test endpoint | `127.0.0.1:34369` |

Tracking and account creation are disabled. The container has no default route.
Docker did not publish a port on the internal bridge; a loopback-only host
forwarder supplies the endpoint. Its concurrent connections, per-connection
bytes and absolute lifetime are capped, including blocking socket I/O.
The port is a local runtime observation, not an application default.

Independent root verification passed 12 local source/bounds/I/O tests and a live
synthetic BNCS init/auth-info probe. The valid init received the expected echo
and authentication challenge; omitting the init byte received no response.
Challenge expectations are derived from the selected upstream configuration
and handler source. No account, password, key or original client was used.

The private build recipe is `.scratch/classic-network-20261016/realm/Dockerfile`;
the corrected probe and forwarder are under `realm/followup-bounds/scripts`.
Root commands, complete logs, current script hashes and acceptance are in
`.scratch/classic-network-20261016/root-verification/`. Earlier failed builds
and the initial unbounded probe evidence are retained separately; the root
receipt is the accepted scope. These private scripts are not a published CLI
or a CLI conformance claim.

This closes NET-C01 only. Login, W3GS negotiation, original 1.29b LAN joins and
mixed-client matches still need their own implementation and runtime evidence.

## Integrated framing and TCP caller

BNCS (`0xFF`) and W3GS (`0xF7`) use distinct family bytes and a four-byte header
whose little-endian length includes that header. The source references are the
pinned PvPGN `bnet_protocol.h` and GHost++ `gameprotocol.h`/`gameprotocol.cpp`.
The BNCS connection-init byte is outside this frame format.

The codec borrows a complete input or assembles fragmented frames with caller
byte/count ceilings and fallible reservations. Validated frame bytes are private.
A malformed or over-budget stream becomes terminal, including a bad tail after
a valid frame in one chunk. The TCP caller validates framing policy before
connecting or writing and rearms each partial read/write against one absolute
deadline. It accepts an explicit socket address; the reference probe alone is
restricted to the owned loopback endpoint.

At the framing integration, root verification against isolated committed sources
passed all 100 native network checks, including 15 framing/TCP
cases, and ten actual WASI codec tests. Strict native/WASM checks, rustdoc and
both library builds pass. A fresh probe compiled against the integrated library
matches the pinned realm's init/auth-info exchange and its missing-init negative
case. Deadline propagation is tested with explicit per-write remaining budgets;
no elapsed-time performance threshold is used. Commands and receipts are in
`.scratch/classic-network-20261016/root-framing/`.

This advances NET-C02 framing only. No W3GS peer, original client, account or key
was used. Authentication, remaining message bodies, lobby negotiation and
sustained mixed-client play remain open.

## Typed authentication challenge and join refusal

The challenge reader accepts an already validated BNCS frame, a caller-selected
reply variant and the expected versioncheck filename/equation bytes. It borrows
those packet fields without allocating, checks terminators and caller string
caps, and requires the selected variant's exact signature tail. The equation
is compared with the selected profile; it is not executed. The source contract
is PvPGN `9cd173f4e02ba3d9f8f15a67ca308b5eb78723e4`. This is a challenge
reader, not account authentication or signature verification.

The W3GS reader requires packet id `W3GS_REJECTJOIN` and exactly one
little-endian reason word. The three named reasons follow GHost++
`6ba90f426e96f80fe013a468d1e07151a510adbe`; any other value remains raw.
Every decoded reason is a refusal. Another packet, including slot-info join,
is an error for this reader and never counts as admission.

Synthetic native and WASI tests cover malformed fields, selected-profile
mismatches, wrong packet families/ids and typed parsing after the existing
stream decoder/TCP caller. The root realm probe uses the integrated reader
and profile fields loaded from the pinned local server sources. No account,
key, original client or gameplay session is involved. Root evidence is in
`.scratch/classic-network-20261016/root-typed-messages/`. NET-C02 remains partial.

## Raw slot and seed records

`slot_seed` reads the caller-selected `W3GS_SLOTINFO` or `W3GS_SLOTINFOJOIN`
body from an already validated frame. The length prefix, record count, nine
fields per record, four seed bytes and fixed join suffix follow the pinned
GHost++ source. A complete shape is checked before retaining slot records.
`playerSlots` stays separate from the number of records, including a zero-record
packet. Unknown field values and the join suffix remain raw.

Native and actual WASI tests exercise truncation at every body prefix, wrong
family/profile, count/size mismatch and both borrowed and streamed frame callers.
This adds no slot store, matchmaking, client admission or RNG implementation.
The seed's presence does not show that either client has applied it or that
their streams agree. NET-C05 remains open. Root evidence is in
`.scratch/classic-network-20261016/root-slot-seed/`.

## Opaque ordered action bodies

`Frame::incoming_actions` and `OwnedFrame::incoming_actions` check the pinned
GHost++ incoming action bodies, including subrecord lengths and the caller's
record ceiling. Each player id and payload stays in wire order. The first
incoming form retains its little-endian interval; the second form keeps its
two prefix bytes raw. Empty queues stay distinct from a malformed checksum or
subrecord tail. An invalid later record refuses the whole message.

The outgoing reader retains the connection-supplied player id, four checksum
bytes and borrowed payload. The pinned receiver refuses player id 255; the
reader preserves that rule. It does not derive an id from the action body.
Checksum bytes are retained without verification, and payloads are not decoded
as game orders or translated to the project's movement protocol.

Twenty-nine scoped native frame tests and twenty-four actual WASI tests pass,
including ordered non-empty/empty actions, count and length refusal and a valid
record followed by a malformed tail. Strict native/WASM Clippy, rustdoc and both
library builds pass. No live action exchange or original-client match was run.
NET-C06 remains open; root evidence is in
`.scratch/classic-network-20261016/root-actions/`.

## Selected-map fingerprint comparison

`Frame::bind_selected_map` and `OwnedFrame::bind_selected_map` now compare a
`W3GS_MAPCHECK` with borrowed caller-selected map/prelude bytes and explicit
edition/version/source identities. The protocol path is caller-supplied and
compared byte for byte; a resolver path or display label is not substituted.
The result has private fields and read-only accessors. It borrows the selected
map rather than retaining a second payload. It is not a lobby-ready capability.

The contract follows pinned GHost++
[`SEND_W3GS_MAPCHECK`](https://github.com/ghostBot/ghostplusplus/blob/6ba90f426e96f80fe013a468d1e07151a510adbe/ghost/gameprotocol.cpp)
and [`CMap::Load`/`XORRotateLeft`](https://github.com/ghostBot/ghostplusplus/blob/6ba90f426e96f80fe013a468d1e07151a510adbe/ghost/map.cpp).
Size and CRC-32 cover the whole selected file. The existing map parser validates
its header/archive/trailer relation, then the bounded MPQ reader extracts only
the embedded archive members. XORO and SHA-1 cover the applicable common and
blizzard scripts, protocol separator, preferred map script and ordered map
members. Nonempty map preludes override the external ones; an empty override
uses the selected external bytes. Root-script precedence is retained when both
script paths exist. A missing map script, failed extraction, ambiguous member,
incomplete required source or mismatched external edition/version refuses.

Archive/table/packed/member limits precede the relevant allocations. An explicit
total hashing budget counts the whole-file CRC input and each mixed member;
the next member is charged before decoding it. Each member buffer is dropped
before the next. These are input bounds, not a total process-memory guarantee.
The packet's four unspecified prefix bytes remain raw. No checksum is replaced
with project FNV/SHA-256, and SHA-1 is not treated as authentication.

Four synthetic groups cover truncation at every packet prefix, missing and
invalid selected maps, all fingerprint mismatches, overrides, alternate script
paths, exact/short budgets, all mixed member positions, borrowed/streamed frames
and byte identity. They pass natively and under actual WASI execution. Independent
C++ vectors compiled from the pinned XOR/rotate, CRC and SHA sources match the
fixtures. Strict native/browser-WASM checks and rustdoc pass. The full native
network suite also passes against the current worktree, which includes separate
project-discovery work in `transport.rs`; this slice does not change that file.
Commands and source hashes are in
`.scratch/classic-network-20261016/root-map-check/`.

No map-size response, transfer, countdown, revision-bound lobby publication,
original-client exchange or mixed-client match was run or implemented here.
The shared map parser's supported-container policy still applies; this is a
pinned sender-profile check, not proof of every original client check. NET-C05
and both original LAN host directions remain open.
