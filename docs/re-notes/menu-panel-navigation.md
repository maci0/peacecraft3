# Source-selected menu panel navigation

MainMenu and SinglePlayer now share a checked panel controller. Each panel keeps
its own mesh/material handles, global clock and source-selected Birth, Stand and
Death plans. The outgoing group waits for its longest source clip; shorter clips
hold their endpoint. Incoming panels enter Stand independently. Excess frame
time carries across phase boundaries. These scheduling and control-replacement
choices are recreation conventions, not measured native timing.

A navigation request validates the destination FDF/include graph and its control
layer before changing displayed controls. The outgoing owner stays displayed
until the admitted controller reaches the incoming phase. Further navigation is
ignored during an active transition. Source/edition changes cancel pending
navigation and scene audio. Other routes keep immediate navigation.

An unavailable animation cannot lock a valid submenu: after successful target
control admission it opens immediately with a contextual animation warning.
Busy transitions and stale source selections still refuse. No substitute
animation or duration is invented.

## Sound and import admission

Sound event paths and volumes join the selected model's SNDX/KEVT records through
AnimLookups.slk and AnimSounds.slk. The sound-range operand comes from the
selected Blizzard.j SetSoundVolumeBJ/PercentToInt AST. Linear normalized gain is
an implementation convention; native mixing and audible parity remain open.
Original resource bytes and identities remain separate from the checked PCM
decoder view supplied to playback.

The browser transaction admits the script, tables, model, textures and startup
waveforms before replacing its asset store. Missing/malformed required inputs
retain the old store/revision. Navigation-only admission failure retains startup
and emits PANEL_NAVIGATION_DISABLED. A typed unsupported archive sound codec
instead emits PANEL_AUDIO_UNAVAILABLE while retaining the admitted model,
textures, phase plans and all other playable sound dependencies. Desktop loading
uses the same capability separation. Unavailable events retain their original
frame, path and gain requests, grouped by qualified source path, separately
from playable PCM. No placeholder waveform or successful decode is invented.

The exception is limited to the recognized Huffman and ADPCM+Huffman profiles;
reserved masks still refuse. Every other required dependency is checked before
fallback is accepted. Invalid decoded PCM, missing members, ambiguity and
source-parser failures do not take that exception. Browser codec fallback
requires exactly one matching archive: if extraction stops with multiple sources,
their undecoded equality is not established. The undecoded compressed stream's
integrity and PCM contents cannot be certified without its missing codec.
Only editions with an admitted background are considered, so a base-only import
does not require or enable TFT panels.

Scene event voices are cancelled on successful source replacement, edition/reset
and intro entry. Each browser Blob URL is released once even if an ended event
or rejected play promise arrives after cancellation. Failed replacement preserves
the existing scene. Native voices retain the existing scene destruction owner.

Application limits bound model/script/table/sound metadata, event work, decoded
images and unique sound input. They are project policies, not game values or a
whole-heap/GPU bound. Native panel reads check selected packed and decoded
member sizes before retaining the caller's existing extraction/codec behavior.
Browser reads use explicit packed/decoded limits before payload allocation and
intersect each sound allowance with the remaining aggregate input budget.
This does not bound archive-open tables, decoder RSS or backend PCM allocation;
cached-origin receipts may be absent.

## Evidence boundary

The actual explicitly selected Legacy destination FDF gate admits MainMenu and
SinglePlayer controls. Earlier full original panel loading/spawning stopped at
Sound\Interface\RightGlueScreenPopDown.wav because unavailable audio aborted
the whole geometry batch. The supported reader still lacks the required local
Huffman metadata. A bounded diagnostic now distinguishes zero matches for the
current consumer instruction shape in the exact Legacy executable from an
ambiguous match; it does not establish an alternative table format. An
independently qualified compatibility Storm.dll probe succeeded, but no metadata
was transferred between installations. These failures do not establish a
malformed WAV or a missing archive member.

Synthetic controller, parser, material preflight and browser transaction tests
cover source timing, unequal layer durations, event boundaries/budgets, rollback
and cleanup. They do not prove original rendering, audible playback or native
animation dispatch. The full-screen opening gate remains open: owner-specific
panel motion left a central gap. A source FDF separately binds CampaignMenu's
SlidingDoors sprite to Campaign-Fade.mdl. Both qualified variants are stationary
geometry with animated material opacity, so they are fade overlays rather than
a geometric split curtain. Texture appearance and startup use remain unverified;
no campaign binding is silently reused for the main menu.

Earlier root checks before audio/geometry separation: 219 app tests (nine explicit ignores), strict native/WASM
all-target/all-feature Clippy, native/WASM rustdoc, release WASM and formatting
pass. Chromium executes 49 existing transaction checks and 71 panel checks,
including 12 required rollback cases plus four mixed-codec-failure orderings.
Reserved-mask rollback, base-only gating and actual event cancellation pass;
requests are local GET/Blob only, with zero uploads/sockets/page errors. Node
unit tests pass 52 with one explicit original-input skip and no module warnings.
The fixture preparer's source-phase dependency discovery has two synthetic tests;
a new full original-data running-app import has not been accepted for this batch.

## Audio-independent panel acceptance

The current root checks pass four library tests, 250 app tests (11 explicit
local-input ignores), two CLI tests, 74 map-loading tests and seven PCM tests.
Strict native/WASM all-target/all-feature Clippy, both app builds, rustdoc and
formatting pass. Explicit original gates admit panel graphics from the qualified
Legacy source and the separate compatibility source; compatibility playable
pop-down PCM also remains admitted. This does not claim playable Legacy Huffman
audio. The original client was not executed.

The generated release WASM executes 93 panel checks plus 49 existing transaction
checks in Chromium, including 12 required rollback cases, four combined-failure
orderings, base-only gating and audio URL cleanup. A separate original-data
local-repack run passes Main, Video Options, return, Single Player and return
with six reviewed captures. Hanging chains, metal surround and Quit housing now
render; Main/Single Player transitions settle before their destination capture.
All 15 requests are local code GETs, with zero uploads, sockets or page errors.
The fixture's 150 members stay local and unchanged. The 24-file code-only site
package was tested locally; no public deployment is included in this change.

Root receipts are retained in `.scratch/menu-panel-audio-root-20261016/`, and
captures in `.scratch/original-options-browser/menu-panel-audio-root-20261016-run-1/`.
Earlier baseline and initial compile failures remain recorded as failures.
Options-specific panel animation, two unavailable codec-0x81 sounds, washed-out
sky and full original-client visual/audio parity remain open.
