# Snapshot framing and identity admission

`pcx_net::decode_world_payload` checks every declared unit head and optional target,
and exact payload consumption, before allocating unit or identity metadata. The
same checked record reader is used for preflight and construction. Span endpoint
overflow produces `PayloadSpanOverflow` before slicing. Existing representable
truncation diagnostics retain their kind and required/available byte counts.

A fully framed snapshot must contain unique unit IDs. Repeated IDs produce
`DuplicateUnitId` with the snapshot tick, raw ID and first/conflicting record
ordinals. This includes identical repeated records: choosing one by wire order
would leave correction ambiguous. Unique IDs, including zero and the maximum
word, remain unchanged; records retain supplied order, owner, coordinates, targets
and raw next-ID counter. The existing nonzero target-flag interpretation remains.

Whole framing precedes identity validation. Thus a frame with an early repeated
ID and a later truncated target reports truncation before allocating metadata;
a fully framed repeated ID reports the identity error. The public regression
checks this intentional multi-error precedence.

Native regression tests reproduce the old behavior: an eight-byte forged count
requested 114,688 bytes of unit storage before failing; duplicate-ID snapshots
could change client correction based on record order. The corrected malformed
frame cases request zero allocator bytes on native and actual 32-bit WASM.
Valid input still allocates storage and an identity index. A three-record fixture
requests 188 bytes native and 180 bytes WASM in these builds; these measured
layouts are not production constants or memory limits.

Eleven identity/framing consumer groups, nine allocation/result cases and the
checked endpoint arithmetic probe run on actual WASM without host imports.
The arithmetic boundary is a private-helper contract, not a claimed reachable
public tiny-input overflow. Decode-then-correct tests preserve client predicted
and authoritative worlds, consensus, pending inputs and history on rejection;
valid snapshots still reach the existing correction code.

Allocator instrumentation is test-only and forwards the exact pointer/layout
contracts to System. Its unsafe-code exception is scoped to that implementation,
with safety contracts beside each operation. Run the allocation test serially:

```sh
cargo test -p pcx-net --locked --test snapshot_allocations -- --test-threads=1
```

These are project protocol and correction guarantees. They do not establish
native Warcraft wire format, IDs, tick/RNG behavior or rendered multiplayer.
Fully framed input still has no caller-selectable total allocation budget;
nonfallible serialization, direct mutable World admission and exhausted-ID
recovery remain separate. Callers supplying World directly bypass byte admission.
