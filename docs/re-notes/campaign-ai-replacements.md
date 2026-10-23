# Campaign replacement count

The selected `common.ai` declares `SetReplacementCount(integer) -> nothing`.
Its `SetReplacements(easy, normal, hard)` helper chooses the script-authored value
from the current difficulty. Human02's red AI calls it after `CampaignAI` returns.
No replacement quantity or difficulty table is copied into the host.

## Selected-code evidence

The reviewed executable has SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Registration `0xab69bc` binds the NUL-terminated name and `(I)V` signature to
handler `0xab7450`. The handler resolves the owned AI context and copies its
integer argument to context `+0x1d4` on both resolution paths. It contains no
clamp, unsigned conversion or immediate unit action.

`CampaignModePolicy` now checks this registration and the full setter alongside
the existing campaign-context operands. It derives the matching storage field
from both writes and checks the null-context error/resolver/argument/return
relationships. Synthetic fixtures use another field and accept a coherent change
to both operands; truncations and mismatched relationships refuse. Runtime state
does not copy that native memory layout.

The bounded consumer `0xabdda0` walks linked records, tests an absent linked
reference and compares record `+0x44 <= context +0x1d4` as signed integers. It
selects between `0xac2e00` and `0xac3070` through the source structure classifier.
Those paths inspect real units/abilities and submit requests; they are not a
license to spawn arbitrary same-owner replacements. The two incoming dispatch sites
are in handler `0xab4900`, whose registration/name join identifies `FillGuardPosts`.
That native remains unsupported in the host. The record's full lifecycle and
replacement-production timing are not established by this audit.

These are shipped-byte/native-code findings. Local evidence remains ignored in
`.scratch/campaign-basics-native/set-replacement-count-registration.json` and
`.scratch/replacement-count/`. There was no new original-client runtime experiment.
The retained registration receipt has SHA-256
`c76dce401568ca58c2dc951858db5ed63a9b03c3467ddf28104d0455587d4f42`;
the consumer evidence `rea-replacement-consumer.json` has SHA-256
`f12d080e79e1dca8b366f2d17816da493d25d21af686a6db68313ba57cf6a372`.

## Canonical host behavior

The existing per-player campaign registry retains `Option<i32>`: unknown until
the first setter, then the exact signed script value. Zero and negative values
are preserved. The setter requires admitted campaign-context operands and an
active canonical AI player. It changes no units, orders, resource banks, other
players or unrelated flags. Repeated starts retain the context; successful
retirement removes the count with it. Existing failed-start rollback removes
only the newly created context.

`AiHost` checks the owned AI-common declaration before main/world lending and
requires exactly one integer at dispatch. `NeutralStack` forwards the canonical
runtime player to the existing world. `campaign_replacement_count(owner)` observes
that same state; it is not a second player store or a substituted public native.
The production startup already admits `CampaignModePolicy`, so extending that
source-bound policy needs no separate startup option or policy module.

This implements the setter's state transition. It does not implement guard-post
population, training, resource costs or replacement waves. `FillGuardPosts` and
other unsupported production/controller calls continue to refuse explicitly.

## Validation and frontier

The final checks pass 1,687 core tests, 55 app-library tests and 638 app-binary
tests. They cover exact signed values through both integer extremes, inactive/
unknown players, independent owners, sleep/repeat starts, context retirement,
failed-start rollback, malformed declaration/arguments, nonstock field operands,
and every truncated decoder window. The explicit selected-image decode passes.
The published two-program `MapScriptSession` fixture checks independent quantities
before and after sleep alongside unchanged combat and other context state.

Selected Human01 activation and the explicit-victory-stimulus Continue gate pass,
as do strict affected native all-target and WASM library/binary Clippy checks.
These are headless execution checks; they do not prove mission completion.
The unchanged Human02 opening-minute test still fails, now at `GetUnitCount` at
7.312500 seconds after passing `SetReplacementCount`. Its assertion remains
unchanged, and its exit 101 is retained. Exact commands/results are under
`.scratch/replacement-count/gates-final/`; earlier evidence is preserved separately.
No new original-client or rendered app run was made for this setter.
The default desktop build and formatting also pass. The ignored build receipt
`.scratch/replacement-count/build-receipt.json` has SHA-256
`9e8e173cf41e07c78645f84549470b991eb69ccddf7d544687eee2d1553eb11e`.
The retained default executable has SHA-256
`f71a987c7303a3ea5a278ba8278e39079d8508ef0c9dc47f9a251a55c4c0c4c7`;
the final app-test executable, including the unchanged frontier test, has SHA-256
`c62ad925539e1f6dc7d522cf1dc73cac4ef95540996e5f44d2222255686810af`.
