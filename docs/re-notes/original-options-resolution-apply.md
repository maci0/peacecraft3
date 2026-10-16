# Portable resolution Apply

The current Resolution control changes the window/browser render extent. It
does not choose an exclusive physical display mode, bit depth or refresh.
No original resolution presets or preference values were copied into this
implementation.

The selector may list source-qualified frameless rows for current monitors, or
primary-monitor enumeration rows, from an admitted client profile. Apply still
changes only the client render extent.
See [the display contract](original-display-modes.md).

Previously Apply saved preferences before Winit processed the resize, then
reported the requested dimensions as applied. The regression fails against that
implementation. The replacement validates and freezes one candidate, records the
selected source/revision and actual Winit window identity, and submits its size
once. Applied settings, active audio and persisted preferences remain unchanged
while that request is pending. Existing version-1 render-size preferences remain
readable; startup restoration uses the same reported-size check without rewriting
the saved intent.

A main-thread First system follows Winit's previous Last submission. It reads
the matching Winit window's client extent instead of trusting the mutable Bevy
Window or this app's synthesized WindowResized events. A reported match permits
saving and publication, followed by the normal Main-menu transition. Unchanged
sizes and audio-only changes retain the direct validated path. Source/window
replacement and changed drafts cannot publish a stale candidate.

The current reported client size need not occur in the source-qualified choice
list. Keeping that size allows an audio-only Apply after startup fitting or an
external resize. Apply refreshes the actual Winit report before validating this
case; a new unavailable size and an explicitly selected row that disappeared
still refuse. This does not add the current size to the original-derived list.

If Winit recreates the native window for the same Bevy entity, abandoning a
pending request restores that component's prior size and canvas-sizing flag.
A different replacement entity keeps its own settings. Both cases discard the
old size observation so the next host report refreshes applied state; abandoning
the request does not save preferences or claim that restoration was confirmed.

Cancel, unavailable confirmation, a bounded operation deadline or save failure
request restoration. Success is reported only when the previous client extent
is reported. For an initially parent-sized browser canvas, the existing JS owner
resumes percentage sizing after that restoration was submitted; restoring the
Bevy flag alone would leave Winit's pixel CSS in place. A failure to resume that
policy is reported separately. Late Cancel during restoration retains normal
Cancel navigation. Without Cancel, save failure preserves the dirty candidate for retry. A failed
restore reports an available observed extent or its absence; it does not rewrite
preferences or claim rollback succeeded. The watchdog is project operation
policy, not a recovered original duration or a performance gate.

Filesystem/localStorage, a window system and audio are not one atomic resource.
Browser audio can still fail after storage succeeds. That branch keeps committed
settings, reports the post-save audio failure and stays in Options; it does not
enter the pre-save rollback path or claim nothing changed.

The current integration also passes an actual X11/Winit transaction with no
offered sizes and a cleared cached observation. Audio-only Apply obtains the
native client size directly, saves at that size without requesting a resize,
and leaves the offered list empty. The same run passes save-failure restoration
and Cancel. The stable application suite passes 340 tests. Evidence is retained
under `.scratch/menu-ui-native-root-20261017/verification-5/`; this is narrower
than a rendered Options or exclusive-display acceptance test.

## Observation limits

Winit's reported client size is narrower evidence than compositor acceptance or
a presented frame. In particular, the installed Wayland backend can update its
stateless client-size field synchronously inside request_inner_size, before a
compositor configure. Web similarly reports canvas/backend state. This owner
therefore confirms a reported client/render extent after backend submission,
not a physical mode switch, compositor acknowledgement, depth/refresh change or
native rendering fidelity. Rendered browser gates must remain distinct.

The Linux exclusive API limitation does not prevent a future faithful fullscreen
presentation using a source-sized render target. That path must include the whole
menu, UI picking coordinates and presentation, rather than applying a 3D-only
resolution override and claiming every layer changed.

## Checks

The actual baseline regression saves an unobserved size and fails. Corrected
state tests cover delayed/nonmatching observations, save-once behavior, Cancel,
late Cancel during restore, dirty-draft preservation, source/window replacement,
deadline/restore failure and startup restoration without writes. A further
regression catches the first valid host report leaving applied settings at the
configured startup size. The corrected observation updates that baseline while
preserving a deliberate draft and a pending saved-size restoration.

An explicit Linux real-window gate uses no original assets and an empty private
configuration directory. It exercises the production Winit request/report path,
real preference persistence, a directory collision causing atomic-rename failure,
reported restoration with old preferences/audio, and Cancel. It passes on the
current Wayland session. Earlier minimal test-plugin setup failures remain failed
receipts; adding the required asset/image/accessibility/focus plugins fixes the
harness rather than changing production behavior. This gate does not draw a game
frame or establish compositor presentation.

The complete native suite passes four library tests, 270 app tests with 13
explicit local-input ignores, and 83 integration tests. Full native/WASM builds,
strict all-target/all-feature Clippy and formatting pass. The explicit real-window
gate also passes against this final source.

Evidence is retained in `.scratch/options-portable-resolution-root-20261016/`.
The first actual browser run fails before Apply: canvas/client extent already
changed while applied state retained the startup seed. A diagnostic rerun bypasses
only that initial assertion and completes the three save witnesses and four
captures. It is not final acceptance. Those captures also expose full exception
stacks obscuring Options. Browser preference/audio failures now retain their
details in the console and present bounded first-line text in the menu. The
complete browser gate now passes against the corrected release. A build or
synthetic state test alone does not establish rendered acceptance. The browser gate
also tests a failed first save before any fixed size is committed, then changes
the synthetic browser host and requires canvas/applied state to follow the parent
while preserving the dirty candidate and absent preferences. Against the
pre-hook release, this extended gate fails on the retained pixel CSS after a
reported successful size restoration. Eight focused JS checks cover the existing
preview behavior, explicit percentage restoration and unavailable-canvas refusal.

The final release-WASM browser run passes all four save witnesses and five
reviewed captures. Each save observes the candidate canvas bitmap and physical
client extent before storage. Failed initial and later saves retain absent/old
preferences and active audio, with retryable drafts. Initial percentage sizing
survives a subsequent container resize. Post-save audio refusal remains explicitly
committed; Cancel retains the committed state. All 15 browser requests are local
code-only GETs, with zero uploads, sockets, blocked requests or page errors. The
selected original fixture is a local repack imported through File input, not a
retail-disc or patched-installer acceptance test. Original assets and captures
remain local. Physical display modes, compositor presentation, audible output,
reload and complete original Options fidelity remain separate checks.
