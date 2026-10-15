# Original display-mode enumeration boundary

Static target: explicitly supplied Legacy Warcraft III.exe, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`,
qualified executable version 1.29.2.9231. This does not authenticate the release
of adjacent archives. No game or hardware enumeration was executed.

## Observed native data path

One file-backed code region, `0x7d5000..0x7d5800`, SHA-256
`cd6e6319d62ebefbed7a0a42759b518b0f1c325c5c959f470ccff99e49683eb0`,
contains an aligned complete mode-enumeration routine at `0x7d55d0..0x7d577f`.
Selected PE imports establish USER32.dll EnumDisplayDevicesA at IAT `0xe66788`
and EnumDisplaySettingsA at `0xe66784`.

The outer loop visits device indexes starting at zero until API failure. It
admits devices whose StateFlags have both attached-to-desktop and primary-device
bits. The inner loop similarly visits mode indexes starting at zero until
failure. Returned DEVMODEA fields are filtered by two color-depth alternatives
and an unsigned width minimum. The observed operands are 16/32 bits and 640
pixels, respectively: these are source diagnostics, **not production constants
or a resolution preset list**. This local body has no explicit height, refresh
or aspect filter; unread storage helpers and later consumers remain outside
that bounded negative.

Each offered record preserves device index, mode index, width/height, color
depth and refresh frequency. Width/height occur twice; their distinct meanings
are unresolved. Native record size is 40 bytes. No user-facing label is built
here, and final container order/deduplication is not proved.

The API index contract and output field names agree with
[Microsoft EnumDisplaySettingsA](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-enumdisplaysettingsa)
and [DEVMODEA](https://learn.microsoft.com/en-us/windows/win32/api/wingdi/ns-wingdi-devmodea).
Installed public Windows declarations independently qualify the x86 structure
layout. Documentation does not establish this client's UI or first-run policy.

## What this changes about the implementation plan

The current application reduces device modes to `(width,height)` pairs. That
loses fields the original routine retains. Future mode choices must preserve
stable device/mode identity, depth and refresh, and distinguish a window render
size from an exclusive display mode. Browser render sizes are a separate host
capability; a browser cannot claim this OS-exclusive enumeration contract.

Do not paste observed filter/default values into code. A checked profile may
read them from a caller-supplied, qualified executable, or another independently
qualified source must supply equivalent data. A fixed familiar list of classic
resolutions would not reproduce this device-derived path.

The connection to ResolutionMenu, label formatting, sorting/grouping, supported
saved-mode matching and first-run/default fallback is still unresolved. Merely
finding an enumeration or a null-input reset path proves none of those choices.
The wider Options implementation remains open; no menu setting was enabled by
this evidence-only slice.

## Verification and retained evidence

Local receipt: `.scratch/legacy-display-mode-contract-20261016/HANDOFF.md`,
SHA-256 `38b5a13cda7d0348d6528449e3ff82e6d966d319aac4bb0b849c923e30dd0d12`.
Non-call control flow for both contained routines has no gaps or out-of-region
edges. Exact target identity is unchanged before/after. Public-layout checks
and the selected-import check pass; no source/dependency/build changed.

The initial inspection command ended with a Python module-shadowing error
after recording the region and hashes. That failed command is preserved as a
failure. Later checks consume retained instruction metadata; no body reread was
used to claim a successful original inspection. No raw code, game asset,
credentials or original preference contents are included in the repository.
