# Campaign slow chopping

The reviewed Legacy 1.29.2.9231 executable has SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
The owned `common.ai` declares `SetSlowChopping(boolean)` returning nothing.
`CampaignAI` calls it with true after the group-flee setup. Flag admission now
lets unchanged Human02 reach `GroupTimedLife` at 7.218750 seconds; that is not
a completed opening minute or campaign.

## Selected-code relationships

The primary registration at `0xab697d` binds the terminated name and signature
to handler `0xab74a0`. The handler resolves the running AI context and sets or
clears its flag. The reader derives the mask and verifies the complementary
signed immediate, context-resolution branches and both returns. Initial state
is not guessed from the native's name.

The canonical owner query at `0xac3e80` reads the same field and bit. The tree
path calls it at `0xb79774`, conditionally selects a float from the image's data
section, and sends that value in the destructable packet. The original integer
harvest amount still goes to the carry helper at `0xb797bc`. On return,
`0xb79878` repeats the owner query and selects an instruction-derived integer
instead of the carried quantity before player credit. These are separate
damage, carry and credit inputs, not a chop-interval multiplier.

A modified-unit branch can adjust the deposit through another callback. Its
native flag semantics are not mapped to all live host states. The selected
flag also gates a controller update outside the explicit worker path; that
controller behavior remains unqualified.

## Original-client pair

Two matched RoC diagnostics retained the same player-nine peasant, tree, depot,
unordered player-eight control and complete `Ahar.Har1=7` override. Seven was
an experimental map edit, not a production rule. Only the diagnostic identity
and constant-flag AI differed. The minimal AI set the flag, wrote its own gold
bank to 17 and slept; it did not call `InitAI` or create captains. The map
witnessed gold zero before starting the AI and waited for 17 before issuing
its explicit `autoharvestlumber` order.

| Flag | Observed tree life | First lumber / gathered credit |
| --- | --- | --- |
| false | 50 → 43 → 36 | 10 / 10 |
| true | 50 → 49.9 → 49.8 | 1 / 1 |

The two decrements were first observed at 5.563 and 6.688 in both runs. Both
traces completed at 11.750 with 67 periodic samples, including 20 after first
credit. The control's order, position, tree life and banks stayed unchanged.
Native string output rounds the displayed life values; these observations do
not establish exact binary arithmetic or all timing behavior.

This pair establishes an effect on an explicit map-issued harvest, including
without `InitAI`. It supports preserving the authored carry/capacity cycle
while changing tree damage and deposit credit separately. It does not support
scaling the harvest interval or multiplying every cargo update by a guessed
slow rate. Production quantities are read from the checked image operands.

The false and true map SHA-256 values are respectively
`72c14287246d119300c53d0bc62240c3f4443f462a34ebb7f3647f00a6dbc3ae`
and `e5fc5c6ccf88ab3476ecc3577a2b73bea3e873f98d551fbac157fdec5c314956`.
Both used the reviewed executable and reported Reign of Chaos. The strict
collector checks source identities, gold acknowledgment before orders,
finite values, complete sample sequences and the actual termination condition.
All three collector tests passed, including malformed and truncated fixtures.
Both original clients are stopped; their containers and raw receipts remain
preserved. This was a diagnostic Wine reference, not rendered parity or
unchanged-campaign acceptance.

## Integration contract

The existing per-player campaign registry stores the explicit flag, initially
unknown until its setter runs. The world resolves ownership through canonical
players and units. The policy binds the ordinary damage and deposit operands;
malformed quantities refuse at admission. No second worker or unit store is
needed.

At a qualified tree hit, true selects the policy's damage while the authored
`Ahar` amount continues to fill carry and trigger the existing capacity return.
At a positive deposit, true selects the separate policy credit through the
existing atomic bank/gathered update. False, inactive owners and unadmitted
policy retain their existing behavior. Unknown active state and unsupported
worker modifiers must refuse before affected writes. The flag does not gate
ordinary movement or change the authored swing interval.

Re-reading current owner and flag at each affected seam is a recreation choice
supported by the native queries; mid-order toggles and ownership changes were
not measured by this pair. Modified workers, native callback effects, the
separate controller path, partial returns and broader edition/race parity
remain fidelity questions.

Combined validation passes 1,661 core tests and 610 app tests, ten slow-chopping
reader checks, sixteen flee reader checks and six new-heroes reader checks,
including the explicit selected-image decodes. The published two-player fixture,
selected chapter startup and victory Continue pass, as do strict native and
WASM Clippy and formatting. A red/green worker regression confirms that an
accepted return walk continues when the flag becomes unknown, refuses before
deposit, and credits once after a qualified retry.

The unchanged opening-minute gate still refuses `GroupTimedLife(boolean)` in
Human02 at 7.218750 seconds. Human01 publishes 152 units before Continue. This
is not campaign completion. Combined logs are retained under
`.scratch/campaign-flee-integration/slow-chopping-consumer-*`.

Build/source/marker receipts remain local under `.scratch/slow-chopping-probe/`, with superseded
r1 artifacts preserved separately. Mapped native windows and hashes remain
in the ignored `campaign-basics-native` notes. No original asset bytes are
tracked.
