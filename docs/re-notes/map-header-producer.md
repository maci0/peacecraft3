# Exact Legacy nonzero callback metadata source

A confirmed instruction neighborhood supplies the metadata pointer used by the
nonzero `0x71fb90` callback: it is the second caller argument of the short wrapper
at `0x71d750`. The directly invoked dispatcher calls that callback with mode one
and, on its nonnull allocation path, initializes the stream cursor to zero.
Combined with the published writer proof, this connects the third output word to
that caller-supplied metadata object's scalar `+4`.

This closes one pointer/serialization producer relationship. It does **not**
identify the scalar's semantic name, the upstream procedure assigning it, an
actual selected map/cache association, a default on failed reads, or an active
lifetime. `GetPlayerNeutralPassive` remains unsupported. No Host implementation,
game execution or campaign success is claimed.

## Identity, retained-state check and candidate boundary

The target is the same supplied x86 Legacy `Warcraft III.exe`, raw size
**14,858,960 bytes**, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Its established PE VERSIONINFO is 1.29.2.9231, which does not identify adjacent
archives. No World Editor or compatibility-image address was transplanted.

The snapshot `/tmp/peacecraft3-neutral-context-20261014.snap` was parsed
selectively as JSON, not printed as a whole. Its SHA-256 is
`cc37e0a2ae15a05b42841ff43d4b1ae9f0ca723192f1fcf9d2b1d733cadb9eaa`.
The four retained function-query entries and eighteen Evidence records have the
exact target digest. No retained parameters/normalized result mentions
`0x71fb90`, and no typed incoming-reference result for that callback is present.
That is a statement about retained coverage, not absence of native callers.
No cold provider import, timeout/retry, provider configuration or live runtime
operation was performed.

One permitted raw little-endian callback-pointer scan was therefore used over
the initialized `.text` bytes: VA start `0x401000`, file offset `0x400`, raw size
**10,897,920 bytes**, SHA-256
`eb1e29492127d3dfaa5fc7bea439da9bb8293e92d4b1178634ac6d8300817c79`.
It found three literal occurrences preceded by `0x68`:

| Candidate instruction VA | Status |
| --- | --- |
| `0x71d53e` | Previously established zero-mode read wrapper; not expanded |
| `0x71d766` | Selected and confirmed by instruction-neighborhood inspection |
| `0x71d8da` | Uninspected candidate; no caller/producer claim |

These are **untyped raw-byte candidates**, not provider xrefs or a complete
indirect-call census. A bounded 512-byte neighborhood preceding the selected
candidate yielded a prologue-shaped boundary at `0x71d750`. Its decoded
instruction sequence reaches the exact callback push and returns at `0x71d77e`.
The direct callee `0x70e8e0` was then inspected only to establish callback ABI,
mode and local stream initialization. No writer sweep followed.

New GNU objdump ranges, checked against initialized PE file mappings, are
half-open:

| VA range | Bytes | SHA-256 of original file range |
| --- | ---: | --- |
| `0x71d766..0x71d7b0` | 74 | `bb42f0b28c71fe40302c28f852ba1bc651fb177f94bf7e3b30f9c5af56eb8310` |
| `0x71d750..0x71d780` | 48 | `ac7a63bf8f8b9d0aaa775dc14afdd0e4b6ed167e45068b9b13a1c8e9263139cf` |
| `0x70e8e0..0x70eae0` | 512 | `51d08cd5bc8c4e99f983cd6712340c6c03d062703fd34645d1c2ee8694f3d98f` |

The first window confirms the candidate push but includes an adjacent procedure;
its adjacent instructions are excluded from conclusions. The short wrapper is
covered through its return. Dispatcher conclusions stop at its return at
`0x70ea0b`; adjacent bodies in that range are not analyzed. Private JSON, scripts,
range hashes and disassembly stay under `.scratch/map-header-producer-proof/`.
No original contents are added to tracked fixtures or uploaded. These are new
local byte observations, not new provider Evidence IDs.

## Confirmed record producer and callback ABI

The short wrapper's frame pointer defines a 16-byte local record at `EBP-0x10`:

1. `0x71d756..0x71d759` loads caller argument two (`EBP+0xc`) and writes it to
   local `EBP-0xc`: **record `+4`**.
2. `0x71d75c..0x71d75f` loads caller argument three (`EBP+0x10`) and writes it to
   local `EBP-4`: **record `+0xc`**.
3. `0x71d762..0x71d773` passes record address, callback `0x71fb90`, literal
   `0xea9c90`, and caller argument one to direct callee **`0x70e8e0`**. The
   wrapper cleans sixteen argument bytes and returns without another EAX
   assignment. No leading metadata scalar is initialized in this body.

The literal was already established as W3I-associated in the preceding
investigation. Its path/open/source-priority chain remains on hold and was not
expanded. The wrapper record's `+0` and `+8` slots are not initialized by the
visible body; the proved nonzero callback mode consumes `+4` and `+0xc` instead.
No default is invented for unused slots.

The dispatcher reads its third argument into EBX (the supplied callback), saves
its fourth argument (the record), and at **`0x70e97a..0x70e983`** explicitly calls:

`callback(stream_wrapper, 1, record)`

Thus this is a statically confirmed nonzero-mode invocation, not a callback-role
hypothesis inferred from a literal. The previously proved callback then passes
record `+4` and `+0xc` to writer `0x71f590`. Consequently the writer's metadata
pointer is **`0x71d750` caller argument two**, and its third/format-selector
argument is caller argument three. Neither argument receives a semantic field
name from this transport.

## Stream initialization, result paths and scalar distinction

When its allocation result is nonnull, the dispatcher sets stream fields `+4`,
`+8`, `+0xc` and **`+0x10`** to zero at `0x70e953..0x70e968`; it separately sets
`+0x14` to all-one bits. The published scalar writer uses stream `+0x10` as its
advancing cursor. Therefore this actual dispatcher establishes the initial-zero
cursor premise of the preceding third-word serialization proof, conditional on
valid backing/refill behavior. The callback writer emits a leading tag and
metadata `+0`, then the metadata **scalar `+4`**, four bytes at a time. No format
value or metadata word is encoded as a production implementation constant.

Three unrelated meanings of `+4` must stay distinct:

| Object | Meaning established by this chain |
| --- | --- |
| Local callback record `+4` | Pointer supplied from wrapper caller argument two |
| Stream wrapper `+4` | Stream field zeroed before serialization, later used by write primitives |
| Caller metadata object `+4` | Existing DWORD supplied to the third-word scalar writer |

Zeroing the stream field is not a default or producer for the metadata scalar.
The inspected wrapper and dispatcher never assign the metadata scalar. Its
upstream assignment remains missing.

Failure/success paths also remain separate:

- A null callback takes a diagnostic call and assigns EAX zero before the
  cookie-check return tail. In this wrapper the callback address is explicit.
- A null allocation result sets the stream argument to zero, but the dispatcher
  still invokes the callback with mode one. It does not skip serialization or
  initialize the metadata scalar. The writer's directly called primitive
  dereferences its stream receiver; no safe allocation-failure result follows
  from the visible dispatcher branch.
- Callback EAX zero is tested at `0x70e988..0x70e98a`; the dispatcher conditionally
  invokes stream virtual `+0x10`, then assigns zero before its return tail.
  Virtual cleanup/allocation failure contracts remain unproved.
- Callback EAX nonzero enters the later output path. Its directly called output
  helper `0x711930` is uninspected; its result is saved, conditional virtual
  cleanup is invoked, and the saved result is placed in EAX before cookie
  checking. This does not prove successful output, a same-map cache association,
  native field meaning or stable lifetime.

The [published preceding slice](map-header-build-context.md) remains the source
for writer primitives, read/default distinctions and the concrete active-context
setter/read edge. This new transport proof does not close their unresolved live
ownership, cache validity or null-input fallback initialization.

## Stop boundary and exact next probe

One nonzero callback pointer-producer relationship is closed, so this bounded
slice stops here. No second candidate, caller sweep, old path helper, source
priority, QPC function, World Editor target or game process was investigated.
The typed-callback-reference result remains unavailable in the retained snapshot;
confirmed bytes are reported separately from provider-typed references.

The next specific query is **typed incoming references to `0x71d750` in already
retained/aligned exact-target state**. Select the actual caller supplying its
second argument, prove that argument's metadata object identity, and inspect
only a positively anchored assignment to that object's scalar `+4` or its
construction source. If that caller/assignment is absent from retained state,
record the exact analysis-state blocker rather than choosing an adjacent address
or assigning an editor/build label from corpus values. The scalar's semantic
producer/name, upstream default/failure values and selected live map/context
remain unresolved.
