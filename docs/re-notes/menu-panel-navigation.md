# Source-selected menu panel navigation

The later [campaign scene review](campaign-menu-scene.md) adds the outgoing
Single Player phase and source-backed campaign background/camera ownership.
It also records the source-driven full-screen fade and remaining entrance,
control-fade and rendering gaps.

## Transition review, 2026-10-24

Fresh 1280×960 TFT captures compare the original 1.29.2.9231 executable
(`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`)
under isolated Wine/OpenGL with the current desktop application. Main → Options
and Options → Main expose two confirmed presentation defects:

- **P1: controls are detached from transition presentation.** Original outgoing
  buttons disappear before the hanging housing travels away; incoming buttons
  appear near the end of its entrance. Current outgoing buttons remain stationary
  over the exposed background, and incoming buttons are already visible while
  their housing is still entering. `menu_sprites::advance` commits the next
  control owner at Death → Birth; `build_control_layer` replaces the tree then,
  without a transition visibility/opacity envelope. `position_screen_nodes`
  continues assigning the settled FDF coordinates. Correct source clip selection
  alone therefore does not reproduce menu switching.
- **P1: logo visibility cuts instead of fading.** The original logo fades out
  on entry to Options and fades in on return. `show_logo_for_screen` only selects
  Hidden/Inherited from the currently admitted FDF; the captures show the logo
  retained through the outgoing panel motion and then abruptly removed.

A third **P1 source-wiring gap** remains: `Navigation::open` schedules only
Main ↔ Single Player and Main ↔ Options. Campaign, load, replay, skirmish and
multiplayer navigation still replaces the screen immediately. This is established
by the caller, not a newly captured original transition for every listed route.
Campaign has a distinct authored composition; do not invent a Campaign panel
triplet or reuse MainMenu clips for it.

Local evidence is retained in `.scratch/menu-motion-review/`: original and native
`options-forward-fast`/`forward-fast` and `options-back-fast`/`back-fast` frame
series, contact sheets and monotonic input/capture intervals. Early PNG sampling
was too slow for timing comparison; the later PPM series supplies the motion
review. Sampling intervals differ and software rendering affects throughput, so
this review establishes ordering/visibility defects, not an exact duration or
easing measurement. A first default-renderer original attempt produced a black
capture and exited; it is retained separately and is not comparison evidence.

No runtime behavior changed in this review. The next correction must synchronize
control and logo presentation with outgoing/incoming phases, derive any timing
from original inputs, and validate fresh rendered transitions. Browser/audio,
the full-screen opening, profile alternates and campaign transition choreography
were not requalified here.

### First presentation correction

The shared control root now hides through the source Birth/Death phases and
reveals on Stand. The next FDF can still be admitted and mounted during Birth,
but its buttons no longer float over the background while the housing travels.
Category morphs retain their right-hand controls. Empty/unavailable panel plans
and screens outside this scheduler keep their previous navigation capability.
This is a coarse phase-boundary correction: native control opacity ramps and
the precise early reveal within the final entrance frames remain open.

Logo playback now selects the model's named Death alpha keys during Main's exit,
and replays Birth on return. Both directions use the existing panel clock;
independent entrance playback uses real presentation time. Sequence timestamps,
duration and Bézier values remain model-derived. Reversed sequence intervals
refuse sampling, and unavailable exit curves are reported rather than replaced.

Synthetic checks discriminate nonstock sequence start/duration, exit versus
entrance keys, missing/non-finite/reversed input and the control visibility
boundary. The actual selected logo's Birth/Death endpoints pass the explicit
local-input gate. The final app suite passes 55 library and 640 binary tests;
strict native and WASM Clippy pass. These checks establish implementation and
source handling, not full transition parity. Evidence is retained in
`.scratch/menu-motion-review/checks-final/`. The earlier desktop build was
intentionally interrupted to add the malformed-duration guard; its receipt is
preserved separately in `checks/`.

One scoped REA callback-decompilation attempt hit the provider's Ghidra startup
deadline before producing a function result; the session was closed. No native
fade timing was inferred from it. Full-screen opening, other screen routes,
profile alternates, category body fades and audible parity remain unfinished.

The final desktop binary builds successfully with incremental compilation off.
The first link attempt failed with undefined hidden LLVM symbols in incremental
objects; its log is retained, and the fresh-object build passes without source
changes. The original edition-switch and destination-control gates each pass
one explicitly selected test; preliminary zero-match invocations are retained
and are not counted as verification.

A fresh 1280×960 TFT desktop run confirms Main → Options → Main no longer leaves
buttons floating while the housings travel, and the logo fades out and back in.
The same run reaches Single Player and the campaign selector. This is selector
reachability, not chapter activation. The campaign capture still shows the
Single Player housing/background retained behind campaign controls: campaign
composition and its transition remain a visible frontier. Raw frames, input
times, contact sheets and build/source hashes stay local under
`.scratch/menu-motion-review/native-final/` and `checks-final/build.json`.

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

## Options owner

Options navigation now independently admits the exact two-token source selectors
`Options Birth`, `Options Stand` and `Options Death`, together with the MainMenu
triplet. Main/Single Player admission remains separate: missing or malformed
Options sequences, sound dependencies or material plans do not disable that
route, and missing Single Player does not disable an otherwise admitted Options
route. Both route candidates check their full source geometry/material plans
before publication. Known unsupported audio codecs retain the same explicit
capability state described above. Event occurrences remain scoped to the
admitted source phases, without replaying shared MainMenu sounds twice.

Main to Options and back use the existing retiring Death, incoming Birth, then
Stand scheduling convention. While that clock is on Options Stand, a Gameplay,
Video, or Sound page plays source `Options Morph` on the skin's left sprite
layer and then holds `Options Stand Alternate`. The right sprite keeps its
Options Stand clock, including its authored looping behavior. Its skin key is
retained through loading and spawning; neither filenames nor geoset numbers
choose that role. Only the left layer controls the category transition duration.
Cancel from that hold plays `Options Morph Alternate` on the left, then Options
Death and Main Birth on both. The blank category page stays on Options Stand. Clip
intervals, loop flags, poses, UV/texture selection, opacity and sound frames
remain source-derived. This dispatch order is a recreation convention, not
proof of the original native transition caller. No screenshot-fitted panel
placement is introduced. Missing optional animation retains the admitted FDF
navigation with an explicit unavailable diagnostic.

The first category capture revealed the left plate but incorrectly hid the
right housing and chains. That capture was rejected despite passing navigation
and text-alignment assertions. The selected right models have visible Options
Stand geometry that their category sequences hide; the original runtime witness
retains both housings. This supports the per-layer recreation dispatch above,
without establishing the native dispatcher's internal architecture. The right
layer's unused category dependencies cannot disable the left transition.

The corrected per-layer controller passes 357 application tests, the explicitly
selected original panel gate, strict native/browser Clippy, the browser build
and formatting. Ten actual source-clock tests execute on native and Wasmtime;
this is not a full Bevy WASI build. The generated browser WASM passes 128
synthetic hydration assertions, including category dependency failures that
retain Options controls and geometry.

A fresh actual-app import of the local 152-member source closure shows the left
plate and both right housings/chains together. The resolution value is vertically
centered, and Cancel completes on Main. The capture reports no state change
during collection, no external requests, uploads, sockets or page errors. Original
packed category waveforms remain on the explicit unsupported-codec path; no
audible parity is claimed. Open-list artwork, lighting, typography, unsupported
settings and complete native fidelity remain open. Root receipts and reviewed
captures are in `.scratch/options-category-root-20261017/layer-policy-1/`.
The source closure remains private; the code-only package was not published.

The requested Options tab is retained until target FDF admission and incoming
control-owner commit. Tabs within Options preserve the draft and start only
those category plate sequences. Cancel restores applied settings before
requesting Main; Apply retains existing settings validation/storage behavior
before its request.
Further actions remain blocked until the incoming phase settles.

Options integration checks pass four library tests, 257 app tests (11 explicit
local-input ignores), two CLI tests, 74 map-loading tests and seven PCM tests.
Native/WASM builds, all-target/all-feature Clippy, rustdoc and formatting pass.
Both explicit original source gates now preflight Options as well as Main/Single;
the compatibility playable-PCM gate remains green. Actual release WASM passes
103 panel checks plus 49 existing Chromium transaction checks, including
independent absent/malformed owner cases, base-only gating, rollback and audio
cleanup. Existing Node unit tests pass 56 with two explicit local-input skips.

The separate original-fixture browser run passes six reviewed Main/Video Options/
Main/Single Player/Main captures. Both incoming and return transitions settle;
Options shows the source three-button housing instead of the Main five-button
housing. Fifteen local code GETs, zero uploads/sockets/page errors. The unchanged
150-member local repack already contains the needed source sound dependencies.
This is not a retail-installer, native-audio or native-visual parity claim.
Receipts and the preserved earlier failures are in
`.scratch/options-panel-root-20261016/acceptance-1.json`. The public demo is not
updated by this batch. Unsupported settings and washed-out sky remain open.

A native-only regression now drives production `advance` with synthetic clip and
sound records, then reads spawned audio players and authored gains. It verifies
left category events, continued right Stand without a restarted opening event,
and the Options Death slice on both layers after the real return request. The
regression and 363 application tests pass (18 explicit gates ignored), with
strict native/browser Clippy and formatting. Evidence:
`.scratch/category-pop-advance-20261017/root-verification/`. This adds no runtime
behavior and proves neither audible parity nor full Bevy WASI support.


## Panel presentation clock (2026-10-18)

The panel system uses `Time<Real>` instead of Bevy's generic, capped virtual
clock. The five-resolution diagnostic exposed the consequence at 4K: a Video
shortcut reached an Options transition that had advanced too little despite
several seconds of elapsed time. A deterministic regression reproduces the
loss with manually supplied time: one 600 ms update previously advanced the
panel by only the framework's 250 ms cap, producing fewer authored sound events
than smaller updates covering the same interval. Those numbers describe the
synthetic experiment and framework policy, not copied game durations.

The system now advances existing panels after asset-store invalidation and
before navigation actions create a new phase. A new request does not consume the
preceding frame's delta. Source invalidation still discards the old scene before
its clock or sound events run. Source clip durations, opacity curves, sound
ordering, dispatch budgets and busy-transition input refusal are unchanged.

The production-system regression compares phase, local/global elapsed time and
ordered sound gains across different frame partitions and virtual-clock pause.
The full application suite passes 645 tests, with 23 explicit fixture ignores;
strict native/all-target and WASM library/binary Clippy checks pass. Actual 4K
rendered validation is retained separately under
`.scratch/menu-panel-clock-root-20261018/`.

This is a presentation-clock implementation choice addressing observed frame
loss, not proof of the original native clock API or input timestamps. Bevy's
virtual clock, simulation scheduling, background/particle clocks, RNG and
material blending are not changed. Pixel parity and whole-application
load-independent playback remain open.


### Actual 4K validation

The frozen `c90730a` source builds both native and WASM applications. Separate
3840×2160 runs receive the same eleven-event journal as the earlier diagnostic.
All four checkpoints now show the requested Main, Options landing, Video body
and return Main states. Each WASM after-capture snapshot has the expected screen
and `transitionActive: false`; raw images were also visually reviewed. The
original reference images are retained from the prior verified 4K run, with no
new original process launched. Twelve exact RGB comparisons still differ.

Evidence: `.scratch/menu-panel-clock-root-20261018/verification-final-1.json`;
local three-column report: `comparison-1/index.html`. Both app runtimes are
stopped. No source alpha values were changed, no version mask applied, and the
public site was not rebuilt or deployed.

The first native check also exposed a harness-only command-filter omission:
its pointer-position query was refused before journal delivery. That attempt
is retained and stopped. The corrected filter admits the bounded read, rejects
clamped positions, and clears a stale remembered point on failed movement.
Forty-seven Python harness tests and four Node checks pass; the successful native
rerun exercises the real filter, not a mock of it.
