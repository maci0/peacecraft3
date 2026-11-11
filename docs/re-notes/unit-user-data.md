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

## Recreation

`unit_user_data_host.rs` keeps one `Option<i32>` per unit. A write stores it; a null or removed
unit reads 0 and ignores a write. Open: the value of a unit nothing has written. A store of 0 to
`[+0x488]` sits in a long field-initialisation run (`0x6a7cda`, also `0x6a8f4e`), but the function
containing it was not traced to unit creation, so a read of an unwritten value refuses with
"unit user data was never written". Also not read: whether a same-callback read after
`RemoveUnit` still sees the stored value (the recreation returns 0).
