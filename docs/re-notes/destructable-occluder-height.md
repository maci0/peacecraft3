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

## Height plane write and wide footprint (2026-11-11, partial)

Same exe. The height plane writer is `0x4c5770(sight map, destructable, skip, entry, assign)`,
called with `(obj, 0, 0, 0)` after placement (`0x6d0f2f`) and with `(obj, 0, entry, 1)` by the
setter (`0x4a7c4b`, `entry = 0x5c3150([obj+0x34])`, a lookup that creates a UnitUIDef entry
when missing).

Observed:

- `0x480cc0` converts the object position to a lattice cell (`(pos - [map+0x4c]) * [map+0x74]`
  rounded by `0x9a07b0`, clamped to `[map+0x64]`, `[map+0x70]` minus one). The plane is the
  16-bit array at `[map+0x38]`, row shift `[map+0x6c]`.
- Without a footprint (`0x4c594a`): the single centre cell receives
  `(terrain(centre path cell) << 7) + round([obj+0x148])` as 16 bits; `assign` stores it, else
  the maximum with the old cell is kept. This is what `HeightGrid::occlude` models.
- With a footprint, gated on `entry != 0` and `[obj+0x20] & 0x10` (`0x4c58b5`..`0x4c58c3`),
  `0x4dfe50(entry, quadrant, callback 0x4c56e0, ctx)` visits every cell of a `w x h` rectangle,
  `w = [entry+8]`, `h = [entry+0xc]`, with signed offsets `(x, y)` in pathing cells from the
  centre path cell. With `A = (w+1)>>1` and `B = (h+1)>>1`: quadrant 0 covers
  `x in [-A, w-1-A]`, `y in [-B, h-1-B]`; quadrant 1 covers `x in [B-h+1, B]`,
  `y in [-A, w-1-A]`; quadrant 2 covers `x in [A-w+1, A]`, `y in [B-h+1, B]`; quadrant 3 covers
  `x in [-B, h-1-B]`, `y in [A-w+1, A]`. `0x4c6ab0(facing)` returns 0, 1 or 2 when the facing
  (radians) is within 0.001 of 0, pi/2 or pi, else 3. The pixel value of the footprint is not
  used. The callback (`0x4c56e0`) writes lattice cell `centre + (offset >> 2)` per axis
  (`shr 2` at `0x4c56e6` and `0x4c56fd`; equality with the grid shift at `0x4c632a` is not checked) with
  `((terrain(path cell) << 7) + occH + 0x40) & ~0x7f`, honoring the same skip/assign modes.
- `fatLOS` is read from the destructable record at `[rec+0x48]` (`0x6ce300`, `0x6d1c9f`): when
  nonzero creation registers the object in the list behind `0x6faea0` (0x2c-byte records keyed
  by object, rectangle filled by `0x6ff290` from the same entry's `[+8]`, `[+0xc]`) and sets
  flag `0x400`. The setter never reads it.
- Creation copies `[spawn+0x50]` to `[obj+0x148]` and sets `[obj+0x20] |= 0x10` when
  `[spawn+0x54] != 0` (`0x6bdb96`..`0x6bdba5`); the spawn struct is filled from the destructable
  record at `+0x88` and `+0x8c` (`0x6d0ac7`, `0x6d0afd`).

Not decoded, so wide occluders still refuse:

- Which `DestructableData` columns `+0x88`/`+0x8c` and flag `0x10` are. The record layout read
  from the loader (`0x6b28a0`..`0x6b3300`: `occH` at `+0xb0`, `pathTexDeath` at `+0x88`) does
  not match the creation code's use of `+0x88` as an occluder height, so the gate is unresolved.
- Where `entry[+8]`/`[+0xc]` come from for a destructable id (UnitUIDef entry; the pathing
  texture size is the likely source and is unverified).
- Whether the footprint branch applies to `fatLOS` destructables at all: the gate does not read
  `fatLOS`, and the consumer of the `0x6faea0` list was not located.

The recreation keeps refusing a positive height on a `fatLOS` destructable. The override path
inside the sight grid is covered by `sight_grid_takes_the_stored_occluder_height_over_the_catalog_height`
(`destructable_host_tests.rs`).
