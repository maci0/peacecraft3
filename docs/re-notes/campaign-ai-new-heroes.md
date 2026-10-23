# Campaign new-hero flag

The selected Legacy 1.29.2.9231 executable has SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Its owned `common.ai` declares `SetNewHeroes(boolean)` returning nothing.
The unchanged Human02 opening-minute gate now passes that call and stops at
`SetHeroesFlee` at 7.218750 seconds after captain creation.

## Selected-code evidence

The primary registration selects handler `0xab72d0`. It resolves the running
AI context and sets or clears one bit in its flag word. The reader derives
the mask from the signed immediate operands and verifies that clearing preserves
the other bits. A missing native context takes an error path.

The reviewed initializer `0xabf960` clears the flag word; its conditional OR
does not include this bit. That is evidence for the reviewed initializer path,
not a default copied into the host. The recreation reports unknown flag state
until the first explicit setter.

The bounded consumer review found two positively identified AI-context whole-word
reads, at `0xac950c` and `0xac9b70`, which consume a different bit. Other raw
displacement matches do not establish receiver identity. No gameplay consumer
of this flag is qualified by that review; this is not proof it is globally unused.

## Integration boundary

The privately integrated host stores the explicit transition in the existing canonical
per-player registry. Ownership comes from the running AI context, not the
mutable script `ai_player` global. The setter does not create heroes, revive
units, change targeting or assign captain membership. Those effects require
separate qualification.

The published two-program session checks separate flags before and after sleep,
alongside unchanged targeting, gathering and captain-state assertions. Six
reader tests pass, including the selected executable and malformed terminated
strings. Combined checks pass 1,617 core tests, 610 app tests, strict native and
WASM Clippy, formatting, selected startup and Continue startup. The unchanged
opening-minute gate passes this setter and refuses `SetHeroesFlee`; it does not
complete the minute or campaign.

Receipts remain under `.scratch/campaign-flee-integration/new-heroes-*` in the
private integration checkout. Local source evidence and supersession receipts
remain under `.scratch/campaign-basics-native/` in the main checkout. No original
bytes are tracked; no new original-client or rendered behavior check was run.
