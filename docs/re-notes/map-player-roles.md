# Authored map player roles

The first four words of a `war3map.w3i` player record are player number,
controller kind, race and fixed-start flag. They are independent fields. The
record ordinal is not the player number, and the fixed-start flag is not a
start-location index or a colour.

`pcx_map::info::Player` retains its existing public fields and the complete input
bytes for compatibility. Their historical names are misleading. New consumers
use `number()`, `control()`, `race_id()` and `fixed_start()`. Unknown flag words
remain in the record and return `None` from the boolean accessor. The human-slot
iterator filters controller kinds; race counts use the race word. Neither query
establishes lobby occupancy or the local client's identity.

The supplied Human01 member was compared with the supplied map script parsed by
`pcx-script`. All five records agree with `SetPlayerController`,
`SetPlayerRacePreference`, `ForcePlayerStartLocation` and `DefineStartLocation`.
The two sparse player numbers stay sparse. Start-location indices are joined
through the script calls, and both coordinate words agree bit for bit. Native
and WASI executions of this comparison produce identical output. This is
shipped-source evidence, not a new original-client runtime observation or a
claim that the adjacent archives share one release identity.

Synthetic version-18 and version-25 tests place a human controller in slot 8 and
a neutral controller in slot 1, so confusing a controller with a player number
fails. They also check all four word meanings, unknown flag retention and exact
canonical bytes. Existing framing tests continue to cover truncated records.

Normal campaign Host preparation remains incomplete. These roles must not be
turned into inferred local-client, lobby, difficulty or colour defaults. The
legacy `mapx w3i info` diagnostic still uses historical raw-field labels; its
existing output contract was not changed by this reader correction.
