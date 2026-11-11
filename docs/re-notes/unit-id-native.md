# UnitId native

Image: Legacy `Warcraft III.exe` 1.29.2.9231, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`. Static disassembly only; no
original-client run. Implementation: `crates/pcx-play/src/unit_id_host.rs`.

## Handler (observed)

- Registration: name string `0xe7087c`, handler `0x4af290` (pushed at `0x49c69b`..`0x49c6a8`).
  `OrderId` (`0x4a3be0`, table lookup `0x477030`), `AbilityId` (`0x4891b0`, `0x476f10`) and
  `UnitId` read three separate name tables of the script context at `[0x113b6f4]`.
- `0x4af290`: no context gives 0. Otherwise it reads the C string of the string argument
  (`0x469010`: `[arg+8]` then `[+0x1c]`, null for a null string) and calls `0x4770c0`, which returns
  -1 for a null string, a missing table (error `0x57`) or a name the table does not hold, else the
  stored id. The handler turns -1 into 0 (`cmove` at `0x4af2c3`).
- The lookup is `0x47b180` on the hash table at `[context+0x20]+4`: hash `0x433890` upper-cases
  ASCII letters and maps `/` to `\`, and the key compare is `_strnicmp` (`0x433490` through
  `0xa77598` to `0xa99c10`, which folds `A`..`Z`). The name match is case-insensitive.

## Table fill (observed)

- `0x69af80` walks the unit definition list (`0x113e8dc`/`0x113e8e4`) and, per definition, calls
  `0x47c6e0` (jump to `0x47c5c0`) with the unit's rawcode (`[def+0x14]`) and a string
  (`0x468850` of `[def+0x2a8]`). It is the only caller.
- `0x47c5c0(id, name)`: an empty name registers nothing. A name that exists with the same id
  stays. A name that exists with another id is not replaced: the new entry takes the name plus a
  counter formatted with `0xe6e7ac` (`0x47c620`..`0x47c652` loops until unused). The first
  definition to register a name owns it.

## Inferred, not traced

The text registered is taken to be the `UnitUI.slk` `name` column (`footman` for `hfoo`,
`hydralisk` for `zhyd`). The slot read at `[def+0x2a8]` was not matched to the column loader
(`0x6b3c00`..`0x6b4800` registers the columns and puts `regenMana` at `+0x2a8` in that listing,
so the loader offsets and this read are not reconciled). The `UnitUI` `name` is the reading that
gives the shipped call below its unit.

## Shipped use

RoC NightElf02 `war3map.j:1567`: `String2UnitIdBJ("hydralisk")` through `Blizzard.j:2998`, creating
`zhyd`. In `UnitUI.slk` `hydralisk` is the `name` of `zhyd` only.

## Recreation

`with_unit_id_lookup` admits the native only when the program declares
`constant native UnitId takes string returns integer` on `CreateUnit`'s source. The first call
reads every shipped unit row's `UnitUI` name, lower-cased, into a table. Rows without a `UnitUI`
cell register nothing (shipped rows such as `oang` have none). Answers: null string, empty or
unknown name give 0; a known name gives its rawcode.

Refused:

- a name carried by two or more shipped rows (the registration order that decides the owner is
  not decoded);
- tables where no unit row has a `UnitUI` name.

Open: map-defined custom units are not enumerated, so a custom unit that shares a shipped name
would own it or not depending on registration order. `UnitId2String` (`0x4af250`) and `AbilityId`
are not implemented.
