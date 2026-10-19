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

## Executed label preparation

An optional `ObjectHost::admit_sound_parameter_loader` supplies one missing
executed label after sound identity and label validation. Its source-qualified
definition appends under aggregate table/definition/metadata bounds. A matching
source identity, digest, profile and label column reuses the existing table;
only its definition list grows. Old bank/definition keys keep their meaning.
Missing, wrong or over-budget bindings fail before assignment changes. Cached
labels use the ordinary lookup without source work. Explicit banks remain valid
without this loader.

The campaign caller discovers SLK members from its retained reader's SoundInfo
namespace, rather than a chapter-specific table list. Exact `SoundName` or
`SoundLabel` headers identify the lookup column on tables with parameter fields
(`Volume` or `Flags`). Reference-only tables can contain the same labels without
owning sound parameters. Full versus dialogue schema
comes from the authored headers; unsupported requested schemas and labels with
multiple owners refuse. Unrelated table schemas do not contribute labels.
Only requested rows undergo numeric profile admission; malformed unselected
numeric rows do not supply defaults or prevent unrelated valid assignments.
All tables keep selected source provenance, edition/version identity and digests.

Source work is application policy: at most 32 table reads on initial indexing,
4 MiB per table, 32 MiB aggregate input, coordinate bounds of 10,000 rows and
256 columns, 1,024 retained definitions, 1 MiB assignment metadata and 128 new
label requests. Adapter identities/labels are capped at 16 KiB. Successful and
failed index results are retained for that session, so a missing member is not
read on every later call. New binding/session admission is required to change
the selected inventory. These are input/work bounds, not original engine values
or total parser heap bounds. Parameter assignment still does not establish
effective playback mappings or audible behavior.

Verification: 481 native Host cases pass (seven explicit ignores), as do the
application suites, 25 actual Cargo WASM cases and 19 targeted WASI parameter
cases. Native/WASM builds, strict lint and formatting pass. The supplied Human01
script's 51 distinct parameter labels bind through this production adapter:
16 full-profile rows and 35 dialogue rows across five selected table byte inputs.
The initial source check exposed a same-label animation reference; the corrected
schema filter and synthetic collision regression pass. Selected table bytes and
label counts match retained fingerprints. The probe's explicit mount order has
different archive provenance from the earlier hand-selected table set; native
winner/release equivalence is not established. No original game, audible or
rendered campaign check ran. Evidence is retained in
`.scratch/campaign-parameter-source-root-20261019/verification-final-2.json`.

The campaign's executed label constructors now share this index through a
retained `OnceLock` result. A parameter assignment after constructor loading
performs no second table read; the actual factory regression measures that work.
PCM lookup additionally requires `FileNames`, keeping legacy MIDI/DLS labels in
their separate format. Parameter lookup retains its own existing schema checks.
