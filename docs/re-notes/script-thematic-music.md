# Thematic music starts and seeks

The selected client is Legacy `1.29.2.9231`, executable SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
These are native-code and shipped-script observations, not original-client
audibility or sample-position comparisons.

## Source joins

The registered `PlayThematicMusic` wrapper at `0x4a4200` passes its resolved
string and an image operand to `0x5d9170`. `PlayThematicMusicEx` at `0x4a4220`
passes the string and the caller's integer to the same routine. The routine
retires the previous thematic handle before creating its replacement; it does
not compare paths. A successful nonzero starting offset goes through
`0x5da9f0`, the same routine selected by `SetThematicMusicPlayPosition`.

That setter first checks the thematic handle, then compares the integer against
an image sentinel. Otherwise it forwards the raw integer through `0x7a00e0`
and `0x7c97a0` to the imported Miles stream/sample millisecond-position API.
The selected image supplies a zero starting offset and a minus-one sentinel.
Production reads those operands from checked instruction windows; it does not
copy them into rule constants. Other negative integers reach Miles as raw
32-bit inputs in the original. Their decoder behavior remains unqualified, so
the recreation refuses them for an existing request. Seeking with no request
does nothing after rules admission. The sentinel does not alter the request.

`EndThematicMusic` reaches `0x5d6b60`, retires and clears the thematic handle,
and calls the regular-music resume path. Regular-music fades and precise
completion/resume scheduling remain outside this change.

The selected `Scripts\Blizzard.j` from `War3x.mpq`, SHA-256
`7ca28f0ce47712a9ab6832c22bb0d0c9d2844b5831fb465b9d792731a3d5243e`,
converts the extended start and seek helpers' seconds through multiplication
and `R2I` before calling the millisecond natives. Frozen Throne `HumanX01.w3x`
from `War3xLocal.mpq` actually starts BloodElfTheme at 9.58 seconds through that
helper. Its map-script SHA-256 is
`28ead18fd2fdc46dcc259de54f0c4d3dc5781af48e37aa5d6361670320cf66ca`.
The local disassembly receipt is `/tmp/pc3-lane07-music/native-evidence.txt`;
no executable, script, or audio bytes are stored in the repository.

## Shared request and application consumer

Map-music state now holds one thematic request for plain and extended starts,
end, and seek. Each start gets a revision even if its path is unchanged. A seek
changes only its position revision. Interleaved calls in a script callback
therefore leave an ordered final request instead of competing plain/Ex getters.
The old unqualified Ex/seek getters describe retained refusals only.

Selected-client rules enter through `campaign_inputs`, `MapStartupInputs` and
assembly. Generic fixtures may omit them: existing plain music remains
available, while extended playback and seeks remain explicit refusals. Rules
must be attached before playback; conflicting declaration sources refuse.
Invalid paths, unsupported offsets and revision exhaustion leave the accepted
request unchanged. Path copying is bounded and fallible.

The app observes start revisions to restart a repeated path, carries the
requested offset into `PlaybackSettings`, and applies later seeks to either a
pending playback component or its existing `AudioSink`. Seek failures are
reported instead of restarting at an invented position. Session retirement
discards the revisions together with the existing music state and entities.
Coalescing calls between app updates is a presentation choice, not proof of
native subframe scheduling or decoder sample alignment.

## Volume and remaining scope

Native `SetMusicVolume` at `0x5da420` targets the separate regular-music handle.
It saves an unsigned minimum of its argument and an image maximum for later
regular starts, while forwarding the original argument to a current regular
handle. The thematic constructor instead supplies its own full volume. Applying
the script regular-music volume to every `CampaignMusic` entity would change
thematic behavior. This change preserves the existing user music bus and does
not admit regular volume or seek, ordinary play/stop/resume, or fades.

Music-disabled call admission, negative Miles positions other than the
sentinel, decoder seek errors/beyond-end behavior, exact fade curves and
original-client playback timing remain fidelity gaps. Neither successful JASS
dispatch nor a null audio sink establishes original-client audible parity.

## Checks

Synthetic checks cover nonstock image operands, malformed windows/call joins,
foreign declaration sources, public VM interleaving, identical-path restarts,
no-request/sentinel seeks and atomic rejection. The explicit local-source helper
test executes the unchanged selected Blizzard helper bodies with the selected
common declarations and color initializers; unrelated prelude startup is
excluded. It is not an unchanged-map startup test.

Focused app checks cover restart/completion state and, with local Human01
inputs, executed host calls reaching playback components, pending-sink seeks,
retirement and a real seek on a muted null sink. Their receipts must be checked
separately from the core policy/parser gates.

The final isolated core run is
`/tmp/pc3-lane07-music-validation/4/results.json`: all eight explicitly selected
thematic tests passed, including the local executable and helper-body checks.
The music suites passed 15 and 9 tests respectively, each retaining one explicit
local-input ignore. Native and WASM all-target Clippy with warnings denied and
both `pcx-play` library builds passed. These are core checks; application and
original-client observations are separate evidence classes.

Application checks are **pending** at this commit. The app state regression,
Human01 entity lifecycle probe and null-sink seek probe have not yet produced
passing receipts. Core checks do not establish an app sink or campaign horizon
pass; feature acceptance awaits those application checks.
