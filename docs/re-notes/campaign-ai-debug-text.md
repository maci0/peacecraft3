# Campaign AI diagnostic text

The selected release client accepts `DisplayText`, `DisplayTextI`,
`DisplayTextII` and `DisplayTextIII` with no visible output. Each primary
registered handler consists of an immediate `RET`. They read no arguments,
format no text and publish no messages. The recreation preserves that behavior
while retaining source and signature admission.

## Selected-byte evidence

The selected 1.29.2.9231 executable has SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
The retained selected `common.ai` declares these four natives at lines 7–10;
its `Trace`/`TraceI`/`TraceII`/`TraceIII` functions invoke them when the script's
`trace_on` flag is set. `PrepTime` reaches `TraceI` after computing its wave
preparation quantity. Script argument evaluation and that authored flag remain
unchanged.

| Native | Primary registration | Handler | Signature |
| --- | --- | --- | --- |
| `DisplayText` | `0xab605c` | `0xab48b0` | `(IS)V` |
| `DisplayTextI` | `0xab6070` | `0xab48c0` | `(ISI)V` |
| `DisplayTextII` | `0xab6084` | `0xab48d0` | `(ISII)V` |
| `DisplayTextIII` | `0xab609b` | `0xab48e0` | `(ISIII)V` |

Each registration binds the pointed NUL-terminated name/signature bytes, handler
and registrar `0x8ccc80`. Each selected handler's first byte is the x86 return
instruction. This conclusion follows the primary bindings independently of the
separate generic stub registrations elsewhere in the image.

The bounded REA memory observations are retained under
`.scratch/ai-display-text/`:

- Handler region: `ev_c34687d7d03ddacbec9ffb696b56f8effc73ae07899cac1dfda68cd927cd101f`.
- Registration region: `ev_c56329e7f0d09710f926a0dc7e144cc8350f824d66257fd1969ba14b91d5cab7`.

Ghidra did not provide procedure records for these return-only entries; those
refusals are preserved alongside the successful mapped-byte reads and bounded
disassembly. No decompiled body or new original-client runtime observation is
claimed. The behavior is scoped to the admitted selected-image bindings.

## Shared-world integration

The existing `CampaignModePolicy` reader verifies these additional registrations,
strings and return instructions before admitting the campaign profile. Runtime
calls require that admission and validate the integer/string signature. The
player integer and format placeholders are intentionally not interpreted by
the empty handlers; null strings and signed integer values are accepted. No
message record, player identity, clock update, global switch or persistent
diagnostic store is created. The mode flag itself need not be enabled, and
no running-player flag is required by these unconditional handlers.

Ordinary map text still uses the existing
[local timed-message system](script-text-messages.md). Its contents, revision,
capacity, duration and presentation are unaffected by AI diagnostic calls.
Unsupported AI natives continue to refuse by name.

Tests cover all four source contracts, malformed arity/type/source/duplicate
declarations, absent source admission, null strings, arbitrary player integers,
and unchanged live message contents/revision/handle count with a full message
budget. The published two-program fixture calls the family before and after
sleep. Combined validation passes 1,699 core tests, 55 app-library and 638
app-binary tests, the published fixture, the actual selected-image reader,
Human01 startup and victory Continue, strict native/WASM Clippy and the default
desktop build. The
unchanged Human02 opening-minute test now reaches `GetFoodMade` at 8.312500
seconds, after the earlier `DisplayTextI` failure at 7.312500 seconds. That
test remains red. Receipts stay under `.scratch/ai-display-text/gates/`;
the complete campaign and its production system remain unfinished.
