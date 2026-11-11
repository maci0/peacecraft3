# UnitIgnoreAlarm and UnitIgnoreAlarmToggled

Image: Legacy `Warcraft III.exe` 1.29.2.9231, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`. Static
disassembly only; no original-client run.

## Natives

- Registration rows at `0x49de71` (`UnitIgnoreAlarm`, signature `(Hunit;B)B`, handler `0x4af2d0`)
  and `0x49de88` (`UnitIgnoreAlarmToggled`, `(Hunit;)B`, handler `0x4af330`).
- Both resolve the unit with `0x483940`; a null or removed unit returns false.
- Both then call `0x6d6570(code, 0, 0, 1, 1)` on the unit with `code` from the getter `0xb167f0`
  (`mov eax, 0x41616c72`, class code `Aalr`). `0x6d6570` has a switch for a few cached class
  codes (`Aalr` is not among them) and otherwise walks the unit's ability list (`[unit+0x3e8]`),
  returning the first ability whose class code (`vtable+0x1c`) equals `code`. The `0, 0, 1, 1`
  arguments make it compare class codes (not aliases) and accept abilities whether or not their
  flag `0x20` is set. No match returns null, and the native returns false.
- `UnitIgnoreAlarm(unit, flag)`: `flag != 0` runs `or [ability+0x20], 0x80` (`0x4af309`),
  `flag == 0` runs `and [ability+0x20], 0xffffff7f` (`0x4af318`). It returns true.
- `UnitIgnoreAlarmToggled(unit)`: returns `([ability+0x20] >> 7) & 1` (`0x4af363`).

## The ability class

`CAbilityAlarm` (name string `0xf17304`, getter `0xb16090`, name slot `0xf1704c`, vtable
`0xf16ff4`) has class code `Aalr` (`0xb160a0`). `AbilityData.slk` carries one row, alias `Aalr`,
code `Aalr`. Its constructor `0xb16774` calls the shared ability constructor `0x46d010`, whose
object base `0x46d0d0` clears the flags at `[this+0x20]` (`0x46d127`), so the ignore bit starts
clear. The alarm class functions around `0xb164f0..0xb16700` read the `Aalr` ability data row;
what an alarm does when it fires is not decoded.

No `UnitAbilities.slk` row lists `Aalr`, and the only references to the code constant in the
image are inside the alarm class and the natives. A unit therefore has the ability only when a map
adds it (`UnitAddAbility`) or lists it in a custom unit row.

## Recreation

- `autocast_rules.rs` reads the class code from the getter both natives call, after checking the
  set, clear and read instructions (`Rules::alarm_code`). The ignore bit is stored per unit as
  `UnitAbilities::alarm_ignored` (the alias of the matching ability).
- `unit_hero_host.rs` (`unit_alarm_call`) walks the unit's ability list in order and picks the
  first alias whose `AbilityData` `code` equals the class code. No such ability: false, nothing
  changes. The cache and the mirror-image snapshot refuse a unit with the bit set, as for
  altered autocast state; removing the ability by alias refuses while its bit is set.
- The ability list is the recreation's list order; the original walks its own list order. Both
  start from the unit's authored `abilList`, so the first match agrees unless a unit lists two
  alarm-class abilities (not observed).

Open: the effect of the bit on the alarm behaviour (nothing in the recreation generates
alarms), and the default flags for an alarm ability a map adds at runtime (the constructor chain
above was read, the add path was not).
