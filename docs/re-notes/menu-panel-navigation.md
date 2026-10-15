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
instead emits PANEL_STARTUP_DISABLED and omits that panel model; this preserves
previous static-menu import behavior without guessing codec data. The exception
is limited to the recognized Huffman and ADPCM+Huffman profiles; reserved masks
still refuse. Every other required dependency is checked before fallback is
accepted. Invalid decoded PCM, missing members, ambiguity and source-parser
failures do not take that exception. The undecoded compressed stream's integrity
and PCM contents cannot be certified without its missing codec.
Only editions with an admitted background are considered, so a base-only import
does not require or enable TFT panels.

Scene event voices are cancelled on successful source replacement, edition/reset
and intro entry. Each browser Blob URL is released once even if an ended event
or rejected play promise arrives after cancellation. Failed replacement preserves
the existing scene. Native voices retain the existing scene destruction owner.

Application limits bound model/script/table/sound metadata, event work, decoded
images and unique sound input. They are project policies, not game values or a
whole-heap/GPU bound. The compatible archive read still bounds decoded member
size rather than packed extraction work; cached-origin receipts may be absent.

## Evidence boundary

The actual explicitly selected Legacy destination FDF gate admits MainMenu and
SinglePlayer controls. Full original panel loading/spawning currently stops at
Sound\Interface\RightGlueScreenPopDown.wav: the supported reader lacks the
required local Huffman metadata. The exact Legacy executable's supported table
consumer probe refused; an independently qualified compatibility Storm.dll probe
succeeded, but no metadata was transferred between them. These failures do not
establish a malformed WAV or a missing archive member.

Synthetic controller, parser, material preflight and browser transaction tests
cover source timing, unequal layer durations, event boundaries/budgets, rollback
and cleanup. They do not prove original rendering, audible playback or native
animation dispatch. The full-screen opening gate remains open: owner-specific
panel motion left a central gap. A source FDF separately binds CampaignMenu's
SlidingDoors sprite to Campaign-Fade.mdl. Both qualified variants are stationary
geometry with animated material opacity, so they are fade overlays rather than
a geometric split curtain. Texture appearance and startup use remain unverified;
no campaign binding is silently reused for the main menu.

Current root checks: 219 app tests (nine explicit ignores), strict native/WASM
all-target/all-feature Clippy, native/WASM rustdoc, release WASM and formatting
pass. Chromium executes 49 existing transaction checks and 71 panel checks,
including 12 required rollback cases plus four mixed-codec-failure orderings.
Reserved-mask rollback, base-only gating and actual event cancellation pass;
requests are local GET/Blob only, with zero uploads/sockets/page errors. Node
unit tests pass 52 with one explicit original-input skip and no module warnings.
The fixture preparer's source-phase dependency discovery has two synthetic tests;
a new full original-data running-app import has not been accepted for this batch.
