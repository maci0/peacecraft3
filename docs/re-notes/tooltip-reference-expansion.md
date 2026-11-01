# Tooltip `<row,field,modifier>` value formatting

## Shipped-byte evidence

Across all 282 game `.txt`/`.fdf` files, exactly two three-part tooltip
modifiers occur: `%` (351 uses, e.g. `<AIsp,DataA1,%>`) and `.` (6 uses,
all Devotion Aura `AHad`, e.g. `<AHad,DataA1,.>` with authored values
`1.5` / `3` / `4.5`). Every other reference is a plain two-part
`<row,field>`. The `AHad` text lives in `Units\HumanAbilityStrings.txt`;
no dataset overrides it (`Custom_V0` carries the same `AbilityData.slk`
values).

## Native-code evidence

The template expander in the reviewed client image
(`Warcraft III.exe` SHA256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`)
spans roughly `0x5cef30` with a five-state jump table at `0x5cf204`
(`0x5cefc0`, `0x5cf0e5`, `0x5cf016`, `0x5cf058`, `0x5cf14b` in order).

The modifier dispatch at `0x5cf0e5` reads one character after the second
comma and skips two bytes:

- `0x25` (`%`): `mulss 100.0f` (constant bytes `00 00 c8 42` at `0xe69598`),
  then state 4.
- `0x2b` (`+`): `cvtss2sd`, clear-sign-bit mask (bytes `ff ff ff 7f …` at
  `0xe78370`), `cvtpd2ps`, then state 4.
- `0x2e` (`.`): sets the per-template flag at `ebp-0x94` to 1, then state 4.
- Anything else: straight to state 4 with the flag untouched.

State 4 (`0x5cf14b`) tests the flag first: clear always takes the integer
path (`cvttss2si` truncate toward zero, printed with `%d` from `0xe6ddd8`).
Set, it truncates, converts back, and compares with `ucomiss`: exact
equality (ordered) prints `%d`; anything else prints `%.1f` (format at
`0xe91fac`). The flag is initialized once per template expansion and has
no other writer in `0x5cef30..0x5cf210`, so a `.` selects float-aware
rendering for that token and every later token in the same string; an
unknown single-character modifier behaves like plain truncation.

Consequences the recreation pins: a plain `<row,field>` holding `1.5`
prints `1`, not `1.5`; `<X,DataA1,.>` with `1.5` prints `1.5` and with `3`
prints `3`; `%` scales by exactly-representable `100.0f` and then
truncates (`0.333` becomes `33`).

## Open questions

- Whether the original client is lenient (integer path) or refusing on
  multi-character or unknown modifiers: shipped data never exercises them,
  so the recreation refuses them as unadmitted input.
- The `%`-then-`%.1f` combination (sticky `.` before `%` with a fractional
  scaled value) follows the same code path but has no shipped example.
- `%.1f` rounding is assumed to be the C library's round-half-even on the
  `cvtss2sd`-widened value, matching Rust's `{:.1}` on the same binary
  value; no divergence has been observed on shipped decimals.
