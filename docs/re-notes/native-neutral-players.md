# Neutral-player getter: exact Legacy target

This is a bounded static native investigation. `GetPlayerNeutralPassive` remains
unsupported in the game Host. No MapInfo value, source corpus observation or
familiar player limit is substituted for the missing context initialization.

## Binding and retained evidence

The target is the locally supplied Legacy `Warcraft III.exe`, PE x86, version
1.29.2.9231, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Adjacent archives and the older extracted Game.dll are not identified by this
executable version. REA 6.4.0 reopened
`/tmp/peacecraft3-script-native-handoff-20261014.snap` with Ghidra 12.1.3 and
analysis profile
`c95018f497b511226b067817a25ac96a08e093e61362ab933904e15985ed51ee`.
The snapshot retains 12 Evidence records. No original asset contents are added
here, and no executable is run by this static probe.

The registration evidence connects `GetPlayerNeutralPassive` at `0xe78300`,
signature `()I` at `0xe6f674`, and getter `0x4946b0`:

- Name seed: `ev_0d20d0485b960faa0a19fa0cde970dfce38c61b7f05a3a60061b095af11b3555`.
- Name xrefs: `ev_ef894844e342bd5896ee7fd895d252116d8fafae60a4c2de57c4769c8f9ff972`.
- Registration bytes: `ev_691bd4ee1ce864c26cb7fbfb72c8e0d8083c39b36439207f42dae42e84b63a8e`.
- Signature bytes: `ev_bbf073840c0e36a891774ab89386df4c9e6afeee77ad7f003e30a41a25e55984`.

## Getter data path and return convention

The complete getter bytes are retained in
`ev_dc4963e439464554962db5b149b7f5445e4c0c4da9a17125a4673216e5a8feef`.
Exact instruction evidence separately covers its global read and calls:
`ev_0993c0e7ed788f6630e2209267b500e50afdd49f209776eed7526e14b7acd4c6`,
`ev_36261d85956dc96c306442c86d3af9dc702dca10fa7bfeceb1c1cac893a20603`,
and `ev_d9078d97fdf4794cd432e8779353fef65af4676030724efc4557120d8449e37f`.
Address-bounded `objdump` against the same executable corroborates the decoded
instructions through the return at `0x4946d6`.

The getter reads global pointer `0x113b6f4`, reads its `+0x30` pointer, and passes
the address of that object's `+0x24` subobject in ECX to `0x469010`. That helper
returns the pointer at `subobject->+0x8->+0x1c`, or zero when the intermediate
`+0x8` pointer is zero. It does not return an arithmetic player count.
Evidence: `ev_6bdba903f8ca3f04f97418ae6aa98ef583377959c57ab6cee0c4075a18a41ea9`.
The previously mistaken callee `0x468010` is excluded from this data path.

The returned pointer is pushed as one stack argument to `0x704330`; the getter
cleans four stack bytes. It compares helper EAX **unsigned** against `0x17ac`,
then executes `SBB EAX,EAX`, `AND EAX,0xfffffff4` (sign-extended immediate), and
`ADD EAX,0x1b`, followed by `RET`. Therefore the getter returns integer **15**
when the helper result is below `0x17ac`, and **27** at or above it. This is a
static instruction-derived branch result, not an observed live value or an
implementation constant. Failure sentinel `0xffffffff` takes the unsigned
at-or-above branch; it must not be confused with a successful context read.
The getter has no null guard before its first context dereferences.

## Helper and fallback source

Retained `0x704330` assembly is in
`ev_8a8f0935262b16243edf305ba8640bcf763a8902026df872716eca4f22af6514`.
Its cached pseudocode incorrectly presents the function as `void`; assembly
shows EAX return values, so that inferred type is not accepted as the ABI.

- A zero input calls `0x707650` and returns the DWORD at its result `+0x13c`.
- A nonzero input initializes a local buffer through `0x4728a0`, passes the
  pointer and buffer to `0x703ff0`, and returns the buffer's DWORD at `+4` on
  success or `0xffffffff` on failure.
- Security-cookie checking occurs before return. The caller's subsequent EAX
  comparison and the assembly return path are stronger evidence than the
  decompiler's `void` label.

An exact bounded disassembly of `0x707650..0x707655` shows it returns address
`0x113f858`. Thus the null-input field is at **`0x113f994`**. The owning object's
name, that field's semantic meaning, its initializer and its valid lifetime are
not established by this pointer arithmetic. The first portion of `0x703ff0`
selects its nonzero input or the same fallback object's `+0x38` address and
copies into a bounded local buffer. This alone does not identify a build number,
player-count record or map metadata field.

## Investigation boundary

The first new live query is `xrefs(0x113b6f4)`, to find the context global's
initializer/setter. Cached snapshot operations can answer without a live
Program, but a new address requires Ghidra's immutable import/default analysis
startup. No broad symbol searches or timeout retries are used. Follow-up work
must distinguish typed read/write references, exact source procedures, runtime
pointer chains and unknown semantic fields.

The context writer/setter-input chain is established below; metadata field
meaning and fallback initialization remain open. The native stays fail-closed
and original campaign gates remain blocked.

## Final probe ledger

The live query completed once, without a timeout retry. Global xrefs returned
1837 analyzed source addresses:
`ev_8383999c07a01ff88709dbcf2a08b75adc9c5179382de32e2184c538c2543a62`.
This is address-reference coverage, not a claim that all context users or
indirect writes were inspected. A bounded raw literal scan was used only to
prioritize candidate direct writers; it was not accepted as typed xref proof.

### Context installation and setter input: source established

The typed write instruction at **`0x4791bc`** belongs to procedure **`0x4790d0`**
and stores ESI into `0x113b6f4`:
`ev_12f6506c1379a9be5986b0b4017bf7a36e42dc67682506f2246275ef951ae239`.
Its complete procedure dossier is
`ev_00d7e940c0dc8323b7360bbbd53ebdc10bf1bf7247dcf35cf5837ff4a163a0ef`,
body `0x4790d0..0x4796ff` (inclusive). Assembly and typed references establish:

1. The procedure clears a previous context pointer at `0x479172` after its
   cleanup path. If the global is still zero at `0x4791ab`, it calls
   **`0x471860`** with stack arguments `(6, 0)`, takes its EAX result into ESI,
   and installs it at `0x4791bc`. A zero result takes the error path. The numeric
   factory selector's type/class meaning is not inferred from its value.
2. The procedure reads context `+0x30` at `0x4791e7`. When its input object EDI
   has `+0x4a8 == 0`, EBX is zero. Otherwise it calls **`0x468850`** with ECX
   pointing at input `+0x4c0` and uses returned EAX as EBX.
3. It sets ECX to `context->+0x30 + 0x24`, pushes EBX, and calls
   **`0x468d70`** at **`0x47921e`**. Exact call evidence:
   `ev_7827893cef4f6d70a0b7fa577b1723c36592ed000fc4772538e994b595f259b9`.
   This is the setter call on the same subobject later read by `0x469010` in
   the native getter. The setter body and input accessor body are not inspected
   in this slice, so their ownership/copy/lifetime behavior remains unverified.
4. The same procedure passes that selected EBX-derived value into metadata
   routines and references the literal `war3map` at `0x479419` (`0xe68bc0`).
   This establishes a map-loading connection. It does not by itself identify
   input `+0x4c0` or a metadata DWORD as a named MapInfo field.

The dossier's inferred `void` return type and implicit ECX calls are not used
as source-level contracts. Exact assembly gives the explicit stack/register
input chain. Context `+0x24` itself is separately assigned at `0x4791de`; it is
**not** the same location as `context->+0x30 + 0x24` used by this getter.

### Metadata return source: structural connection established, meaning open

Outgoing references for **`0x703ff0`** are retained as
`ev_3b0e12997b305932ea6752e5b8c4cefc79fcab08ac52fe8817dd784b42679b13`.
They include exact calls to `0x708ea0` (`0x704109`), `0x71cf40` (`0x704116`),
and `0x6fa990` (`0x704155` and `0x7041e3`). There are no unresolved call edges in
this procedure's retained reference result; this does not establish callee
semantics. The original-byte bounded disassembly corroborates that:

- The metadata/cache record is held in ESI. When an output buffer is requested,
  `0x7041dd` points at record `+0x18` and `0x7041e3` calls `0x6fa990` with that
  pointer and the caller's output object.
- `0x704330` subsequently reads output `+4`. Without inspecting `0x6fa990` and
  the decoder feeding the record, that structural connection is insufficient
  to name output `+4` as editor version, game build, player count, or a limit.
- The zero-input getter-helper branch instead reads fallback object `+0x13c`,
  absolute `0x113f994`. No initializer/write to that field was established.
  A bounded exact-address literal scan found no direct `.text` literal for
  that absolute field address; indirect object-relative writes were outside
  that scan and remain open. This negative result does not imply zero/default
  behavior, immutability, or absence of a setter.

### Diagnosed blocker and concrete next probe

**The native remains unsupported.** Its static integer-return convention and
context writer/setter-input chain are established, but the compared metadata
field's semantic identity, fallback initializer, and runtime selection/lifetime
are still unverified. Choosing a branch from map/corpus values would conceal
these gaps. No actual invocation or original game outcome was observed.

The next bounded probe should begin with **`0x6fa990`**, specifically the transfer
into output `+4`, then follow the corresponding writer in **`0x708ea0`** or
**`0x71cf40`**. Separately inspect **`0x468d70`** and **`0x468850`** for the exact
stored pointer/copy contract, and **`0x471860`** for the factory's `+0x30` owner.
For the fallback field, start from callers/xrefs of the exact singleton accessor
**`0x707650`** and track object-relative `+0x13c` writes; direct references to
`0x113f994` alone will miss them.

A later narrowly scoped live probe can break at **`0x47921e`** and
**`0x4946c6`**, recording the setter's ECX/stack input, context pointer chain,
helper EAX and getter return for one declared map/configuration. The first is
input/state evidence and the second confirms branch input. That live result
would still need map/input provenance and could not establish all editions or
map/campaign cases from one run. No game execution was undertaken here.

REA was closed cleanly with an additional local retained snapshot:
`/tmp/peacecraft3-neutral-context-20261014.snap` (10547316 bytes, 4 primitive
entries, 18 Evidence records). This file retains complete large responses that
were abbreviated by the command transport; no dossier query was rerun merely
to obtain its truncated text. The original 12-record snapshot is unchanged.
Only this new document is part of the repository slice. No implementation,
shared build, manifest, lockfile, frozen file or git operation changed.


## Follow-up: exact output-word transfer and narrower writer boundary

This follow-up reopened the 18-record extended snapshot with the
same exact executable SHA-256, REA 6.4.0 and Ghidra analysis profile recorded
above. No game Host implementation or previous frozen code was changed.

### Exact transfer: native-byte observation

A bounded GNU objdump 2.47 disassembly of the same original executable, beginning
at the retained function entry **`0x6fa990`**, establishes:

- `0x6fa994`: EBX receives the source pointer from the one stack argument.
- `0x6fa999`: EDI receives the destination pointer from ECX.
- **`0x6fa9ab`**: EAX receives the DWORD at source **`+4`**.
- **`0x6fa9ae`**: that EAX is written to destination **`+4`**.
- The function returns the destination pointer in EAX and uses `RET 4` at
  `0x6fab5e`. The retained body ends at `0x6fab60` inclusive.

Combining this exact transfer with the already retained caller edge at
`0x7041e3` proves the physical data path:

`cache record +0x18 -> source +4 -> output +4`, hence the word read by
`0x704330` comes from **cache record `+0x1c`**. The transfer itself is a four-byte
bit copy; it does not parse, clamp, sign-extend, or convert that scalar. This
establishes storage width and source offset, **not** the field's logical signed
type, domain, editor-version/build identity or player-limit meaning. It also
does not establish initialization, validity, aliasing guarantees, or lifetime.

The copy function performs additional array/member copying beyond this word;
those operations are not taken as proof of the owning record's semantic type.
The unsigned getter predicate and failure sentinel described above remain
separate facts. This observation does not authorize treating a failed metadata
read as a successful field value or implementing the 15/27 branches as constants.

### Why the two proposed writer edges do not close the numeric-field proof

The exact bounded bodies were inspected from original bytes using the retained
procedure ranges, rather than an image-wide disassembly:

- **`0x708ea0..0x7091cb`**, 812 bytes: it uses literal **`.wts`** at `0xea8610`,
  **`STRING`** at `0xea8608`, and **`//`** at `0xea8604`. Its instructions scan
  numbered text entries/braces and populate a string-entry object. This is
  native WTS/localization work; its observed writes do not establish a writer
  for the metadata scalar at output `+4`.
- **`0x71cf40..0x71d067`**, 296 bytes: it passes authored metadata text buffers,
  starting at metadata **`+8`** and later offsets, to **`0x708a90`**, with the
  string-entry object as ECX. It has no direct write to metadata `+4`, and no
  visible call argument points at that word. That is a bounded instruction
  observation; callee memory safety and arbitrary alias side effects are not
  independently proven here.

Thus these two edges cannot be accepted as the numeric field's decoder merely
because they are callees of the surrounding metadata routine. The previous
candidate-writer list is narrowed by actual code; no MapInfo/corpus correlation
was used to name the word.

### REA outcome and precise remaining blocker

The first new live operation was only
`read_bytes(address="0x6fa990", length=48)`. REA returned
**`provider_timeout`**, operation `read_bytes`, **`timeout_ms=330000`**, while
initializing its immutable/default-analyzed Program. It did not return a
successful native Evidence record. The already queued exact instruction/byte
follow-ups were stopped, the helper/server exited, and a process check found no
remaining owned REA Ghidra/helper process. No query was retried and no larger
function dossier or image-wide search was requested. The retained 18-record
snapshot remains unchanged; no new successful REA snapshot is claimed.

Consequently the new observations above are explicitly **original-byte bounded
objdump evidence**, independently bound to the executable identity that REA
validated on reopening. They are not mislabeled as newly completed REA Evidence
or runtime observations. The prior Evidence IDs remain valid for the caller
edges and function bounds; they do not stand in for the failed new byte query.

**Narrower blocker:** the physical word at cache record `+0x1c` is now located,
but its parser/initializer and semantic identity remain unproven. The localization
edges are insufficient. The fallback object's `+0x13c` initializer and context
lifetime/selection remain separate unresolved gates. `GetPlayerNeutralPassive`
continues to fail closed in the Host.

**Concrete next address:** inspect **`0x71d4e0..0x71d55a`** (123 bytes), the
**earlier** call at **`0x7040c7`** that receives the initialized metadata output
object before WTS/localization processing. The exact call and body bounds are
already retained in
`ev_3b0e12997b305932ea6752e5b8c4cefc79fcab08ac52fe8817dd784b42679b13`.
Trace only the callee that writes destination `+4`; do not assume that a wrapper
or a filename proves its meaning. Also retain **`0x6f9310..0x6f9415`** as the
initialization boundary at **`0x7040af`**, to distinguish initialized/default
state from parser success/failure. Neither additional function was queried in
this follow-up. This is the next bounded probe, not a measured decoder claim.


## Static fallback: two exact wrapper/initialization ranges

This is a fresh, bounded static
inspection with GNU objdump 2.47, **not** a REA retry. `sha256sum` again matched
the exact executable digest recorded above. The only disassembled ranges in
this step were `0x71d4e0..0x71d55a` and `0x6f9310..0x6f9415`, inclusive.
No REA session, original game execution, shared build or image-wide search ran.
All preceding reviewed/provider sections remain byte-for-byte unchanged.

### Wrapper argument flow

The complete 123-byte **`0x71d4e0`** body reads its second stack argument into
ESI at `0x71d4eb`; in the previously retained caller, this is the initialized
metadata object. When its fourth stack argument is nonzero, it calls
`0x71d180` with that object, then `0x71d340` with object `+0x3880`, and
`0x71d0c0` with object `+0x3890`. It explicitly writes zero at object `+0x38a0`
and `+0x38b0`, and calls `0x71e6b0` on object `+0x38c0`. The called helpers'
resource/ownership effects are not established here.

At the tail, the wrapper builds a local stack record:

- `0x71d537`: the third stack argument goes into local record **`+8`**.
- `0x71d54b`: the metadata object pointer goes into local record **`+0`**.
- `0x71d53e`: address **`0x71fb90`** is pushed as an argument.
- `0x71d543`: literal address `0xea9c90` is also pushed; its contents were not
  inspected in this two-range scope.
- `0x71d54e`: it calls **`0x70e320`**, with the caller's first stack argument,
  those two addresses, the local record pointer, and zero as stack arguments.
  It cleans 20 bytes and returns without a subsequent EAX assignment.

There is **no direct write to metadata `+4`** in this wrapper. Address
`0x71fb90` is supplied to another routine, not directly called by the visible
instructions. A callback/decoder role is a concrete next hypothesis, not an
established dispatch contract. The dispatcher must be inspected if its argument
use or success convention is needed; preserving EAX at this wrapper's return
does not alone establish what that return value means.

### Initialization boundary

The complete **`0x6f9310..0x6f9415`** body preserves input ECX in ESI and calls
**`0x4728a0`** at **`0x6f931a`** with that same object. It then directly clears
the DWORDs at object **`+0x3880` through `+0x38dc`**, in four-byte increments,
and returns the object pointer in EAX. These explicit high-offset clears do
**not** establish that object `+4` is zero or otherwise defaulted: the leading
region's initialization is delegated to the uninspected `0x4728a0` callee.
The high-offset words' semantic types are also unverified.

### Frozen result and next exact boundary

Raw transfer/initialization facts are now tighter, but the compared field still
has no proven semantic identity. The next candidate is **`0x71fb90`**, specifically
whether the address passed to `0x70e320` writes metadata `+4` through local-record
`+0`. **`0x4728a0`** is the separate initialization/default boundary. Inspect
**`0x70e320`** only if establishing the callback ABI or success/failure contract
requires it. None of these three additional functions was inspected in this
step, and no meaning was assigned from the unresolved literal's name/address.

`GetPlayerNeutralPassive` remains unsupported. A raw 32-bit copy, a delegated
initializer and an unverified dispatch address do not identify an editor/build
field, prove fallback state/lifetime, or justify hard-coded branch results.
The old unsigned failure-path observation remains independent of those gaps.


## Callback producer and leading-field failure boundary

This follow-up uses GNU objdump 2.47 against the exact Legacy executable SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
It does not reopen/retry REA, execute the game, or add new provider Evidence IDs.
The retained snapshot/profile and prior Evidence remain those recorded above.
The reviewed preceding 19,612 bytes are preserved, SHA-256
`e42117bf223b4a34be01127ec487c6dd0110da3a3ae12e441b53d2158d842f83`.
Raw executable bytes stay local.

### Bounded coverage and identities

All intervals below are half-open. The initial callback/initializer windows
include adjacent padding/functions; only the bodies ending at `0x71fbc0` and
`0x472942` establish the callback/initializer findings. Following calls was
restricted to the metadata-leading-field reader path. The parser window is
partial, not its entire body; trailing partial instructions are not interpreted.

| Inspected VA range | Bytes | SHA-256 of supplied executable range |
| --- | ---: | --- |
| `0x71fb90..0x71fd10` | 384 | `e95a73514af02fde310e8dc9f865eb961eae2dde04648db45d49813f8ad3e185` |
| `0x4728a0..0x4729c0` | 288 | `f186c82ce7fb5972a4aa917b9c6554dd3f2759da97dbc431e4e0f0849ab61ecd` |
| `0x71e8d0..0x71ead0` | 512 | `cd73df88d2bf5e468d9857ee2dd6215e0bc1e1b0837fa5fbe7657605bdc34c08` |
| `0x45c280..0x45c340` | 192 | `1b8e46279fafb4d99ccd94b84dd2b9731aebd4f3980a77dce00f52cc4ca4d507` |
| `0x45c130..0x45c1b0` | 128 | `199a43d070f5f3f75381504ea47f7c267e69fc2d4a54d1a6423c98209a1d9fd1` |
| `0xea9c90..0xea9cb0` | 32 | `1157bc2546646569c749253211a42cf48f8adced724e68fac6fa22ac693118cb` |

### Producer path: physical field, not semantic substitution

`0x71fb90` reads its third stack argument as a record pointer. When its second
argument is zero, `0x71fbaf..0x71fbbc` forwards the first argument, record `+0`
and record `+8` to `0x71e8d0`. The prior wrapper supplies the metadata pointer
at record `+0`. Its nonzero branch instead forwards record `+4` and `+0xc` to
`0x71f590`; that separate path was not inspected. The dispatcher has not yet
proved which branch is selected for the retained getter's input.

In the selected parser window, ESI receives the first argument and EBX the
second (the metadata destination). At `0x71e913`, `0x45c280` reads an initial
word into a local. An unsigned comparison rejects values above `0x19` by
setting EAX to zero and jumping outside this window to `0x71f570`. Values
below `0x10` skip the two leading destination reads. For initial values
`0x10..0x19`, `0x71e92f..0x71e93d` calls `0x45c2b0` on metadata `+0`, then
metadata **`+4`**. This locates a concrete producer, conditional on that tag and
branch. Calling the initial word a format/version tag is inference from its
control role; no authored field name is assigned here.

The directly called `0x45c2b0` requests four bytes through `0x45c130`. On success,
it reads a DWORD at `(source +4) - (source +8) + (source +0x14)`, writes that
DWORD to its destination and advances source `+0x14` by four. Thus the physical
representation is a four-byte x86 little-endian word, with no visible numeric
conversion. Signedness, units and editor/build semantics do not follow from
this copy. `0x45c280` has the same visible four-byte read behavior.

### Defaults and failures remain distinct

The complete `0x4728a0..0x472942` initializer zeros DWORDs at object
`+0x408..+0x43c` (four-byte increments), `+0x3850`, and the unaligned `+0x3879`,
then returns the object. It has no calls and **does not write object `+4`**.
Combined with the previously inspected `0x6f9310`, neither initializer supplies
an established leading-field default. The caller's allocation/previous contents
remain relevant, especially when the initial parser tag is below `0x10`.

`0x45c130` returns zero if its unsigned end position exceeds source `+0x10`,
or if the directly dispatched refill at source vtable `+8` fails. Those failure
paths set source cursor `+0x14` to source `+0x10` plus one (32-bit arithmetic).
The four-byte reader then **leaves its destination untouched** and returns the
source pointer, not a per-read Boolean. The parser does not test a Boolean
between its two leading reads. Its later overall completion/error propagation
is outside this window; the earlier getter unsigned sentinel branch does not
prove how this parser failure is translated.

The wrapper's directly referenced literal at `0xea9c90` contains `.w3i` followed
by a terminator. This is shipped-byte evidence of a W3I-associated wrapper;
it is not proof of the dispatcher ABI, exact archive member/input stream,
field name, or actual map context selected at runtime.

### Frozen blocker and one next action

The conditional physical writer is identified; semantic identity and default
are not. `GetPlayerNeutralPassive` remains unsupported, and original campaign
execution remains blocked. No Host constants or behavior are inferred from the
getter's static branch results.

The next specific action is a bounded entry inspection of **`0x70e320`** to
establish how the known `.w3i` literal and `0x71fb90` callback are used, which
callback branch it selects, and whether it initializes/preserves the leading
metadata destination on missing input. Start with `0x70e320..0x70e520` only;
follow a directly evidenced producer/error edge only if needed. This function
was not inspected here. Runtime applicability, context lifetime and field-name
corroboration remain separate gates even if that dispatch connection closes.


## Dispatcher: callback ABI and input/default boundary

Only the authorized half-open range **`0x70e320..0x70e520`**, 512 bytes, was
inspected with GNU objdump 2.47. Its original-byte SHA-256 is
`96eabc7385211bf53675e7fffb2929aa3a5b9d9a56ed249aafe5af3d9a79b5e8`.
The exact Legacy executable SHA-256 remains
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
The dispatcher body ends at `0x70e493`; padding and the start of the adjacent
function inside the authorized window are not used to extend its semantics.
Raw disassembly and range identity stay under `.scratch/neutral-dispatch-*`.
No provider retry, game execution, additional delegate inspection or build ran.
The preceding 25,133 bytes remain unchanged,
SHA-256 `8666d58b0afc38d8fabe83a83cd0054670c28948cc4b40155c9db0237d295629`.

### Verified argument and callback flow

The dispatcher takes five stack arguments. It loads argument 2 into ESI,
argument 3 (callback) into EDI, argument 4 (record) into EBX, and later argument
5 into ESI as a selector. The retained `0x71d4e0` wrapper supplies the known
`.w3i` pointer, `0x71fb90`, the local metadata record, and zero in arguments
2 through 5 respectively.

At `0x70e363..0x70e375`, argument 1 and a local buffer are passed with capacity
`0x104` to `0x433500`. If argument 2 is nonzero, `0x70e37c..0x70e389` passes
that pointer and the same local buffer to **`0x4d28b0`**. Thus `.w3i` demonstrably
participates in the buffer-preparation call. Appending/replacing an extension,
selecting `war3map.w3i`, or choosing an archive member remains inference until
that helper is inspected; the literal alone does not establish those operations.

For the wrapper's zero argument 5, `0x70e399` passes this buffer to `0x438e30`.
A nonzero return is required before `0x439570` receives the buffer, two local
output addresses and literals `1, 0`. The nonzero-selector alternative calls
`0x438e80` and `0x4395d0` instead. These delegates were not inspected, so their
file/archive lookup policy, output types and error details remain unproved.

After both zero-selector calls succeed, `0x70e416` requests `0x18` bytes through
`0x440610`. On a nonnull result the dispatcher constructs a wrapper with:

- `+0`: literal `0xe6c818` (a table address, inferred vtable from later calls).
- `+4`: local output from `[ebp-0x10c]`.
- `+8`: zero; `+0xc`: `0xffffffff`.
- `+0x10`: local output from `[ebp-0x110]`.
- `+0x14`: zero.

The prior four-byte reader uses these fields as buffer/base/limit/cursor. This
connects loader outputs to the source wrapper, without assigning names or
ownership contracts to the uninspected loader delegates.

At **`0x70e45b..0x70e45f`**, it explicitly calls the callback with three arguments:
**`callback(stream_wrapper, 0, record)`**. Therefore the known wrapper really
selects `0x71fb90`'s **zero branch**, which forwards record `+0` metadata to
`0x71e8d0`; this is no longer an unverified callback-mode hypothesis. Combining
this wrapper's zero initial cursor with the previously proven reader shows:
when the initial tag and both leading destination reads succeed, metadata `+4`
is sourced from **input byte offset 8**, the third four-byte word. This is a
physical input position, not an established field name or editor-version rule.

### Success, missing input and defaults

A null callback first calls `0x43eca0` with `0x57`, then returns zero. The meaning
of that reporting helper is not established. Zero returns from either selected
input delegate branch to `0x70e350`, returning zero **without invoking the
callback**. Those visible paths do not initialize metadata `+4`.

The allocation-failure path sets the stream-wrapper pointer to zero and still
calls the callback; no null-stream check is visible here. Whether allocation
can return null under this caller's actual conditions is not established.
It must not be replaced by invented successful data or a guessed safe default.

The callback's EAX is saved in EDI. A nonnull stream wrapper is then passed to
its table slot `+0x10` with argument `1`; the local loader output is passed to
`0x43a940`. Finally EDI is restored to EAX and returned. These are observed
cleanup calls; exact destructor/free contracts are unverified. The callback's
numeric return survives them, but its overall parse-success/error convention
still depends on the uninspected tail of `0x71e8d0`. The dispatcher neither
examines nor directly defaults metadata `+4`.

### Frozen boundary and next exact action

The callback-mode and input-wrapper connection is proved statically. Exact
input filename/member selection, semantic identity of the third input word,
caller allocation/default state and runtime map-context applicability remain
unproved. Existing writer/reader findings and the unsigned getter failure path
remain separate; `GetPlayerNeutralPassive` stays unsupported and original
campaign execution remains blocked.

Stop at the essential delegate: the next exact action is **`0x4d28b0`**, starting
with a bounded `0x4d28b0..0x4d29b0` window, to establish what the supplied `.w3i`
pointer does to the local buffer. This delegate was not queried here. After
that, the directly evidenced zero-selector loader boundary is `0x439570`, not
an image-wide search; inspecting it would require a separately authorized
bounded step. No Host implementation follows from this dispatcher inspection.


## Path helper: dot truncation and suffix delegate boundary

The only inspected half-open interval is **`0x4d28b0..0x4d29b0`**, 256 bytes,
GNU objdump 2.47 against exact Legacy executable SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
The range's original-byte SHA-256 is
`f351f5a2cf5ce7907153d68db6f01858d40caaa2d92b2dcbdefcab2cc12131bc`.
The target helper ends at `0x4d28fb`. Adjacent padding and the separate routine
starting `0x4d2900` lie inside the authorized window but do not establish this
helper's behavior. Raw output/identity remain local in
`.scratch/neutral-path-4d28b0.objdump` and `.scratch/neutral-path-range.json`.
No REA/provider retry, delegate expansion, input-loader inspection, game
execution, Host change or build ran. The preceding 30,504 bytes are preserved,
SHA-256 `b651632673f76fd875e547acde828f738e47a042966ad0f9ea0931b39947f7d1`.

### Directly verified mutations and argument flow

The first argument, the dispatcher's local path buffer, is held in ESI.
`0x4d28b8` calls **`0x433910(buffer)`** and subtracts one from its EAX result.
If that index is negative, it skips scanning. Otherwise it examines bytes at
`buffer + index` while decrementing the index:

- Byte `0x2e` (dot): `0x4d28de` writes zero at that byte, then exits the scan.
- Byte `0x5c` or `0x2f` (backslash or slash): exits without that write.
- Other bytes: decrement and continue while the index is nonnegative.

Thus, for the scanned region, a dot encountered before a separator truncates
the buffer at that dot. It does not walk past a separator to remove an earlier
directory-component dot. The relationship of the initial index to the true
string end depends on the uninspected `0x433910` contract.

The second argument is pushed at `0x4d28e2`. The helper calls `0x433910(buffer)`
again, then adds the buffer pointer to the returned value. That computed pointer
is pushed as argument 1 to **`0x433570(computed_pointer, second_argument)`**.
The retained dispatcher supplies the `.w3i` literal as this second argument.
This proves how the literal reaches the final operation and where its target
pointer comes from. It does not, without inspecting the callee, prove that the
operation copies the literal or terminates the resulting string.

**Inference, explicitly bounded:** the backward dot/separator scan, truncation,
second length-like call and suffix-like final call strongly support replacing
a final-component extension, or appending the supplied suffix when no dot was
found. Treating `0x433910` as strlen and `0x433570` as a string copy is still a
contract inference, not a new semantic proof. Consequently no exact resulting
filename or archive member identity is assigned here.

### Return and failure coverage

There is no visible Boolean check after either `0x433910` call or the final
`0x433570` call, and no separate failure return branch in this complete helper.
It returns with EAX left by `0x433570`. The previously inspected dispatcher
ignores that result and continues into its input delegates with the same local
buffer. Negative scan-index handling is not proof of a reported error.

No destination-capacity argument is passed in the visible call to `0x433570`;
its handling of nulls, truncation, termination or capacity cannot be inferred
from this caller. The dispatcher has a `0x104`-capacity preparation call, but
that alone does not establish the later delegate's bounds behavior. Missing
input and metadata defaults remain at the previously recorded loader/parser
boundaries. This routine itself does not access metadata `+4`.

### Frozen next boundary

The local dot-truncation behavior and `.w3i` argument connection are established.
Exact suffix-copy semantics and therefore the final input name remain blocked
on essential delegated contracts. Stop without expansion: next inspect
**`0x433910`**, starting with the bounded `0x433910..0x433990` interval, to verify
the scan index/end-position contract. The subsequent separate boundary is
**`0x433570`** for the final operation; neither delegate was inspected here.
`0x439570` was not queried. Field-name identity, runtime context and original
campaign execution remain unproved; `GetPlayerNeutralPassive` stays unsupported.


## String contracts: bounded length window and complete copy wrapper

GNU objdump 2.47 inspected only these half-open ranges from the exact Legacy
executable SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`:

| Range | Bytes | SHA-256 of original range |
| --- | ---: | --- |
| `0x433910..0x433990` | 128 | `78c1d8971115401f6af98a40a1d792be7185a32e6a8a8b41705124cd9dc502b1` |
| `0x433570..0x433670` | 256 | `7b379852da691ad432d510e6eb31457599f0d1465f85ce3e03b6c67f427aac48` |

Raw output and hashes are local `.scratch/neutral-string-*` artifacts.
The preceding 34,741 note bytes are preserved, SHA-256
`8d4a184de0a422c3404e8f29470899514f2a37b067e89b6008ada4940a456084`.
No out-of-range delegate, loader, provider or runtime execution was inspected.

### Length behavior proved within the window

`0x433910` checks its pointer. A null pointer calls `0x43eca0(0x57)` and returns
zero; the reporting helper's wider contract remains unknown. For a nonnull
pointer it scans initial unaligned bytes for NUL, then reads aligned DWORDs
and tests their individual bytes when its candidate-zero expression fires.
The complete return path for a zero fourth byte (`0x43397f..0x433988`) returns
the byte pointer distance from the original argument, excluding the NUL.
This establishes byte-string-length behavior for that visible termination case.
The candidate expression can also resume scanning and write a global word at
`0x112e58c`; this window does not prove a side-effect-free implementation.

Coverage is incomplete: the first-byte and alignment-byte exits branch to
`0x4339a1`/`0x4339a4`, and the second-byte exit to `0x433995`, beyond the authorized
stop address. The third-byte return sequence begins at `0x433989` but is cut at
`0x433990`. These tails were not read. Therefore the entire strlen-like contract
for every termination alignment is not silently promoted to complete proof.

### Copy wrapper: exact source count and terminator

The complete `0x433570..0x4335c8` body checks both destination (argument 1) and
source (argument 2). Either null calls `0x43eca0(0x57)` and returns zero without
the visible copy/terminator path. For nonnull pointers, its own byte-by-byte
loop counts source bytes up to but excluding NUL; this counting loop does not
use the incomplete length helper.

It selects `min(source_length, 0x7ffffffe)` with an unsigned comparison, then
calls **`0xa5ad40(destination, source, selected_count)`** at `0x4335b6`. After
that call it explicitly writes a zero byte at destination plus selected count
(`0x4335be`) and returns the original source length (`0x4335c2`). Thus the
wrapper's count, terminator placement and return convention are established;
the count limit is not a destination-capacity check. The result is not a
Boolean success code. The adjacent routines in the inspected window are not
used as substitutes for the uninspected delegate.

For the already identified `.w3i` literal, this source scan produces count four
and places the final terminator four bytes after the computed destination on
normal return. The instructions that transfer those four nonzero bytes are
inside **`0xa5ad40`**, outside both authorized ranges. Its memcpy-like argument
shape is evidence of a candidate contract, not proof of the copied bytes.

### Output interpretation and stopped boundary

Combining the prior helper with these observations yields a specific conditional
transformation: the last dot found before a slash/backslash is replaced by NUL;
a length-like result selects the end of the truncated buffer; a four-byte
transfer is requested from the `.w3i` literal there; an explicit NUL follows.
**If** the outstanding length tails return the same byte count and `0xa5ad40`
performs the requested transfer, the output is the original path prefix with
its final-component extension replaced by `.w3i`, or that suffix appended when
no dot was found. This conditional output is substantially narrower than an
unexplained call list, but is not yet claimed as fully proved native output.

Stop at the essential transfer boundary **`0xa5ad40`**; a next bounded entry
window would be `0xa5ad40..0xa5ae40`. The other exact coverage gap is the short
**`0x433990..0x4339b0`** termination-tail window. Neither was inspected here.
There is no expansion to `0x439570`. Even closing these string contracts would
still leave archive/input-loader identity, leading metadata defaults, semantic
field identity and runtime context unproved. `GetPlayerNeutralPassive` remains
unsupported; original campaign execution remains blocked.


## Length tails and transfer-entry branch coverage

GNU objdump 2.47, exact Legacy SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Only the following half-open ranges were inspected:

- `0x433990..0x4339b0`: 32 bytes, SHA-256 `da9f3a11226fba520cd91fa5c46ed03b3f9700a833ca156b6838b75499db3d52`.
- `0xa5ad40..0xa5ae40`: 256 bytes, SHA-256 `7194079879fccbeed2d8b506a146b6e5ed1fbf0a11b143a1e7f5901ee92c51e0`.

The preceding 39,312 bytes remain unchanged, SHA-256
`a006a12cdfd23be55e338ae1016443656ca2682efcf7970b6cb4928739774f26`.
Raw ranges/disassembly remain in `.scratch/neutral-tail-transfer-*`,
`.scratch/neutral-length-tails.objdump` and `.scratch/neutral-transfer-entry.objdump`.

The formerly cut `0x433910` return tails now close its observed byte-length
contract: the third-byte tail returns the already adjusted pointer distance;
`0x433995` subtracts three from the advanced DWORD pointer and then the original
pointer; `0x4339a1` restores the DWORD start for a first-byte zero; the alignment
byte loop joins `0x4339a4`. Each returns the position of the NUL excluding it.
No external delegate is needed for these nonnull string-counting paths.

The transfer entry loads destination/source/count into EDI/ESI/ECX. If unsigned
`destination > source` and `destination < source + count`, it branches to
`0xa5aff4`, outside the window. Otherwise counts below `0x20` branch at
`0xa5ad63` to **`0xa5b23b`**. The `.w3i` count is four and therefore takes this
small-count branch under the separate non-overlap precondition. That branch's
copy instructions were not yet inspected. A visible large-count conditional
path uses `rep movsb` and returns destination, but it does not prove the
four-byte case. No standard-library identity is assigned from its address or
optimization style; these are original instruction facts, not symbol evidence.

The stop boundary is `0xa5b23b`, not the large-count body continuation. Exact
final suffix bytes remain unproved until that applicable branch is inspected.
No loader/provider/runtime execution ran; metadata semantics and defaults remain
independent. `GetPlayerNeutralPassive` remains unsupported.


## Count-four transfer: extension output established statically

GNU objdump 2.47 inspected only **`0xa5b23b..0xa5b33b`**, half-open, 256 bytes,
from exact Legacy executable SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
The original range SHA-256 is
`3ac23b1f476bc3711308a1a80ea35591404533c3a88b79c3b24ee91cf2517ec0`.
Raw output/identity are `.scratch/neutral-count4-entry.objdump` and
`.scratch/neutral-count4-range.json`. The preceding 41,481 note bytes are
preserved, SHA-256
`d8c1c3550e1e89bd252abc38007a6d76f444634b969ca6fa588195ae4eb1f74b`.
No jump table, extra target block or external delegate inspection was necessary.
No loader, provider retry, game execution or implementation change ran.

### Exact applicable branch

The prior transfer entry selects this branch for count four when its separate
backward-overlap condition is false. At `0xa5b23b`, masking ECX with `0x1f`
leaves four. EAX saves that count; shifting ECX right by two gives one.
**`0xa5b247` loads exactly one DWORD from `[ESI]`; `0xa5b249` stores that DWORD
at `[EDI]`.** Source and destination advance four bytes, ECX decrements to zero,
and the loop exits. `0xa5b256..0xa5b25b` computes the saved count modulo four,
which is zero, so the byte-tail loop is skipped. There are no calls on this
count-four path.

At `0xa5b270..0xa5b276`, EAX is restored from the original destination stack
argument, saved ESI/EDI are popped, and the routine returns. Thus this specific
branch copies exactly four bytes without conversion and returns the original
destination pointer. The applicable terminal return is fully inside the window.
Other routines/instructions after that return are not evidence for unrelated
count cases. No general memcpy/memmove or named standard-library identity is
claimed; the copy behavior follows directly from the observed load/store.

### Composed path output and return convention

For a valid, NUL-terminated path buffer with sufficient destination storage,
under the non-overlap precondition, the already observed `0x4d28b0` operation
is now established rather than merely a suffix-copy hypothesis:

1. Compute the path's byte length and scan backward from its last character.
2. Stop at the first slash/backslash, or replace the first dot encountered
   before such a separator with NUL. This selects the last dot of the final
   path component, not an earlier directory-component dot.
3. Recompute the resulting byte length and transfer the four literal bytes
   `.w3i` to that end position using the proved count-four branch.
4. The complete `0x433570` wrapper writes the NUL byte immediately after those
   four bytes.

Consequently the final-component extension is **replaced by `.w3i`** when that
component contains a dot; otherwise **`.w3i` is appended** to the existing
buffer. No basename or directory is invented by this transformation. The
copy wrapper returns source length four, overriding the transfer routine's
pointer return; the path helper forwards that numeric result. The dispatcher
ignores it. Four is a source byte count, not proof of a Boolean success contract.

### Separate limitations and remaining campaign blocker

The transfer entry's alternate overlap condition remains independently recorded:
unsigned destination strictly above source and below source plus count branches
to `0xa5aff4`. That path was not inspected. Neither large-count behavior nor
invalid-pointer/buffer-capacity safety is proved by this small-count case.
No live execution or symbol-based standard-library identification is implied.

This closes the bounded string-contract question. It does **not** identify the
actual incoming path's basename, establish archive-versus-filesystem lookup,
prove which supplied map/context is selected at runtime, or name metadata `+4`.
The zero-selector dispatcher's input-producing delegate **`0x439570`** remains
uninspected, as requested. Its exact connection to the transformed path and
loaded source bytes is the next separate input-identity boundary if authorized.
Leading-field defaults, overall parser failure translation and native context
lifetime remain open. `GetPlayerNeutralPassive` stays unsupported and original
campaign execution remains blocked.


## Input-buffer core: transformed path, outputs and unresolved selection

Only **`0x439570..0x439770`**, half-open, 512 bytes, was inspected with GNU
objdump 2.47 against exact Legacy executable SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Original range SHA-256:
`119c99fd9a85d28df94d94f068601e52b6c3550577a48a94b6f7df4b6e879a65`.
The complete wrapper ends at `0x4395c0`; the complete core `0x439600` ends at
`0x43973d`. Adjacent alternate-wrapper/start-of-next-function bytes are not
used to extend this call path. Scratch-only artifacts are
`neutral-input-439570.objdump`, `neutral-input-range.json` and the preserved
prefix. The preceding 45,713 note bytes are unchanged, SHA-256
`85dc8ba9e183ae794b58fffd2a53db233cecd431d0b66d55d59b249c74ee77b9`.
No outside delegate, import symbol, provider or live runtime was queried.

### Path and mode connection

`0x439570` computes a mode word from global `0x112ed28`: its low two bits,
except that if signed global `0x10db4d8 <= 0` and `0x112ed28 == 0`, bit zero
is forced on. These are instruction-level conditions; archive preference,
edition, mount state and the semantic names of those globals are unproved.

The wrapper passes eight arguments to `0x439600`: zero, its caller's path,
buffer-output pointer, count-output pointer, extra-count argument, the computed
mode, the caller's last argument, and zero. For the retained dispatcher call,
these are `(0, transformed_path, &buffer, &count, 1, mode, 0, 0)`.
Thus the verified `.w3i` path reaches this core without another visible path
transformation in the wrapper.

At `0x439635..0x439647`, the core initializes a local handle output to zero and
calls **`0x439c20(0, transformed_path, mode, &handle)`**. The path pointer saved
in ECX before the local stack-slot overwrite is explicitly passed. This is
the unresolved selection boundary: its call shape suggests opening/resolving
an input, but archive/filesystem policy and the resulting handle identity
cannot be proved from this caller alone.

### Buffer/count producer connection

On a nonzero return from `0x439c20`, the core obtains a count via two visible
branches. If handle `+0x110` differs from `0xffffffff`, it calls import slot
`0xe66198` with that word and zero, using EAX as the count. Otherwise it loads
handle `+0x118`, then that object's `+8`. Other import slots at `0xe6617c` and
`0xe66180` bracket this selection; synchronization is an inference, not a
resolved import-name proof. The branch shapes alone do not establish a
filesystem-versus-archive classification.

The count plus the extra-count argument is passed to `0x440610`. Its returned
pointer is then passed, with handle and count, to **`0x43a1d0`**. For this
caller the complete visible argument list is `(handle, buffer, count, 0, 0, 0, 0)`.
Neither delegate's full contract is inspected here; in particular their byte
source, allocation guarantees and transfer behavior are not replaced by names.

On a nonzero return from this handle-to-buffer operation, the core requests
`0xa5b900(buffer + count, 0, extra_count)` when extra count is nonzero, then
stores the pointer through the caller's buffer output and the count through
its optional count output. The dispatcher's extra count is one; that extra
storage is **not** added to the returned count. This establishes exactly where
the dispatcher stream wrapper's buffer `+4` and limit `+0x10` come from: the
core's produced pointer and count, after the handle-to-buffer success gate.
The fill helper's bytes were not inspected and are not assumed as new proof.

### Actual result/default boundaries

A missing buffer-output pointer enters the reporting-helper/zero-return path.
With a nonnull output pointer, the core clears it before checking the path.
A null path then reports and returns zero without reaching the optional
count-output clear. For valid path/output arguments, the optional count output
is cleared before the selection call. Accordingly not every invalid-argument
path initializes every output.

A zero selection return skips allocation/transfer. A zero handle-to-buffer
return enters the cleanup path, which passes a nonnull allocated pointer to
`0x440c70`; outputs remain zero. Successful transfer publishes outputs only
at `0x4396e9..0x4396f5`. A nonnull handle is passed to `0x437fc0` before the
final return. Exact allocation/cleanup contracts remain delegated.

At `0x439707..0x439712`, EAX is zeroed and AL becomes one iff the caller's
buffer output is nonzero. Thus the visible core's final result is **nonnull
published buffer**, not an independently checked byte-count/parser-success
code. No allocation-null guard is visible before the transfer delegate.
All such outcomes concern loading outputs; this routine never initializes
metadata `+4`. Its zero outputs are not a default for that metadata field.

### Frozen blocker and next essential delegate

The transformed `.w3i` path's connection to the dispatcher buffer/count outputs
is established through the core's explicit gates and stores. Actual bytes and
archive/filesystem selection remain unproved. Stop at **`0x439c20`**, whose
role is selection/resolution of the path into the handle used here. A next
bounded entry window would be `0x439c20..0x439e20`; it was not inspected in this
slice. There is no image-wide search or automatic expansion to other delegates.
The unknown input basename, metadata field meaning/defaults, parser failure
translation and native map-context lifetime remain separate. An executable
campaign Host is not supplied by this bounded identity trace;
`GetPlayerNeutralPassive` remains unsupported and campaign runtime remains blocked.


## Source selector and one resolved PE import

GNU objdump 2.47 inspected only **`0x439c20..0x439e20`**, half-open, 512 bytes,
from exact Legacy executable SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Original code-range SHA-256:
`f40e4b38852856670e0eea21d019b9238a0597981f5005127b95be76bfaa7c6e`.
The routine continues outside the window; its shared tail is not inspected.
Raw disassembly is `.scratch/neutral-selection-439c20.objdump`.
The preceding 51,377 note bytes are unchanged, SHA-256
`54e1883579a7bf60e4ef541426db92337e93ab31c018a7155fd79a3b87497d81`.

### Single-entry import identity: metadata, not a guessed API

A bounded PE32 descriptor/thunk/name traversal resolved only IAT VA
**`0xe66198`** (RVA `0xa66198`). Its descriptor is RVA `0xcd7170`,
OriginalFirstThunk RVA `0xcd73c0`, FirstThunk RVA `0xa66160`, index **14**.
The corresponding lookup thunk at RVA `0xcd73f8` contains RVA `0xcd83b0`;
the ordinal flag is clear. The import-by-name record has hint **496** and name
**`GetFileSize`**, with DLL **`KERNEL32.dll`**. Descriptor timestamp and
forwarder-chain fields are zero. Hint 496 is not an ordinal import identity.
This binds the earlier call slot to shipped PE metadata; it does not observe
which runtime DLL address the operating system resolves or any file access.
No other import entry was named or queried as a target.

| Selected metadata | RVA | Bytes | Original-byte SHA-256 |
| --- | --- | ---: | --- |
| descriptor | `0xcd7170` | 20 | `89d44d243eef4bfa6681ae1ebb313801672746ae1bfb8e0b9228e55a60186816` |
| DLL | `0xcd8c7c` | 13 | `f8efc1f27ef6c525f7fd20dcb8d65e8197e97410eced20db4d323dfbf230a2a4` |
| lookupThunk | `0xcd73f8` | 4 | `c21f7891bb53edfb8d94a9e2bc50e56f6b9b4a1d1b476126b1a0197833041dc5` |
| iatThunk | `0xa66198` | 4 | `c21f7891bb53edfb8d94a9e2bc50e56f6b9b4a1d1b476126b1a0197833041dc5` |
| importByName | `0xcd83b0` | 14 | `599ea3886cf2daba46b74aee3a181bcc7195f09e0396a4f784346c1be0969ed8` |

The structural resolver validates file/RVA bounds and the candidate thunk's
terminator before its selected index. It does not perform an image byte search.
Metadata provenance is `.scratch/neutral-single-import.json`; its temporary
resolver is `.scratch/resolve-neutral-single-import.py`.

### Supplied path and essential classification boundary

The selector receives `(0, transformed_path, mode, &handle)` from the previously
proved loading core. It clears the optional handle output, then tests that the
path's first byte is nonzero and the output pointer exists. The visible invalid
path reports with `0x43eca0(0x57)` and returns zero. It dereferences the path
before that combined validity test, so a null path pointer is not visibly
handled here; the preceding core already rejects null path arguments.

Following an import-slot call and a conditional `0x435f20(0x100001)` call when
global `0x112ed34` is zero, `0x439cb8` invokes **`0x435cb0`** with the first
argument, supplied path, mode, two local output addresses and a local path
buffer. For this caller the first argument is zero. The classifier's EAX
selects branches: zero jumps to `0x439f5b`; two enters the following opener
path; any other nonzero value enters the alternative path at `0x439d7f`.
Those result meanings, path rewriting and selection policy are delegated;
no archive-versus-filesystem category is inferred merely from the numbers.

### Visible result-two opener and handle field connection

The result-two branch passes the classifier's local path buffer and numeric
arguments to **`0x4380c0`** at `0x439ceb`. That return is compared with
`0xffffffff`; equality sets EBX to zero and jumps to the uninspected tail.
On the other branch, when mode mask `0x4` is clear, a wrapper-producing call at
`0x436bb0` is used. The original supplied path is passed to `0x433500` for
wrapper `+8`; the classifier's path buffer is passed to `0x4335d0`, whose return
is stored at `+0x10c`. The opener return is stored at **wrapper `+0x110`**,
other words are assigned at `+0x144` and `+0x14c`, and the wrapper pointer is
published through the handle output. EBX becomes two before jumping to the
shared tail. Wrapper allocation, string helper, opener and final-return
contracts were not inspected in this slice.

This precisely connects the earlier loading core's `+0x110` word to an opener
return, and its later import-slot call to the named `GetFileSize` import. It
supports a file-handle role for that branch's word; it does **not** prove the
opener implementation, classify all source modes, or establish actual runtime
input. The loading core's `+0x110 == 0xffffffff` branch and a possible API result
with that same numeric word must not be conflated: no explicit check of the
`GetFileSize` return is visible in the earlier core.

Mode mask `0x4` also has a direct-output path here, and an alternative branch
calls the same opener using another local object's `+8` path before a distinct
import-slot call. The retained `0x439570` mode derivation produces only low-two-bit
values, so mask `0x4` is clear for this particular call chain. The applicable
non-two branch reaches `0x439de3`, supplies colon/backslash/slash bytes to
`0x433410`, and continues beyond `0x439e20`. Character-search semantics and
archive input identity are not assigned from that partial path.

### Stopped failure/selection boundary

Shared cleanup/result tail **`0x439f5b`** lies outside coverage. No successful
final return is proved for the visible publication branch solely from EBX's
assigned value. Likewise a classification/opener failure's complete cleanup,
reporting and returned code remain beyond this window. Visible output clears
and sentinel comparisons are retained without inventing metadata defaults.

Stop at **`0x435cb0`**, the essential source-classification delegate: it consumes
the supplied path/mode and produces the result and local data selecting the
opener branch. A next bounded entry window would be `0x435cb0..0x435eb0` if
separately authorized. No delegate was expanded here. Archive/filesystem
selection, actual input basename, metadata `+4` semantics/defaults and native
map-context lifetime remain unproved. `GetPlayerNeutralPassive` remains
unsupported and original campaign execution remains blocked.


## Classifier: path candidate versus indexed-record outputs

GNU objdump 2.47 inspected only **`0x435cb0..0x435eb0`**, half-open, 512 bytes,
from exact Legacy executable SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Original range SHA-256:
`82ecd00be1b7d6fadae7b9b89c98959de94849657a9d573baefe63c438b177af`.
Artifacts remain local: `.scratch/neutral-classifier-435cb0.objdump` and
`.scratch/neutral-classifier-range.json`. The prior published note prefix is
57,627 bytes, unchanged SHA-256
`0608fd92d34e9f86ebbdce982b14b697cbd55ddf194b9fff67a9379caa600560`.
No delegate, literal contents, provider or runtime was queried outside this
window. The indexed-success epilogue extends beyond its stop address.

### Actual arguments, initial outputs and candidates

The previously proved selector passes `(0, supplied_path, mode, &A, &B, path_out)`.
At entry this classifier clears A and B if their pointers exist, but does not
initialize `path_out`. It uses global `0x10db4d8` only when its signed value is
positive; otherwise it treats that value as zero. A nonzero first argument
restricts processing to an equal current value. Current `+0x258 > 0` skips its
normal processing. Following a failed candidate, current `+4` supplies the next
value, with signed nonpositive values ending traversal. These are list-shaped
instruction relationships, not a proved mount order or source-priority policy.

For a nonnull current value, a nonzero current `+0x150` redirects the owner
pointer used by subsequent lookup; otherwise the owner is current itself.
If the string-like area at current `+0x154` begins with a nonzero byte, three
`0x4334d0` calls receive that area, uninspected literal `0xe6a10c`, and the
original path, using one local buffer and a remaining-size word initialized
to `0x104`. The resulting local buffer becomes the candidate path. Otherwise
the supplied path remains the candidate. Concatenation/prefix and separator
semantics of that delegate/literal are not established here.

### Complete result-two branch and its input producer

When `mode & 3` is nonzero, the classifier considers **`0x4356d0`** only if
owner is null or owner `+0x114` equals zero. At `0x435dbe`, it passes
**`(candidate_path, mode, &temporary_path)`** to that delegate. A nonzero return
branches to `0x435e29`. There it passes the temporary path, caller `path_out`
and capacity `0x104` to `0x433500` if that output pointer exists, then sets EAX
to **two** and returns through the fully observed `0x435e5d` return.

This establishes the producer and forwarding connection for the selector's
result-two opener branch: its local path buffer comes from `0x4356d0` through
that forwarding helper. It does not establish whether the producer checks a
filesystem file, resolves another virtual source, or synthesizes the path.
The number two is not a source-category proof.

### Indexed-record branch and bounded return coverage

If the result-two producer is skipped or returns zero, a nonnull current
value allows **`0x437040(owner, candidate_path, global_word, global_byte)`**,
using the word at `0x112ed3c` and byte at `0x112ed3e`. Return `0xffffffff`
continues traversal. Any other return enters the record path at `0x435e5e`.

The instructions compute a record from owner `+0x140`, using a 16-byte
lookup stride and the lookup record's `+0xc` word as an index with a
`0x2c`-byte stride into owner `+0x13c`. This is physical indexed-table evidence;
hash/archive/member identity and field types remain unproved. A signed
nonnegative word at the computed record `+0xc` reports `0x3ee` and joins the
zero-return path. With `mode & 4` set, an additional `0x1ff00` mask test can
join the same error path. The retained caller's low-two-bit mode keeps that
additional mode mask clear.

On the remaining path, owner is stored through A and the computed record
pointer through B when those output pointers exist. At `0x435ea8`, EAX is set
to **one**. The window stops during its epilogue at `0x435eaf`; a complete final
return for this branch is not yet observed. The output stores and assigned
one are proved independently of that missing tail.

### Failure, source identity and essential stop boundary

When traversal has no further positive current value, the complete observed
failure path calls `0x43eca0(2)` and returns zero. The invalid-record path uses
reporting value `0x3ee` before joining that return. Reporting values are not
assigned platform error names. A/B begin as zero and only the indexed-success
path publishes them in this window; path output is separate and is only
forwarded on result two. None of these clears/defaults target metadata `+4`.

The earlier PE metadata binding **`KERNEL32.dll!GetFileSize`** remains a proved
named import for the downstream handle `+0x110` branch. It does not turn this
classifier's numeric returns, table shapes or globals into proved filesystem,
archive, edition or priority categories.

Stop at **`0x4356d0`**, the essential producer for the complete result-two
path: candidate path and mode go in, a temporary path and nonzero/zero result
control the downstream opener path. A next bounded window would be
`0x4356d0..0x4358d0` if separately authorized. The separate indexed producer is
`0x437040`, and the short epilogue boundary is `0x435eb0`; neither is expanded.
Actual input source identity remains blocked on these contracts, not on a
missing call-address inventory. Original path basename, metadata semantics,
defaults and native context lifetime remain unproved. `GetPlayerNeutralPassive`
stays unsupported; original campaign execution remains blocked.


## Result-two path producer: output builder and acceptance gate

Only **`0x4356d0..0x4358d0`**, half-open, 512 bytes, was inspected with GNU
objdump 2.47 against exact Legacy executable SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Original range SHA-256:
`70a23e83452205130d27a21063ca6b19f28c8820c8d2b7bf2066646987a05e10`.
The complete producer ends at `0x435749`. The unrelated routine at `0x435750`
and partial builder at `0x435850` fall within the authorized window; the former
is not used to infer this producer's semantics. Artifacts are local
`.scratch/neutral-path-producer-4356d0.objdump` and
`.scratch/neutral-path-producer-range.json`. The preceding 63,248 note bytes
remain unchanged, SHA-256
`fcd464575c545b356ee8d8529c6d60f1729bc5df37c3f349d15cef27814edeb1`.
No external delegate/import identity, provider or runtime was queried.

### Complete producer contract visible at its caller

Arguments are candidate path, mode, and output path pointer. If `mode & 3` is
zero, EAX remains zero and the routine returns without visible output writes.
The producer does not clear its output at entry.

When `mode & 2` is set, `0x4356ef` calls
**`0x435850(candidate_path, output_path, 1)`**. The returned EAX is not used as
an acceptance result: immediately afterward it calls **`0x435660(output_path)`**.
A nonzero result exits with that result unchanged. A zero result permits the
next branch only if `mode & 1` is set; otherwise zero is returned.

The `mode & 1` branch first calls **`0x436fa0(candidate_path)`**. On a nonzero
return it calls `0x435850(candidate_path, output_path, 0)`, then
`0x435660(output_path)` and returns the latter's result unchanged. On a zero
return it passes output path, candidate path and capacity `0x104` to `0x433500`,
then calls `0x435660(output_path)` and returns that result unchanged.

Thus the classifier's complete result-two path is now tied to a concrete
acceptance gate: **a nonzero `0x435660(output_path)` result**, following one
of the specified builder/forwarding branches. It is not a direct success
result from the builder, and the producer itself does not return category two;
its caller converts any nonzero producer result into two. These instruction
branches do not prove numeric source priority, edition policy or a file/archive
category for either mode mask.

### Partial builder coverage and actual output limit

The authorized window also contains the beginning of `0x435850`. It uses a
cached global buffer at `0x112ec10`. If its first byte is zero, an **unresolved**
import slot `0xe66160` receives `(0, global_buffer, 0x104)`; its EAX is passed
to `0x4394a0`. Further visible calls supply the global buffer and slash byte to
`0x4333d0`, optionally write a zero byte at that return pointer, and pass the
buffer, literal address `0xe695c4`, and `0x104` to `0x433140`.

At `0x4358b9` the builder tests its third argument. The nonzero branch supplies
the candidate and a slash byte to `0x433410`; coverage ends during the instruction
at **`0x4358ce`**. This does not establish the final output path, basename
handling, global-buffer origin or imported API name. The actual builder return
and output mutation contract remain incomplete. The previously proved
`KERNEL32.dll!GetFileSize` binding is a different slot (`0xe66198`) and is not
reused to name this unresolved import.

### Failure and essential boundary

The complete producer contains no separate reporting/error-code helper and no
validation result check on its builder/forwarding calls. A failed first gate
may still have changed output before another branch runs; a final zero return
does not prove that output was cleared or untouched. Only its zero/nonzero
acceptance behavior is established. These path outputs are not defaults for
metadata `+4`.

Stop at **`0x435660`**, the essential acceptance delegate: it consumes the
constructed output path and supplies the result that determines the classifier's
result-two branch. A bounded next entry window would be `0x435660..0x4356d0`.
The separate builder coverage gap starts at `0x4358ce`; any continuation must
start there or earlier at an instruction boundary, not at an arbitrary split
byte. Neither delegate was expanded beyond this authorized range.
Actual loaded input identity, metadata field semantics/defaults and native
context lifetime remain unproved. `GetPlayerNeutralPassive` remains unsupported;
original campaign runtime remains blocked.


## Path acceptance: named attribute import and exact failure result

Only **`0x435660..0x4356d0`**, half-open, 112 bytes, was inspected with GNU
objdump 2.47 against exact Legacy executable SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Original range SHA-256:
`5bba51ed588f5f0c80422cddde7906d86f239756c7794364d6928fd08ba25e6e`.
The complete body ends at `0x4356ca`. The preceding 67,704 note bytes are
unchanged, SHA-256
`c2367bf6e61ea69b44a5f086501be4dcab922c8d655c61f1ecf4c90d94fa1606`.
Raw evidence is `.scratch/neutral-acceptance-435660.objdump` and
`.scratch/neutral-acceptance-import.json`. No builder, predicate, provider or
native execution was expanded. Only one import slot was resolved by PE metadata.

### Proved single-entry import binding

IAT VA **`0xe661c0`**, RVA `0xa661c0`, is descriptor RVA `0xcd7170`,
OriginalFirstThunk RVA `0xcd73c0`, FirstThunk RVA `0xa66160`, index **24**.
Lookup thunk RVA `0xcd7420` refers to import-by-name RVA `0xcd8452`:
**`KERNEL32.dll!GetFileAttributesW`**, hint **490**, ordinal flag clear.
Descriptor timestamp/forwarder-chain values are zero. The hint is not an ordinal.
The descriptor/thunk/name traversal was bounded by PE structure and terminators;
there was no byte search or runtime import resolution.

| Selected metadata | RVA | Bytes | Original-byte SHA-256 |
| --- | --- | ---: | --- |
| descriptor | `0xcd7170` | 20 | `89d44d243eef4bfa6681ae1ebb313801672746ae1bfb8e0b9228e55a60186816` |
| DLL | `0xcd8c7c` | 13 | `f8efc1f27ef6c525f7fd20dcb8d65e8197e97410eced20db4d323dfbf230a2a4` |
| lookupThunk | `0xcd7420` | 4 | `938fdfc4a639468cc461073ab30f9aba38d19ff6ba6ae9056d8c7b76b116923d` |
| iatThunk | `0xa661c0` | 4 | `938fdfc4a639468cc461073ab30f9aba38d19ff6ba6ae9056d8c7b76b116923d` |
| importByName | `0xcd8452` | 21 | `621cc8168ab700fe22bb4039387439b31927299f58019d145dc9de7b77b5b683` |

### Call path and raw result contract

For a nonnull path, `0x43569e` first calls the **unresolved** import slot
`0xe661d0` with six arguments `(0xfde9, 0, path, 0xffffffff, local_buffer, 0x104)`.
The argument pattern suggests a conversion operation, but no API name, encoding
or full output contract is assigned from that shape. This second import was
not resolved under the single-entry authorization.

If that first import returns nonzero, `0x4356af` calls the named
`GetFileAttributesW` import with the local buffer pointer. This establishes an
actual static connection to the Windows path-attribute API for that path,
stronger than an inferred generic source-category label. It does not establish
the builder's exact path, successful runtime file access, later opener API or
archive priority.

The shared return sequence is exactly **`(~(EAX >> 4)) & 1`**, logical shift.
Thus a returned word with mask `0x10` set produces zero; that mask clear
produces one. A returned `0xffffffff` produces zero by the same bit test;
there is no separate sentinel comparison or error-detail query here. No extra
attribute-bit selection or source-category code is implemented in this gate.

### Null and first-import failures are not the same sentinel path

A null input calls `0x43eca0(0x57)`, then explicitly sets EAX to zero if that
helper returns normally. A zero return from the first import also branches to
that explicit EAX-zero instruction. **Both then execute the same shared
inversion and return one**, not zero. Whether the reporting helper can terminate
or raise before returning remains outside coverage; that qualification applies
to the null-input path. The first-import-zero path itself has no reporting call.

Consequently this gate must not be silently reconstructed as a conventional
"conversion or path query failure returns false" check. On the observed normal
return path it accepts the first-import-zero case and does not call
`GetFileAttributesW` in that case. There is no output parameter and no metadata
`+4` initialization in this routine. The caller can proceed with its previously
built output path after such a nonzero gate result; that is distinct from proof
that the path was valid or existed.

### Established identity and remaining exact boundary

The classifier's result-two path now has a named Windows path-attribute query
connection and a fully observed local result formula, including its unusual
pre-query failure path. This still does not prove the actual loaded source or
identify the incoming basename/context. The named downstream `GetFileSize`
import is separately proved; combining two named imports does not establish
uninspected opener/loader semantics or archive ordering.

The remaining import-contract boundary is **IAT `0xe661d0`**, requiring a
separately authorized single-entry PE resolution if needed; no function address
or API name is guessed for it. The builder's missing continuation beginning at
instruction `0x4358ce` remains separate. Neither it nor `0x436fa0` was expanded.
Field meaning/defaults, source lifetime and original campaign runtime remain
unproved. `GetPlayerNeutralPassive` stays unsupported.


## Single conversion import: identity and recorded argument contract

This slice performs only bounded PE descriptor/thunk/import-by-name traversal
for **IAT VA `0xe661d0`** on the exact Legacy executable SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
No executable code range, builder, predicate, provider or native runtime is
newly inspected. The preceding 72,745 note bytes are preserved, SHA-256
`b7b909504bd3c1ced57eab099fe11ee55d6d39325e0d1b3a4f407e71a79f16b7`.
The structural resolver and result stay local in
`.scratch/resolve-neutral-conversion-import.py` and
`.scratch/neutral-conversion-import.json`.

### Named import proved by shipped metadata

IAT RVA `0xa661d0` belongs to descriptor RVA `0xcd7170`, with
OriginalFirstThunk RVA `0xcd73c0` and FirstThunk RVA `0xa66160`, index **28**.
The selected lookup thunk at RVA `0xcd7430` contains import-by-name RVA
`0xcd8498`. Its ordinal flag is clear; hint **871** and name
**`MultiByteToWideChar`** belong to DLL **`KERNEL32.dll`**. Timestamp and
forwarder-chain fields are zero. Hint 871 is not an ordinal import. This
identity is derived from PE metadata, not guessed from the six-argument shape.
No other entry is resolved in this step.

| Selected metadata | RVA | Bytes | Original-byte SHA-256 |
| --- | --- | ---: | --- |
| descriptor | `0xcd7170` | 20 | `89d44d243eef4bfa6681ae1ebb313801672746ae1bfb8e0b9228e55a60186816` |
| DLL | `0xcd8c7c` | 13 | `f8efc1f27ef6c525f7fd20dcb8d65e8197e97410eced20db4d323dfbf230a2a4` |
| lookupThunk | `0xcd7430` | 4 | `5e2beaf23253fb051108e625f14896ae942c7355734a839f34b096e7f1993632` |
| iatThunk | `0xa661d0` | 4 | `5e2beaf23253fb051108e625f14896ae942c7355734a839f34b096e7f1993632` |
| importByName | `0xcd8498` | 22 | `154edd3bad225054efe8d8871ec8087b960200652d5c70a20f9bd60046ffd2b8` |

### Primary SDK mapping, distinct from native instruction evidence

[Microsoft's MultiByteToWideChar reference](https://learn.microsoft.com/en-us/windows/win32/api/stringapiset/nf-stringapiset-multibytetowidechar)
identifies the parameters as code page, flags, input pointer, input byte count,
output wide-string pointer and output character capacity. It documents UTF-16
output, `-1` input length including NUL, capacity measured in wide characters,
a successful converted-character count, and zero on failure.
[Microsoft's code-page identifiers](https://learn.microsoft.com/en-us/windows/win32/intl/code-page-identifiers)
identify **65001** as UTF-8. These are primary documented API contracts, not
measurements of this executable running on a particular Windows version.

Applying that signature to the **already inspected** six arguments at
`0x43569e` yields:

| Parameter | Recorded argument | Documented interpretation |
| --- | --- | --- |
| CodePage | `0xfde9` = 65001 | UTF-8 |
| dwFlags | zero | Actual caller flags; no strict-error flag is supplied |
| lpMultiByteStr | supplied path pointer | Input byte string |
| cbMultiByte | `0xffffffff` = signed -1 | NUL-terminated input, including NUL |
| lpWideCharStr | local buffer | UTF-16 destination |
| cchWideChar | `0x104` = 260 | Destination capacity in wide characters |

Thus conversion identity is closed: on documented success, the local wide path
is passed to the separately proved `GetFileAttributesW` import. The recorded
flags are not substituted with strict malformed-input behavior. Actual operating
system behavior, path bytes and execution outcome remain unobserved.

### Failure paths preserved without invented defaults

The prior instruction trace still controls the **caller's** unusual outcome.
When `MultiByteToWideChar` returns zero, it branches to explicit EAX-zero and
then the shared formula `(~(EAX >> 4)) & 1`, producing **one without calling
GetFileAttributesW**. No extended conversion-error query is visible; failure
cause is unknown. This is not the documented API's own return value forwarded
unchanged, and must not be reconstructed as automatic false on conversion error.

On conversion success, an attribute-query return `0xffffffff` instead produces
**zero** through that formula. Null-input reporting followed by normal return
is the separately recorded EAX-zero/one path. These outcomes do not initialize
metadata `+4` or establish its default. No Host implementation follows from the
resolved conversion identity.

### Closed slice and remaining source-identity boundary

The conversion slot is fully identified within this metadata-only scope.
Builder continuation from instruction `0x4358ce`, `0x436fa0`, and the actual
opener implementation remain uninspected here. No source-category/priority,
incoming basename, selected runtime context, field semantics or lifetime is
inferred from the API identity. `GetPlayerNeutralPassive` remains unsupported;
original campaign runtime remains blocked.


## Complete builder body: module handle origin and output normalization

GNU objdump 2.47 inspected only **`0x435850..0x435a50`**, half-open, 512 bytes,
on exact Legacy executable SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Original range SHA-256:
`5703fa0869c30e227d592e69a05f849bfb227cc7497dbb37bfd36ac1ba83ca04`.
The complete builder ends at `0x435931`. Adjacent functions within the window
are not used to infer its path contract. Artifacts remain local in
`.scratch/neutral-base-builder-435850.objdump`,
`.scratch/neutral-base-builder-range.json` and `.scratch/neutral-module-origin-import.json`.
The preceding 77,560 note bytes are unchanged, SHA-256
`272d5a7e4e9c4e7444361ed7a807749de737d3b73d59af410bc62f4b6f535c55`.
No builder delegate, other import, literal contents or runtime was queried.

### One named import and corrected stack grouping

IAT VA **`0xe66160`**, RVA `0xa66160`, belongs to descriptor RVA `0xcd7170`,
OriginalFirstThunk RVA `0xcd73c0`, FirstThunk RVA `0xa66160`, index **zero**.
Its lookup thunk contains import-by-name RVA `0xcd82a0`: DLL **`KERNEL32.dll`**,
name **`GetModuleHandleA`**, hint **533**, ordinal flag clear. Timestamp and
forwarder-chain fields are zero. This is a metadata-proved module-handle API,
not a guessed module-filename API.

| Selected metadata | RVA | Bytes | Original-byte SHA-256 |
| --- | --- | ---: | --- |
| descriptor | `0xcd7170` | 20 | `89d44d243eef4bfa6681ae1ebb313801672746ae1bfb8e0b9228e55a60186816` |
| DLL | `0xcd8c7c` | 13 | `f8efc1f27ef6c525f7fd20dcb8d65e8197e97410eced20db4d323dfbf230a2a4` |
| lookupThunk | `0xcd73c0` | 4 | `724f1e28ce8c1893cd773210e4fe279c5769d1385ef71cbe091d88eb7314a999` |
| iatThunk | `0xa66160` | 4 | `724f1e28ce8c1893cd773210e4fe279c5769d1385ef71cbe091d88eb7314a999` |
| importByName | `0xcd82a0` | 19 | `217ef264962dfcf872d3b54e696b59bf9e726a8d985fdd715092b0fc94320ccb` |

[Microsoft's GetModuleHandleA signature](https://learn.microsoft.com/en-us/windows/win32/api/libloaderapi/nf-libloaderapi-getmodulehandlea)
has one module-name argument; NULL selects the executable used to create the
calling process. It returns a module handle on success and NULL on failure.
This documented contract is separate from observing the target execute.

This identity corrects the earlier unresolved-call argument grouping. The
three pushes before `0x435880` are **not three GetModuleHandleA parameters**.
The imported one-argument call consumes the null argument; the other pushed
words remain for the next call. `0x435887` therefore passes
**`0x4394a0(module_handle, global_buffer_0x112ec10, 0x104)`**. The remaining
stack cleanup at `0x435898` balances that call and the following two-argument
helper. Prior unresolved prose must be read as raw pushed-word evidence, not
a proved three-parameter API signature. No earlier reviewed bytes are rewritten.

### Cached base and candidate selection

Arguments to the complete builder are candidate pointer, output pointer and
selector. It initializes the cached base only if byte `0x112ec10` is zero.
The module handle is passed to `0x4394a0` without an explicit null-result check.
Afterward, `0x4333d0(global_buffer, slash_byte)` is called. A nonzero return
pointer is directly overwritten with a NUL byte. The cached buffer, uninspected
literal pointer `0xe695c4`, and capacity `0x104` then reach `0x433140`.
Module filename extraction, search position and literal operation remain
unproved delegate contracts; the module handle alone does not supply a filename.

If the builder's selector is nonzero, `0x433410(candidate, slash_byte)` runs.
When its result is nonzero, the candidate pointer becomes returned pointer plus
one; otherwise the original candidate remains. A zero selector keeps the
original candidate. This proves pointer selection, but not whether the helper
finds a first/last occurrence or implements another matching rule. Accordingly
"basename stripping" is still an inference rather than a complete contract.

### Final output pipeline and directly observed byte mutation

At `0x4358e3`, unresolved IAT slot **`0xe665e8`** receives a local buffer pointer,
literal pointer **`0xe6a110`**, cached base pointer, and selected candidate
pointer. A formatting-like operation is plausible, but no API name, literal
format or complete produced string is assigned from that shape.

`0xa793bc` next receives output pointer, local buffer and `0x104`. Its return
is not checked. The already closed `0x433910` length contract is applied to the
output. The subsequent complete loop examines bytes through that length,
stopping on an encountered NUL, and directly replaces every encountered
**backslash byte `0x5c` with slash byte `0x2f`**. This normalization is proved by
load/compare/store instructions, independent of the uninspected formatter and
output-transfer helper.

There is no final explicit Boolean/result assignment. The loop loads bytes
into AL, so the earlier length in EAX is not a preserved return-length contract;
its caller already ignores the builder return and obtains acceptance from the
separately proved attribute/conversion gate. No safe output default or checked
failure branch is established by the complete body. The formatter/local-buffer
and transfer outcomes remain delegated; the cached base may also be reused
without reinitialization when its first byte is nonzero.

### Closed coverage and essential remaining boundary

Body/tail coverage is complete; **full final pathname semantics are not**.
The module-origin connection is now named and the final slash normalization is
proved. Stop at **`0x4394a0`**, which consumes the documented executable module
handle and cached buffer/capacity, and is essential to prove the base bytes.
A next bounded entry window would be `0x4394a0..0x439570` if separately authorized.
The formatter/import/literals remain additional distinct contracts, not license
for an automatic expansion. No `0x436fa0` or other delegate was inspected.
Actual input basename, archive priority, selected native context and metadata
meaning/defaults remain unproved. `GetPlayerNeutralPassive` stays unsupported;
original campaign runtime remains blocked.


## Module-handle buffer: named path origin, returned length and failure flow

GNU objdump 2.47 inspected only **`0x4394a0..0x439570`**, half-open, 208 bytes,
on exact Legacy executable SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Original code-range SHA-256: `c3cf7e822a71208ff3370225c7c8d3627923527515649b7bd33816fd0a316bf7`.
The complete body ends at `0x439554`. Padding and the isolated return at
`0x439560` do not extend the target contract. Artifacts are local
`.scratch/neutral-module-buffer-4394a0.objdump`,
`.scratch/neutral-module-buffer-range.json` and
`.scratch/neutral-module-filename-import.json`. The preceding 83,734 note bytes
are unchanged, SHA-256
`2bba4d35325c04fd4e8bf1e46bb24472a5c1c7a49367d942b836ea2d2b18c488`.
Only the one essential origin import is resolved; no delegate or runtime ran.

### Named module filename import

IAT VA **`0xe661b0`**, RVA `0xa661b0`, is descriptor RVA `0xcd7170`,
OriginalFirstThunk RVA `0xcd73c0`, FirstThunk RVA `0xa66160`, index **20**.
Lookup thunk RVA `0xcd7410` identifies import-by-name RVA `0xcd840a`:
**`KERNEL32.dll!GetModuleFileNameW`**, hint **532**, ordinal flag clear.
Descriptor timestamp/forwarder-chain values are zero. This identity follows
from selected PE metadata, not from a conventional three-argument call guess.

| Selected metadata | RVA | Bytes | Original-byte SHA-256 |
| --- | --- | ---: | --- |
| descriptor | `0xcd7170` | 20 | `89d44d243eef4bfa6681ae1ebb313801672746ae1bfb8e0b9228e55a60186816` |
| DLL | `0xcd8c7c` | 13 | `f8efc1f27ef6c525f7fd20dcb8d65e8197e97410eced20db4d323dfbf230a2a4` |
| lookupThunk | `0xcd7410` | 4 | `09e8b3220b2c06a06ef082054a3d0aafedc2820b9b465550a0e6f5af30e0f7c3` |
| iatThunk | `0xa661b0` | 4 | `09e8b3220b2c06a06ef082054a3d0aafedc2820b9b465550a0e6f5af30e0f7c3` |
| importByName | `0xcd840a` | 21 | `dd812438952c5d7d84a83d661fb72c3202d83a36d04590e7046f6db6f2cf8d99` |

### Origin and result contract from primary API documentation

[Microsoft's GetModuleFileNameW reference](https://learn.microsoft.com/en-us/windows/win32/api/libloaderapi/nf-libloaderapi-getmodulefilenamew)
identifies the arguments as module handle, wide output buffer and capacity in
WCHARs. It returns the module's qualified path on success, with character
length excluding NUL; zero indicates failure. Insufficient capacity can return
the capacity with truncation, and termination details differ on Windows XP.
These documented cases are not an observation of this target's Windows host.

At `0x4394e8`, this helper calls that API with its argument 1 module handle,
a local buffer, and `0x104` capacity. The already proved caller obtains its
handle with `GetModuleHandleA(NULL)`. Thus successful documented origin is the
calling executable's module path; no basename is supplied by assumption.
The API's return is saved in EBX immediately afterward. There is no zero,
capacity-equality or extended-error check before the next operation.

### Second operation, byte output and exact local return

Unresolved import **`0xe661d4`** is called with eight recorded arguments:
`(0xfde9, 0, local_wide_buffer, 0xffffffff, output_pointer, caller_capacity, 0, 0)`.
This is a conversion-shaped call, but its identity/signature/output encoding
are **not** proved in this single-import slice. The numeric UTF-8 code-page
identifier does not itself name an unresolved API. No additional import was
resolved to make that inference look like evidence.

A zero return from the second call exits with EAX zero. A nonzero return starts
a loop over the caller's output pointer up to `output + caller_capacity`,
stopping at NUL; backslash bytes are directly replaced with slash bytes.
Afterward `0x439545` restores **the first GetModuleFileNameW return** to EAX.
Therefore this successful local path returns the saved module-path WCHAR count,
not the second operation's result or an output byte count. Its outer builder
ignores that returned value, as already observed.

Null output pointer takes `0x43eca0(0x57)` and returns zero if reporting returns
normally. The output is not cleared at entry. A failed first API still proceeds
to the second call with the local buffer; if that call returns nonzero, the
helper performs byte normalization but ultimately returns the saved first result
zero. Neither the first API's failure nor the second call's zero return proves
that the cached output was initialized to a safe empty/default path. No metadata
`+4` field is written by this helper.

### Closed origin and remaining encoding boundary

The module-path **origin** and the complete helper's length/error/control flow
are established. Actual emitted byte encoding is stopped at the unresolved
single import **`0xe661d4`**, which needs a separately authorized PE entry
resolution if that contract is required. No code continuation is missing here.
The cached base's later search/literal mutation and final formatter remain
separate contracts; none of the formatter IAT, transfer helper, predicate or
opener was expanded. Runtime path bytes, source priority, selected context and
neutral-player metadata semantics/defaults remain unproved.
`GetPlayerNeutralPassive` stays unsupported and campaign runtime remains blocked.


## Module byte conversion and conditional map-header correspondence

This slice resolves **only PE IAT VA `0xe661d4`** on exact Legacy executable
SHA-256 `3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
There are no additional executable-body, literal or delegate reads. The
preceding 88,949 note bytes remain unchanged, SHA-256
`bd49c7a54b7d5d487d49849b75020a19d3d6f5ba4bed23845d4230054cb9c56f`.
The selected structural metadata is `.scratch/neutral-module-byte-import.json`;
its temporary resolver is `.scratch/resolve-neutral-module-byte-import.py`.
Repository header/evidence comparison below is read-only, not a new native or
original-asset measurement. No provider, game, Host or build ran.

### Conversion identity proved by PE metadata

IAT RVA `0xa661d4` belongs to descriptor RVA `0xcd7170`, OriginalFirstThunk
RVA `0xcd73c0`, FirstThunk RVA `0xa66160`, index **29**. Lookup thunk RVA
`0xcd7434` contains import-by-name RVA `0xcd84ae`: DLL **`KERNEL32.dll`**,
name **`WideCharToMultiByte`**, hint **1297**, ordinal flag clear. Descriptor
timestamp and forwarder-chain fields are zero. The hint is not an ordinal.

| Selected metadata | RVA | Bytes | Original-byte SHA-256 |
| --- | --- | ---: | --- |
| descriptor | `0xcd7170` | 20 | `89d44d243eef4bfa6681ae1ebb313801672746ae1bfb8e0b9228e55a60186816` |
| DLL | `0xcd8c7c` | 13 | `f8efc1f27ef6c525f7fd20dcb8d65e8197e97410eced20db4d323dfbf230a2a4` |
| lookupThunk | `0xcd7434` | 4 | `981193477686431d7c714c1873b433a6a74f9c0a9345cfc605d6851b2cb7ed13` |
| iatThunk | `0xa661d4` | 4 | `981193477686431d7c714c1873b433a6a74f9c0a9345cfc605d6851b2cb7ed13` |
| importByName | `0xcd84ae` | 22 | `1023021e390ebcf261e6fea309ae4725bef40d99ea5204f483836553d6f3f623` |

### Primary SDK mapping and preserved native return distinction

[Microsoft's WideCharToMultiByte reference](https://learn.microsoft.com/en-us/windows/win32/api/stringapiset/nf-stringapiset-widechartomultibyte)
maps the eight parameters to code page, flags, UTF-16 input, input-character
count, byte output, byte capacity, default character and default-used pointer.
The already recorded call is `(65001, 0, wide_path, -1, output, capacity, NULL, NULL)`:
UTF-8 output, zero flags, NUL included, byte-count capacity. Success returns
bytes written including NUL for this input-length form; failure returns zero.
This documented signature is connected to a metadata-proved name, not inferred
from 65001 before resolving the import. It is not target execution evidence.

The previously inspected helper still does not check the first
`GetModuleFileNameW` result for zero or truncation before invoking conversion.
A conversion-zero path returns zero. A nonzero conversion performs observed
backslash-to-slash normalization, then returns the saved **GetModuleFileNameW
WCHAR count**, not converted bytes. The caller ignores that value. No cached
output clear or safe default is established on either failure; current-host
encoding behavior and actual emitted path bytes were not measured.

The later acceptance gate is a different direction (`MultiByteToWideChar`)
and retains its separately proved unusual result: conversion zero yields one
without querying attributes; attribute `0xffffffff` yields zero. Closing the
module's output conversion does not erase that distinction.

### Read-only physical comparison with `pcx-map/info.rs`

The inspected source is `crates/pcx-map/src/info.rs`, SHA-256
`08a0695b709fbe73f66f9fa925d01e55abf5c9fa79c6514dc2ac40b66323c623`.
Its `Cursor::new` starts at zero and `u32` reads little-endian four-byte words,
advancing by four on success. `MapInfo::parse` currently accepts **18 and 25**,
reads version at offset zero and `constant_a` at offset four, then reads
`constant_b` at **offset eight** because both accepted versions meet
`VERSION_SECOND_CONSTANT = 0x10`. This is current source behavior, not a claim
that only version 18 exists based on its older module prose.

The exact Legacy instruction evidence already retained here independently
shows an initial word accepted through `0x19`, leading metadata reads for
initial words `0x10..0x19`, and successful four-byte reads into metadata `+0`
and `+4` from a stream with initial cursor zero. Therefore, **if the same
complete version-18/25 payload is supplied and these reads succeed**, the word
represented by Rust `constant_b` is physically the word assigned to exact
Legacy metadata `+4`: input offset eight. This correspondence does not need
any compatibility DLL addresses transplanted into the Legacy image.

For lower initial versions, the exact native parser skips the leading reads;
for truncated/failed reads it leaves the destination untouched. Rust rejects
unsupported versions or short data with a typed error. Accordingly the
conditional matching does not extend to native default/failed-read state.

### Historical labels and semantics are still different evidence classes

The module prose labels the third word "B — build constant", cites writer
`0x6f7267f0` and reader `0x6f738910` from older `game.dll`, and describes that
artifact as "1.28c". Those are existing source claims, not new native proof
of a field name in the exact Legacy executable. Its original-asset header test
uses the extracted-tree input and skips when local requirements are absent;
it illustrates a previously recorded word value, not exact Legacy active state.
No test or new original-asset extraction was run in this slice.

The already published identity evidence in
[glue-coordinate-runtime.md](glue-coordinate-runtime.md) identifies
`wc3/game/app/game.dll` SHA-256
`6d21bd9a0f9fbc8446f455c9e89ac994fed68174426fe608fcb9baefd4dec53c`
as VERSIONINFO **1.26.0.6401**, while the exact Legacy executable is
**1.29.2.9231**. Individual PE identity does not identify adjacent archives.
Consequently the source's "1.28c" label is not used here as an artifact-version
fact, and neither its addresses nor reported example values are assigned to
the Legacy image. The generic name `constant_b`/historical build label is not
an editorVersion, engine-build, player-limit or lifetime proof.

### Closed slice and semantic/context blocker

Module output conversion identity is closed. Native path-helper expansion is
held: no new formatter, basename predicate, opener or literal investigation
follows this slice. Physical offset-eight correspondence is established only
under the explicit same-payload/successful-read conditions above.

Still missing are the semantic producer/name of that authored third word in
the exact target, actual loaded input/map identity, and proof that the active
getter context/cache record refers to the same metadata object at the relevant
lifetime. Successful parsing or matching widths alone cannot close those gates.
Failure/default paths remain independent. No implementation is inferred from
compatibility DLL names or static branch results; `GetPlayerNeutralPassive`
stays unsupported and original campaign runtime remains blocked.
