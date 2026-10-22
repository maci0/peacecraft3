# Hero attribute queries

`GetHeroStr`, `GetHeroAgi` and `GetHeroInt` (`unit whichHero, boolean includeBonuses`,
returns integer) are admitted on the executed hero by `unit_hero_host.rs`. Admission
requires the declaration in the selected install's `Scripts\common.j` and the same
declaration source as `CreateUnit`. The declarations are plain `native`, not
`constant native`.

## Source evidence

- Selected `Scripts\common.j` (read through `pcx_mpq::MpqSet`, the production archive
  set): `native` declarations at lines 1516-1518. Gate:
  `selected_install_common_declares_the_hero_attribute_natives`, run with
  `PCX_WAR3_INSTALL` set to the local Legacy install; skips when unset.
- `GetHeroStatBJ` (`blizzard.j:2710-2718`) forwards `includeBonuses` unchanged. This is a
  BJ forwarding fact only; it does not qualify the native's semantics.

## Native evidence (selected `Warcraft III.exe`, SHA-256 `3f2ed012...570eed`, 1.29.2.9231)

Static disassembly (`llvm-objdump`), file offsets through PE section raw pointers. The
earlier claim of "no direct pointers" was a search bug (ripgrep in UTF-8 mode); a
C-locale byte search finds them.

- Name strings: `.rdata` VA `0xE7208C` (`GetHeroStr`), `0xE72098` (`GetHeroAgi`),
  `0xE720A4` (`GetHeroInt`). Signature `(Hunit;B)I` at `0xE72080`.
- Primary registration block at `0x49D741`-`0x49D776`: each call pushes
  signature, name, handler, then calls registrar `0x8CCC80`:
  - `GetHeroStr` name `0xE7208C`, handler `0x492B70`
  - `GetHeroAgi` name `0xE72098`, handler `0x492830`
  - `GetHeroInt` name `0xE720A4`, handler `0x4928A0`
- Second registration block at file `0x8B7E22`: the same three names and signature
  with handler `0xCBDA30`, which is `xor eax,eax; ret` (a constant-zero stub). Which
  registration the script compiler binds is unresolved; the registrar's duplicate-name
  behavior is not yet read.
- Handler forwarding (primary block):
  - `GetHeroStr` `0x492B70` -> getter `0x692AA0`
  - `GetHeroAgi` `0x492830` -> getter `0x690C20`
  - `GetHeroInt` `0x4928A0` -> getter `0x691760`
- Handler gate (identical shape in all three): the unit handle resolves through
  `0x483940`, and the handle table lookup `0x4707F0` must succeed. The entry's `+0xC`
  is `0x2B61676C` and its `+0x20` is zero. This is generic live-handle validation, not
  a combat-death test. The original verifier used the same `0x483940`/`0x4707F0` gate
  for `SetUnitExploded` and found that dead but still registered units pass it, while
  removed units fail it. The rawcode byte `+0x37` must be uppercase (hero), and flag
  `+0x164` bit `0x40000000` must be clear. Otherwise the handler returns integer 0.
  - Removed unit: fails the handle gate, so reads 0. Shape verified; the removed-unit
    test asserts 0.
  - Null: returns 0 (`0x483940` yields no unit).
  - Non-hero (lowercase rawcode): returns 0 in the primary handler's uppercase branch.
    The constant-zero registration returns 0 for every input. Both bindings therefore
    return 0, and the implementation returns 0 without deciding which binding is live.
  - Dead but registered: passes the handle gate. The primary binding then runs the getter,
    which the wrapper does not gate on death. The constant-zero binding returns 0. So the
    dead-hero result depends on the unresolved binding and is unqualified; the
    implementation returns the value for dead heroes without asserting it is native.
  - Flag `+0x164 & 0x40000000`: meaning not qualified.
- STR getter `0x692AA0` (partial): the result starts from `[unit+0x3FC]+0x98` plus a
  growth-derived term from `0x7FB2B0`, `0x9A1620`, `0x99FFE0` and `0x9A07B0`. When the
  `includeBonuses` argument is zero, the routine subtracts the return value of
  `0x6D66D0`; when it is nonzero, the subtraction is skipped. The branch shape is verified.
  The routines `0x7FB2B0`, `0x9A1620`, `0x99FFE0`, `0x9A07B0` and `0x6D66D0` are not
  decompiled; their identities and the contents of `+0x98` are unqualified.

## Current implementation and status

Implementation (`unit_hero_host.rs`): the value is the source-derived base growth plus
retained permanent strength (`includeBonuses` false) or plus held passive items too (true).

Qualified by the static evidence above: the handler-to-getter mapping for all three
attributes, the handle gate (live-handle validation, not combat death), the 0 result for
removed, null and non-hero units under either binding, and the `includeBonuses` branch
shape. The dead-hero result is not qualified; it depends on the unresolved binding.

Not qualified, and therefore inference: whether `+0x98` contains the tome's permanent
strength, whether `0x6D66D0` contains exactly held passive items, the growth terms, and
which registration binds in the script compiler. The model's split of permanent tome
gains into the `includeBonuses` false total is inference until those are read.

## Boundaries

- A held intelligence passive joins `GetHeroInt(true)` through the same shared held-item path
  as strength and agility. `GetHeroInt(false)` stays at the base total.
- With `includeBonuses` true, a held item that refuses also refuses `GetHeroStr`,
  `GetHeroAgi` and `GetHeroInt`.
- `SetHeroStr`/`SetHeroAgi`/`SetHeroInt` are not implemented.

## Next static query

Decompile or annotate `0x6D66D0` (includeBonuses subtraction), `0x7FB2B0` (base term
source) and `0x8CCC80` (duplicate-name registration behavior), and read the function
containing the second registration block (file `0x8B7E22`).

## Tests

`inventory_powerup_host_tests.rs`: `hero_attribute_queries_split_base_permanent_and_held_totals`
and `hero_attribute_queries_read_zero_for_ordinary_units_and_null` (synthetic fixture),
`hero_attribute_declarations_must_match_the_admitted_signature` (synthetic text), and the
local-install gate above.
