# Executed campaign transmission panel

`campaign_messages` now projects executed `SetCinematicScene` state using the
selected receipt-bound `UI\\FrameDef\\UI\\CinematicPanel.fdf` and its
`EscMenuTemplates.fdf` declarations. It retains receipts for the FDF files,
`war3skins.txt`, resolved font and images; session replacement releases the tree,
images, fonts and materials. No original contents are retained in tracked fixtures.
The selected local race preference chooses artwork aliases with the same
race-then-Default reconstruction policy as the campaign console. A missing race,
font, art, anchor or malformed record is a consumer diagnostic.

The previous centered text-only transmission used fixed percentage placement and
the trigger-message font. Speaker/dialogue anchors, dialogue width, font alias,
font height, authored colors, wrapping and shadow now use the original records.
Rich-text color runs and text come from the executed scene. The portrait blank
backing/cover and the cinematic border pieces use their authored rectangles and
artwork. Layout stays relative to the campaign UI viewport. Inactive campaign
views hide the transmission root as well as timed messages.

Background sampler addressing follows `BackdropTileBackground`: tiled layers
repeat using the authored tile-size-derived repeat counts, while stretched
backgrounds and edge slices clamp. Cache identities include that per-use mode, so
a shared image path used by both layers retains both sampler choices. This is
the rendering translation of the authored flag, not proof of native sampler
implementation. A synthetic test checks shared-path separation and both axes.

The source border atlas mapping and horizontal sampling reuse the existing menu
renderer. These are recreation policies, not new native renderer evidence. The
native text frames are intrinsically sized: the current projection primes one
source font height for the speaker anchor, as the menu renderer does, then Bevy
shapes visible text. Actual shaped speaker height feeding the following anchor,
portrait model rendering/animation, scene player-color interpretation, and paired
native/browser/4K comparison remain open.

Two cinematic border backgrounds use unequal four-component insets. Their
component order is not established by the existing renderer, so those background
layers are omitted and the exact frame is reported in `CampaignMessages.last_error`.
Their independently supported border pieces still render. Equal-inset portrait
backgrounds render. This is an explicit partial transmission surface, not a full
cinematic/dialog fidelity claim. JASS script-button dialogs are a separate, unimplemented surface. There is no
live `DialogCreate`/`DialogDisplay`/`DialogAddButton` state, handle lifecycle,
button-event dispatch or renderer in the current gameplay Host. Parsing
`ScriptDialog.fdf` declarations does not implement those natives. Its dynamic
button placement and sizing also require evidence; no content or layout is
substituted. The local unchanged Human01 map does not call these dialog natives
directly, while the shipped `Blizzard.j` wrappers and game-over helpers do use
them. Those paths remain unsupported and must not be presented as working.

Per-source reads and raster decode allocations are bounded; decoded transmission
images have a 64 MiB aggregate ceiling. These limits are application resource
policy, not original game measurements. Synthetic tests mutate the font/width
and check the anchor chain and RGB/RGBA inheritance; a separate ignored gate
requires an explicit local original FDF directory. Parser tests establish source
derivation, not rendered appearance or native behavior.

A fresh native Xvfb/Vulkan software-rendered Human01 run on 2026-10-20 visibly
shows the executed speaker and speech text with source border/portrait backing.
The deliberately omitted backgrounds expose the terrain behind them; the portrait
model remains absent. Local capture: `.scratch/campaign-integration-20261020-v6/speech.png`.
This verifies this native surface only, without original-game/browser comparison.
