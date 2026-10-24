# Campaign AI harvest assignment rollover

`ClearHarvestAI()` rolls the owning AI's current harvest assignments into the
previous cycle. It does not issue a stop order, cancel explicit harvesting,
discard carried resources or change the gathering/captain association.
`CampaignBasicsA` calls it before checking combat and requesting gold/wood
workers. Assignment rollover and issuing worker orders are separate operations.

## Selected-source evidence

The selected `common.ai` declares a nonconstant no-argument native returning
nothing. Its `CampaignBasicsA` first queries the farm's food value, calls
`ClearHarvestAI`, then calls `CaptainInCombat`, `HarvestGold` and `HarvestWood`.

The reviewed 1.29.2.9231 executable has SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
The primary registration at `0xab63b9` binds `ClearHarvestAI`, `()V`, handler
`0xab44d0` and registrar `0x8ccc80`. The handler resolves the current AI context
and forwards it to `0xabc550`. It does not use the separate generic stub entry.

That helper walks the context's linked units and obtains their AI components
through `0x6933c0`. For each component, its flag transform is:

| Current flag | Previous flag | Current set | Current clear |
| --- | --- | --- | --- |
| `0x01` | `0x04` | clear current, set previous | clear previous |
| `0x02` | `0x08` | clear current, set previous | clear previous |

Unrelated flags are preserved. The helper's calls resolve references and units;
the inspected body contains no unit-order, position or resource-bank operation.
The following `HarvestGold`/`HarvestWood` handlers call a different assignment
helper. The [gold harvest investigation](campaign-ai-gold-harvest.md) now
qualifies a one-worker cycle and its zero-worker control; autonomous selection
and resource transfer remain unimplemented.

Bounded REA byte observations:

- Registration: `ev_81ec99fddb71a7c573a31be54d84efa4a65035b021563c70ce2ee0d0eb320526`.
- Handler: `ev_b9d3c2d20c59f26937ccea1191e7e41b8b7ee20f82e64d4c99dad91579a7155b`.
- Rollover body: `ev_80b3f4ede7c2aaab11bf05d4e13e2d3229a4b3f86bdde5305309585331051620`.

Ghidra did not supply a procedure record for the requested helper. Its refusal
is retained alongside successful mapped-byte reads, checked PE windows and
bounded disassembly under `.scratch/clear-harvest/`. These are static observations;
no new original-client run or decompiled body is claimed.

## Shared-world implementation

`GatheringPolicy` additionally binds the registration, NUL-terminated strings,
owned-context control flow, rollover body and unit-component accessor. It
derives both current/previous mask pairs and validates distinct single bits,
signed OR/AND semantics, common field access and control-flow targets.

Accepted assignments live on canonical unit records, not a copied native linked
list or second unit store. No accepted managed assignment is represented by
`None`; explicit map harvest orders do not create one. The call requires source
admission and an active canonical AI player, then rolls only that owner's
existing assignments without allocation. Ownership transfer, death/removal and
AI retirement discard that context's assignment state. Failed preflight leaves
the units unchanged.

Tests seed canonical assignment records to discriminate the transform, all
current/previous combinations, repeated clears and unrelated bits. They also
cover player isolation, missing policy/inactive context refusal, source ABI and
argument errors, sleep/resume forwarding and ownership/retirement cleanup.
An explicit-order synthetic harvest cycle checks that clearing assignments preserves the
explicit worker's order and motion and still credits its carried load.
The published two-program fixture calls the native before and after sleep.

This slice does not allocate workers, mine gold, choose AI towns or implement
`HarvestGold`/`HarvestWood`. Those calls continue to refuse until connected to
the canonical resource and order runtimes. A successful empty rollover is not
evidence for autonomous harvesting or a complete campaign strategy.

Combined verification passes 1,707 core tests, 55 app-library and 638 app-binary
tests, the published two-program session, actual selected-image decoding,
Human01 startup and victory Continue, strict native/WASM Clippy, formatting
and the default desktop build.
The unchanged Human02 opening-minute test passes this call and now refuses
`CaptainInCombat` at 8.312500 seconds. Its full failure remains retained under
`.scratch/clear-harvest/gates/`; chapter completion and rendered parity remain
unverified.
