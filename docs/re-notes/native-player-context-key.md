# Exact Legacy context-key comparison

The existing-record lookup has a conditional byte-equality guard. When the
comparison returns normally, both supplied pointers and all accessed regions
are readable, contents remain stable throughout the read, and both strings
terminate before the supplied bound, its zero result means exact byte content
and length equality through NUL. This is not evidence of case/slash folding,
lookup completeness, actual selected-map identity or a valid live context.
`GetPlayerNeutralPassive` remains unsupported; no player result, metadata word
or threshold is introduced as an implementation constant.

## Target and evidence boundary

The sole target is the supplied x86 Legacy `Warcraft III.exe`, 14,858,960 bytes,
SHA-256 `3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Its previously verified PE VERSIONINFO is 1.29.2.9231. This does not identify
adjacent archives. Complete-file identity and initialized PE raw mappings were
checked before the new bounded static reads. GNU objdump observations are not
new REA Evidence IDs or observed target execution. No game, Wine, debugger
attachment, provider repair or original asset upload occurred.

The retained snapshot has no typed incoming-reference result for the previous
serialization wrapper `0x71d750`. One raw rel32 E8-candidate scan over initialized
`.text` found no direct-call candidates for that exact target. It says nothing
about indirect callers or reachability and establishes no metadata-field name.
That route was left unresolved rather than expanded into a caller sweep.

The directly established context-key delegates were then inspected in these
half-open original-byte windows. Each is 512 bytes; conclusions use only the
complete selected body, excluding adjacent procedures and incomplete tails.

| VA window | File offset | Original-range SHA-256 |
| --- | --- | --- |
| `0x4696d0..0x4698d0` | `0x68ad0` | `c4f5dcd8e80ee50f038592590ab4388884e3b5684d606233c5bf1cd4bc6e5f54` |
| `0x4335d0..0x4337d0` | `0x329d0` | `85c7e908c838cc86e7122e1dcc4a926bd9197f47de26fdc66405dd5a09ca480d` |
| `0x433450..0x433650` | `0x32850` | `49a7c2a50f9b4b0def40c93bd94ccd1d6eba48f7787eabf5223ce4b02c7f7499` |
| `0xa77520..0xa77720` | `0x676920` | `261b309dfd418205ef48c163556ff0590a76b274d311e7e8316f854411ae37a2` |

The selected bodies end at `0x469756`, `0x433617`, `0x433490` and `0xa77598`
respectively, with exclusive end addresses. Full local commands, disassembly,
range metadata and corrected ledger are retained under
`.scratch/native-player-resume-20261015/`.

## Guard on a returned existing record

The retained setter/creator chain in
[map-header-build-context.md](map-header-build-context.md) selects a record for
the receiver later read by the neutral-player getter. Newly inspected lookup
`0x4696d0` requires a candidate's DWORD `+8` to match `0x433890(input)`, then
passes the pointer at candidate `+0x1c` and the supplied input to `0x433450`.
Only a zero comparison result returns that candidate. The hash, manager and
traversal contracts remain uninspected; this proves a positive guard, not that
all matching records will be found.

The wrapper's both-nonnull path calls `0xa77520` with those two pointers and
bound `0x7fffffff`, preserving EAX. Its null-pointer reporting/return path is
separate and is not treated as byte comparison or a safe fallback.

The complete nonnull comparison body has no calls. It compares unsigned raw
bytes; unequal paired DWORDs fall back to byte comparison at the same position.
The first unequal byte determines a negative or positive result. Equal bytes
stop at matching NUL or count exhaustion. Equal DWORDs use a zero-byte detector
and also consume count. No case conversion, slash substitution or locale table
is used in this body. A shorter terminated prefix mismatches the longer string
at its NUL. Without termination before the bound, zero proves only equality
within the compared bound.

Thus the key pointer actually passed from candidate `+0x1c` satisfies that
comparison relation under the stated premises. Saying the field still contains
the same pointer at a later observation additionally requires record-field and
storage stability through that observation. No snapshot/atomic-equality,
object-bound read safety or concurrency guarantee follows from DWORD read-ahead.

## Creation and remaining gameplay boundaries

The creator's retained call stores `0x4335d0`'s result at record `+0x1c`.
The newly inspected wrapper obtains the already-proved nonnull NUL-excluding
length from `0x433910`, requests length plus one from `0x440610`, and passes
that result, input and count to `0xa5ad40` before returning the allocation
result. There is no visible allocation-result null check before transfer.
The allocation ABI/failure contract, length-addition overflow, arbitrary-length
transfer and ownership/lifetime are not closed by this shape.

The earlier [neutral-player note](native-neutral-players.md) already proves
`0x433910`'s length tails and the applicable **count-four** transfer path. Those
ranges were reused without rereading; count-four proof is not generalized to
arbitrary key lengths. A frozen intermediate local report called the length
contract unknown; its separate retained-contract correction and final combined
ledger supersede that statement.

These context records' pointer `+0x1c` must not be confused with the separate
metadata cache's scalar `+0x1c` or parsed metadata scalar `+4`. The actual input
map/key/cache association, successful metadata source, missing-input/default
paths and live context lifetime remain unresolved. No field meaning or Host
behavior is inferred from matching offsets, corpus values or a familiar helper
shape. The next useful investigation is the actual selected-context input and
cache association, not another guessed serialization-field name.
