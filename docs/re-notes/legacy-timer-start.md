# Legacy TimerStart clock and callback binding

Bounded static evidence for Legacy PE32 `Warcraft III.exe`, VERSIONINFO
`1.29.2.9231`, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
This resolves the previously unread JASS selector in
[legacy-simulation-clock.md](legacy-simulation-clock.md). It is not a live timer
test or a scheduler implementation.

The session has no connected REA tools. A read-only CLI diagnostic reported
missing Codex registration; no configuration or provider was changed. The
investigation used the existing bounded `objdump` method, checked PE raw-section
bounds, direct-call decoding and vtable reads. Commands, disassembly, fourteen
range digests and a static verifier remain local under
`.scratch/timer-start-source-root-20261017/`. No executable was launched and no
original bytes or generated original contents are committed.

## Observed native path

1. Registration at `0x49be2c` passes signature `(Htimer;RBC)V` at `0xe6fc00`,
   name `TimerStart` at `0xe6fc10`, and function `0x4ad0f0` to registrar
   `0x8ccc80`. A second table at `0xcb7114` registers that name with the common
   zero-return stub `0xcbda30`. Its runtime selection was not investigated.
2. `0x4ad0f0` resolves the timer through `0x483220`. A non-null result forwards
   timeout, periodic and handler to `0x4fe6f0`; null returns without that call.
   Invalid-handle behavior inside the resolver remains unqualified.
3. `0x4fe6f0` obtains or creates the callback record at timer `+0x40`, then
   passes the supplied handler to `0x4e4a80`. That function replaces record
   `+0x24` with the argument, including zero. The timeout is copied through the
   timer's `+0x44` value object.
4. The start call at `0x4fe724..0x4fe733` passes timeout, message `0x80204`, this
   timer, periodic, and **zero** to `0x7fbf80`. That wrapper forwards the last
   argument unchanged to virtual slot `+0x8` on the timer subobject at `+0x24`.
   Its constructor installs vtable `0xe7baf8`; that slot contains `0x7fb820`.
5. `0x7fb820` tests the last argument at `[ebp+0x18]`. Nonzero sets subobject
   flag `0x4` and selects `AGILE_DATA+0x48`; zero selects `AGILE_DATA+0x44`.
   The separate `[ebp+0x14]` argument controls periodic flag `0x2`. Construction
   clears the flags. Restart with an existing queue entry marks that entry and
   clears both selector/periodic bits before applying the new inputs.

**Derived binding:** this functional JASS path selects the earlier note's
game-time queue `+0x44`, independently of periodicity. It does not select the
raw-real-time queue. Application clock production, speed mapping, QPC calibration
and actual game-time advancement during startup or pause remain open.

## Callback boundary

Timer object vtable `0xe7cc34` has dispatch slot `+0xc` at `0x4fdf90`. For message
`0x80204`, it calls `0x4fe540`. That function sets timer context through
`0x4ebf00`, invokes object virtual slot `+0x10`, then passes the callback record
to `0x4e4720`, before restoring context through `0x4ebce0`.

`0x4e4720` reads that same record `+0x24`. A zero handler returns without calling
the executor. A nonzero handler is forwarded to `0x474ff0`, whose observed path
obtains execution state through `0x8cc130` when needed and calls `0x8cc230`.
Executor flags/statuses and continuation ownership were not established. The
object virtual call precedes the callback wrapper; this alone does not prove
when subscribed trigger actions complete relative to the handler.

The queue-fire path `0x7fc390` prepares one-shot removal or periodic rescheduling
before invoking the retained object's dispatch slot. It also has a segmented
timeout path using runtime data at `0x115ea08`. Callback restart/destroy/pause,
long-timeout rounding and equal-expiry ordering still need behavioral checks.

## Verification and implementation limits

| Raw range, end exclusive | SHA-256 |
| --- | --- |
| `0x4ad0f0..0x4ad114`, native entry | `50db69b2438f138dd59df8e1242ae9e5f164b106e30a4fa8cd3dbf9f565cb9f6` |
| `0x4fe6f0..0x4fe73e`, object start | `17aa2d28f701880872230d698cc973cb4917c838f8125c2f2ebc3f1ef9c6ee1a` |
| `0x7fb820..0x7fb938`, virtual start | `c495bce69f60ef23f70dbe46a40d915db1451924a02c536e541d051c271c8a95` |
| `0x4fe540..0x4fe5cd`, callback delivery | `dfb1a6469e92f8892fd61beae9d6cd3c0fdb0483d8aa6e75a1eb1131e162c36f` |

The local verifier rechecks all fourteen ranges and the registration, forwarding,
selector and callback relationships. These are static checks, not runtime parity.
`ObjectHost` still refuses `TimerStart`; the Human01 startup refusal is unchanged.
No measured operand was copied into a simulation constant; `TICK_MS` is unchanged.
A recreation may use a different queue architecture while preserving observable
behavior. Delivery must share the existing session's VM and identities and
preserve callback time and lifetime changes, with timing/order differences explicit.
