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
