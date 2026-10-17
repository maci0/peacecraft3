# Source-derived script camera input

`CameraMargins::read` combines an explicitly selected config member with a
caller-reviewed `CameraMarginBinding`. The binding names fingerprinted ranges in
one complete, SHA-256-qualified PE: the section name, four ordered key names and
the little-endian floating-point conversion scale. None of those asset contents
or measured target addresses is embedded in production code. Origins and the
complete-image/config fingerprints remain available on the result.

The existing PE validator is reused. Selected ranges must be initialized,
readable, in bounds and hash-matching. Explicit caller budgets precede image
hashing, PE parsing and config processing; the selected-window aggregate uses
`RuleInputBudget::profile_bytes`. Names must be nonempty NUL-terminated UTF-8,
keys must be distinct, and the scale must be positive and finite. These bounds
are project input policy, not native engine limits.

This is an explicit source binding, not a native-rule decoder or authentication
of a caller's claimed semantic roles. The caller must qualify the selected
references independently. The text reader supports a strict selected-section
subset: UTF-8/BOM, ASCII-insensitive section/key matching, decimal signed integers,
whitespace, `//` comments and whole-line semicolon comments. Missing or repeated
selected sections/keys, malformed selected lines, invalid integers and nonfinite
scaled results refuse with source context. No missing-field default, archive
priority, cache behavior or general native text grammar is inferred.

## Shared execution Host

`ObjectHost::with_camera(margins)` admits exact nonconstant `GetCameraMargin` and
`SetCameraBounds` declarations through the existing signature/source validator.
Script function overrides and repeated admission refuse. Margin calls index the
derived input array; unsupported indices refuse rather than pretending to
reproduce native exceptional-input behavior. Bounds retain all eight executed
finite values and preserve their float bits. Invalid calls leave prior bounds
intact. Bounds remain unset until assigned. No camera projection, movement,
viewport, culling or renderer integration is introduced by this storage.

## Qualified source evidence

The supplied Legacy executable has SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`
and measured PE VERSIONINFO `1.29.2.9231`. The explicitly selected first
registration points to `GetCameraMargin` at `0x491530`. The complete 92-byte
getter ends at `0x49158c`, SHA-256
`91482f5378a6436e866d1b55ba80964bec0a3d55fa7ca531e676d6002035ec25`.
Independent objdump and Capstone agree on all 28 instruction boundaries.

Its four-entry switch table at `0x49158c` selects the referenced key strings in
argument order. Each valid case passes its key, the referenced section string
and an integer zero default to `0x4304c0`. The common return path converts the
integer result to float and multiplies by a referenced scalar at `0xe78348`.
The complete 170-byte callee ends at `0x43056a`, SHA-256
`fd6d7c65b2fd6d954814fe3795de5aacd45f90b1e8ba64058730a0518775888f`.
Its nested callees were not read. The alternative registration getter and active
runtime family remain unestablished. No game, Wine, provider or REA run occurred.

A bounded local census checked three named config paths in five explicitly
selected adjacent archives. `War3.mpq::UI\MiscData.txt` and its `War3x.mpq`
counterpart have identical member bytes, 9,954 bytes, SHA-256
`d097d3cbb4a03719a97591900b45ab42fdb847385819c3d5e7dd7b47d7d9b516`.
The getter's section and key references match the config member. This establishes
a usable caller-selected data correspondence, not the native filename resolver,
archive priority or adjacent archives' release identity. The actual gate selects
the `War3.mpq` member explicitly and reads its contents and the PE scalar locally.

## Verification and next frontier

Eight synthetic tests vary source names, ordering, scale and integer values;
exercise missing/duplicate/malformed fields, fingerprints, initialized ranges,
budgets, invalid scale/overflow, exact declarations, explicit Host admission,
atomic bounds replacement and actual owned-VM calls. All 244 native play tests
pass with five explicit original-input ignores. Strict all-target/all-feature
native/WASM Clippy and WASM library generation pass. The eight public-API checks
also execute as actual 32-bit WASM, mask 255 with zero imports.

With the retained qualified Human01/common/BJ and independently approved player
profiles, globals and `config` still complete. The same admitted VM executes
eight margin reads and one bounds assignment; returned values match the derived
inputs and stored bounds match the actual script argument bits. It then refuses
`SetDayNightModels`, map-script span `384173..384351`, after 548 cumulative native
calls and 444 arena identities. Config itself completes at call 538.

The catalog remains the explicitly qualified compatibility stock-reader policy
with 94 unresolved records, not a target catalog or playable campaign. No original
payload is saved or published by this gate. Evidence, exact commands, immutable
failed attempts and successful receipts are local in
`.scratch/camera-margin-root/`. Launcher/default Host admission, rendered-camera
behavior, day/night model loading and campaign completion remain open.


## Executed bound getters in the startup coordinate profile

`ObjectHost::with_camera_bound_getters()` is a separate opt-in after
`with_camera`. It checks the four exact constant no-argument real-returning
getter declarations, rejects script overrides and duplicate-source declarations,
and requires their source to match `SetCameraBounds`, `GetCameraMargin` and
`CreateTimer`. Existing callers without the opt-in retain their prior behavior.
No initial bounds are supplied. A getter before a successful executed assignment,
or with arguments, refuses. Invalid later bounds assignments preserve the prior
complete eight-value record and its getter results.

The admitted profile takes the coordinate extrema over all four executed corner
pairs. It neither assumes a corner ordering nor reapplies the camera margins;
the map already used those margins in its assignment. Reads allocate no handles
and leave the original eight float values intact. This profile is for an
unrotated bound coordinate frame, qualified by the caller. Camera rotation,
movement, projection, viewport cropping and rendered bounds remain unsupported.

### Evidence and limits

The local Legacy image identified above has complete getter bodies at
`0x491320..0x49135f`, `0x491360..0x49139f`, `0x4912a0..0x4912df` and
`0x4912e0..0x49131f` (exclusive ends). Their non-null paths return stored fields.
`SetCameraBounds` at `0x4a70a0..0x4a719c` forwards four corner pairs to
`0x5e2c30..0x5e30d1`. That routine transforms corners before taking extrema.
The transform builder at `0x5e5520..0x5e5604` depends on camera state, so raw
coordinate extrema are not claimed to reproduce rotated bounds. Capstone and
GNU objdump independently decode the selected ranges; nested transform/trig
helpers and the active registration family were not fully established.

A corrected bounded Wine/Xvfb probe of that exact original executable records
nine probe entries: begin, the executed source assignment, six getter snapshots,
and end. At the unchanged initial orientation, all four getter results match
coordinate extrema for the authored assignment, an irregular synthetic set of
corners, the same corners in reverse order, and a smaller synthetic set. The
probe restores the complete authored assignment and verifies identical getters
before and after the original `InitBlizzard`. Decimal `R2SW` output is evidence
for those exactly representable values, not a native raw-bit transport. The first
attempt wrapped the wrong call and restored zero bounds; it is preserved and
rejected. Only the corrected attempt is accepted. Neither run uploaded assets.

The pinned [Warsmash Jass2 reconstruction source](https://raw.githubusercontent.com/Retera/WarsmashModEngine/f9e0aeed4be372d6016519d0e97b384aa873f374/core/src/com/etheller/warsmash/parsers/jass/Jass2.java)
also implements four-corner coordinate extrema. That is supporting reconstruction
source evidence, not original runtime proof or proof for camera rotation.
No observed map dimensions, margins, orientation or bounds become implementation
constants. Synthetic tests exercise admission and source/contract failures,
unset reads, every nonfinite/invalid setter position, reordered corners,
integer promotion, owned-VM globals and unchanged arena size.

The unchanged Human01 gate now checks eight successful getter results against
the corrected original observation, including float-bit equality after parsing
the exact decimal values. It retains `bj_mapInitialPlayableArea` and
`bj_mapInitialCameraBounds` with the source's margin arithmetic, reaching 903
identities and 144 rectangles with all prior unit/trigger/event checks intact.
The next refusal is `GetPlayerTechResearched` in Blizzard at `441600..441652`,
common declaration `113417..113526`; the selected declaration returns `boolean`.
The observed arguments target source player 0 and tech `Rhrt`, with
`specificonly` true. No missing research state is interpreted as false.

All 370 native and 370 actual WASI play tests pass, with five explicit
original-input ignores. Strict native/browser Clippy passes. Evidence, failed
attempts and exact command receipts are local under
`.scratch/camera-bounds-root-20261017/`. This is startup VM state, not an
application Host factory, rendered-camera parity or a playable campaign.
