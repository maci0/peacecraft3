# Explicit stock-object asset readers

`Editor::from_reader` accepts caller inventory, a source label, explicit input
limits and a fallible optional-byte callback. The callback owns source selection,
archive priority and extraction budgets. Only `Ok(None)` means absent; a read
failure stops construction and retains member, source label and its typed cause.
Present malformed SYLK likewise retains the typed parse error and both labels.
No source/edition priority or release identity is inferred.

Inventory count is checked before reader calls. Supplied asset lengths and checked
aggregate byte arithmetic are checked before table/profile parsing. Exact ceilings
are accepted. These limits apply after the callback produces its Vec: they do not
bound the reader's extraction allocations, parsed tables/profiles or total process
memory. Callers need an independently bounded extraction path.

The existing `open`/`open_in` APIs use this shared constructor. Their configured
MpqSet selection and unlimited input policy remain, but only `NotFound` becomes
absence. A present extraction failure previously became an absent table and a
successful Editor; it now propagates with member/source context. This diagnostic
change is intentional and does not establish native archive ordering.

A synthetic MPQ reproduces the old demotion using a present UnitData member whose
compression mask is unsupported. The corrected legacy archive API returns the
actual typed MPQ cause. Six supplied-reader groups cover missing/read/malformed/
valid data, exact limits, refusal before parsing and inventory-before-read; they
also run on actual 32-bit WASM without host imports.

An explicit local archive-set comparison (`wc3/game/app`, release unestablished)
compiles the previous constructor source independently and compares it with the
corrected implementation. All 19 present tables, 342,522 parsed cells, 1,443 field
definitions and 836 unit identities agree, as do inventory and other exposed
object-family indexes. Original data and detailed gate logs stay local.

Existing profile-text permissiveness, encoding handling, schema anomalies and
metadata/profile duplicate policy are not tightened by this constructor. Parsed
stock values do not prove native stats, conversions, overlay precedence or
playable map execution. The supplied-reader path permits a caller-configured
resolver to retain those source receipts rather than silently discarding errors.
