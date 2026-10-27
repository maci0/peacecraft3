# Explicit unit vertex color

The selected Legacy native registration identifies
`SetUnitVertexColor(unit, integer, integer, integer, integer) -> nothing` in the
same common-script source as unit creation. Its retained executable identity is
SHA-256 `3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Local disassembly and registration readback are retained under
`.scratch/unit-vertex-color-20261026/`; no original bytes are included here.

## Native evidence

The public setter resolves the unit, skips an absent resolution or absent model,
then saturates each signed channel to the RGBA-byte range. It packs and writes
all four channels into one existing unit color field and invokes that unit's
model-color update virtual. Repeated calls replace this field. Ordinary creation
and type-change paths seed the same field from a rawcode-keyed type color getter.
The setter therefore replaces that initial unit color instead of multiplying the
previous assignment. Team-color assignment is a separate native and field.

The ordinary model update forwards the complete packed color to the model. It
also contains special status-color branches and a conditional additional color
composition. Those branches have not been fully classified here. The exact
native type-cache writer linking every metadata channel to the packed initial
field remains an internal evidence gap; the recreation's existing initial unit
tint continues to use the resolved UnitUI red/green/blue metadata. There is no
original runtime color/pixel comparison in this slice.

## Canonical contract

The existing executed unit retains `vertex_color: Option<[u8; 4]>`. `None` means
no script assignment: the consumer keeps its source-definition tint and authored
model opacity. `Some` contains the explicit saturated channels; RGB replaces the
initial unit tint, and RGBA multiplies authored model/geoset color and opacity.
It never accumulates against the prior script assignment. No source alpha value
is invented for an unassigned unit. The renderer reads live state rather than
rebuilding the actor for each fade update.

The native declaration is optional for unrelated programs and must have the
loaded signature and creation source when present. Null and typed-retired units
are no-ops; unknown and wrong-family identities retain errors. The mutation does
not change ownership, team color, activity, visibility, vitals or gameplay clock.
Retirement removes the one canonical record; no record is resurrected.

The existing game-cache encoding and Mirror Image snapshot do not yet qualify
explicit color persistence. Their preflights refuse an explicit color before
allocation or mutation rather than silently discard it; no cache version changes.
Transformation resets/persistence, special status-color precedence, attachment
and particle inheritance, material blend behavior and original rendered parity
remain separate fidelity questions. Core synthetic tests establish state and
trust-boundary behavior, not pixel parity.

## Renderer consumer and validation

Each actor retains its source layer blend/depth flags and reads the unit's live
RGBA on every sample. Model animation resamples authored geoset color/opacity
before the explicit unit multiplier is applied, so repeated fades do not compound.
Unassigned units retain the existing source tint. Zero composed alpha hides only
the bound geoset; it does not change the canonical unit's `shown` state.

For fractional unit opacity, solid and cutout layers use alpha blending with the
cutout override disabled; full unit opacity restores their original layer flags.
This is a recreation choice to render the authored fade, not proof of the native
filter-selection algorithm or ordering. Existing blend/additive layers retain
their source modes and depth flags. Fractional unit opacity on modulate layers
explicitly reports an unqualified state rather than silently ignoring alpha.
Particles and attachments have not been extended by this consumer.

Synthetic actor tests check source fallback, nonstock channel scaling and live
replacement. Material tests check reversible mode/depth/cutout handling. An
explicit headless Vulkan readback checks opaque, fading solid, fading cutout and
additive samples with composed authored opacity. These passes establish the
material consumer, not selected-model or original-client pixel parity.
