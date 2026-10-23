# Campaign continuation

Implementation draft; synthetic application checks pass. Selected-install
continuation admits Human02, whose startup still refuses on `SetResourceAmount`.

## Selected-script evidence

The supplied Legacy `common.j` declares `ChangeLevel(string newLevel, boolean
doScoreScreen)`. In the selected `Blizzard.j`, `SetNextLevelBJ` only assigns
`bj_changeLevelMapName`. `CustomVictoryOkBJ` and `CustomVictorySkipBJ` call
`ChangeLevel` when that global is non-null, or `EndGame` otherwise.
`CustomVictoryQuitBJ` always calls `EndGame`, even when a next map is stored.
Human01's victory action supplies the next member through `SetNextLevelBJ`.

These are shipped-script observations for executable version 1.29.2.9231
(SHA-256 `3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`).
They do not establish the native score screen's internal transition timing.

## Recreation boundary

The destination belongs to each executed `ChangeLevel` request. An ordinary
`EndGame` has no destination; ambient next-level state cannot turn Quit into
Continue. Requests that disagree about their destination stop with an explicit
runtime error rather than selecting one arbitrarily. The existing any-true score
flag policy remains an application choice for repeated requests to one target.

With scores requested, the exiting session's context and destination remain on
the score state. Its existing OK control requests continuation. The state is
cleared only after the admitted next session starts successfully. Without scores, the exit consumer requests
continuation immediately. This scheduling is a recreation choice, not a native
runtime observation.

The target member is resolved through the exact resolver retained by the exiting
binding. Source lookup uses the member inventory without reading every member or
reading the map twice. Rebinding reuses the selected scripts, catalog sources,
required tables, field requirements and input ceilings. Map overrides and the
catalog are rebuilt from the next map. Binding revision increases with checked
arithmetic. The normal configuration admission and startup path publishes the
new session; no second startup implementation is introduced.

An old-context request, missing reader, missing or malformed map, or exhausted
revision refuses. Admission failures preserve the previous bindings, context,
receipts, reader and score request, and appear in campaign diagnostics. Startup
after successful admission has its existing independent error reporting.

Preparation reads the install origin from the retained common-script receipt and
the expansion mode from the retained resolver. Changing launcher controls cannot
substitute a different install or edition for continuation. Native executable
reads remain bounded and pass the existing reviewed-image admission.

An admitted next map whose startup refuses keeps the previous score presentation.
A later Continue press rebinds the same target with a new checked revision and
retries preparation/startup. The old context is accepted for this retry only
while the pending transition still names the current admission. A completed
transition cannot be retried by a stale score request.

## Verification and limits

Synthetic tests exercise dialog-dispatched changes, successful next-script
execution, no-score continuation, conflicting requests, failed admission and old
context refusal. The selected-install test invokes the selected BJ victory
functions explicitly; it is a transition stimulus, not evidence that a player
completed Human01.

The private candidate passed 1,190 core tests and 839 application tests (55
library, 603 binary and 181 map-loading), with 35 core and 60 application tests
ignored. Strict Clippy, formatting and the WASM compile check passed. The frozen
inputs are recorded locally in `app-inputs-7.sha256`; the subsequent core handoff
changes only continuation wording and a test name.

The later source-pinning and startup-retry candidate, frozen in
`app-inputs-9.sha256`, passed 843 application tests (55 library, 605 binary and
183 map-loading), with 61 ignored. Strict Clippy, formatting and WASM compilation
passed. Its six continuation cases run in both the binary and map-loading suites;
they cover retry after startup refusal and retirement when another map starts.
An explicit selected-install test also passed after replacing the launcher with
a nonexistent install and the opposite edition: preparation retained the
admitted executable origin and the resolver's original version constant.

This executable lookup supports the native campaign menu's retained
`InstallSource`, whose origin is the installation directory. A generic
`MpqSource` origin is an archive filename, not an executable location; such a
configuration cannot use this native startup producer and refuses during image
loading. No executable is inferred from the current launcher as a fallback.
Supporting other native providers needs an explicit retained executable input.

The explicit selected-BJ victory stimulus activates the score OK control and
admits `Maps\\Campaign\\Human02.w3m`. Its startup then refuses at
`war3map.j:911` on the unsupported `SetResourceAmount` native. The selected gate
therefore fails; a running Human02 session and mission completion are not proven.

Score rows, replay recording, restart/load, campaign unlock persistence and
original-client visual or transition timing parity remain separate gaps. A
source-backed score layout is still required to render its OK control; a missing
layout is reported rather than replaced with invented UI.

## Score control ownership

The score FDF, rather than the underlying menu FDF, owns control positions.
Buttons use their existing action consumer; the normal menu navigation consumer
cannot interpret a score press. Button labels read their style, skin font binding
and font bytes through the exiting context's selected resolver and reuse the
dialog rich-text renderer. They retain the shared button's pushed-text behavior.
The replay-saved control remains hidden because no production recording can
produce that state.

An already-attempted score asset load returns before mutably accessing the cache.
This keeps an idle frame from marking it changed and rebuilding the score root,
which previously discarded button presses. A lifecycle test retains the root
across an idle update. Fonts are loaded once per score presentation from its
resolver and released with the score root; unrelated font assets survive.

Standalone labels whose geometry still requires text measurement are named in
diagnostics and omitted individually. They cannot prevent the valid OK control
from spawning. This is partial score presentation, not proof of native score
layout fidelity; elapsed-time labels and score rows still need their measured
layout and qualified values.
