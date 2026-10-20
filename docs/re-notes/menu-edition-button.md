# Edition-switch button art

The main-menu FDF supplies the RoC icon family. Rebuilding that unchanged tree
when the edition changes therefore leaves the green icon in both menus. The
original client overrides the normal, pushed and disabled backdrop images so
the button advertises the other edition.

The reader qualifies the supplied Legacy 1.29.2.9231 executable by SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
It reads the button/frame names and image paths through the initializer's string
operands. The branch at `0x559fc3` selects the two image families; the selected
recreation edition supplies that choice. Paths and image contents are not copied
into production constants. Unsupported images and mismatched FDF bindings refuse.

The control builder applies all three paths before spawning the tree. Dimensions,
anchors, borders, insets, highlights and input dispatch retain their FDF values.
Edition changes already invalidate the displayed controls. Browser hydration also
imports the client-selected image dependencies for each available background
edition before publishing its archive transaction. A failed image lookup preserves
the previous store, revision and client image.

## Validation and limits

The frozen candidate passes 579 host-library tests and 413 application binary
tests, strict native/all-target and browser-WASM lint, both full application
builds, formatting and diff checks. An explicit supplied-client test reads both
families. Synthetic checks cover bounded strings, unsupported clients, all three
state changes, preserved chrome/layout and atomic rejection of mismatched frames.

The compiled production WASM imports all six source images byte-for-byte through
its actual archive API. Three missing alternate-state images and a changed client
image each refuse without changing the previous committed state. The existing
particle-import/rollback runtime also passes. These execute without DOM or network;
they do not prove rendered pixels.

Fresh window/browser capture attempts were refused by the current environment:
Xvfb cannot bind its display sockets, and the browser controller cannot create a
localhost socket. Those attempts stopped before captures; no fresh original or
three-target visual match is claimed.

A separate private offscreen journal runs the current application systems on
software Vulkan, with Winit disabled and the cameras targeting an image. Its six
TFT/RoC/TFT captures at 1024×768 and 3840×2160 show the edition icon changing in
both directions. The 1024 captures retain their labels. At 4K, labels disappear
after the first edition switch despite the label-fill log entries. This is an
unresolved observation in the offscreen journal, not a verified window/browser
regression or full visual acceptance. All owned render processes are stopped.

Local command receipts, source identities,
fixtures and failures are retained under
`.scratch/menu-edition-icon-root-20261020/`. Original contents remain local.
No public deployment, tooltip-text correction, background/particle fidelity or
campaign playability is established by this change.
