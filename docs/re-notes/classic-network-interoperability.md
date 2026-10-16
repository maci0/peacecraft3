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

Build from pinned upstream source in Docker, disable tracking, expose only a
loopback test endpoint, and use an isolated internal runtime network. Retain
source/base/image digests, effective configuration, command exit codes and a
bounded real BNCS response probe. Runtime success must be added here only after
root review of actual logs. A listening socket alone does not close NET-C01;
a BNCS response alone does not close login, W3GS join or crossplay gates.
