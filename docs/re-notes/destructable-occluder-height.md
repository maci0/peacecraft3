# Destructable occluder height natives

Reviewed exe `Warcraft III.exe` 1.29.2.9231 (SHA-256 `3f2ed012...9570eed`, ImageBase `0x400000`).
Addresses are virtual. "Observed" is read from the assembly or shipped data.

## Natives (observed)

- Registration (`0x8ccc80` table, name strings `0xe71968` and `0xe71988`): `0x49d1d0` pushes
  `GetDestructableOccluderHeight` with handler `0x491ca0`, `0x49d1e4` pushes
  `SetDestructableOccluderHeight` with handler `0x4a7c10`. The second registration run at
  `0xcb84b8` and `0xcb84cc` uses the shared stub `0xcbda30` for both.
- Get `0x491ca0(handle)`: resolves the handle with `0x481470`; a null result returns the shared
  float zero (`0x112d88c`), otherwise it returns the float at `[destructable+0x148]`.
- Set `0x4a7c10(handle, height)`: resolves the handle with `0x481470`; a null result does nothing.
  Otherwise it stores the height in `[destructable+0x148]`, then calls `0x5c3150([destructable+0x34])`
  and `0x4c5770(destructable, 0, result, 1)` on the map object (`[0x113b6f4]+0x34`), the same
  refresh routine the `CreateDestructable` path calls after placement (`0x6d0f2f`). Any float is
  stored; there is no range check in the setter.
- The destructable data loader (`0x6b2f32`..`0x6b2fbb`) reads `occH` after `texFile` and before
  `flyH` into the type record, at `[type+0xb0]`. `DestructableData.slk` carries `occH` and `fatLOS`.

## Not decoded

- The copy of the type's `occH` into `[destructable+0x148]` at creation was not located (the
  creation path `0x48de70` to `0x6d0fe0` to `0x6d09a0` was read, no store to `+0x148` found).
  The initial value is taken from `docs/re-notes/campaign-minimap.md`: the retained comparison
  matched all 12,288 height cells using the catalog `occH` of live destructables.
- What `0x5c3150` and `0x4c5770` do with the new height. The sight builder (`vision_native.rs`)
  is assumed to read the stored field; unverified.

## Recreation

- `destructable_host.rs`: `GetDestructableOccluderHeight` returns the height stored by the setter,
  else the catalog `occH` through `DestructableCatalog::sight_occlusion`; a null destructable reads
  `0.0`. `SetDestructableOccluderHeight` stores the height per handle (`destructable_occluder`);
  a null destructable is ignored. A missing catalog or an unresolved record refuses.
- `vision_native.rs` takes the stored height over the catalog `occH` for that destructable, and the
  existing refusals still apply to the stored value: a negative or non-finite height, a height above
  the `i16` range and a positive height on a `fatLOS` destructable.
