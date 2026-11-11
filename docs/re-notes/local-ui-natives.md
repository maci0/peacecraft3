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

## SetCameraTargetController

- Registration push at `0x4a0311` (name `0xe762cc`), handler `0x4a7370`.
- The handler gets the game object (`0x5ec5e0`) and its camera at game `+0x294`
  (`0x5ead20`). A null camera sets last-error `0x57` and returns.
- The unit comes from `0x483940`. A null or removed unit returns with no write.
  `0x4a7370` never reaches the clearing branch of `0x5e3740`.
- Otherwise it calls `0x5e3740(unit, xoffset, yoffset, inherit, 0)` on the camera.
  That routine type-checks the unit object, takes a reference on it and stores it at
  camera `+0x44` (releasing the previous one), stores the two offsets at
  `+0x50`/`+0x54` and zero at `+0x58`, the orientation flag at `+0x48` and the
  last argument at `+0x4c`. This native always passes `0` for `+0x4c`.
- The second caller of `0x5e3740` (`0x4a724e`) passes `1` there. It is another
  native and is not decoded here.
- Several routines in `0x5e1a4e`..`0x5e3e47` write zero to camera `+0x44`. Which
  script camera calls release the controller, and the per-frame follow rule, are
  not decoded.
- Recreation: `ObjectHost::camera_target_controller` returns the last accepted
  unit, offsets and orientation flag. The camera does not follow the unit yet.
  While a controller is set, `GetCameraTargetPosition*`, `PanCameraToTimed` and
  `SetCameraQuickPosition` refuse, because their interaction with the controller
  is open. Source: `camera_view_host.rs`.
- Chapter that calls it: RoC `Orc03`, through `SetCameraTargetControllerNoZForPlayer`
  (`Scripts\Blizzard.j:1321`) with offsets `0.0`, `0.0` and orientation `false`.

## SetCameraField

- Registration push at `0x4a02e6` (name `0xe76298`), handler `0x4a71a0`.
- The handler takes the camera as `SetCameraTargetController` does (null camera: last
  error `0x57`) and calls `0x5e30e0(field, value, duration, 1)`. The last argument
  marks the value as degrees: angle fields are scaled by the degree to radian pair at
  `0xe92320` and `0xe92328` before they are stored.
- `0x5e30e0` first calls `0x5e43a0(field)`, which releases a running ramp on that
  field. Selectors above 9 (compared unsigned) do nothing. For selectors 0 to 9, a
  duration whose magnitude is below the constant at `0xe8df60` sets the field at once
  (`0x800ff0` or `0x803930`); a longer one starts a timed change. The timed branch
  was not read in detail.
- Recreation: `camera_view_host.rs` applies the value through the same field ramp that
  `CameraSetupApplyForceDuration` uses (stored in degrees, linear easing on the
  game clock, a recreation choice). The selector domain the host admits comes from
  the script constants, so the selector guard cannot be reached with the stock names.
- Chapter that calls it: RoC `Orc03` (first refusal after the target controller).
