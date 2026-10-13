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
