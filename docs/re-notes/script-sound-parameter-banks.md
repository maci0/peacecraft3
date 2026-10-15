# Explicit multi-source sound parameter admission

`sound_parameters::Banks::read` accepts caller-selected `Input` records. Each
record binds a table's bytes and source label to its `AssetIdentity`, exact label
column, selected labels and explicit `Profile`. Admission checks the entire
collection's counts, allocation layouts, table-byte and metadata budgets before
parsing tables or copying source metadata. Selected labels must be unique across
all banks, even when two definitions would have equal values. There is no table
or archive priority and no search through unrelated columns.

`Profile::Full` retains the existing `Bank::read` requirements and `Values`.
`Profile::Dialogue` requires its label column, `DirectoryBase`, `FileNames`,
integer `Volume`, opaque `Flags` and integer `version`. It refuses the presence
of the eight full-only parameter headers: `Pitch`, `PitchVariance`, `Priority`,
`Channel`, `MinDistance`, `MaxDistance`, `DistanceCutoff`, and `EAXFlags`. Their
absence stays observable in the retained table. Required text cells must exist;
empty or placeholder text stays verbatim, without numeric coercion. This schema
records source declarations; it supplies no inferred spatial or playback values.

`ObjectHost::with_sound_parameter_banks` installs an admitted collection once,
after the existing resource and native-declaration checks. Each executed
`SetSoundParamsFromLabel` stores a private bank/definition locator. The source
resource, creation flags, fade inputs, EAX input, and other sounds remain intact.
Missing labels fail before assignment mutation. A failed collection read cannot
install a partial collection.

`sound_parameter_assignment` returns the assigned profile, source identity,
source label, table digest, selected label column, owning table and exact row,
together with typed full or dialogue parameters. `sound_parameter_banks` exposes
the collection's source receipts. Existing single-bank installation and full
queries remain available. `sound_parameter_bank` returns a bank only for exactly
one full-profile source; multi-bank and dialogue callers must use the collection
or assignment query. `sound_parameters` returns full parameters for any assigned
full record, and a typed `UnsupportedSoundParameterProfile(Profile::Dialogue)`
error for a dialogue record. An assigned dialogue record is distinct from an
unassigned sound.

Synthetic parser and VM tests cover provenance, colliding row numbers, unrelated
lookup-column matches, cross-bank duplicate labels, aggregate refusal before
decode, late failure without installation, dialogue field absence, numeric
malformation, opaque text preservation and the compatibility getter refusal.
These tests establish admission and state ownership. They do not establish
native playback, effective flag interpretation, gain normalization, spatial
mapping, encoded-resource support or supplied-map execution. Supplied-source
validation must use an explicit local manifest and continue the same admitted
Program/context; no resource names or parameter values are embedded here.

Normal locked native/WASM builds and strict Clippy pass with the collection and
PCM-view changes together: 305 native tests pass, with five explicit ignores.
All 15 parameter test groups also execute in actual 32-bit WASM. The retained
same-context Human01 gate still reaches the same missing second file binding,
preserving config completion, first sound assignment and authored duration;
this is compatibility evidence, not a complete-manifest or campaign run.
