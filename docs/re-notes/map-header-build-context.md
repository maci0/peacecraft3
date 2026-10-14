# Exact Legacy map-header word and context boundary

The third input word is physically connected to metadata `+4`, including a
write-side counterpart in the exact Legacy executable. Its semantic name and
upstream value assignment remain unproved. The context setter now has a concrete
write into the getter's subobject, but this does not establish the selected map,
input bytes, successful metadata read or valid live lifetime.
`GetPlayerNeutralPassive` remains unsupported. No Host behavior or campaign
execution follows from this evidence.

## Target and bounded evidence

This execution-free investigation uses only the supplied x86 Legacy
`Warcraft III.exe`, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`,
raw size **14,858,960 bytes**. Its previously established VERSIONINFO is
**1.29.2.9231**; that does not identify adjacent archives. No compatibility DLL
addresses were transplanted and no game or Wine process was run.

The retained callback range identities and
[neutral-player note](native-neutral-players.md) were read first. The aligned
snapshot `/tmp/peacecraft3-neutral-context-20261014.snap`, SHA-256
`cc37e0a2ae15a05b42841ff43d4b1ae9f0ca723192f1fcf9d2b1d733cadb9eaa`,
provided the context installation dossier and getter-accessor evidence:

- `ev_00d7e940c0dc8323b7360bbbd53ebdc10bf1bf7247dcf35cf5837ff4a163a0ef`
  for `0x4790d0`;
- `ev_6bdba903f8ca3f04f97418ae6aa98ef583377959c57ab6cee0c4075a18a41ea9`
  for `0x469010`.

No cold REA load, provider change, timeout retry or broad function sweep occurred.
New direct-call/delegate ranges were disassembled with GNU objdump after checking
the complete target digest, raw size and initialized PE-section file mapping.
These have local range hashes, not new provider Evidence IDs. Half-open ranges
used for the conclusions are:

| VA range | Bytes | SHA-256 of original file range |
| --- | ---: | --- |
| `0x71fb90..0x71fd10` | 384 | `e95a73514af02fde310e8dc9f865eb961eae2dde04648db45d49813f8ad3e185` |
| `0x71e8d0..0x71ead0` | 512 | `cd73df88d2bf5e468d9857ee2dd6215e0bc1e1b0837fa5fbe7657605bdc34c08` |
| `0x71f590..0x71fb90` | 1536 | `fc978968a13cbf5078fae79e80dc93114325cd4f954912023f2dcf58733e60f5` |
| `0x45cf90..0x45cff0` | 96 | `710032c980057dbf07bb17683a258590d2f8e50933f2284c5166c8dbc986d2bc` |
| `0x45cff0..0x45d070` | 128 | `60f957e8a5b9a6f869fe5195616be09e1e5515bea631d1e68007cbc8ca03fdff` |
| `0x468d70..0x468f70` | 512 | `97575fd4cf32065b03756c29447cdf7bb25510f58057db84a1d5be9a6b750eee` |
| `0x468850..0x468920` | 208 | `0e3ec94bc682d48aed5816dba67044ec94cd5f2006fcd4376f9fa7f7eb1dba8b` |
| `0x469650..0x4696d0` | 128 | `879dc718ed4669ab1bf7d84beeb31633416f92942e480cb8a9febe72c94407ab` |
| `0x71f500..0x71f590` | 144 | `7c41e3faf5911b14eb42d4290495a8ff68fc8d72a9e41663d2cb9541cb7bd224` |

Windows can contain adjacent padding/procedures. Conclusions use the identified
bodies only: callback through `0x71fbc0`, writer through `0x71fb88`, scalar writer
through `0x45d03e`, tag writer through `0x45cfde`, setter through `0x468ddb`, input
accessor through `0x46885d`, and record creator through `0x4696b2`. The parser
entry/return windows are partial. A first writer-tail window began inside an
instruction; its initial decoding was rejected and superseded by the complete
entry-anchored writer range. All local artifacts, including that rejected window,
remain under `.scratch/map-header-context-proof/`; no original contents are
included in this note or uploaded.

## Third-word write counterpart

The previously established zero-mode callback invokes reader `0x71e8d0` with
record `+0` as metadata destination. For a successfully read initial tag in
`0x10..0x19`, its leading reads put the second and third stream words into
metadata `+0` and `+4`. Lower tags skip those reads; individual read failures
leave destinations untouched. This remains the conditional physical offset-eight
correspondence in the preceding note.

The callback's **nonzero** mode is now separately inspected. At
`0x71fb9c..0x71fba5`, it passes callback record `+0xc`, record `+4`, and the stream
argument to `0x71f590`. These are different local-record slots from the zero-mode
reader. The retained read wrapper/dispatcher does not establish a nonzero-mode
caller, so those pointers must not be silently identified with its read record.

The called routine's leading assembly establishes a writer counterpart:

1. `0x71f596..0x71f5a4` selects initial tag `0x12` or `0x19` from whether argument
   three is zero. At `0x71f5b0` it passes that tag to `0x45cf90` with the stream in
   ECX.
2. `0x71f5b5` obtains the metadata pointer from argument two. It pushes its
   DWORD at `+0` at `0x71f5ba`, then its DWORD at **`+4`** at `0x71f5c1`, calling
   `0x45cff0` for each.
3. The directly called tag/scalar primitives write the supplied DWORD at the
   stream's observed buffer/cursor address and advance cursor `+0x10` by four.
   They perform no visible numeric conversion. Therefore, conditional on a
   stream starting at cursor zero and valid backing/refill behavior, metadata
   `+4` is the authored third word at output offset eight.

The writer **reads an already existing caller metadata word**. Its leading
writes do not stamp a fresh editor version or engine build. That does not prove
that a preceding producer never assigned such a value; it identifies the missing
upstream assignment rather than disproving a possible field interpretation.
No semantic label can be recovered merely from copying, tag selection, an
unsigned getter comparison or corpus values.

The complete writer has a visible normal epilogue assigning EAX one at
`0x71fb7f`, followed by return at `0x71fb88`. The scalar primitives may call stream
vtable `+0xc` for backing storage and do not test its EAX before the write. A
normal writer return therefore is not proof of successful allocation/output or
valid third-word contents; virtual method/error/exception contracts are open.

## One concrete context installation link

The cached `0x4790d0` assembly connects the selected input value to the receiver
used by the native getter. When input `+0x4a8` is nonzero, `0x479207..0x479212`
invokes `0x468850` on input `+0x4c0` and saves returned EAX in EBX. Otherwise EBX
is zero. At `0x47921a..0x47921e` it passes EBX to `0x468d70` with ECX equal to
`context->+0x30 + 0x24`.

The complete input accessor `0x468850` returns its receiver's `+8->+0x1c`, or
zero if `+8` is null. The newly inspected setter `0x468d70` concretely writes
receiver **`+8`**, rather than being an unproved setter-name hypothesis:

- A zero argument increments the word at shared record `0x10dba50 +4`,
  invokes the previous receiver `+8` object's virtual method `+8` when nonnull,
  then installs shared record `0x10dba50` at receiver `+8` (`0x468d90`). It does
  **not** directly clear receiver `+8` or prove the shared record's `+0x1c` is
  null at an active lifetime.
- A nonzero argument passes it to `0x4696d0`; on a zero result it calls
  `0x469650` with that same argument and two zeros. The meaning of the manager
  returned by `0x468f80` and of the lookup/creation contracts remains open.
- When a selected result is nonzero, the setter increments selected `+4`.
  It invokes the old receiver record's virtual `+8` method when nonnull and
  installs the selected result at receiver `+8` (`0x468dd4`). The layout is
  reference-count-like, but exact ownership/destruction rules are not established.

The retained getter accessor `0x469010` subsequently reads that same receiver's
`+8->+0x1c`. This closes a **conditional structural installation/read edge**.
It does not establish that installation executed, that no later mutation occurred,
or that the read string/key identifies the supplied map's parsed metadata.
The selected record is not proven to be the metadata object whose scalar `+4`
is compared after the later cache/parser path.

The directly called creator `0x469650` stores the result of `0x4335d0` into
its returned record's `+0x1c` at `0x4696a9`, after passing the setter input to
that helper. It can also preserve an already equal pointer. The helper's byte
copy/allocation result contract, lookup equality, allocation failure behavior
and record lifetime are uninspected. No proof of same-content key identity is
substituted for those boundaries. The setter's conditional zero-result handling
does not establish that its creator can safely return null.

A later cached setter call at `0x4793d4` targets **context `+0x560`**, not this
getter subobject. It is not silently treated as another write to the same field.

## Success, default, failure and lifetime ledger

| Path | Established | Still unproved |
| --- | --- | --- |
| Successful leading reader calls | Conditional third input word copied into metadata `+4` | Field name/upstream authored producer and actual active input |
| Lower initial tag | Leading scalar reads skipped | Leading field initial/prior contents; no zero/default inferred |
| Failed individual scalar read | Destination untouched under the preceding primitive proof | Whole-parser error propagation and resulting cache validity |
| Reader initial tag above `0x19` | EAX zero assigned before jump to shared tail | Complete parser failure/runtime applicability |
| Reader normal tail | EAX one assigned at `0x71f56b` before cookie-check call `0xa56bd8` | Uninspected middle, cookie-helper return contract and read-success implications |
| Nonzero callback writer | Existing metadata `+4` supplied to four-byte stream writer | Earlier assignment/semantic name, actual nonzero caller and virtual stream failures |
| Context setter with zero input | Shared record installed at receiver `+8` | Shared `+0x1c` initialization/mutation and actual fallback selection |
| Context setter with nonzero input | Selected record installed at getter receiver `+8` | Lookup/copy identity, live ownership and actual selected map/cache |
| Getter's null-input fallback | Prior note identifies a separate object field | That field's initializer/default/valid lifetime |

The parser return window tightens the location of normal/error tail assignments;
it does not turn parser completion into proof that the leading word was read.
The already observed unsigned getter failure-sentinel branch remains independent
of successful metadata parsing. No branch result, threshold or field value is
implemented as a constant.

## Exact blocker and next probe

The bounded slice answers where the third word is serialized and gives one
concrete context write/read edge. It does **not** identify an editorVersion,
engine-build, player-limit or other semantic name for metadata `+4`. Cached
`/tmp/peacecraft-format-native-mapinfo-*.json` writer/codec dossiers identify a
**different** target, `World Editor.exe` SHA-256
`d198d2ca47fc29d4901f3de27ff17619824afef5af7e42b9ff17cbe03723d7b7`.
They were rejected as exact Warcraft-executable semantic proof. No primary-source
convention or older compatibility-image address supplies that missing link.

The next focused probe is **typed incoming references to `0x71fb90` in the same
already aligned exact-target analysis state**, selecting the caller that actually
initializes callback record `+4` and invokes a nonzero mode. Inspect only that
caller's metadata construction/stamp of scalar `+4`, and its source expression.
If no retained/already active result can supply those typed references, that is
the analysis-state blocker; do not cold-load/retry or invent a caller from an
adjacent address. Separately, closing live applicability requires proving the
installed context's selected `+0x1c` key/cache association and lifecycle. The old
W3I path/open/source-priority chain remains on hold; no expansion occurred here.
