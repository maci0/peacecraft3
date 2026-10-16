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

`classic_frame` now provides a separate bounded BNCS/W3GS header codec, native
TCP caller and bounded unicast LAN query. Typed readers cover the selected
authentication challenge and W3GS join refusals, slot records, discovery
advertisements, selected-map checks and opaque action subrecords. This is not
realm login or completed W3GS lobby/gameplay support.

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

## Original-host LAN discovery query

The supplied original `1.29.2.9231` executable
(`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`)
hosted an unmodified supplied melee map through its LAN UI. Its private Wine
container used an internal Docker network, no published ports and no default
route. A specific limited-broadcast route was added inside that container's
network namespace. The production reference harness was unchanged; the private
launch receipt records the network specialization, namespace and route changes.
The actual client/capture extent was 1280×960 at 96 DPI.

Captured original broadcasts contained `W3GS_CREATEGAME` with product bytes
`50 58 33 57` and version word `29`. A search using those observed fields returned
a 129-byte `W3GS_GAMEINFO`. Different-version and unknown-product stimuli had no
response within their explicit bounds; a following matching control responded.
Initial browser captures before hosting contained no packets and establish no
search-request layout or unique cause of that silence.

`game_info` reads the selected product and complete version word, game name,
host counter/key, dimensions, checksum/path/name bytes and fixed tail. Slot
counts do not allocate records or imply occupancy policy. The stat-mask codec
follows the pinned GHost++ `UTIL_DecodeStatString` relation. Terminators, fixed
shape, canonical odd-byte encoding and derived decoded size are checked before
the fallible decoded allocation. The original reply has 20 bytes beyond the
older sender's final stat zero. They remain opaque; no checksum or signature
meaning is assigned. Strings stay bytes, without UTF-8 loss or path rewriting.

The native `game_info::udp::query` sends one search to an explicit unicast
endpoint, counts every receive attempt including foreign peers, detects an
oversize datagram with one spare buffer byte, and rearms each socket operation
to the remaining absolute deadline. Failures do not resend. It returns a
validated `OwnedFrame`; it neither collects broadcasts nor edits a lobby.
The compiled production caller successfully queried the original host and
retained the same advertised map path/checksum/port and all extension bytes. A mismatched
selected profile refuses the retained reply.

Synthetic tests cover the sender fixture, product/version mismatch, every
truncated prefix, stat bounds/shape, opaque extensions, streamed frames and
actual loopback UDP success, foreign-peer exhaustion, oversize and pre-expired
policy. Pure parsing executes under WASI; native UDP is excluded on WASM.
The full native network suite, strict native/browser-WASM checks, rustdoc and
the browser-target library build pass. Local commands, source identities and
runtime receipts are under `.scratch/classic-network-20261016/root-game-info/`
and `.scratch/original-game-vm/classic-lan-host-1280-1/`.

This is a query of a known original host address under Wine. Broadcast host
collection, launcher integration, TCP join, the reverse host direction,
original-to-original play and sustained mixed-client simulation remain open.
No original map contents were transferred. NET-C03 is not closed by discovery.

## Original-to-original lobby baseline

Two independent, unmodified copies of the supplied `1.29.2.9231` install joined
one LAN lobby under Wine. Both source manifests contain the same 564 files and
the executable identity above. Each client used its own prefix and 1280×960,
96-DPI framebuffer on the isolated internal network. Both lobby captures show
the host and peer. No game was started.

A controlled leave/rejoin retained 28 complete Ethernet packets: no packet
truncation, capture drops, TCP gaps or conflicting overlaps. The earlier capture
retained only discovery and is not handshake evidence. The new connection's
64-byte client stream and 226-byte host stream pass the integrated `Decoder`,
including one-byte fragmentation and `finish` checks. They contain, in order:

| Direction | W3GS messages |
| --- | --- |
| Client → host | REQJOIN, MAPSIZE, PONG |
| Host → client | SLOTINFOJOIN, PLAYERINFO, MAPCHECK, SLOTINFO, SLOTINFO, PING |

The join request's host counter and entry key equal the advertisement. The
integrated slot reader accepts all three slot messages. After the peer reports
that it has the map, its slot's download status changes from 255 to 100. No
map-part message occurs in this capture. The pong body equals the host's ping.

The selected standalone map, found under the supplied install's `Maps/`
directory, passes `bind_selected_map` against that original MAPCHECK: whole-file
size/CRC-32 and member-stream XORO/SHA-1 all match. Both supplied War3 and War3x
archives contain identical external common/blizzard script bytes; either copy
gives the same comparison, without inferring archive priority. The map has no
applicable embedded prelude overrides. A one-byte map mutation and absent local
bytes both refuse. The advertisement's path/XORO and its 20 extension bytes equal
the MAPCHECK path/XORO/SHA-1 in this one observation. The discovery parser still
retains that extension opaquely.

The four seed bytes differ across the initial join and later slot updates.
Keep each executed update: an early lobby seed is not an established game-start
seed or proof of matching random streams. The native join request also has a
longer address tail than the older GHost++ receiver describes; its unknown bytes
must not become guessed defaults or an assumed socket-address layout.

Private capture, command receipts, reassembly self-checks and compiled production
reader results are in
`.scratch/classic-network-20261016/root-original-join-audit/`; the two runtime
directories are `.scratch/original-game-vm/classic-lan-{host,join}-1280-1/`.
These establish an original-to-original lobby baseline and one original map
fingerprint comparison. PeaceCraft TCP join, the reverse host direction,
countdown, action exchange and sustained mixed-client play remain open.

## Verified-map size reply

`MapMatch::map_size_reply` encodes the pinned receiver's 13-byte MAPSIZE form:
the caller's four unspecified prefix bytes, the protocol's "have map" flag and
the already verified whole-file size in little-endian order. It allocates
nothing. The prefix is explicit; equality with MAPCHECK's prefix is not assumed.
`MapMatch` can only be obtained through the selected-map fingerprint comparison,
so a missing or mismatched map does not produce this reply through that path.

This constructs bytes only. It does not send them, set a lobby-ready flag or
establish host acceptance. Revision-bound session publication and TCP join still
need integration. The original capture supplies a separate offline comparison
for the complete reply; no original bytes are embedded in the synthetic tests.

The generated reply equals the original client's captured reply byte for byte.
Five map tests execute successfully under WASI; strict native/browser-WASM
Clippy, rustdoc, formatting and both library builds pass. The native suite has
128 passing checks with four socket tests explicitly excluded after this
session's sandbox denied their socket operations. The failed full-suite log is
retained; no new live network result is claimed. Evidence is under
`.scratch/classic-network-20261016/root-map-size-reply/`.

After runtime access was restored, the unfiltered native suite passed all 132
checks, including those four socket tests. The same five WASI tests, strict
native/browser-WASM checks, rustdoc, formatting and both library builds passed
again. These later logs are in `followup-unrestricted/`; the earlier permission
failures remain intact. This still does not establish a live PeaceCraft join.

## Explicit join request and live Rust lobby probe

`GameInfo::join_request` now builds REQJOIN using the checked advertisement's
host counter and entry key. The caller supplies the client listen port, peer
key, name, unspecified byte and address tail. The pinned GHost++ receiver
requires a nonempty name and at least ten tail bytes. The longer native tail
stays opaque; no address, count or unknown-word default is embedded in code.
The name remains bytes, without UTF-8 conversion. Empty/embedded-NUL names,
short tails and caller/wire length overflow refuse before the fallible packet
allocation and copy.

A bounded private Rust caller uses that encoder, the production discovery and
frame/slot readers, `bind_selected_map` and `map_size_reply` against the original
host. The host assigns player 2, displays the probe in its lobby and updates its
download status to 100 after the verified size reply. The probe answers the
host's pings and disconnects at its explicit stop time. It does not start the
game or receive map parts. A separate run with a one-byte mutation of the local
map refuses at CRC-32 comparison before sending MAPSIZE or publishing readiness.
The original local map is unchanged.

This probe replays the explicit unspecified/address-tail fields and MAPSIZE
prefix from the same-build native capture. It does not generate a new native
address record for the probe's own internal endpoint. Peer-to-peer reachability,
persistent application lobby transport, launcher integration, the reverse host
direction and gameplay therefore remain open. It is a measured protocol
handshake, not full NET-C03 or NET-C05 acceptance.

Two synthetic encoder groups cover byte order, raw client fields, non-UTF-8
names, malformed input, exact/short caller limits and the full 16-bit frame
boundary. All 134 native checks and 34 actual WASI classic-codec tests pass,
with strict native/browser-WASM Clippy, rustdoc, formatting and both library
builds. Private commands, packet streams and original UI captures are under
`.scratch/classic-network-20261016/root-live-join/`. The first integrated probe
completed the handshake but reported its planned expiry as an error; that run
is retained separately from the corrected completion handling.

## Generated 1.29 IPv4 join address

Pinned GoWarcraft3
[`Join.Serialize`](https://github.com/nielsAD/gowarcraft3/blob/bfda02e6ef6481fd41f4009f730ac7b7723d5883/protocol/w3gs/packets.go)
and [`WriteSockAddr`](https://github.com/nielsAD/gowarcraft3/blob/bfda02e6ef6481fd41f4009f730ac7b7723d5883/protocol/buffer.go)
explain the longer native tail: a counted reserved block precedes an IPv4
socket record. This sender profile has two reserved bytes, little-endian
AF_INET, network-order port/address and zero padding. It matches the retained
original request when supplied the original container IP and the request's
listen port. This is source-profile and captured-byte evidence, without a
claim about identical native implementation internals.

`join_ipv4_tail_129` now constructs that profile from an explicit `SocketAddrV4`
without allocating. It neither infers an edition/version nor binds or discovers
an endpoint. The existing raw-tail API remains available for other explicitly
selected profiles. No original endpoint or asset data is stored as a default.

The live probe selects its routed IPv4 address, binds its own TCP listener and
uses that assigned port for both listen-port fields. It generates the tail with
the production helper and does not read the retained REQJOIN. The original host
accepts it, assigns its slot, marks the verified map at 100 and exchanges a
ping/pong before the probe's bounded disconnect. An initial attempt immediately
after asking the native peer to leave received `Full`; that refusal is retained.
The successful attempt followed a fresh UI observation of an open host slot,
without changing the encoder or relaxing admission checks.

Synthetic mixed-byte-order and boundary endpoint checks pass, along with all
135 native checks and 35 actual WASI classic-codec tests. Strict native/browser-
WASM Clippy, rustdoc, formatting and both library builds pass. Evidence is under
`.scratch/classic-network-20261016/root-join-address/`.
Listening on the advertised endpoint is not proof of peer-to-peer exchange.
Persistent application lobby transport, launcher integration, countdown and
gameplay remain open; NET-C03 is still not closed.

## Continuous bounded native TCP connection

`tcp::Connection` retains the socket, decoder, queued frames and partial next
frame across caller responses. Its `CallPolicy` deadline and read/write/frame
ceilings cover the entire connection; calls cannot renew or reset them. Each
socket wait uses the remaining deadline. Write refusal reports the attempted
request and remaining allowance. The caller can read the actual local endpoint
without selecting a client listen port implicitly.

Transport, framing and budget errors are terminal and shut down the socket.
Partial writes are not retried by a later call. EOF validates the decoder,
shuts down the socket and remains an empty result; partial EOF is an error.
The existing one-shot `exchange` uses the same I/O implementation and retains
its request/reply contract. Its scripted test adapter is test-only.

Seven new regression groups exercise a real multi-step loopback exchange,
queued frames and a deterministic partial next header, cumulative byte/frame
bounds, counter exhaustion, expired queued frames, partial writes/EOF and actual
peer closure after refusal. The initial counter-exhaustion regression failed
before the remaining-allowance check replaced the saturating counter. All 142
native checks and strict native/browser-WASM Clippy, rustdoc, formatting and
library builds pass. The native socket module is excluded on WASM; the 35 pure
classic-codec tests still execute successfully under WASI.

The private original-host probe now calls this production connection for join,
map reply and ping/pong instead of maintaining a separate socket/decoder loop.
The host confirms its verified map status at 100. The client address is generated
from its actual TCP source IP and bound listener port. Delivered-frame artifacts
are complete frame sequences, not raw TCP captures. The refactored one-shot
caller also passes the pinned local PvPGN challenge and missing-init controls;
no account or key is sent. Commands and retained failures are under
`.scratch/classic-network-20261016/root-tcp-connection/`.

This blocking native primitive needs application worker, cancellation and
revision-bound publication wiring. It does not admit a map/player, select
assets, implement lag/reconnect policy or prove peer-to-peer/gameplay fidelity.
