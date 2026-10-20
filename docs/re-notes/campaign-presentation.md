# Executed campaign UI input and cinematic filters

This slice consumes executed script calls on the existing `ObjectHost` and the
published campaign view. It does not supply missing startup inputs, create live
units or make Human01 playable. Immediate interface visibility and user control
now reach the ordinary HUD/input consumer; positive interface fades remain
unsupported.
The FDF declaration catalog in `ingame-cinematics-dialogs.md` remains separate.

## Source evidence

The local legacy executable has SHA256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Private static reads are retained in
`.scratch/campaign-presentation-root-20261019/presentation-entries-0.json` and
`presentation-consumers-4.json`. These are bounded decoded windows, not a new
original-game run or proof that every branch was reached. Candidate linear
scans that stop at embedded data are not evidence of absence.

The functional `EnableUserUI` entry at `0x48fdb0` writes UI object offset `0x1c0`.
The input predicate at `0x5f6cb0` requires both `0x1bc` and `0x1c0`. The separate
`EnableUserControl` and `ShowInterface` entries call different methods. Therefore
this implementation gates activations and picking; it does not hide the HUD or
pause the simulation merely because `EnableUserUI(false)` ran. Unknown initial
UI state remains absent until an executed call supplies it.

Start/end color wrappers at `0x4a7640` and `0x4a74d0` read each integer's low byte.
The UV wrappers at `0x4a7740` and `0x4a75d0` sort each pair of axis endpoints.
The duration wrapper stores the supplied float. Texture, blend, mapping and
Display entries configure/show the same filter. The source common declarations
and `BLEND_MODE_BLEND` / `TEXMAP_FLAG_NONE` initializers identify the supported
symbolic modes; their numeric enumerands are not copied into the implementation.

## Recreation behavior and boundaries

The startup caller admits ten declarations with checked signatures and common
source ownership. Fields stay absent until supplied. Display requires a complete
explicit filter. Invalid calls and unsupported modes refuse without replacing the
previous displayed filter. Color bytes wrap, UVs sort, and RGBA interpolation is
component-wise. Zero duration selects the end fields immediately. Positive
intervals use the admitted timer clock; update chunking and wall-clock load do not
select the sample. This timing policy is a recreation choice, not a claim about
the original renderer's clock. Negative/non-finite duration and non-finite UVs
are explicit unsupported inputs.

The renderer resolves the executed path through the reader bound to the
published session's receipts. It reuses the existing bounded BLP/TGA image decoder.
Read/decode ceilings are application allocation policies, not original game caps.
A failed replacement texture preserves the previous successful display and
reports the source error. Successful session replacement clears old image and
material assets; leaving the campaign deactivates its UI camera. HUD roots target
that camera, while a transparent picking barrier and activation gate consume the
UI flag. The selected `GlobalStrings.fdf` `KEY_MENU` suspends the view; Escape fires
`EVENT_PLAYER_END_CINEMATIC` for the local player.

Rendering uses the existing 3D stack's linear compositing space, with an explicit
transfer from the filter's numeric source RGB after texture/tint multiplication.
Alpha stays linear and is applied once by the blend stage. Linear/clamp sampling
is a reconstruction policy. Original sampling, gamma/blend behavior and visual
parity at all resolutions remain open; source parsing and synthetic GPU readback
cannot close those gates.

## Native real argument boundary

The unchanged Human01 probe initially stopped at `SetCineFilterStartUV`: its
helper supplies integer literals to four declared real parameters. Fixing that
inside the filter alone would leave the same defect at other natives. The VM
now converts integer arguments at a native's real parameter before suspension,
callback dispatch or host execution. It shares the `I2R` conversion, including
`RealOps::integer` when a selected policy is installed. Already-real arguments
and integer parameters stay unchanged. Invalid signatures/arity still fail
before dispatch. This is a recreation of the declared native ABI, not a new
observation of the original compiler's conversion opcode.

Four synthetic VM tests distinguish selected conversion from the unbound cast,
check wide integers and conversion failure, and exercise both suspension and
same-VM callback dispatch. This narrow correction does not complete real
storage, user-function parameter/return or native-return coercion. The retained
older broad coercion candidate predates the current callback implementation;
its replacement of the VM was not applied.

## Selected assets, validation and remaining frontier

The retained Human01 script asks for a TGA mask while the supplied legacy archive
contains its same-stem BLP. The consumer tries the exact path first, then the BLP
format alias only after `MissingAsset` across the mounted source set. A listed
but unreadable asset, a budget error or malformed exact image does not trigger
that fallback. The selected path and byte provenance remain in the receipt.
Native priority when both formats exist is not established by this policy.

The unchanged Human01 factory probe passes all ten calls and retains the actual
filter snapshot. The selected BLP passes the production bounded decoder. Without
an explicit local client selection, the next refusal is `GetLocalPlayer`.
A separate private client selects the single supplied user-controller row,
without a numeric slot default; that run reaches `IsPlayerInForce`. This is an
explicit recreation client selection, not a newly measured original local-player
identity. Neither run is a normal application bootstrap or a playable chapter.

Validation covers the full native script suite, play library/integration suites,
application library/binary, map-loading and published-audio suites; strict native
and WASM lint/builds; nine targeted actual WASI cases; and 26 actual Cargo WASM
cases executed in Node. The Vulkan readback checks source-channel transfer and
single alpha application at corners/center in two small targets and
3840 × 2160. This is synthetic artwork over a 3D view, not an original-game 4K
comparison or browser rendering. Evidence and command exits are retained under
`.scratch/campaign-presentation-root-20261019/`.

Automatic Host input preparation, force predicates, later cinematic calls, live
actors and controls, original/native/WASM visual comparison and complete chapter
playability remain open. No original-game runtime was launched for this slice.


## Immediate interface and control follow-up

The optional same-source contracts now include `ShowInterface(boolean, real)`
and `EnableUserControl(boolean)`. Their state starts absent. The startup branch
actually requested by Human01 has zero fade duration: the loaded BJ helper
forces an immediate change while `bj_gameStarted` is false. Nonzero and non-finite
interface durations refuse atomically; they are not silently converted into
immediate changes. The native's positive-duration frame animation remains a
required follow-up.

Additional bounded static reads of the same selected executable are private in
`.scratch/campaign-interface-root-20261019/`. `ShowInterface` reaches `0x5f3760`.
Its show branch writes `1` to the control flag at UI offset `0x1bc`; its hide
branch writes `0`. `EnableUserControl` reaches `0x5ebd50` and writes that same
flag on its respective branches. The separate `EnableUserUI` flag at `0x1c0`
is preserved. Therefore a successful immediate interface change updates both
the requested ordinary-interface visibility and the control flag. A subsequent
control call can change input without showing the HUD, and showing the HUD does
not undo an explicit `EnableUserUI(false)`. Neither operation advances or pauses
the logical game clock.

The ordinary HUD mount marks its viewport root as `CampaignInterface`, separately
from `CampaignHud` camera routing. After all Update-stage mounts and before UI
layout/visibility propagation, the consumer suppresses that root's display and
visibility. Display suppression matters because an explicitly visible Bevy child
can ignore a hidden parent's visibility. Restoring the root uses its saved display
and visibility rather than overwriting source child states, colors, fonts or
geometry. Late-mounted roots observe the current request on their first layout;
session clearing or a replacement with absent interface state restores the root.
Cinematic message/filter roots are not marked as ordinary interface and remain
separate. Existing input/picking consumers use both UI and control flags.

Synthetic Host tests cover the declared-real native argument boundary, ordering,
unknown initial state, source/qualifier checks and atomic refusal. Published
session tests cover actual HUD/input changes, continuing logical time, late roots,
source-hidden children, session replacement and rejected candidates. A targeted
Vulkan regression hides and restores an explicitly visible child at small and
3840 × 2160 targets while leaving a separate cinematic layer present.
Native/WASM strict builds and actual WASI/Cargo-WASM execution have separate
command receipts in the private evidence directory.

This is ordinary-HUD/control application, not complete native cinematic UI.
The native also switches other interface surfaces; cinematic panels, portrait/
transmission content, positive-duration interface animation, native viewport
changes and original rendered comparison remain open. The unchanged private
Human01 prefix now passes both immediate calls and reaches `EnableOcclusion`.
Normal startup input preparation and playable actors/controls remain unfinished.
