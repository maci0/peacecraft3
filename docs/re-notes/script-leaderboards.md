# Script leaderboards

The opt-in `ObjectHost::with_leaderboards` implements the Human02 board path:
`CreateLeaderboard`, `DestroyLeaderboard`, `LeaderboardDisplay`,
`IsLeaderboardDisplayed`, `LeaderboardGetItemCount`,
`LeaderboardSetSizeByItemCount`, `LeaderboardAddItem`,
`LeaderboardRemovePlayerItem`, `LeaderboardHasPlayerItem`,
`LeaderboardGetPlayerIndex`, `LeaderboardSetLabel`, `LeaderboardGetLabelText`,
`PlayerSetLeaderboard`, `PlayerGetLeaderboard`, `LeaderboardSetLabelColor`,
`LeaderboardSetValueColor`, `LeaderboardSetStyle`, and `LeaderboardSetItemLabel`.
Missing declarations and other leaderboard/multiboard APIs remain unsupported.
Present non-constant ABIs and the `leaderboard`/`player` ancestry must belong to
the selected `CreateTimer` source. Admission is configured in the production
campaign factory, with finite application budgets for live boards, aggregate
rows, player assignments and UTF-8 text. Those budgets are resource policy,
not authored game limits.

Each board has a typed identity in the existing session arena and one canonical
live record. Player-to-board assignments are distinct from row ownership and
board display. Public read-only state exposes those assignments, ordered rows,
explicit display, requested display-item count, title, native-order style flags,
RGBA byte colors and a mutation revision. Getters read that same state without
changing revision. `LeaderboardSetSizeByItemCount` retains its executed count;
it does not change the number of stored rows. Destroy invalidates the identity,
removes its assignments and reclaims live board, row and text budget. Historical
handles remain bounded by the existing arena. Replacing a session replaces its
host and all leaderboard state.

Text and rows are validated and fallible allocations completed before semantic
mutation. Errors preserve existing boards, associations, text accounting,
revision and arena identities. They retain the native name through the ordinary
host error and VM source diagnostic path. Duplicate native row additions,
absent-player index/removal, negative/out-of-range indices or size, non-byte
color arguments, NUL text and wrong handle families refuse explicitly. Null/stale
board handles refuse except for the qualified display/destruction wrappers below. These are safe reconstruction contracts where original
invalid-input behavior is unqualified. A player may explicitly disassociate
with `PlayerSetLeaderboard(player, null)`; an unassigned player query returns
null. Original-client null and disassociation parity has not been measured.

Constructor presentation defaults are not invented: title, display, requested
size, style and colors remain unset until assigned. Title/display native queries
refuse that unqualified state. A new board has no rows or player assignments.
The authored Human02 path assigns title and display before querying them, and
sets its style and colors before showing the training counter to the player.

## Shipped-script and runtime evidence

The unchanged Human02 route completed canonical Barracks construction, with
1500 life, then stopped at the selected `CreateLeaderboard` declaration
`143821..143894` through `Scripts\\Blizzard.j:5775`, partial host time
86.078125 after 5508 completed ticks. The local receipt and log are
`.scratch/campaign-determinism-20261025/human02-base-2-gold-depot-receipt.json`
and `human02-base-2-gold-depot.log`. This is pre-implementation evidence, not a
post-fix replay or a completed chapter.

Human02 script lines 2236–2242 create/assign/display the board, add the player
row, set style and both colors, and replace its row label. Later training events
replace that localized label; they do not update its numeric value. Cinematic
branches hide/show the board. The disable trigger waits before destruction.
The selected Blizzard helpers implement row replacement via has/remove/add and
resize via stored row count, subtracting one when the title string is empty.
`LeaderboardGetPlayerIndexBJ` adds one to the native index, establishing the
zero-based native index consumed by the item-label setter. Percent-to-byte
color conversion stays in the supplied script. No title/counter contents,
percent conversions or player counts are copied into production code.

Synthetic tests cover the compiled BJ-shaped row lifecycle and real WTS
resolution, independent players/boards/assignments/display, zero-based ordered
rows, empty text, values/style/colors, removal/destruction/disassociation,
budget reclamation, malformed ABI/ancestry/foreign declarations, duplicate
admission and atomic invalid-input failures. These are host-state tests, not
original-client behavior or rendered-fidelity evidence.

## Source-derived presentation

The names-only consumer reads the canonical board assigned to the local player.
It uses the published binding resolver and retains FDF, template, skin, font
profile, font and texture receipts. It retires the tree on hide/destroy and the
loaded assets on session replacement. It owns its own root rather than replacing
the console HUD submission. Cinematic script display changes remain canonical.

`leaderboard_layout::Rules` admits the reviewed 1.29.2.9231 PE identity and reads
instruction operands at runtime. The normal board top-right attachment follows
GameUI+3fc to CWorldFrameWar3 (constructor 5e77e7 / 608c10 and RTTI), whose two
GameUI anchors at 5e7b35..5e7ba0 provide its insets. The existing admitted message
viewport supplies the GameUI extent; no second native viewport binder is added.
The normal branch 5f3d67..5f3ddb attaches the board to that WorldFrame. Root width
comes from `Leaderboard.fdf`; inspected requested-size, title and row-label paths
do not establish automatic root-to-text width growth.

List parameters originate at 633293..6332d5. Requested row count is capped by
the operand at 62aa88; height uses row height/gap, vertical insets and title state
at 62aa80. Title visibility rewrites container anchors at 652bb0. List slots at
529a30..529db5 use native spacing and twice the horizontal inset for slot width.
Name/value column relationships at 657830..657a99 anchor names to the value text,
icon or row edge according to effective row style. Intrinsic value text would
need measured source-font width; the current names-only branch uses the native
inside span and refuses value/icon presentation rather than guessing a column.

Both row texts read `FontHeights/LeaderBoard` from `UI\\MiscUI.txt` and resolve
`MasterFont` through the selected skin (6395cf..639755). Shadow color/offsets
are read from the row initializer. FDF title template styling remains separate:
`LeaderboardSetLabelColor` updates row labels, not the title. Selected FDF
backdrop art, equal insets and border flags are parsed by existing helpers.
The omitted background tile size uses the source-derived corner-size fallback:
profile corner/background fields 120/124 bind to frame 248/24c at
897a79..897aa7; constructor 87a764 initializes background size, and
87ae25..87ae75 selects corner size when the background divisor is zero.
No extracted scalar or original asset content is stored in production constants.

## Effective row history and limits

Native setters 64eee0/64f080 update current row colors; newly added rows use the
player palette path 62967b -> 5c60d0, not the last global color snapshot.
Canonical rows therefore retain optional effective style/colors. Global setters
update existing rows; new rows copy qualified name/value flags, while icon
creation defaults are resolved from admitted native rules and palette colors
remain unqualified until explicitly set. Human02 adds its row before explicitly
setting style and both colors. Add-after-color/style synthetic tests distinguish
that history with non-stock colors.

The current renderer explicitly refuses visible titles, scrolling, icon/value
columns, unqualified palette colors and alternate timer/multiboard parents.
Those APIs/branches are not silently emulated. Native row overflow/wrapping and
font raster parity remain unverified. Operand/parser and headless ECS checks do
not prove original-client rendered parity or full mission completion. Original
assets and disassembly windows remain local under ignored scratch paths.

## Display/destruction of absent leaderboards

Human02 leaves its leaderboard global assigned after its delayed destruction at
script line 2577, then the cinematic queue/cancel code calls the display helper
at 2955, 3179 and 3400. The unchanged `human02-victory-6-work` run stopped through
`Blizzard.j:5789` after 44415 completed ticks, 63 ticks of the failing batch,
partial host time 694.0. That receipt identifies the failing native, not which
cinematic invocation produced it; no scene-origin or chapter-completion claim
is made here.

Pinned 1.29.2.9231 native inspection qualifies the absent-object branch. The
functional registration maps `LeaderboardDisplay` to 4a1d60 and
`DestroyLeaderboard` to 48f4a0. Both call typed resolver 4822b0 and return when it
returns null. Null input follows 4822e2..482303 directly to a null return. The
resolver's typed object conversion and active-link check at 482328..482338 /
4afb30 reject retired agent links (link field 20). Destruction hides the live
board and calls its virtual destruction slot: leaderboard vtable e7e3d8 slot
5c -> 511040 -> 7f2180 -> 8b8860, which retires that same agent link. This is
native-code evidence, not a fresh original-client runtime probe. Local bounded
disassembly and selected-image identity are retained under
`.scratch/leaderboard-destroyed-20261026/`.

Only `LeaderboardDisplay` and `DestroyLeaderboard` treat null or a known typed
leaderboard tombstone as absent: they return without changing revision, budget,
assignments or live records. A repeated destruction cannot reclaim resources
twice, and display cannot resurrect the removed board. Never-admitted handle
identities and wrong handle families still refuse with source context; their
original invalid-input behavior is not inferred from this guard. Other native
getters/setters retain their existing null/stale refusal. Signature validation
still precedes this branch. The typed arena tombstone, canonical destruction and
renderer retirement paths remain unchanged.

Regression checks distinguish live, destroyed, null, wrong-family and unknown
identities, preserve a separate live board, retain stale getter/setter errors,
and verify fresh creation can reuse reclaimed budgets with a distinct identity.
