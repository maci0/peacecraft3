# Local presentation natives

Image: Legacy `Warcraft III.exe` 1.29.2.9231, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`. Native-code
evidence only (static disassembly, `rizin`/`objdump`). No original-client run.
Each native is registered by pushing signature, name and handler, then calling
`0x8ccc80`.

The recreation stores the state each native writes. It renders none of it.

## SetReservedLocalHeroButtons

- Registration push at `0x49fa18` (name `0xe753c0`), handler `0x4a9aa0`.
- The handler fetches the game object (`0x5ec5e0`, arguments `1, 0`) and calls
  `0x650e20` on the object at game `+0x41c` (the hero button bar). That routine
  stores the argument, unclamped and unvalidated, in the dword at bar `+0x184`,
  then tail-calls the bar layout routine `0x6471f0`.
- Return type is `nothing`. There is no null or range path in the handler.
- Recreation: `ObjectHost::reserved_local_hero_buttons` returns the last integer
  (`None` before any call). Source: `hero_button_reserve_host.rs`.
- Open: how the layout routine consumes the field (reserved slots on the hero
  button column). The bar is not drawn, so the value has no visible effect yet.
- Chapter that calls it: TFT `UndeadX06` (`war3map.j:2182`, argument 2).

## SetAltMinimapIcon

- Registration push at `0x49f8fa` (name `0xe75228`), handler `0x4a6ea0`.
- Nothing happens when the game object global at `0x113b6f4` is null.
- The string argument is converted with `0x469010`. A null or empty string
  ends the call with no write, so no call clears the icon.
- `0x708dc0` tests the `TRIGSTR_` prefix (`0xea85f0`). A match is replaced by
  the text `0x708d70` finds in the map string table (`0x477b90`); an empty
  result also ends the call.
- Otherwise the handler calls `0x64c9b0` on the object at game `+0x400` (the
  minimap). It releases the old texture (`+0x1a0`), stores the path in the string
  at `+0x1cc`, loads the texture with `0x71ba80`, and drops the handle again when
  the load status at `+0x310` is not `0x10`.
- `0x65be9b` is the minimap loop's skip test for units with bit `0x200` at unit
  `+0x48c` (`UnitSetUsesAltIcon`, see `unit-alt-icon.md`). The routine that draws
  those units with the alternate icon is not decoded.
- Recreation: `ObjectHost::alt_minimap_icon` returns the last accepted path
  (`None` before one). Null and empty paths are ignored. A `TRIGSTR_` text
  refuses, because this host has no map string table. The texture load and its
  failure path are not modeled. Source: `alt_minimap_icon_host.rs`.
- Chapters that call it: TFT `HumanX01` (`war3map.j:7476`) and `UndeadX08`
  (`war3map.j:8396`), both with a literal `UI\Minimap\Minimap-Tower` path.
