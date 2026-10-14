# Authored world import boundary

`pcx-sim::world` imports data independently of the existing fixed-tick movement
world. It retains each placement's file ordinal, rawcode, creation number,
position, angle, scale and all other parsed state. Creation numbers are not
allocated simulation handles. Positions are not rounded to fixed point.

`ImportedWorld::from_map` requires explicit install, edition and map provenance,
then reads MapInfo, the authored pathing raster and placed units/items. Missing
or corrupt required members return errors containing the map and member name.
The lower-level `from_authored` also requires provenance. Neither path invents
missing map contents, pathing flags, object stats or placement defaults.

`ImportedWorld::from_map_aligned` adds an explicit checked consumer route,
returning the raw imported world and `GridAlignment` derived from that same
map's terrain/pathing dimensions. It requires a valid `war3map.w3e`, preserves
member extraction/parse failures and returns supplied source identity plus both
raw dimension pairs when the uniform integer relationship is refused. Validation
precedes placement parsing/catalog resolution. The existing `from_map` and
`from_authored` remain raw, unaligned imports for compatibility. This uniform
relationship is project import policy, not original-game admission, world units,
height conversion or WPM flag semantics. The explicit original-world fixture opts
into it; broader raw original sweeps retain their existing scope.

This route adds terrain extraction/parsing to existing import work. Each current
Map member extraction copies and opens the embedded archive. Terrain decoding
retains raw bytes and decoded corner/slot arrays; the checked importer drops
those arrays before pathing/placement import. No caller packed/decoded/archive
budget is added, so the operation is not a bounded-memory import guarantee.

`MetadataCatalog` keeps unit and item overlays separate and resolves their bases
against the supplied Editor tables. Definitions are interned once per rawcode.
The runtime metadata determines field identity, declared type and source
SLK/profile column. Base cells remain verbatim text; map overrides retain their
wire-decoded values. No integer signedness, combat scale, damage formula or life
percentage conversion is inferred. Required fields check availability only;
they do not certify numeric or gameplay validity.

Missing rawcodes, unsupported object kinds, ambiguous custom families, custom
base chains and missing metadata are explicit resolution problems. Unavailable
fields retain metadata and their missing/unsupported/indexed status. All authored
modifications remain available, including unknown fields and selector words.
Profile applicability flags are not exposed by the current metadata API, so
missing profile cells remain explicit rather than being filled or claimed
applicable. The catalog follows the existing Overlay stock-base semantics; it
does not invent inheritance from other map-modified bases.

The signed placement owner word is native-code evidence. The local Legacy World
Editor SHA-256 is
`d198d2ca47fc29d4901f3de27ff17619824afef5af7e42b9ff17cbe03723d7b7`.
The unit reader `0x551650` reads the first two-byte word into the unit-state
structure (`ev_df95363f2c5857553dd4489f23c7745a039b5feb7e022bc5673ae0a18e43265d`).
Accessor `0x47a590` copies that same first word
(`ev_04ff8a3caf99ee0ea9d77cd1037a20bec2cb5ecceaf368803f3791f9af85101a`).
Script generator `0x53c9f0` compares it as a signed short while grouping
`CreateUnit` records for generated player functions
(`ev_78d6730312f665f860d6443165a0c3a422b92038cd08933061c5c3cfcc2de869`).
The importer retains that signed owner on unit definitions. It does not map
neutral slots, infer owner relationships from legacy MapInfo field labels, or
assign unit ownership to items.

Every non-None parsed selection record is reported as unverified. In particular,
`Packed(0)` is retained and reported rather than assumed to mean no random
selection. `Unclassified(641)` stays raw, with no guessed payload or behavior.
Nonfinite placement coordinates, scale or angle are reported. Unsupported
records are retained with raw identity, ordinal and creation number, not dropped.

## Verification

The isolated-target strict suite passed: eight existing movement tests and three
new synthetic import tests. Synthetic cases cover exact authored placement,
signed ownership, distinct unit/item identity, catalog reuse, supplied stats,
missing required fields/rawcodes, raw 641 preservation, nonfinite placement and
missing provenance. Two original tests are explicitly ignored without their
required environment variables. No successful skip is counted as original proof.

Local original tests used the Legacy directory and the explicitly supplied
identity `Legacy target executable VERSIONINFO 1.29.2.9231; archive adjacency not
version proof`. Human01 imported all 161 placements: 155 unit records resolved
into 53 unique definitions with HP cells, and six `sloc` records remained named
as missing rawcodes. Resolved override fields were exercised. Human01 contained
no preplaced items; a bounded search through actual archive-listed `.w3m/.w3x`
names selected Human03, whose 153 placements include two imported items with
ItemData HP cells. Resolved cells are checked against their metadata-selected
source table rows, and override values against authored modification records.
Counts are observations, not acceptance constants or embedded runtime data.

```sh
CARGO_TARGET_DIR=/tmp/peacecraft-format-strict-target cargo test --locked -p pcx-sim

# Set these to the locally supplied original assets and explicit provenance.
PEACECRAFT3_WORLD_INSTALL="$install" \
PEACECRAFT3_WORLD_MAP="$map_member_or_local_path" \
PEACECRAFT3_WORLD_EDITION="$source_identity" \
CARGO_TARGET_DIR=/tmp/peacecraft-format-strict-target \
cargo test --locked -p pcx-sim --test world_import -- --ignored --nocapture
```

This gate proves data import, not playable campaign behavior or native gameplay
parity. The existing movement tick is a chosen project policy. Trigger-created
objects, start-location semantics, random-selection execution, neutral remapping,
placement state conversion, combat and event ordering remain outside this slice.
Original assets and extracted/generated contents stay local.
