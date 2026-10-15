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
