# Unit resource amounts

Target: selected Legacy `Warcraft III.exe` 1.29.2.9231, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.

## Native registration

The primary registration block registers the three natives with these
signatures and handlers:

| native | signature | handler |
|--------|-----------|---------|
| `SetResourceAmount` | `(Hunit;I)V` | `0x4a9ac0` |
| `AddResourceAmount` | `(Hunit;I)V` | `0x489890` |
| `GetResourceAmount` | `(Hunit;)I` | `0x495300` |

## Handler behavior

All three handlers resolve the argument handle through `0x483940`, then the
object through `0x4707f0`. They require the resolved object's `+0xc` dword to
equal `0x2b61676c` (the generic unit handle tag, not a gold-mine tag) and
`+0x20` to be zero, then call `0x696b00` with the resolved unit. A null or
failed resolution, another family, or a failed `0x696b00` gate skips the write
and returns zero for the getter.

`0x696b00` is `mov eax,[ecx+0x164]; shr eax,17; and eax,1; ret` (bytes
`8b8164010000 c1e811 83e001 c3`). The reviewed evidence does not establish
whether bit 17 means alive, not removed, valid-handle, or another object
state; the recreation therefore does not claim a death check. It retains a
removed unit's amount only on the removal tick, then treats the tombstone as
disposed on later ticks. A followup probe removed a mine at 5.000 and measured
zero reads with ignored writes at 5.063 and 6.063, so the later-tick branch is
measured; the exact native reclamation phase remains unqualified.

## Ability slot initialization

The ability constructor/loader at `0xb60d90` writes the slot before any script
runs. It formats the key `DataA1` with `sprintf`-style format `"%s%d"` at
`0xe8de98`, field prefix `DataA` at `0xe9eea4` and index 1, looks the key up
through `0x6b06a0`, converts the result through `0x9a07b0`, and stores it:

```
0xb60db1  push 0x112d88c; lea ecx,[edi+0x74]; call 0x800ff0
0xb60dc0  push 1; push [0x10dce1c] ("DataA"); push 0xe8de98 ("%s%d")
0xb60dde  call 0x6b06a0                  ; look up DataA1 on the ability
0xb60dec  call 0x9a07b0                  ; integer conversion
0xb60df3  mov [edi+0x70], eax            ; resource slot
```

`DataA1` is the metadata field `Gld1` (abilitymetadata.slk row 276): slk
`AbilityData`, data index 1, type `int`, min 0, max 99999, specific to
`Agld`. The gold-mine ability `Agld` is mapped to the implementation class
`Agmb` at `0xb62bf0`:

```
0xb62bf0  push 0x11b5330                   ; registry
0xb62bf5  push 0x41676d62                 ; 'Agmb'
0xb62bfa  push 0x41676c64                 ; 'Agld'
0xb62bff  call 0x7f6360                   ; insert(Agld -> Agmb)
```

`0xb626a0` returns `0x41676d62` (`Agmb`) and `0xb60d80` returns
`0x41676c64` (`Agld`). In the shipped `Units\AbilityData.slk`, `Agld` is row
341 with `levels=1` and `DataA1=12500` (column 23). The offline probe saw a
fresh `ngol` start at `amount=12500`, confirming that constructor seeding from
this row is observable. The recreation therefore reads `DataA1` from the
selected `AbilityData` table for a unit whose `abilList` contains `Agld`.

The other gold-mine abilities also carry `DataA1=10` (`Aegm` row 300, `Abgm`
row 202), but fresh `egol` and `ugol` were observed at `amount=0`. The reviewed
class mapping binds only `Agld` to `Agmb`, so the recreation seeds only through
`Agld`; a resource-class unit without that alias starts at zero. This is an
asset-derived proxy for the class binding, not an assertion that `Aegm` or
`Abgm` constructors are unreachable in every native path. The
`war3mapUnits.doo` placement amount is still not derived: the probe's placed
mines were created by script, so a map-authored non-default placement amount
remains an unverified boundary.

## Family classification

`0x6d6570` returns the ability object only when the unit owns it. The shipped
`Units\UnitData.slk` records the family in the `buffType` column (column 23):
`ngol`, `egol` and `ugol` carry `resource`; every other row carries another
token such as `_`, `factory`, `buffer` or `townhall`. The recreation therefore
classifies a unit from its `UnitData` `buffType` metadata, not from rawcodes.
This is an asset-authored proxy: the reviewed class mapping covers `Agld`
(`ngol`'s `abilList` entry) to `Agmb`, but not `Aegm` (`egol`) or `Abgm`
(`ugol`) to the same implementation class, and the ability runtime is not
modelled.

The internals reach the same ability slot:

- set `0x69d8a0`: `0x6d6570` with ability id `0x41676d62` (`'Agmb'` in the
  script integer encoding, returned by `0xb626a0`), then `mov [eax+0x70], arg`
  and `0x5f6d60`.
- add `0x67a620`: the same lookup, then `add [eax+0x70], arg` and `0x5f6d60`.
- get `0x6929c0`: the same lookup, then `mov eax, [eax+0x70]`; a missing
  ability returns zero.

Only the three native handlers call `0x69d8a0`, `0x6929c0` and `0x67a620`.
The constructor at `0xb60d90` seeds the same slot from `DataA1`; no other
reviewed native path seeds it, and no reviewed path reads a placement amount
from `war3mapUnits.doo`.

No bound, sign, or clamp check appears in these paths. Set stores the raw
`i32`; Add is a wrapping `i32` addition.

## Recreation boundary

`ObjectHost::with_resource_amounts` is a caller opt-in after
`ObjectHost::with_unit_catalog`. `SetResourceAmount` anchors admission; the
getter and adder are admitted when declared. The opt-in requires the
non-constant setter and adder contracts and a source shared with `CreateUnit`,
and stores the caller's selected `DefaultTables` so creation can read the
`Agld` seed.

The retained amount is one `Option<i32>` on the existing unit record, so the
existing unit budget bounds it. Creation seeds `Some(DataA1)` for a
`buffType=resource` unit whose `abilList` contains `Agld`, and `Some(0)` for a
resource unit without that alias. Non-resource units keep `None` and answer
zero to the getter while ignoring the setter/adder, matching the foreign-unit
probe. An admitted resource unit must have readable ability tables; a missing
or malformed `Agld`/`DataA1` is reported rather than replaced by an invented
default. Set stores the raw integer; Add wraps on `i32`. Null handles are
no-ops; another handle family is refused.

`RemoveUnit` captures the current amount and the game clock into the tombstone.
On the same game tick, `GetResourceAmount` still returns the captured amount
and Set/Add mutate it, matching the probe's `removed-get=123` then
`removed-after-set=7`. On later ticks the recreation answers zero and ignores
writes, matching the followup probe's 5.063 and 6.063 records after removal at
5.000; the exact native reclamation phase is not established. `KillUnit`
leaves the live unit
record in place, so its amount remains readable and writable, matching
`kill-before=777`, `kill-after-get=777`, `kill-after-set=5` and
`kill-after-add=7`.

Human02's script creates the authored mine and then calls
`SetResourceAmount(gg_unit_ngol_0009, 20000)` at line 911. The derived
`DataA1=12500` seed now reproduces the pre-call value, and the script's
explicit set still determines the value used by the gate.

An isolated Human05 probe built a Haunted Gold Mine over an existing mine.
The new structure reported the original mine's 12500 gold during construction
and after completion, whereas the separately created resource-family unit in
the earlier probe initially reported zero. The native build callback `0xb70220`
calls `0xb27980`, which links the two mine objects and copies the resource
amount. Constructor seeding alone does not cover target-ordered mine
construction. The construction host now copies the original amount onto the
new resource site at construction start and copies the site's remaining amount
back when a completed site with positive gold is killed. The original keeps
its amount while hidden. The zero-gold branch of `0xb27600` does not copy gold
or show the original; it calls life setter slot `0x124` with zero. That
setter's death sequence is not established, so a nonpositive teardown still
refuses, as does cancellation.
