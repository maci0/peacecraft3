# The backdrop edge atlas: candidate geometry, order unproven

A backdrop draws its border from a separate `BackdropEdgeFile`, cut into pieces
named by `BackdropCornerFlags`. This note records the measured facts, the current
limitations, and the open blocker. It is read-only; no build.

Sources: `UI\FrameDef\Glue\StandardTemplates.fdf` and
`UI\Widgets\Glues\GlueScreen-Button1-BackdropBorder.blp` (both extracted locally),
the pinned executable
`sha256 3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`
(x86, image base `0x00400000`), and the retained evidence under
`.scratch/glue-coordinate-runtime/`.

## Measured: what the FDF declares

`StandardButtonBackdropTemplate`, the template every main-menu button inherits:

```text
BackdropTileBackground,
BackdropBackground  "UI\Widgets\Glues\GlueScreen-Button1-BackdropBackground.blp",
BackdropCornerFlags "UL|UR|BL|BR|T|L|B|R",
BackdropCornerSize  0.016,
BackdropBackgroundSize  0.032,
BackdropBackgroundInsets 0.004 0.004 0.004 0.004,
BackdropEdgeFile  "UI\Widgets\Glues\GlueScreen-Button1-BackdropBorder.blp",
```

Eight flags: four corners (`UL UR BL BR`) and four edges (`T L B R`).

## Measured: the texture, and a candidate partition

`blpx info` on the edge file:

```
magic BLP1 · width x height 256 x 32 · alpha bits 8 · encoding Paletted
```

256 / 32 = 8, so the texture **can** be cut into eight square 32×32 slices. That is
a **candidate**, not the native framing. The candidate slot count used by the helper
is `width / height` (8 here); the flag list is the **enabled** set and is never a
divisor.

`BackdropCornerSize 0.016` is the on-screen corner size in frame units — kept as
authored, never converted to pixels.

## Current limitations

- **Native slot count unproven.** `BackdropHalfSides` and `BackdropMirrored` are
  authored token names; their presence does **not** establish the atlas layout, the
  slot count, or any mirroring behaviour.
- **Native slot order unproven.** Which slice draws which flag is unknown; the
  flags' written order is authored, and nothing in the FDF or the texture says the
  layout follows it.
- **No native observation of the edge renderer was made.**
- Therefore the helper answers `slice_uv(slot)` (candidate geometry only) and
  **refuses** `slot_for(flag)`, returning `None` for every flag, with a test pinning
  that it never becomes a silent identity. **The API is not ready for rendering.**

## Retained native evidence

Two distinct regions, both bounded static reads of the pinned executable:

| Region | VA range | Size | Digest |
| --- | --- | --- | --- |
| Token `(char* name, u32 id)` pair table | `0xec54d4 .. 0xec5614` | 320 B, 40 pairs | `sha256 8f6a2f3c43b7024f2ee352d1e1228dd8846ee111342c2a750caf73f15a625f01` |
| Handler pointer table `PTR_LAB_00ec9f28` | `0xec9f28 .. 0xec9f94` | 108 B, **27 non-zero entries observed** | `sha256 20b626da1579ab5dc606052a8136c3a8…` |

The 27-entry run is what was **observed**; an observed non-zero run is not the
table's full semantic length, and no length is claimed for it. The `DAT_010e91b8`
table (below) was not length-checked at all.

## Corrected token id lookup

The pair table's first entries and last entry, read from the pinned bytes:

| index | id | name |
| --- | --- | --- |
| 0 | `0x11d` | `Alpha` |
| 1 | `0x11e` | `AlphaMode` |
| 2 | `0x11f` | `Anchor` |
| 6 | `0x123` | `BackdropTileBackground` |
| 7 | `0x124` | `BackdropBackgroundInsets` |
| 8 | `0x125` | `BackdropCornerFile` |
| 9 | **`0x126`** | **`BackdropCornerFlags`** |
| 10 | **`0x127`** | **`BackdropCornerSize`** |
| 16 | `0x12d` | `BackdropHalfSides` |
| 17 | `0x12e` | `BackdropBlendAll` |
| 18 | **`0x12f`** | **`BackdropEdgeFile`** |
| 19 | `0x130` | `BackdropMirrored` |
| 39 | `0x144` | `CheckBoxDisabledCheckHighlight` |

So `BackdropCornerFlags = 0x126`, `BackdropCornerSize = 0x127`,
`BackdropEdgeFile = 0x12f`, and `0x125` is `BackdropCornerFile`. An earlier draft
reported `0x125/0x126/0x12e`; those were wrong by one entry and are superseded by
this lookup. The cause of the earlier discrepancy is **not** established here and is
not claimed.

## Measured: the dispatch, and what it does not establish

`FUN_00895660` (`0x895660 .. 0x89570d`, 174 B) constructs and then dispatches a
frame:

```c
pcVar3 = *(code **)(param_1 + 0x104);
if (pcVar3 == 0) {
    iVar5 = *(int *)(param_1 + 0x2c);                     // selects a table slot
    iVar2 = FUN_00896980(<name>);                         // name lookup
    if (iVar2 == 0)
        pcVar3 = *(code **)(&DAT_010e91b8 + iVar5 * 4);   // table at 0x10e91b8
    else
        pcVar3 = *(code **)(iVar2 + 0x18);                // looked-up ctor
}
piVar4 = (*pcVar3)(param_2, *(undefined4 *)(param_1 + 0x28), param_4);
FUN_00899ab0(param_1, param_2, param_5, piVar4, param_6);  // dispatch
```

`FUN_00899ab0` (`0x899ab0`, 112 B) then indexes
`PTR_LAB_00ec9f28[*(int *)(param_1 + 0x2c)]` by **the same field**:

```c
pcVar1 = (code *)(&PTR_LAB_00ec9f28)[*(int *)(param_1 + 0x2c)];
(*pcVar1)(param_1, param_4, param_2, param_3, param_5);
```

Measured: `+0x2c` selects a slot in both tables. **Not established:** that `+0x2c`
is written from a property token id; that it is assigned at registration; that the
property tokens are handled inside whichever type handler is selected; or any
semantic length of either table. An earlier draft asserted a token-id → handler
binding and then a registration assignment; both are **withdrawn**.

## Verified: pointer membership by equality, not proximity

`FUN_00897ae0` is listed as a caller of the dispatcher and its address is
numerically near the handler range. Numeric proximity proves nothing, so it was
checked for **exact pointer equality**: `0x897ae0` **is** `PTR_LAB_00ec9f28[23]`,
one of the 27 non-zero entries.

## Measured: the name lookup is a runtime map

`FUN_00896980` (`0x896980 .. 0x896a05`, 133 B) is the name lookup the caller uses.
Call convention `(this = ecx, name = [ebp+8])`:

```c
esi = this;
if ([esi+0x24] == -1) return 0;              // bucket mask
hash = FUN_00433890(name);                   // hash of the name
idx  = hash & [esi+0x24];                    // power-of-two bucket count
entry = [esi+0x18] + idx*8;                  // 8-byte buckets
for (node = ...; node; node = [node+4])      // chained collision list
    if ([node] == hash && FUN_00433850([node+0x14], name) == 0)
        return node;                          // record: +0x14 name, +0x18 ctor
return 0;
```

Two facts this settles:

- **`DAT_01161cb4` is the lookup receiver, not a name.** In the caller the name is
  `[ebx+0x1c]` passed through `FUN_00469010` (`LEA ECX,[EBX+0x1c]`), default
  `DAT_00e68b80`; `MOV ECX,0x1161cb4` loads `this`.
- The lookup is a **runtime hash map**, not a static name → index table. This is the
  record shape only; it does **not** prove what assigns `+0x2c`.

`DAT_01161cb0` is called as `(*DAT_01161cb0)(name, arg, buf, 0x104)` in the layout
type handler `FUN_008994f0` (`0x8994f0`, 266 B, `PTR_LAB_00ec9f28[4]`), with a
260-byte buffer. That proves the **arguments** passed to the pointer; it does not
establish the callee's semantics, and the 260-byte size establishes no relation to
any other 260-byte field.

`FUN_00469010` (`0x469010`, 14 B) is a **common string getter**, reached as
`LEA ECX,[frame+0x1c]; CALL 0x469010` in both `0x895660` and `0x8994f0`. Consuming
`frame+0x1c` does not make it a parse writer.

## Observed: a second token table

A second `(char* name, u32 id)` table exists at `0xed1e38`, with ids from `0x12e`
upward and names `Additive`, `AddAlpha`, `Alpha`, `AlphaKey`, `AmbColor`,
`AmbIntensity`, `Ambient`. Only the names and ids were **observed**; no
call site or consumer was read, so no role or label is claimed for this table. Its
ids overlap the FDF range numerically and the two must not be conflated.

## Static scan result, scoped

An exact static search of `.text` (VA `0x401000 .. 0xe65a00`, initialized bytes
only, `rawsize` honoured) for the dwords `0xec54d4` and `0xec5614` finds **zero**
occurrences, and a pointer search of the initialized data sections finds no pointer
to either address.

This is a **bounded negative for those two exact dwords in the checked ranges**. It
is **not** proof of absence of all references, and it does **not** establish that
the parser uses relative addressing. The 40-pair window `0xec54d4 .. 0xec5614` may
itself be **interior to a larger table**, so a reference to a different base would
not appear as either of the two searched dwords.

## The literal, referrer unknown

`FDFile\Handlers\BackdropFrameHandlers.cpp` is at VA `0xec6c6e` (file `0xac5a6e`);
`BackdropFrameHandlers.cpp` at `0xec6c7e`. A bounded scan finds no stored pointer and
no `push`/`mov` immediate to that address. No referrer expression was read, so no
mechanism is claimed for how it is referenced.

## API

`crates/peacecraft3/src/menu_border.rs` — pure `std`, no Bevy, no Cargo change.
Undeclared and candidate-only; not integrated into any render path.

- `BorderError::{NoFlags, UnknownFlag, NotDivisible { width, height }, SlotOutOfRange { slot, slices }, NoHeight}`
  — `NotDivisible` carries **no** flag field; a separate `SlotOutOfRange` covers an
  out-of-range slice request.
- `CornerFlag::{parse, parse_flags}`, `UvRect`.
- `EdgeAtlas { width, height, flags, corner_size }` with `new(flags, width, height,
  corner_size)`, `slices()` (candidate `width / height`), `slice_px()`,
  `slice_uv(slot)`, and `slot_for(flag)` which returns `None` for every flag.
- The flag count is never a divisor.

## States

- Native atlas slot count / slot order: **unsupported**.
- Candidate geometry: implemented, unintegrated, `slot_for` → `None`.
- Blocker: the token id → frame field extraction (the parse path) is not present in
  any retained evidence; the retained `legacy-fdf-*-handler.json` dossiers are
  handlers that consume fields already set on the frame. No new function target is
  established by the scans performed so far.
