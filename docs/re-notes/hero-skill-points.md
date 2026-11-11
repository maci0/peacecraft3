# Hero skill points: UnitModifySkillPoints

Image: Legacy `Warcraft III.exe`, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed` (1.29.2.9231).
Static reads only; no original client was run for this note.

## Registration (observed)

Native table rows (push signature, push name, push function, `call 0x8ccc80`):

| Native | Signature | Primary handler | Stub block |
| --- | --- | --- | --- |
| `UnitModifySkillPoints` | `(Hunit;I)B` | `0x4af4b0` (row `0x49d7a3..0x49d7b2`) | `0xcbda30` (row `0xcb8a8b..0xcb8a9a`) |
| `UnitStripHeroLevel` | `(Hunit;I)B` | `0x4af870` (row `0x49d78f..0x49d79e`) | `0xcbda30` (row `0xcb8a7c..0xcb8a86`) |

`0xcbda30` is `xor eax,eax; ret`. Which block binds at run time is unresolved, as for the hero
attribute natives (`hero-attribute-queries.md`); the primary handler is implemented.

## Handler `0x4af4b0` (observed)

1. The unit handle resolves through `0x483940` and the handle table lookup `0x4707f0`; the entry
   `+0xc` must be `0x2b61676c` and its `+0x20` zero. Null, removed or unregistered units return
   false (`eax = 0`).
2. The rawcode byte `[unit+0x37]` must be `0x41..0x5a` (uppercase, a hero) and
   `[unit+0x164] & 0x40000000` must be clear (the recreation maps this flag to illusions, as the
   other hero natives do). Otherwise false. `GetHeroSkillPoints` (`0x492a90` -> `0x692a50`)
   uses the same gate and returns `[[unit+0x3fc]+0x94]`, so both natives address one counter.
3. `0x697730` repeats the hero check and tail-jumps to `0xb3f670` with `ecx = [unit+0x3fc]`
   (the hero component) and the delta.

## Component routine `0xb3f670` (observed)

Let `pts = [comp+0x94]` (unspent points) and `d` the delta.

- `if -d > pts then d = -pts` (signed 32-bit; `neg` of `i32::MIN` stays `i32::MIN`, so that
  delta is not limited).
- `d == 0` returns false and changes nothing. This covers a zero delta and any decrease for a
  hero with no unspent points.
- `room` is the sum over the five slots at `comp+0xf8 + 4*i` of `max_i - learned_i` where
  `learned_i = [ability+0x50] + 1` (0 when the hero has no such ability, via `0x6d6570`),
  `max_i = [comp+0x110 + 4*i]`, counting only slots with `learned_i < max_i`.
- `d = min(d, room)` (signed). The limit applies to the delta, not to the resulting total, so a
  hero holding points can exceed `room` in total.
- `[comp+0x94] += d` (wrapping), then the skill UI is refreshed (`0x5f76d0`, `0x5f6e50`) and the
  routine returns true. True is returned even when `d` became zero in the last step. No hero
  level, XP or ability level change and no event are visible in this routine.

The result is the native's boolean (`al`).

## Implementation

`UnitModifySkillPoints` in `crates/pcx-play/src/unit_hero_host.rs` (`unit_modify_skill_points`)
follows the steps above. `room` is `UnitAbilities::capacity`, the sum of unlearned ranks of the
admitted `heroAbilList` skills, which the same file already uses to cap points on level gain.

Inferred, not read from the binary: that `comp+0xf8` and `comp+0x110` hold the hero ability ids
and their maximum ranks (the loop shape and the use of `[ability+0x50]+1` as the learned rank
support it); that the five slots equal the recreation's `Rules::slots` skill list.

## Open

- `UnitStripHeroLevel` is decoded only as far as its dispatch: `0x4af870` -> gate as above ->
  `0x69ea60` -> `0xb40c10`, which lowers the hero level, recomputes the XP threshold and
  unlearns ranks. That routine is not decoded, so the native stays unregistered and refuses.
- Which registration block is live (see above).
