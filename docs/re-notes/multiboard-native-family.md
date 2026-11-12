# Multiboard native family: static investigation

Reviewed image: Warcraft III.exe 1.29.2.9231, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
The hash was checked locally. These are static native-code observations, not
runtime observations or a rendered fidelity claim.

## Functional registration

The registration region `49ffeb..4a020e` maps the following entry points:

| Native | Handler |
| --- | --- |
| CreateMultiboard | 48e690 |
| DestroyMultiboard | 48f4f0 |
| MultiboardDisplay | 4a34c0 |
| MultiboardMinimize | 4a3630 |
| IsMultiboardDisplayed | 499470 |
| IsMultiboardMinimized | 499490 |
| MultiboardClear | 4a3490 |
| MultiboardSetTitleText | 4a3920 |
| MultiboardGetTitleText | 4a35f0 |
| MultiboardSetTitleTextColor | 4a3950 |
| MultiboardGetRowCount | 4a35d0 |
| MultiboardGetColumnCount | 4a34e0 |
| MultiboardSetColumnCount | 4a3670 |
| MultiboardSetRowCount | 4a3900 |
| MultiboardSetItemsStyle | 4a37e0 |
| MultiboardSetItemsValueColor | 4a3860 |
| MultiboardSetItemsValue | 4a3820 |
| MultiboardSetItemsWidth | 4a38c0 |
| MultiboardSetItemsIcon | 4a37a0 |
| MultiboardGetItem | 4a3500 |
| MultiboardReleaseItem | 4a3650 |
| MultiboardSetItemStyle | 4a36c0 |
| MultiboardSetItemValue | 4a36f0 |
| MultiboardSetItemValueColor | 4a3720 |
| MultiboardSetItemWidth | 4a3770 |
| MultiboardSetItemIcon | 4a3690 |
| MultiboardSuppressDisplay | 4a39a0 |

Names and signatures reside in `e75dc4..e761ac`; registration calls `8ccc80`.
The existing suppression note establishes the global UI flag separately.

## Observed state relationships

Board wrappers resolve through `482640`; item wrappers use `482510`.
The inspected setter wrappers return when resolution returns null. This alone
does not establish every wrong-family or stale-handle case.

Row count getter `4a35d0` reads board offset `48`; column count getter
`4a34e0` reads offset `4c`. Column setter `5123e0` stores its argument at `4c`
before calling the presentation resolver `511cd0` and forwarding to `65f900`.
Clear `4a3490` calls row setter `5125f0(0)`, then column setter `5123e0(0)`.
Limits and invalid-count behavior in the presentation routines remain open.

GetItem `4a3500` creates an item object and calls `511de0` with the board,
row and column arguments. `511de0` retains the board at item offset `24` and
stores the two coordinates at `28` and `2c`; it does not resolve a cell there.
Thus the wrapper represents board coordinates rather than a captured cell
pointer. The behavior after shrinking and growing the board still needs tracing.
ReleaseItem `4a3650` invokes virtual slot `5c` on a resolved item.

Item setters `512400`, `512460` and `5124d0` first validate the retained board's
agent link, require its retirement word at offset `20` to be zero, then obtain
the presentation object and forward the stored coordinates. This supports
deferred coordinate resolution; it does not yet establish cell bounds behavior.

Color wrappers `4a3720`, `4a3860` and `4a3950` load each integer argument's
low byte and pack ARGB. Out-of-byte integers are truncated in these wrappers,
not rejected or clamped. Width wrappers forward the pointed-to single-precision
real; downstream validation is not established.

`511cd0` lazily creates and retains the presentation object at board offset
`24`, requiring a game UI and a successful `5eda50` check. The object is bound
back to the board through `660250` and initialized through virtual slot `d0`.
Minimize setter `511fe0` retains the argument at board offset `2c`, forwarding
to `65efa0` for nonzero and `65eec0` for zero when presentation exists.

## Open implementation work

Resolver
`482672..482693` explicitly returns null for input zero; the wrappers return
without reaching presentation on that result. Destruction's guard is at
`48f4fe..48f50a`. An experimental null-board implementation passed the gameplay
suite (3,140 passed, 0 failed, 144 ignored), but downstream and strict Clippy
validation had not completed at the release cutoff. That code and its tests
were discarded; this change contains investigation notes only. No new native
admission or rendering is included. Constructor
defaults, destruction, display selection, row/column limits, cell initialization,
global setter history, deferred item bounds, and the authored FDF anchor chain
still require decoding. Other existing refusals remain in place. No original
client runtime or isolated chapter probe has been run for this subset.
Local disassembly is retained only in ignored `.scratch/multiboard-*.asm` files;
no original bytes or listings are included here.
