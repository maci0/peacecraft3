# Source-bound day/night model dependencies

`day_night::Assets::read` admits an explicitly selected set of script-path/model
bindings. Each input carries the exact script name, a source label, an explicit
edition/version/source identity and the locally supplied model bytes. All count,
per-model byte, aggregate byte, metadata-text and platform-layout limits are
checked for the entire set before any model decode or payload copies. Limits
are caller policy for logical input/retention, not native limits or a total
decoder-heap bound.

The existing `pcx-mdl` decoder parses classic MDX framing and light records.
The import refuses unsupported versions, repeated chunks, missing lights and
malformed light framing. It retains complete source model bytes, SHA-256,
source identity, static light fields and every decoded light/node track.
Sequences, global-clock declarations, pivots and other chunks remain in the
complete bytes for the eventual runtime importer. This does not validate every
other chunk's semantic content or interpret light channels, units, clocks,
transforms, color space, tangents or rendering behavior.

Bindings are exact and case-sensitive. There is no implicit extension rewrite,
namespace selection, case folding, archive priority, missing-model fallback or
blank-string reset behavior. A caller can explicitly bind a script `.mdl` name
to a qualified binary `.mdx` member; no asset-derived names are embedded in the
implementation. Duplicate script bindings refuse, even if their bytes agree.

`ObjectHost::with_day_night(assets)` checks the exact nonconstant
`SetDayNightModels(string,string)->nothing` declaration, source ambiguity and
script-function overrides through the existing shared contract validator.
Nothing is selected initially. An executed call resolves both terrain/unit
references before atomically retaining the selected model pair. Missing models,
malformed arguments and over-budget path arguments preserve the prior pair.
Successful selection allocates no session handles or replacement model buffers.
This is real source-backed scene setup state; applying/sampling it in the world
renderer and admitting it through the default launcher remain separate work.

## Qualified local input result

The same retained Human01 Program/context calls `SetDayNightModels` with separate
Lordaeron terrain/unit script references. A bounded census checks those exact
names and their explicitly selected binary counterparts in five named archives
of the supplied Legacy install. Only `War3.mpq` contains the selected binary
members. Each is 1,468 bytes and carries one directional light with a node track
and animated color/ambient-color tracks. Their member SHA-256 fingerprints are:

- terrain: `7655f5b6078dabd6d445f67d49c5fccd47a86eb5ba583c32783ba262edac919a`
- unit: `a5736241e6e11a4624f163f53c7aa44dd13ac577aec5a5860f90b0e435efed49`

The original script names and source member paths are retained only as local
runtime inputs/review bindings. The data reader derives every lighting value
from the supplied model; no measured colors, durations or intensities are copied
into constants. This census does not establish native extension resolution,
archive precedence or the release identity of adjacent archives.

The actual same-context gate now executes this call, selects both qualified
models and matches their names against the captured script arguments. The next
unsupported native is `CreateMIDISound` in a Blizzard.j helper, span
`114713..114745`, reached from Human01 `main`. There are 549 cumulative native
calls and 444 arena identities. Globals and `config` still complete with the
same Program, admission, player profiles and localization. The compatibility
stock catalog still has 94 unresolved records; this is not a playable campaign
or target-catalog claim.

## Robustness and verification

Actual 32-bit WASM testing exposed an unchecked `4 + node_size` in the shared
MDX effect-node reader. A maximal declared node size trapped before returning a
malformed-record error. The shared reader now checks the node against its
already-bounded enclosing record before adding. The existing error API remains
unchanged. Both ribbons and particle-emitter families share this correction
with lights. Two direct decoder tests cover every shared reader, undersized,
oversized and wrapping node declarations, and the valid minimum boundary.

Eight source/Host tests cover changed synthetic lighting values and raw tracks,
whole-set preflight ordering, exact limits, platform layouts, provenance,
malformed/ambiguous models, unassigned state, atomic pair replacement, missing
bindings, argument limits, exact declarations and owned-VM execution. All 252
native play tests pass with five explicit original-input ignores. The complete
model tests and strict native/WASM Clippy/build checks also pass. The original
model corpora parse 3,234/3,234 and 3,249/3,249 with zero failures; these are two
explicit input trees, not executable-derived archive version labels.

Actual 32-bit WASM executes all eight public-API checks plus the direct shared
decoder regressions: nine groups, mask 511, zero imports. The retained baseline
artifact traps on the overflowing node input; the corrected artifact returns
typed failures. Captured commands, immutable failed attempts, fingerprints and
successful outputs are local in `.scratch/daynight-source-root/`. No model or
other original payload was saved or published by the gate. Time-of-day sampling,
GPU lighting, audio initialization and campaign completion remain open.
