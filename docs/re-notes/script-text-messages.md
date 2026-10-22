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

The campaign startup derives the message frame geometry from the selected client
image during map preparation, retains it on the published `ObjectHost`, and refuses
a second admission. The renderer consumes that host's admitted layout on the
published campaign UI camera; it does not read the launcher or re-derive geometry.
It reads `UI\\MiscUI.txt` / `FontHeights/WorldFrameUnitMessage`, then
`UI\\war3skins.txt` / `Default/MessageFont`, and resolves the resulting font through
the reader belonging to the session's binding receipts. Each read is bounded and
its receipt retained. Missing, malformed or unsupported inputs produce an explicit
consumer error. Session replacement removes the old text and font, and the next
root is painted only from the replacement host's layout. Changing the launcher
cannot rebind an already published session. Clearing or logical expiration removes
the visible text; menu suspension hides it. Text colors and line breaks use the
existing HUD rich-text parser.

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

The placed box is derived from the same image, not from copied measurements.
`pcx_play::message_layout::Rules` validates the reviewed image hash, then bounded-reads
the constructor at `0x5e7ca5`-`0x5e863c`, the dynamic consumer at `0x5f4860`, and the
UI-to-normalized conversion at `0xab297d`/`0xab29a0`. It reads the skin keys
(`SimpleConsole`, `WorldFrameMessage`, `WorldFrameUnitMessage`), the frame fields
(`+0x468`, `+0x424`, `+0x428`), both anchor pairs (bottom-centre to top-centre),
the console `SetAllPoints` call target `0x873750`, the unit `SetPoint` target
`0x873a10`, the initializer widths, heights and offsets, and the `.rdata` constants
`0.8`, `0.6`, `0.0005`, `0.098125` and `0.452`. It refuses a mismatched hash,
instruction, key, field, anchor pair, call target or initializer/consumer relation.

Native placement for script `(x, y)` is then, in logical UI units with positive Y
upwards and negative inputs clamped at zero:

- unit left `= message_centre_x + x * (0.8 - message_right + 0.098125) - 0.098125 - 0.25`
- unit bottom `= message_top + y * (0.452 - message_top - 0.0005) + 0.0005`
- unit size `0.5 x 0.25`, wrapping at `0.5` logical width

At `(0, 0)` this reproduces the constructor's own `WorldFrameUnitMessage`
rectangle: left `0.051875`, bottom `0.2305`, width `0.5`, height `0.25` in a
`0.8 x 0.6` UI. The renderer converts that rectangle to percentages of the
campaign view and stacks rich-text lines upward from its bottom edge. No
fallback rectangle is invented when the derivation fails; the consumer reports
the error and keeps the previous frame.

Reconstruction choices remain visible: text is stacked upward from the latest
executed normalized position; the native single-frame position therefore applies
to the whole retained block. Per-message historical positions, fade-out timing,
chat-history integration and an original rendered comparison remain open.
Automatic-duration `DisplayTextToPlayer` and sender-prefixed
`DisplayTimedTextFromPlayer` remain unsupported. Invalid/unknown player handling
is a safety refusal, not a claim about native null-handle behavior.

Synthetic Host tests cover local filtering, actual VM argument conversion,
logical-time chunking, clearing, capacity, failed arguments and declaration
qualification. Factory tests cover the published session and rejected
replacement. Synthetic host tests prove that two published hosts keep their own
decoder-derived layouts and that a second admission is refused. A selected-install
component test injects diagnostic timed text through each published host's own
`GetLocalPlayer` and `DisplayTimedTextToPlayer` natives, then covers launcher
replacement and a lookalike session without that session's publication. Its
synthetic stimulus is not mission evidence; it is ignored because it requires
explicit local inputs. Renderer checks cover rich-text spans and relative sizing; the
explicit Vulkan test covers colors and clearing through 4K. Native/WASM builds
and actual WASI/Cargo-WASM execution are recorded separately in the private
verification receipt. These checks do not establish chapter playability,
original/native/WASM visual parity or complete game determinism.
