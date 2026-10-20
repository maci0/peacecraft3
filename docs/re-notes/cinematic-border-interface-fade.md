# Cinematic border insets and interface fades

Image: `Warcraft III.exe`, Legacy 1.29.2.9231, SHA256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`, image base
`0x400000`. Addresses are VAs unless marked file offset (FO) or RVA. Static
reads use `objdump -d -M intel --start-address=<VA> --stop-address=<VA>` and
`objdump -s`; reference scans use a section-aware PE reader in the local scratch
area (`.scratch/piece05/`, not committed). No Ghidra run is recorded here.

## Unequal border insets: order unresolved

Source: `UI\FrameDef\UI\CinematicPanel.fdf`.

- `CinematicBottomBorder`: `BackdropBackgroundInsets 0.0 0.01 0.0 0.0`, edge
  flags `UL|UR|T`.
- `CinematicTopBorder`: `BackdropBackgroundInsets 0.0 0.0 0.01 0.0`, edge flags
  `BL|BR|B`.

These are the only FDF inset declarations in the local FDF copies whose four
components differ. The file text does not give the component order.

Native property table (verified): an array of 8-byte `{dword token id, dword
name VA}` records in `.rdata`, starting at `0xEC53F0`. Token ids start at 256.
`BackdropBackgroundInsets` is id `0x123` (291). Its record is at VA `0xEC5508`,
and its name string `BackdropBackgroundInsets` is at VA `0xEC5C68` (FO `0xAC4A68`).
The name VA appears once as a 4-byte little-endian value, at VA `0xEC550C`, which is
the name field of that record. An RVA form (`0xAC5C68`) has no 4-byte reference.
The table base and individual records have no direct absolute reference in `.text`
or `.rdata`, so the lookup reaches them through a computed or copied base. The
property-reader handler for id 291 is not found yet.

An earlier note listed the string at `0xEC5F28` and reported no reference. That
was an arithmetic error (the file offset was converted wrongly), and the
three-byte search did not test the four-byte VA. Both are withdrawn.

Community lead, unverified: a Hive Workshop backdrop guide gives the classic FDF
order as right, top, bottom, left (`hiveworkshop.com/threads/ui-what-are-backdrops.315967`).
Under that order the bottom border's `0.01` lands on its top side, where its
decorated edge is, and the top border's `0.01` lands on its bottom side, where its
decorated edge is. This is consistent but not evidence. The order is established
only by the native handler that stores the four floats, or by a runtime
differential with unequal authored insets.

## Interface fades: verified branch and helpers

`ShowInterface(show, duration)` is `0x5F3958` (the entry with the `push 0xff`
reference of the earlier evidence). Verified in its body:

- `0x5F3B7E`: `comiss xmm0, [0xE68BC8]` compares the duration with `0.0`
  (`0xE68BC8` holds `0x00000000`).
- `0x5F3B8B`: `jbe 0x5F3C57`, so a duration at or below `0.0` takes the
  immediate branch. `comiss` with NaN also sets the flags that `jbe` tests.
- `0x5F3B91` (duration above `0.0`): writes `1` to `[esi+0x18C]`, calls the
  vtable at `[eax+0x6C]`, then calls helpers `0x5BF740` (type `0x1D`, frame
  `[esi+0x468]`), `0x5BF6C0` (type `0x1C`, frame `[esi+0x45C]`), `0x5BF6C0` (type `0`,
  frame `[esi+0x464]`) and `0x5BF380` (type `0x1E`, frame `[esi+0x440]`). Each passes
  the duration float.
- `0x5BF380`: when its frame argument is nonzero and the duration is not `0.0`
  (`ucomiss` and `jnp`), it stores the frame at `+0x38`, phase `0` at `+0x34`,
  the duration at `+0x2C`, and two further arguments at `+0x3C` and `+0x40`, then
  calls `0x796D00` and `0x840980`.
- `0x840980`: stores the duration in `+0x10`, starts a timer through `0x840370`
  with callback `0x840AA0` when the duration is above `0.0`, and records the
  handle at `+0x14`. Its getter `0x840A00` returns the timer value through
  `0x83FB10` only while the handle is set.

Timer and value path (verified by the same `objdump` reads):

- `0x840370(seconds, callback 0x840AA0, context)`: calls `0x846030` with the
  float timeout and the callback, and keeps the returned timer handle. This is a
  one-shot timer in the game timer manager (manager `0x1160640`).
- `0x840AA0` (expiry): copies the context's `+0x18` into global `0x10E011C`, and
  the float at `*arg0` into global `0x10E0124`. A zero `+0xC` clears the state
  and calls the listener vtable `+0xC` with `0x10E0114`. A nonzero `+0xC` re-arms
  through `0x840A20`, which cancels and starts a new timer with the stored
  `+0x10` seconds.
- `0x840A00` (getter): returns `0.0` when the handle at `+0x14` is zero or
  `+0xC` is nonzero. Otherwise it returns `0x83FB10(handle)`.
- `0x83FB10`: looks up the timer and returns `0x845ED0(manager, handle)`, then
  releases its reference.
- `0x845ED0`: `remaining = max(0, deadline_ms - now_ms) * 0.001`, with `0.001`
  stored as float `0x3A83126F` at `0xE8DF60`. Remaining time is seconds, and it
  is zero once the deadline passes. `now_ms` comes from `0x796C90`, an indirect
  jump into the clock import.

So the fade's observable value is the seconds left on a one-shot timer of the
authored duration, clamped at zero. It falls linearly in game time from the
duration to zero. Not established: how the frame consumer maps remaining seconds
to alpha or position, which track type (`0x1C`, `0x1D`, `0x1E`) reads which
getter, and whether the full track uses the duration as its denominator. No
alpha curve is recorded here, so no interpolation is implemented.

Notification object (verified):

- `0xE5BCA0` stores vtable `0xE6DD6C` into the static object at `0x10E0114`. The
  object's `+0x8` is `0x10E011C` (kind) and `+0x10` is `0x10E0124` (float). The
  expiry callback writes these only by absolute address, and no code reads them
  by absolute address. Readers must reach them through the object pointer.
- Vtable slot `+0xC` is `0x4692E0`. It calls slot `+4` of its argument, then
  dispatches through `0x440C70` on channel `0x10EC880` with id `-2`. The other
  channel references are in `.text` at VA `0x468E66`, `0x4692F5`, `0x46931A` and
  `0x469406`. Those are the candidate registration or fire sites, not yet read.
- Next reader: the handler on channel `0x10EC880` that loads `+0x10` from the
  notification object. Its alpha or position mapping is still open.

## Consumer state

`crates/pcx-play/src/presentation_host.rs`: a finite `ShowInterface` sets the
visible and control flags at once and keeps `(game time, duration)` only when the
duration is above `0.0`. A negative duration now takes the immediate path, as the
native `jbe` does. A non-finite duration is still refused: that refusal is an
argument policy, not a verified native behavior. The consumer applies visibility
immediately; no rendered fade exists.

## Gates

Parser and host tests do not prove rendered alpha or native timing. The
transmission panel's ignored gate, which needs a local original FDF directory,
covers the border source. The inset order gate waits for the native handler above.

Channel sites (static, `objdump`; FO-to-VA by `+0x400C00`):

- `0x468E50`: when global `0x113B6B0` holds an object, calls its vtable `+8` with
  `1`, clears the global, then pushes channel `0x10EC880`, id `-2`, flag `1` into
  `0x458090` on registry `0x112D7A0`. This is the unregister or cancel side.
- `0x469310`: pushes flag `1`, id `-2` and channel `0x10EC880` into `0x4580F0` on
  registry `0x112D7A0`, then walks a linked list at `[ebx+4]`. It moves matching
  records and returns through `ret 0xC`.
- `0x469400` (the site at VA `0x469406`): calls `0x440610` with channel
  `0x10EC880`, id `-2` and `eax+0x20`. It zeroes the returned record's `+0xC`
  through `+0x1C` (`0x46941C`–`0x46943F`). This is the get-or-create slot that the
  registration path initializes.
- `0x4692E0` (slot `+0xC`): calls the argument's vtable `+4`, then `0x440C70` with
  the argument, channel `0x10EC880`, id `-2` and `0`.
- `0x440C70` and `0x440610` share a prologue. Both check global `0x1130F3C`, call
  `0x43B640` and `0x441870` when it is clear, and raise error code `0x8510007D` or
  `0x85100081` on a null argument. They are the generic handle dispatch and
  lookup, not fade-specific code.

Status: the channel is a generic registry key shared by the trigger and handle
layer. Which record receives the expiry value at `+0x10`, and the frame mapping
from that value to alpha, are not yet established. This page makes no alpha curve
claim from the timer alone.
