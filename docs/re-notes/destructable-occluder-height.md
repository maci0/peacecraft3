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
  existing refusals still apply to the stored value: a negative or non-finite height and a height
  above the `i16` range. A `fatLOS` destructable takes the footprint write (last section).

## Height plane write and wide footprint (2026-11-11)

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
- The record read at `[rec+0x48]` (`0x6ce300`, `0x6d1c9f`) is `walkable`, not `fatLOS` (corrected
  below): when nonzero creation registers the object in the list behind `0x6faea0` (0x2c-byte
  records keyed by object, rectangle filled by `0x6ff290` from the same entry's `[+8]`, `[+0xc]`)
  and sets flag `0x400`. The setter never reads it.
- Creation copies `[spawn+0x50]` to `[obj+0x148]` and sets `[obj+0x20] |= 0x10` when
  `[spawn+0x54] != 0` (`0x6bdb96`..`0x6bdba5`); the spawn struct is filled from the destructable
  record at `+0x88` and `+0x8c` (`0x6d0ac7`, `0x6d0afd`).

## Record columns, gate and footprint source (2026-11-11, decoded)

Same exe. The destructable data table is the hash table at `0x113f2c8` (`0x6d25f0` finds a
record by id). `0x6cfb70`..`0x6cfe00` fills each record from the SLK row; every field is one
loader accessor on a column-index object built by `0x6b26e0`, whose slots (`[this+off]` column
index, `[this+off+4]` metadata) name the columns in order: `0x10` `DestructableID`, `0x38` `HP`,
`0x40` `lightweight`, `0x48` `fatLOS`, `0x50` `numVar`, `0x58` `maxPitch`, `0x60` `maxRoll`,
`0x68` `radius`, `0x80` `pathTex`, `0x88` `pathTexDeath`, `0xb0` `occH`, `0xc0` `walkable`,
`0xd0` `fixedRot`. Record fields that follow from the accessors read there:

- `[rec+0x88]` (`0x6b7ad0`, column slot `0xb0`) is `occH` (float, default `0.0`).
- `[rec+0x8c]` (`0x6aebb0`, column slot `0x48`) is `fatLOS`.
- `[rec+0x48]` (`0x6b9550`, slot `0xc0`) is `walkable`, `[rec+0x90]` (`0x6aed60`, slot `0xd0`) is
  `fixedRot`, `[rec+0x50]` (`0x6b7f60`, slot `0x68`) is `radius`, `[rec+0x6c]` (`0x6b7490`, slot
  `0x38`) is `HP`.

So the creation code (`0x6d0ac7`, `0x6d0afd`; `0x6bdb96`..`0x6bdba5`) sets `[obj+0x148] = occH`
and object flag `0x10` exactly when the type has `fatLOS != 0`. The footprint branch of
`0x4c5770` (gate `entry != 0 && [obj+0x20] & 0x10`) therefore applies to `fatLOS` types.

`entry` (argument 4 of `0x4c5770`): when it is 0 the routine looks it up itself with
`0x5c3150([obj+0x34])` (`0x4c57a0`), so the creation call `(obj, 0, 0, 0)` and the setter call
`(obj, 0, entry, 1)` both use the footprint. `0x5c3150` returns `[uidef+0xcc]`, the type's live
pathing texture; `[uidef+0xd0]` (`0x5c31b0`) is the dead one. The destructable data pass at
`0x6b3815`..`0x6b38aa` reads `pathTex` and `pathTexDeath`, maps an empty string or `_`
(`0xe92074`) to no texture, and stores each through `0x5d26e0` (`0x4dfa70`, which opens the
texture). A destructable whose `pathTex` is empty or `_` has `entry = 0`: the centre cell only.

The texture object (`0x4de9f0`, vtable `0xe7a89c`) reads the TGA with `0x71abc0` and `0x71a7c0`,
which return the header width through the first out pointer and the height through the second;
`0x4de9f0` passes `&[tex+0xc]` first and `&[tex+8]` second. So `[tex+0xc]` is the TGA width and
`[tex+8]` the TGA height (the same reading as `Footprint::dimensions` in `unit_placement.rs`).
The iterator `0x4dfe50` calls the callback with (first, second) offsets; the first runs over
`[tex+8]` values and the second over `[tex+0xc]` values (quadrant 0), and the callback
`0x4c56e0` uses them as x and y (`0x4c570f`..`0x4c5717` add them to the pathing cell x and y).
`0x4c5770` takes the quadrant from `0x4c6ab0` of the object's facing (`[obj]+0xb8` then `+0x1c`
returns the angle; `0x48de75` multiplies `CreateDestructable`'s degrees by `0x3c8efa35`, so the
angle is radians): within `0x10db054` (`0.001`) of `0`, of `0x10db10c` (pi/2) or of `0x10db110`
(pi) selects 0, 1, 2, else 3.

Footprint cell value (callback `0x4c571f`..`0x4c573d`, 16-bit arithmetic):
`((terrain << 7) + h + 0x40) & 0xff80`, where `terrain` is the layer height at the pathing cell
`centre path cell + offset` (`0x7044b0`, the same nearest-vertex lookup as the centre cell) and
`h` is `occH` truncated by `0x9a07b0` (`[obj+0x148]`); the cell is `centre lattice cell + (offset
>> 2)` per axis, written with the larger of the old and new value when `assign` is 0 (creation)
and unconditionally when it is 1 (the setter). The row pitch of the plane is `[map+0x68]`, the
power of two at or above the width, with `[map+0x6c]` its log (`0x4c4c1a`..`0x4c4c44`), so the
callback and the centre pointer index the same array. Cells outside the plane are not bounds
checked in the callback.

## Recreation (wide occluder)

- `Rules` (`native_sight.rs`) reads the footprint shift (`0x4c56e6`, `0x4c56fd`), bias and mask
  (`0x4c571f`, `0x4c5734`, `0x4c5739`), epsilon, quarter and half turn (`0x4c6ac6`, `0x4c6ae7`,
  `0x4c6b1d`) and the degrees factor (`0x48de75`) from the image and refuses a changed
  instruction. `Rules::facing_quadrant` is `0x4c6ab0`.
- `HeightGrid::occlude_footprint` writes the rectangle per quadrant. Offsets follow the
  iterator's four start points and steps exactly; the work budget is charged for the texture's
  cell count before any write, and a cell outside the lattice, a terrain vertex outside the map or
  a quadrant above 3 refuses without a write.
- `Placement::destructable_pathing_dimensions` resolves the type's live `pathTex` through the
  existing script path resolver and footprint loader; `ObjectHost::wide_occluder_sizes`
  (`vision_native.rs`) collects the sizes before each sample, and a `fatLOS` type with a positive
  height and no placement refuses. A type with an empty or `_` texture keeps the centre cell.
- Deviations: a height of 0 writes nothing (the original writes terrain-only values, which can
  raise a cell only where the nearest vertex layer exceeds the cell's own layer maximum); `assign`
  mode is not modelled, the grid is rebuilt each sample from the live objects with the larger of
  overlapping values; the string `none` is read as no texture (the original would try
  `none.tga`, find nothing and keep an empty rectangle, which writes nothing as well).
- Covered by `footprint_writes_the_rotated_rectangle_with_rounded_terrain_height`,
  `footprint_refuses_bad_input_without_changing_the_grid`,
  `footprint_operands_come_from_the_image_and_pick_the_facing_quadrant` (`native_sight.rs`) and
  `a_fatlos_destructable_writes_its_pathing_texture_footprint_into_the_sight_grid`
  (`destructable_host_tests.rs`). No test compares a native height plane dump.
