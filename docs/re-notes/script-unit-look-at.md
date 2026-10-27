# Unit look-at uses the admitted transform joints

The unchanged Human02 interlude reaches an authored `SetUnitLookAt` from Medivh
to Antonidas. Its requested joint is `bone_head`; the previous Antonidas-to-Medivh
request already passed. The failing native did not establish a missing model or
an absent authored joint.

Local selected-asset extraction identifies
`units\creeps\Medivh\Medivh.mdx` in the selected Legacy install's `War3.mpq`:
357,005 decoded bytes, SHA-256
`55b41895e84255dbae19bc93ec1e1ebe0a906dbe29addb67c5bf7571373d9c2d`.
The other four installed MPQs and the campaign map do not contain that member. `Bone_Head` occurs once
in HELP, with object ID 67 and parent 66, and is absent from BONE. The authored
script and model identities, complete local node inventory and extraction
readback stay ignored under `.scratch/unit-look-at-helper-20261026/`.

`unit_model::Model::read` already decodes BONE and HELP into one retained joint
list, using the existing helper-to-transform adapter. Look-at admission now checks
case-insensitive name uniqueness in that same list rather than reparsing BONE
alone. Missing and duplicate names, including duplicates across the two classes,
continue to refuse before mutation. The stored request, target lifetime cleanup,
reset behavior, offsets and animation state are unchanged.

This is connected to the existing visible pose path: `JointPlan` also combines
BONE and HELP, the campaign actor passes those combined joints to skinning, and
`mdx_look_at::apply` corrects the selected joint and its descendants. No new parser,
node alias, replacement joint or separate skeleton store is introduced.

Synthetic tests cover a helper-only target, duplicate HELP names, cross-class
ambiguity, a missing name, atomic refusal, reset and unchanged model/handle state.
The explicit local-original gate derives its requested name from the retained
interlude script and checks it against HELP and the admitted combined model.
These tests establish admission and the existing renderer connection, not an
original pixel or pose comparison. Native turn limits, turn blending and exact
rendered pose parity remain open in the existing look-at implementation.
