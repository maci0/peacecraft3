# Script occlusion boundary

The current campaign ground renderer has no gameplay occluder-fade pass. Its
installation publishes the `NoOcclusionPass` capability. Only that capability
allows the startup caller to admit `EnableOcclusion(false)` on the owned Host.
Without the capability, the native remains unsupported. Enabling occlusion
refuses, including when the capability is present, and a refused candidate
preserves the previously published session.

The optional declaration must have the exact boolean/nothing, non-constant
signature and the same source as `CreateTimer`. A missing declaration cannot
be reached through the Host dispatcher. The call accepts only an actual boolean,
allocates no identity and does not change UI controls or logical time. No original
initial option, occluder membership, color or duration is invented.

The capability describes an implementation constraint, not an observed original
option. It must be removed/replaced when the renderer gains an active pass; that
pass must then consume the script switch. External callers of
`ObjectHost::admit_no_occlusion_pass` are responsible for the same qualification.
The current production caller grants it only through the installed renderer's
resource. A renderer which does not publish the resource cannot silently gain
this admission through the generic startup factory.

## Source evidence

Bounded static reads use the selected 1.29.2.9231 executable, SHA256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
The functional registration reaches `0x48fcd0`, then the camera/controller setter
`0x60c7d0`, the world wrapper `0x703920` and the world setter `0x73f070`.
The supplied boolean reaches the world object's field at `+8`. The updater
`0x73fcf0` returns without advancing its fade records when that field is clear;
it also checks the separate flag at `+4`. This does not make gameplay occlusion
equivalent to GPU occlusion culling or establish every drawing branch.

The same world's configuration path reads `Occlusion/OccluderColor` and
`Occlusion/OccluderFadeTime` from the selected UI profile. Other bounded reads name
`MarkImage`, `MarkColor` and `BuildingsOcclude`. The supplied
`UI\\MiscData.txt` carries those fields. Active rendering must derive its settings
from those assets rather than copying the observed values into constants.

Private windows, source identities, source-prefix execution and verification
receipts are under `.scratch/campaign-occlusion-root-20261019/`. The broad raw
candidate search includes unrelated list/data accesses; those candidates are not
function boundaries or semantic proof. The focused setter/updater windows and
configuration references support the narrower relations above. No original-game
runtime was launched for this change.

## Validation and open work

Synthetic Host tests exercise explicit admission, false/true arguments, missing
and foreign declarations, qualifier/signature errors, and unchanged identities
and clock. Factory tests exercise the actual renderer capability boundary and
rejected replacement. Ground-renderer tests check that its real installation
publishes the capability. Strict native/WASM builds and actual WASI/Cargo-WASM
execution have separate command/exit receipts.

Active occluder eligibility, live world/model association, source-driven fading,
markers, building rules, removal/restoration and original rendering comparison
remain open. The broader occlusion task remains active. Passing the disabled
branch does not establish playable Human01 or full original/native/WASM fidelity.
