# Executed local text messages

`ObjectHost` optionally admits `DisplayTimedTextToPlayer` and
`ClearTextMessages` from the same declaration source as `CreateTimer` and
`Player`. The production campaign startup attaches this state before script
initialization. Missing declarations remain unsupported; incompatible declarations
refuse the candidate without replacing the published session.

Timed text validates an existing player identity and the explicitly selected
local-client input. A different valid player receives no local message. There is
no inferred player slot, adopted handle, wall clock or default duration. Finite
nonnegative durations use the admitted logical game clock, with visibility over
`[start, deadline)`. Invalid arguments and exhausted caller budgets preserve the
previous messages. Clearing removes the retained messages and advances the same
checked revision. Expired messages release their live-record and aggregate-byte
budget on the next insertion. The application's limits are resource policy,
not recovered original message limits.

The launcher consumes this state on the published campaign UI camera. It reads
`UI\\MiscUI.txt` / `FontHeights/WorldFrameUnitMessage`, then
`UI\\war3skins.txt` / `Default/MessageFont`, and resolves the resulting font through
the reader belonging to the session's binding receipts. Each read is bounded and
its receipt retained. Missing, malformed or unsupported inputs produce an explicit
consumer error. Session replacement removes the old text and font. Clearing or
logical expiration removes the visible text; menu suspension hides it. Text colors
and line breaks use the existing HUD rich-text parser.

## Evidence and limits

Shipped-byte evidence: the selected `MiscUI.txt` describes these heights as frame
units for programmatically created UI, and identifies `WorldFrameUnitMessage` as
trigger-based in-game dialog. The skin table names the font file. These values
are parsed, not copied into constants.

Static native evidence: in the selected 1.29.2.9231 executable, the real
`DisplayTimedTextToPlayer` registration reaches `0x48fbe0`, which resolves the
player, checks `0x4be2a0`, obtains the supplied text and forwards the real x/y and
duration to `0x5f4860` with an all-ones color word. That consumer addresses the UI
message frame at `+0x428`. Its initializer at `0x5e85ac` names
`WorldFrameUnitMessage` and then `MessageFont`. `ClearTextMessages` at `0x48a9b0`
reaches the UI clear routine `0x5eb380`. These are bounded static reads, not a new
original-game run or proof of every branch. Private source identity, windows,
checks and original-map-prefix receipts are under
`.scratch/campaign-message-root-20261019/`.

Reconstruction choices remain visible: text is stacked upward from the latest
executed normalized position within the campaign view. The original positions it
relative to a separate native frame and additional native margins. Exact margins,
box width, wrapping, fade-out, chat-history integration and original rendered
comparison are open. Automatic-duration `DisplayTextToPlayer` and sender-prefixed
`DisplayTimedTextFromPlayer` remain unsupported. Invalid/unknown player handling
is a safety refusal, not a claim about native null-handle behavior.

Synthetic Host tests cover local filtering, actual VM argument conversion,
logical-time chunking, clearing, capacity, failed arguments and declaration
qualification. Factory tests cover the published session and rejected
replacement. Renderer checks cover rich-text spans and relative sizing; the
explicit Vulkan test covers colors and clearing through 4K. Native/WASM builds
and actual WASI/Cargo-WASM execution are recorded separately in the private
verification receipt. These checks do not establish chapter playability,
original/native/WASM visual parity or complete game determinism.
