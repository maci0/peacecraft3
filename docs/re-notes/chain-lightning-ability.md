# Chain lightning

The cast is the missile-lightning class inside the `Ocl1`/`Ocl2`/`Ocl3` metadata
family. `useSpecific` also lists healing wave. The split is the class row's
`code`, `LightningEffect` and `Missileart`. Healing wave has lightning and no
missile art. Tooltips are not consulted, including a heal tip that contains
the word "damage". A custom alias binds when its parent's `code` is that class.

`data`, `type`, `minVal` and `maxVal` choose the columns. The integer row is
the number of targets hit, including the primary. The real row whose `maxVal`
is at most 1 is the reduction. The other real row is damage per target. The
ordinal's letter is `DataA` through `DataZ`, the same mapping as ability
overrides. Later hits use `previous * (1 - reduction)`.

`Rng` is caster to primary. `Area` is previous target to the next. Both are
center distances, with no collision radii. Equal distances take the lower
handle. Damage is applied in the cast step through `damage_unit`. `Missilespeed`
is stored on the cast and is not a travel delay. Those choices are recreations.

One spell-effect event names the primary. `organic` and `invu` filter only when
those tokens are present. Stock rows omit `invu` and `vuln` and still hit
vulnerable units, so a missing `vuln` is not a refusal. `damage_unit` does not
reduce invulnerable life even when `invu` made the unit a legal target.

`take_chain_lightning_casts` returns each hop's source, target, positions and
lightning token. A drained cast looks up that token in `Splats\LightningData.slk`
and draws a straight ribbon using that row's texture, width, color, duration,
and texture scale. Duration is the cell value in seconds. The length
sampler repeats on V so `TexCoordScale` is not clamped away. A root
existing-binary run of `campaign_actors::chain_lightning` passed 8 tests on
the filesystem hashes recorded in
`.scratch/campaign-parented-pre2-20261025/root-full-lightning.md`. That is not
an 8-pass Cargo run and not a git HEAD. The earlier 7-pass/1-fail CRLF receipt
and the single fixed row test stay separate.

`AvgSegLen` can split a hop into straight pieces of that row's length. Those
pieces stay colinear, so the cut does not create the visible kink.
`NoiseScale` stays unused until a runtime sample relates that cell to the
kink. No noise offset is invented. `version` is 0 on some shipped rows and 1
on others, including the chain bolts and the healing bolts. The accepted
straight ribbon does not refuse either value. An unknown `version` is
reported and is not full parity. `Missilespeed` is still not travel.
`Missileart` and `Targetart` are ability-profile model paths. Their lifetime
is the model's sequences and particle lifetimes, not
`LightningData.Duration`. Attachment, height, and scale belong to that MDX,
read by the existing model runtime. `Mdlx::nodes` can see `ATCH` headers.
The extra attachment fields after the node are not a separate reader yet.
None of those steps are done. `admit_chain_lightning`
belongs next to
`admit_holy_light` after combat and hero skills. This crate does not call it
from map startup.

Order replacement, unit removal, death and `UnitResetCooldown` drop a pending
cast. Ending one ability's cooldown clears that alias's deadline only.
