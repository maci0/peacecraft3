# Script region requests and subscriptions

## Selected interface and retained state

`ObjectHost::with_regions` admits loaded `CreateRegion`, `RegionAddRect` and
`TriggerRegisterEnterRegion` contracts after rectangle and trigger admission.
`TriggerRegisterLeaveRegion` is supported when its own declaration is loaded.
Signatures, scalar parameter/return types, source identity, no script override
and the selected region/rectangle/boolean-expression ancestry are checked.
`MapStartupBudgets.regions` is an optional caller-supplied `RegionBudget`;
absence keeps these calls unsupported. There is no asset-derived grid size or
native initial state hidden in a default.

Creation uses the same checked identity arena as units, triggers and rectangles.
Each successful `RegionAddRect` retains the actual rectangle identity and its
current finite ordered bounds in an append-only call record. Later rectangle
mutation or removal does not rewrite that historical request. The records are
not a native cell raster or a membership result. Repeated requests are retained
separately under the recreation resource policy. Region count, aggregate
rectangle-addition count, event count and shared identity capacity are bounded
before their corresponding growth; failed calls preserve prior state.

Enter/leave registrations require an actual trigger and region, plus null or an
existing Filter/Condition wrapper. Their event tokens retain the trigger, region,
filter identity and direction. Registration does not evaluate a predicate, run
an action or change the trigger's optional enabled state. Reserved identities
with matching family names are insufficient. No second VM, arena, simulation
world or clock is introduced.

## Evidence boundaries

The selected `common.j` declares the native interface. The selected `Blizzard.j`
`TriggerRegisterEnterRectSimple` helper creates a region, adds its supplied
rectangle, then registers entry with a null filter; the leave helper follows the
same call structure. Supporting those explicit calls does not establish native
cell coverage, boundary inclusion, geometry changes or event ordering.

Synthetic owned-VM tests cover call-time bounds after rectangle movement,
independent and repeated subscriptions, null/filter/condition inputs, disabled
triggers, unexecuted callbacks, invalid and reserved identities, separate budgets,
shared capacity, source/signature/type refusals and an absent leave contract.
The application test uses actual map delivery and the production startup factory;
failed replacement without opt-in or with an exhausted event budget preserves
the previous context, lifetime witness and subscription.

## Unchanged chapter execution

The selected Human01 Program runs through the production factory with the
retained compatibility catalog, qualified startup inputs and explicit verifier
clock origin. Six regions, six rectangle-addition requests and six entry
subscriptions succeed. Every retained rectangle identity and exact `f32` bound
matches its executed constructor; the verifier also checks that no successful
rectangle mutation/removal affects those inputs. No leave subscription is
executed by this prefix; leave support is covered by synthetic tests.

Globals and `config` complete. `main` next refuses `TriggerRegisterDeathEvent`,
map span `184271..184338`, common declaration `81184..81277`. The shared arena
holds 1007 identities, 144 rectangles, 155 units, 33 triggers and 14 condition
wrappers/associations. The earlier explicit disabled state stays assigned, and
the clock does not advance during startup. Main remains incomplete.

Commands, immutable input/source/artifact hashes and outputs are local in
`.scratch/region-root-20261017/`. This is reconstructed script execution and
retention evidence. No new original-client, render or site run was performed.

## Remaining work

Region removal, rectangle/cell subtraction, cell additions, point/unit membership,
region enumeration, event context and delivery remain unsupported. Native
rasterization, repeated-registration identity and later rectangle effects have
not been measured. These retained requests alone do not make a trigger fire.
Full campaign execution, rendered gameplay and the 100-consecutive original
baseline remain open. Original files and extracted contents remain local.
