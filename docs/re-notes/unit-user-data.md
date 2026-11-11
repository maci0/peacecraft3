# Unit user data

Image: Legacy `Warcraft III.exe`, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed` (1.29.2.9231). Static reads
only.

## Handlers (observed)

Native table rows (push signature, push name, push function, `call 0x8ccc80`):

| Native | Handler |
| --- | --- |
| `GetUnitUserData` (name `0xe72eb4`) | `0x497d50` |
| `SetUnitUserData` (name `0xe72ec4`) | `0x4abcf0` |

Both resolve the unit argument with `0x483940` only (no `0x4707f0` table gate, no hero or alive
test). `0x497d50` returns 0 for a null result and otherwise `[unit+0x488]`. `0x4abcf0` does nothing
for a null result and otherwise stores the integer to `[unit+0x488]`. A second registration block
(`0xcb954d`, `0xcb9561`) uses the stub `0xcbda30` (`xor eax,eax; ret`); which block binds is
unresolved, as for the other natives.

`0x483940` yields null for a handle that no longer resolves; `removed-unit-locations.md` observed
that after `RemoveUnit` a handle resolves to null from the next timer callback on.

## Creation default (observed)

The CUnit constructor `0x6756d0` zeroes `eax` (`0x675cc6`) and stores it to `[unit+0x488]` at
`0x675d0d`, so a unit nothing has written reads 0. The stores at `0x6a7cda` and `0x6a8f4e` belong to
a different class (constructor `0x6a71e0`, allocation size `0x8fc`). Other CUnit accesses to
`+0x488` are the getter `0x497d64` and setter `0x4abd05`; nothing else writes it, so `RemoveUnit`
does not clear it. A read through a handle that still resolves returns the stored value; once the
handle stops resolving (next timer callback on) it reads 0.

## Recreation

`unit_user_data_host.rs` keeps one `i32` per unit, initially 0. A write stores it; a null or removed
unit reads 0 and ignores a write. Open: a read in the same callback as `RemoveUnit` returns the
stored value in the original; the host's removed-unit model (shared by every native) returns 0 from
the removal on.
