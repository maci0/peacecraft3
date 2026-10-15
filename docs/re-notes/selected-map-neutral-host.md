# Selected-map neutral-player reconstruction

The selected-map adapter evaluates its verified neutral-player getter only for
one successfully admitted, explicitly selected map. This ownership is a recreation
policy, separate from the native cache, key lookup and default/failure paths in
[native-neutral-players.md](native-neutral-players.md),
[map-header-build-context.md](map-header-build-context.md), and
[native-player-context-key.md](native-player-context-key.md). It does not establish
original campaign execution or implement other player natives.

## Rule input and local trust

`VerifiedNeutralRule::import` (passive) and `import_aggressive` receive a complete supplied PE, a local proof
profile, an independently reviewed profile SHA-256 fingerprint, and explicit
resource budgets. Profile/source labels never authenticate themselves. The
application must supply the approval through trusted local configuration; taking
the fingerprint from the imported profile would defeat this boundary.
`LocalProfileApproval::from_reviewed_fingerprint` is that explicit control-plane
trust grant, not a certificate that review occurred. Keep import errors with the
selected PE/profile source labels at the application's import boundary.

The profile names one complete PE identity and initialized source windows for
registration, native name/signature, getter and header reader. It includes the
registration immediate offsets binding that name, signature and getter. Local
review establishes the semantic roles and physical metadata-word relationship;
matching three immediates is not a new generic proof of native registration ABI.
The importer checks all fingerprints and PE mappings before decoding operands.
The PE must be supported x86 PE32; uninitialized/zero-fill, overlapping, absent,
malformed, ambiguous or unsupported inputs are refused. Code windows require
readable executable code; data windows require readable initialized contents.

The getter comparison immediate, signed arithmetic immediates and conditional
header bounds are read from those verified bytes. No measured source addresses,
thresholds, return IDs, header bounds or generated profile contents belong in
committed code. Profiles and source bytes remain local. Interface names,
instruction/PE format identifiers and DWORD width are independent format data.
The profile's provenance is local review metadata; a file hash alone does not
prove a release label or identify adjacent archives.

SHA-256 uses RustCrypto `sha2::{Digest, Sha256}`. The dependency and lockfile are
required for both native and WASM, with the same input contract. Existing FNV
receipts remain reproducibility joins and are not authentication.

## Admission and execution

Successful `prepare_map_context` retains a private witness containing SHA-256 of
the supplied map and the exact parsed W3I, source identity, existing digest and
an Arc of the selected `MapScripts`. No extra original payload copy is needed.
This witnesses admitted-byte consistency, not an original publisher's signature.
Missing/archive/parse failures retain their existing admission errors.

`SelectedMapNeutralHost::try_new` owns that `Arc<ParsedMapContext>` and a verified
rule. Before global initialization it checks witness/source/receipt consistency,
reparses the retained W3I to compare decoded public fields, checks the
source-derived applicability range and present third word, and requires the
selected Program's zero-argument integer-return native declaration. The private
Arc also prevents replacing the selected program through `Arc::get_mut`.
Each import method binds its exact native name; profiles cannot relabel one
getter as the other. Two adapters can share the same admitted context and
delegate to each other to handle both names without sharing rule operands.

To use this opt-in adapter, construct it in `MapScriptSession::replace`'s Host
factory. The existing factory runs before that same context's `init_globals`;
failed candidate construction or initialization preserves the published session.
A pending selection never supplies metadata to an older executing session.
The application explicitly selects the approved executable rule; map, archive and
executable identities are different provenance domains, not equal labels to guess.
The launcher does not yet load an independently approved rule profile or install
this adapter automatically. Its ordinary Host therefore keeps its existing
unsupported-native behavior until that explicit application wiring is supplied.

Only the verified rule's named zero-argument native consumes the sealed successful word. All
other native, WTS and code-tag operations delegate unchanged. A successful word
with all bits set remains successful data; no native read-failure sentinel or
null/default path is synthesized. No context/rule or an unsupported context is a
refusal. No conventional player IDs or fallback word are invented.

## Verification boundary

Synthetic unit tests exercise the actual admission/parser/Program/Host/VM types,
independent getter operands, unsigned boundary behavior, initialization order,
provenance mutation, same-W3I integrity, signature/argument errors, delegation,
PE/profile tampering, initialized-range refusal and ambiguous instruction input.
They do not establish native live-cache behavior, original map execution or a
rendered result. The supplied real PE plus independently approved local profile
must pass a separate explicit original-input gate before enabling this contract.

The integrated crate passes 154 native tests, with five original-input tests
explicitly ignored by default, and strict all-target/all-feature native and WASM
Clippy. SHA-256 resolves to `sha2` 0.11.1 in the checked lockfile. These checks
cover the public adapter and its admission witness. The affected application
also passes 260 native tests, with eight explicit original-input ignores, and
strict native/WASM checks. Application profile loading and campaign execution
remain separate work.

An explicit local gate imports the complete supplied Legacy executable and a
separately reviewed local profile. The registration/name/signature join is
checked independently; getter/header fingerprints agree with the retained
evidence. Neither the profile nor any original payload is published.

With the separately identified retained Human01 map and its exact common/BJ
inputs, actual admission and the same compiled Program's global initialization
handle both `GetPlayerNeutralPassive` and `GetPlayerNeutralAggressive` with
separately approved rules, then stop at the still-unsupported `ConvertPlayerColor`.
Composing the later [player-color Host](playercolor-host.md) around that same
Program advances all source-authored color constants and stops at `ConvertRace`;
the earlier gate's color refusal is retained as historical evidence.
The earlier single-rule gate's aggressive refusal is preserved separately.
The gate uses an explicitly identified compatibility
object catalog with unresolved records; it does not establish a complete target
catalog or playable campaign. The separate LostTemple patch-common context does
not declare this native and retains the adapter's required-signature refusal;
its ordinary Host baseline first refuses `ConvertPlayerColor`. Those contexts
and their evidence are not interchangeable.

## Explicit first-family player-limit rule

`VerifiedNeutralRule::import_bj_max_players_first_family` adds the exact
`GetBJMaxPlayers` name to the existing checked importer and selected-map Host
path. This is explicit caller opt-in to the reviewed first-registration-family
getter, not automatic discovery of the active native family. Its own getter
bytes supply the comparison, signed arithmetic operands and both branch results;
the passive/aggressive getters and their APIs are unchanged. No player-limit
result or adjustment to another native's result is embedded in this path.

Before using this entry point, local review must independently approve a NEW
PCXNPR profile fingerprint binding the complete executable, exact native name
and signature, first-family registration/getter identity and qualified header
reader. Pass that reviewed fingerprint through `LocalProfileApproval`; the
profile's own labels or digest are not approval. The decoder authenticates the
reviewed windows and registration operands but does not interpret the skipped
context-address/offset/call operands. Their semantic join to the successful W3I
third-word reconstruction remains an explicit local review responsibility.

The separately imported rule can wrap the same admitted `ParsedMapContext` Arc,
Program and owned VM as the two neutral adapters. Admission, provenance, W3I
integrity, signature checking, zero-argument dispatch and delegation retain the
existing contracts. Synthetic tests use independent arithmetic, all three
cross-name bindings, the same-context initializer, contextual errors and sticky
next-native failure. They do not establish which native registration family is
active, native cache lifetime or campaign execution. Launcher installation
remains separate work.

The local first-family profile is now independently approved against the complete
qualified Legacy executable. Registration/name/signature/getter/header windows
were verified before comparing the candidate fingerprint. The complete getter's
context/delegate prefix matches the separately approved passive getter; the
successful-header physical-third-word reconstruction remains the explicit
recreation policy. No active native-family or context-lifetime claim follows.
The same retained Human01 admission, compiled Program and localization owner
passes all three scalar adapters and source-derived symbolic enum initialization,
then refuses `GetBJPlayerNeutralVictim` at Blizzard line 44 with sticky failure.
The affected crate passes 179 native tests, five explicit original-input ignores,
strict native/WASM Clippy and WASM library code generation. Independent approval
metadata is local in `.scratch/bj-max-players-root-approval/`; actual production
gate commands, input hashes and logs are in `.scratch/enum-host-root-integration/`.
