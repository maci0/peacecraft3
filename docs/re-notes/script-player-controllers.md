# Explicit current player controllers

`ObjectHost::admit_startup_controllers` accepts caller-qualified
`(source slot number, mapcontrol)` entries for the exact admitted player-context
Arc. It supplies a bounded current-controller view separate from executed
`SetPlayerController` assignments. It can describe a source slot before its lazy
`Player` identity exists, without creating that identity. A later real player
lookup uses its admitted number; an ordinary reserved object cannot substitute
for a player row.

Without opt-in, the existing getter still returns executed setup and refuses
unset assignments. With opt-in, `GetPlayerController` returns the explicit current
input instead. `PlayerSettings::controller` remains the executed assignment,
including unset. A missing current entry refuses; an explicitly empty snapshot
supplies no default and does not fall back to setup. Invalidation retires the
current input without changing that getter policy. Re-admission recovers it.
Accepted controller writes retain their executed argument and conservatively
invalidate the current input, force participation and slot-state snapshot;
rejected writes preserve all three. This is a recreation freshness contract,
not proof of native runtime controller/occupancy transitions. External changes
require caller refresh/invalidation; no live provider is installed.

Admission requires the loaded getter, setter and explicitly selected
`ConvertMapControl` to share the admitted Player/CreateTimer source, with exact
signatures, constant flags, unambiguous declarations and no function overrides.
It walks source-qualified loaded `mapcontrol` ancestry to `handle`. Control
identities must be actual symbolic entries inside the selected conversion's
domain. Same-family values outside that domain refuse. Tokens are session-local;
context identity does not authenticate runtime provenance.

The admitted source slot count bounds both entry count and signed slot numbers.
Duplicate numbers refuse. Validation precedes fallible copying, and replacement
occurs only after the copy succeeds; failure preserves the previous input.
The snapshot uses at most one retained pair per source slot. It allocates no
player identities or rows. Old and candidate copies can coexist during
replacement; this is not a peak-heap bound. Getter queries allocate no handles.

## Qualified Human01 continuation

This slice reuses the original-client startup census qualified in
[startup settings](script-startup-settings.md). No new game run was needed. That
Wine/software-renderer experiment preserves the map's configuration and records
matching source-slot controller/state pairs before and after `InitBlizzard`.
Executable, map, compile-prelude and runtime edition identities remain separate.
Its generated preload text is read as data, never executed. These inputs are
not universal lobby defaults or an eligibility algorithm.

The private unchanged Human01 verifier resolves the census's named controller
globals through its same initialized VM and supplies the complete 16-slot input
after `config`. Admission creates no players. The source subsequently creates
its own later player rows and reads the supplied current controllers. Every
executed controller getter matches the input, while every retained setup
assignment still matches its source setter calls. The single-player counting
loop completes and sets `bj_isSinglePlayer` to true. This is source execution
from qualified inputs, not an independent native observation of that BJ global.

Prior state remains checked: globals and `config` complete, startup setting
globals match, and the same arena retains 868 identities, 155 units, seven filter
wrappers, 17 forces and 17 explicit memberships. The next refusal is
`CreateSoundFromLabel` requesting the Rescue label, in selected `Blizzard.j` at
`438917..438982`, declared in common at `155708..155881`. This is an unsupported
constructor path, not a failed waveform decode or completed sound initialization.
The label bank and constructor consumer remain separate concerns.

Three synthetic owned-VM/Host tests cover later player creation, aliases,
authoritative current values versus executed setup, omitted/empty/invalidated
input, recovery, malformed queries, invalid writes, invalid slot numbers and
identities, duplicate/over-budget input, selected conversion/source/type checks
and real shared-arena exhaustion. Sparse synthetic controls are fixture data,
not original values. Native and actual WASI execute the same tests. Commands,
source-stability inventories, verifier/include hashes and unchanged-map receipts
remain local under `.scratch/player-controllers-root-20261017/`.

Application Host construction, current-player/lobby providers, dynamic changes,
completed chapter initialization, sound playback and rendered gameplay remain
open. No original assets or generated payloads are committed or uploaded.
