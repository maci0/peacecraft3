# Script world bounds and fog areas

`GetWorldBounds` allocates an ordinary rectangle snapshot of the selected map's
complete terrain extent. Preparation derives that extent from the terrain origin,
pathing grid dimensions and the admitted source placement-cell size; it does not
substitute the camera bounds. Changing or removing a returned rectangle leaves
the retained extent unchanged. The existing original-runtime observations in
[unit world](script-unit-world.md) support the complete-grid interpretation.
Fresh snapshot allocation is a recreation choice, not a measured handle-identity
comparison with the original client.

Fog modifiers share one canonical handle store for rectangle, point-radius and
location-radius constructors. Geometry is copied at creation; later rectangle or
location changes do not move an existing modifier. Start, stop and destruction
control the live vision inputs. Player identity, nominal fog state, finite
geometry, declaration ABI and source ownership are checked before allocation.
The existing shared-vision and before/after-unit phases consume both shapes.

Synthetic tests vary bounds, radii and source enumeration values, remove the
input handles, exercise wrong families and invalid geometry, and check that
refused operations preserve state. Rectangle raster tests distinguish a thin
rectangle from its enclosing circle and preserve exploration on invalid input.
These are implementation tests, not original rendered-fog comparisons.

Shared vision now keeps one placement-grid ledger per materialized player.
`advance_tick` and the campaign runtime sample it after script delivery when
admission has a placement grid and a classified day/night state. The minimap
projects that ledger. It does not sample sight itself. Fog and mask switches
change presentation only. An unchanged source list does not walk cells. The
pitch is the admitted placement cell size, including a shorter final cell when
the extent is not a whole number of cells. This is a recreation grid, not a
traced native fog lattice.

Root ran `fog_refusal_keeps_the_accepted_minimap_pixels` once on
`target/debug/deps/peacecraft3-6a44871ed42f5d19`, SHA-256
`ad74d0470290760d08cee16226f914b695eb5f3f70360cdfa0e908cd8264d565`:
1 passed, 0 failed, 0.02s, with `DISPLAY` and `WAYLAND_DISPLAY` unset and null
ALSA. The same binary's item-birth test passed separately. Filesystem hashes
from immediately after that run are in
`.scratch/campaign-visibility-fog-pixel-20261025/receipt.md`. They are not an
immutable `HEAD` claim. The run does not show terrain or tree occlusion.

The minimap writes fogged pixels only after the ledger projection succeeds.
A missing sample, viewer, or projection leaves the previously accepted image
unchanged. With both fog switches off, the authored image alpha is restored
without a ledger. A published campaign that never creates a unit has no
placement grid, so shared vision stays blocked rather than inventing a pitch.
Terrain and tree occlusion are not applied. Unit records still have no qualified
sight height. Sight-changing upgrades and effects are not applied. Script
queries (`IsVisibleToPlayer`, `IsFoggedToPlayer`, `IsMaskedToPlayer`,
`IsUnitVisible`, `IsUnitFogged`, `IsUnitMasked`, and the location forms) are
not implemented; their null-handle policy is unset. Picking still selects
unseen enemies. Combat acquisition does not consult the ledger. World and actor
fog do not either. Those callers should use `ObjectHost::vision_cell` rather
than a second circle test. Successful mission reveal calls do not establish
those features or complete vision fidelity.

## Per-unit vision grants (2026-10-25)

`UnitShareVision(unit, player, boolean)` now admits its non-constant
`nothing` ABI against the same declaration source as `CreateUnit`. The local
imported RoC and expansion `common.j` copies both declare that ABI; those two
copies have identical bytes (SHA-256
`3c21e998bf80d1e04718bd3f2ee7ef2bf1b83359e5b303a56cf59cd61a2afdcd`).
This is shipped-script evidence, not a native implementation trace.

Executed grants live on the canonical unit, independently of ownership and
player alliances. The ordinary source collector includes that unit for its
explicit viewer, retaining the same day/night radii, unit origin and existing
hidden/dead/life eligibility. It feeds the existing per-viewer exploration
ledger and its consumers. Revocation removes the explicit grant; owner or global
alliance sight can still supply visibility. Removal retires the unit source.
No extra fog store or cinematic bypass is introduced.

Synthetic tests cover duplicate grants, independent global alliance toggles,
revocation, wrong families/types/arity/null refusal without mutation, hiding,
showing, removal, and visible-to-fogged transitions retaining exploration in the
canonical ledger. These verify the recreation contract, not original-client
per-unit/global-sharing precedence, death/revival grant retention, null/stale
native behavior or rendered parity. Null and removed handles refuse explicitly.
The unchanged Human01 observation reached the authored cinematic at partial host
time 45.015625 before this implementation; it stopped at the unsupported native.
That observation is not a post-fix replay or a completed mission.
