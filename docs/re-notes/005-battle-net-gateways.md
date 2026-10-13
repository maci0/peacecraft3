# 005 — Battle.net gateway list

The classic gateway list is shipped *inside the game archive*, at
`BattleNet\bnserver-WAR3.ini`. Both installs have one, and they differ.

## 1.28c — retail Complete Edition, `wc3/game/app/war3.mpq`

```ini
[Server List Version]
VER=1001

[Server Gateways]
1=uswest.battle.net
2=useast.battle.net
3=asia.battle.net
4=europe.battle.net
```

Zones: `8`, `6`, `-9`, `-1` for `US. West`, `US. East`, `Asia`, `Europe`.

## 1.29b — Battle.net legacy, `Warcraft III (Legacy)/War3.mpq`

```ini
[Server List Version]
VER=1014

[Server Gateways]
1=connect-usw.classic.blizzard.com
2=connect-use.classic.blizzard.com
3=connect-kor.classic.blizzard.com
4=connect-eur.classic.blizzard.com
```

Same four gateways in the same order, same zone numbers, but the realms were
renamed from `*.battle.net` to `*.classic.blizzard.com` and the server list
version bumped `1001 -> 1014`. The gateway count and zone set are the stable
parts; the hostnames and version are not, which is why the test in
`crates/pcx-net/src/bnet.rs` asserts the zones and refuses to pin the hostnames.

## What is not here

The wire protocol. 1.29b's Battle.net is encapsulated inside
`Warcraft III Launcher.exe`; the game does not connect to a realm directly, and
there is no user-run PvPGN-style realm for this build to talk to. The bytes
cannot be observed from this machine, so they are not written down here.

`crates/pcx-net/src/bnet.rs` models the gateway list and the client session's
state machine (connect, handshake, version token, check-revision token, lobby,
game), with every transition gated on the realm granting it. That is the
authority model, which is protocol-independent; the packets that would drive it
across the wire remain open.

## Format notes

- CRLF-terminated, as shipped.
- Gateway display order comes from the numbered `[Server Gateways]` keys, not
  from file order within the section.
- A gateway with no section of its own is kept with zone 0 and a display name
  equal to its hostname, rather than dropped.
