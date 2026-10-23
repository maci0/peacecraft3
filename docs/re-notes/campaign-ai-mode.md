# Campaign mode transition

The selected executable has SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
The owned `common.ai` declares `SetCampaignAI` with no arguments or result and
calls it after `GroupTimedLife(false)`, before the campaign's staggered sleeps
and construction threads.

## Selected-code evidence

The primary registration at `0xab6887` binds `SetCampaignAI`, `()V` and handler
`0xab6e80`. The handler resolves the owned AI context and passes the same
boolean argument to `0xac2620` on both reference-resolution paths. That helper
clears the context bit and visits attached unit records through `0x6933c0`,
clearing a corresponding record bit. The later admission path at `0xabb3ce`
derives its record bit from the inverse context bit. These are instruction
observations, not an original-client runtime measurement.

`CampaignModePolicy` derives the mode argument and masks from these operands.
It checks the registration, NUL-terminated name/signature, context resolver,
set/clear complements, unit propagation, later-admission relationship and a
context-bit consumer. The selected masks are `0x4000` and `0x40`; synthetic
fixtures use different values. The other registration points to a short stub;
that does not establish edition selection or inert primary behavior.

The bounded consumer at `0xac39d9` gates a separate controller action. Its
complete gameplay effect is unqualified. The local static receipt is
`.scratch/campaign-basics-native/set-campaign-ai-registration.json`, SHA-256
`c8643818bdc7b7d6dfe2236dcc2cb497eff67d6f318264ad16e5543218c8f3be`.
Original image bytes and disassembly stay local and ignored.

## Canonical runtime state

Mode belongs to the existing active-player registry, using the owned runtime's
player rather than mutable script globals. It remains unknown until the setter
runs. The setter uses the decoded argument and leaves other players, their
flags, units and orders intact. Deactivation removes mode with that context.

For the supported ordinary direct-created, effect-free unit envelope,
`campaign_unit_mode` resolves the current canonical owner and mode lazily. This
represents propagation to existing units, later births and ownership changes
without another unit store or copied native flag layout. Canonical ownership
as attachment is a recreation choice; exact native manager exclusions are not
established. Cached, summoned and effect-history units refuse a mode observation
when their attachment is unqualified. No attack, guard, formation or assistance
rule is inferred from the flag. Unimplemented controller natives still refuse.

The AI host also forwards `GetPlayers` to the existing map-configuration host.
That reads the count written by the map's `SetPlayers`, retains unset-state
refusal, and does not substitute maximum player slots or an AI-local count.

## Validation and remaining frontier

Eight focused tests pass, including the explicit selected-image decode. They
cover nonstock operands, malformed relationships, every truncated window,
strict AI declaration/argument admission, player isolation, sleep, retirement,
existing/later units and owner transfer. The final core suite passes 1,677
tests. The published two-player fixture checks both mode and the map-authored
player count before and after sleep. Selected Human01 menu activation publishes
152 units, and selected victory continuation passes. Strict affected native
all-target and WASM library/binary Clippy, formatting and diff checks pass.

The unchanged Human02 opening-minute test now passes both calls and refuses
`SetReplacementCount` at 7.312500 seconds. Its failure remains recorded; the test
was not weakened to accept it. Logs are local under
`.scratch/set-campaign-ai/gates/` and `.scratch/set-campaign-ai/final/`.
The later [replacement-count integration](campaign-ai-replacements.md) passes
that setter and records the next refusal at `GetUnitCount` at the same clock step.
No new original-client run or rendered menu/campaign comparison was performed.
The selected startup checks are headless execution evidence, not visual
playability or complete campaign-controller acceptance.

The default native application also builds successfully. A fresh dedicated
display attempt exited before app launch: Xvfb could not bind its local
listening sockets in the managed environment. No screenshot or input event
was produced. That failed attempt is retained under
`.scratch/menu-entry-native-20261023/`; the exact native executable and changed
source identities are pinned in
`.scratch/set-campaign-ai/final/build-receipt.json`. A rendered menu-entry
replay is still required.
