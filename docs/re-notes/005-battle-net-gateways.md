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

Both files list four gateways in the same order and with the same zones.
Hostnames changed from `*.battle.net` to `*.classic.blizzard.com`, and the list
version rose from `1001` to `1014`. The test in `crates/pcx-net/src/bnet.rs`
asserts the observed zones without fixing the hostnames.

## Wire protocol scope

This note measures gateway data. PvPGN and GHost++ provide independent BNCS
and W3GS implementations. Exact 1.29b support,
including authentication differences and any required client modifications,
still needs a version-specific test.

`crates/pcx-net/src/bnet.rs` models gateway parsing and response-gated session
transitions. It does not send BNCS packets or implement an original-compatible
game session. See [classic network interoperability](classic-network-interoperability.md)
for primary sources, local reference testing and the open crossplay gates.

## Format notes

- CRLF-terminated, as shipped.
- Gateway display order comes from the numbered `[Server Gateways]` keys, not
  from file order within the section.
- The project parser keeps a gateway without its own section with zone 0 and a display name
  equal to its hostname, rather than dropped.
