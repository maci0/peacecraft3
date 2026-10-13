# The menu font's face → file binding

A `FrameFont "MasterFont", 0.013, ""` record in the FDF names a **face**, not a
file. The file comes from the shipped skin table, and that is where the binding is
read from — the face name is not mapped to a path by any constant in this project.

Read-only, via the prebuilt `target/release/mpqxtract`; no build was run.

## The binding

`UI\war3skins.txt` carries a `// font references` block inside its `[Default]`
section:

```text
[Default]
...
// font references
MasterFont=Fonts\FRIZQT__.TTF
MessageFont=Fonts\FRIZQT__.TTF
ChatFont=Fonts\FRIZQT__.TTF
EscMenuTextFont=Fonts\FRIZQT__.TTF
InfoPanelTextFont=Fonts\FRIZQT__.TTF
TextTagFont=Fonts\FRIZQT__.TTF
```

So **`MasterFont` → `Fonts\FRIZQT__.TTF`**, authored in `[Default]`.

The table is sectioned — `[Main] [Human] [Orc] [NightElf] [Undead] [Default]` — and
the font keys are defined **only in `[Default]`**. The race sections define their
own console art and none of the font faces, so every race resolves the face through
`[Default]`. That is a fallback, and it is the one the shipped data relies on.

## Provenance

| File | Bytes | SHA-256 (first 24) |
| --- | --- | --- |
| `War3\UI\war3skins.txt` | 44,153 | `7b685b0d2c1129d63de0b116` |
| `War3x\UI\war3skins.txt` | 44,153 | `7b685b0d2c1129d63de0b116` (byte-identical to War3) |
| `War3xLocal\UI\war3skins.txt` | 44,353 | `476a485dc692a3312fdd48b2` |
| `War3\Fonts\FRIZQT__.TTF` | 62,316 | `8f798feb09a0e9dc97daf0a5` |

`Fonts\` ships seven faces (`FRIZQT__.TTF`, `NIM_____.ttf`, `THOWR___.ttf`,
`DFHeiMd.ttf`, `dffn_b31.ttf`, `dfst-m3u.ttf`, `tt5500m_.ttf`); the skin table names
`FRIZQT__.TTF` for all six font keys.

## Duplicate and override ambiguity

Measured over all three tables: **`MasterFont` appears exactly once**, and so does
every other font key — there is **no duplicate and no override** in the shipped
data, so the ambiguity the resolver guards against does not arise here.

The resolver refuses it anyway, because the engine's precedence is **not
established**:

* **Every block with the consulted section name is aggregated**, not just the
  first. A repeated `[Default]` therefore cannot hide a second definition: a face
  defined in two blocks of the same section is `Duplicate` even when the two values
  agree, and a conflicting pair is `Duplicate` with both values reported.
* A face that resolves to an **empty value** is `EmptyValue`, never a binding with
  no file.
* An **empty section name** (`[]`) or an **empty key** (`=value`) is `Malformed`.
* A face defined twice inside one block is likewise `Duplicate`.

**The requested-section-first order is caller policy, not the game's order.** No
native observation of the engine's font lookup was made; that a race section may
override `[Default]` is an inference from the table's shape. What is proven is the
data: the keys, their sections, and the file they name.

## API

`crates/peacecraft3/src/menu_skin.rs` — pure `std`, no Bevy, no Cargo change.

```rust
let table = menu_skin::SkinTable::parse(&text)?;         // SkinError::Malformed
let b = table.font_file(face, active_section)?;          // SkinError::UnknownFace / ::Duplicate
b.file      // "Fonts\FRIZQT__.TTF"
b.section   // "Default" — where the winning entry came from
```

* `face` is the caller's `TextStyle::font`; the module has **no** `MasterFont`
  mapping of its own.
* The requested section is consulted first, then `DEFAULT_SECTION` (`"Default"`) —
  **caller policy, not the verified engine order**.
* **All** blocks with the consulted name are aggregated, so a repeated section
  cannot hide a duplicate.
* A face the table does not define is `UnknownFace`; two entries in the winning
  section are `Duplicate { section, face, values }`; an empty value is
  `EmptyValue`. No default face or file is substituted.
* Section identity is preserved on the result, so a caller can tell an override
  from a fallback.

## Probe against the shipped files

The module run over all three real tables (isolated `rustc`, no cargo): 6 sections
each; `MasterFont`, `MessageFont` and `InfoPanelTextFont` each resolve to
`Fonts\FRIZQT__.TTF` **from `[Default]`** for `Human`, `Orc`, `NightElf`, `Undead`
and `Default` alike; each face appears **1×** across the table.

## Not established — do not infer

* **`FIXEDSIZE` is unresolved native behaviour.** The flag is parsed from
  `FontFlags`, but what it does (a reference-resolution pixel size, versus the
  viewport-relative size the FDF's height implies) is **not** determined by the
  name or by the table. It is reported as parsed-but-unapplied, not interpreted.
* **The engine's actual lookup order is inference, not measurement.** That a race
  section can override `[Default]` is inferred from the table's shape; no native
  observation of the font lookup was made. What is proven is the data: the keys,
  their sections, and the file they name.
* **The shadow offset** remains unapplied (see `menu-sprite-rendering.md`); this
  note does not change that.
* No font file is staged, and no shadow mapping is added, pending review.

## Next, if the native order must be proven

The engine-side binding is in the Legacy executable. The concrete next probe is to
break where the FDF's `FrameFont` face is resolved against the skin table — the
same path that resolves `MainMenuLogo_V%d` through `FUN_005c30a0` (see
`docs/re-notes/glue-coordinate-runtime.md`), which already checks CustomSkin, then
the active skin, then the default skin. Reading that function's section lookup
would settle the override order; a repeated REA pass on the 14.8 MB image has
timed out before and is not the plan.
