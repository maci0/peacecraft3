# Simulation clock: exact Legacy target (RE-Q5, partial)

Bounded static investigation. It identifies the native turn scheduler and the
game-time sub-step that drives the agent timer queues. It does not close RE-Q5:
render-frame invocation, several runtime-initialized scale fields and the full set
of sub-step consumers remain open (see *Unknowns*). `pcx_sim::TICK_MS` is a
project convention and is not supported or refuted by this note.

## Binding

- Target: locally supplied Legacy `Warcraft III.exe`, PE32 x86, VERSIONINFO
  `1.29.2.9231` (read from the file, product string `1.29.2.9231 (9d2368a59e)`),
  SHA-256 `3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
- The 1.26.0.6401 `Game.dll` and the `rea/` artifacts derived from it are a
  different build. No address or value here comes from them.
- No REA session was live and the retained REA snapshots cover only the
  neutral-player getter, so this note uses address-bounded `objdump`/`rizin` reads of
  the same file. The method reproduces the retained neutral anchor: registration
  `0x4a19ba` pushes name `0xe78300` with function `0x4946b0`, as recorded in
  [native-neutral-players](native-neutral-players.md).
- Range digests (SHA-256 of raw file bytes per VA range) and disassembly are kept
  locally in `.scratch/legacy-tick-evidence/range-digests.json`; no original bytes
  are committed.

## Observed chain (static instructions)

Each step was read from disassembly of the pinned file.

1. **Native anchor.** Registration at `0x49be57` passes signature `(Htimer;)R`
   (`0xe6fc1c`), name `TimerGetElapsed` (`0xe6fc38`) and function `0x4ad060` to the
   native registrar `0x8ccc80`, the same registrar the neutral getter uses.
2. **Timer elapsed.** `0x4ad060` resolves the handle (`0x483220`) and calls
   `0x4fe470`, which returns `timeout(+0x48) - remaining`. While the timer runs,
   `remaining` comes from vtable slot `+0x18` of the subobject at `+0x24`. RTTI on
   that vtable (`0xe7baf8`) names it `CAgentTimerExtended`.
3. **Remaining.** Slot `0x7fc0b0` reads the global `AGILE_DATA` pointer `0x115e9ec`
   and selects queue `[+0x48]` when timer flag bit 2 (`0x4`) is set, otherwise
   `[+0x44]`. On its `[+0x14] != 0` path it returns
   `entry.expire(+0x4) - queue.now(+0x40)` (soft-float subtract `0x9a00b0`), plus
   period multiples when the repeat count `[+0x16]` exceeds 1.
4. **Schedule.** `0x7fb940` sets `expire = queue.now + max(delay, min_delay)` via
   soft-float add `0x9a01c0` and passes an incremented per-queue counter (`+0x50`) to
   the insert call `0x7fbe00`. `min_delay` is `.data` `0x10db050`, whose initial image
   value is `0.0001f`.
5. **Advance and fire.** `0x7fac80(float *dt, queue)` does nothing while the queue
   pause bit (`+0x4c` bit 0) is set. Otherwise it computes `t = now + *dt`, sets
   `now = t` and calls `0x7f94a0`. `0x7f94a0` pops every entry with `expire <= now`,
   sets `now` to that entry's `expire` before invoking it, re-inserts periodic entries
   at `expire + period(+0x8)`, then restores `now`. Expiry callbacks therefore observe
   their exact scheduled time, not the end of the step. The `t >= [+0x48]` branch
   handles a wrap/rebase and was not analysed further.
6. **Clock drivers.**
   - `0x7f4910(float *dt)` advances queue `AGILE_DATA+0x44` by `*dt` in one call,
     plus three further queue sets rooted at `[0x1161d90]+0x14/+0xbc/+0x164`. It is
     skipped while `AGILE_DATA+0x3c != 0`.
   - `0x7f4820(float *dt)` advances queue `AGILE_DATA+0x48`, plus
     `[0x1161d90]+0x68/+0x110`, in chunks of at most `[0x115e9f4]` (a static
     initializer at `0x40d060` sets it to `299.0f`), then advances the remainder when it
     is at least `[0x112d88c]`. It is skipped while `AGILE_DATA+0x40 != 0`; `0x7f6800`
     sets that word to 1.
7. **Event handlers.** `0x5f24c0` registers handlers through `0x5adb40`:
   event `0x40090092` -> `0x5ea6e0`, which calls `0x7f4910(&event+0x14)`; event
   `0x40090093` -> `0x5ea700`, which forwards `event+0x14` to
   `[0x113d7f8]+0x3fc` (`0x60ad10`); event `0x40090094` -> `0x5ea690`, which forwards
   `event+0x14` to `0x60acb0` on the same object and calls `0x7f4820(&event+0x14)`
   unless `[0x113d7f8]+0x298 != 0`.
8. **Turn scheduler `0x5aefd0(now)`.** It is reached from `0x5ab7f0`, which calls
   it with the clock `0x796ca0`. That clock returns
   `QueryPerformanceCounter * [0x1140630]` when `[0x114062c]` is set, otherwise
   `GetTickCount`. `0x5ac1e0` registers `0x5ab7f0` as callback id 5 via `0x8405d0`.
   The fallback API supplies milliseconds; the QPC scale's initialization was not
   inspected, so its converted units remain conditional. The SDK identifies
   [GetTickCount](https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-gettickcount)
   in milliseconds and
   [QueryPerformanceCounter](https://learn.microsoft.com/en-us/windows/win32/api/profileapi/nf-profileapi-queryperformancecounter)
   in counter counts. Per call:
   - `elapsed = now - [+0x2614]` (unsigned clock delta), stored raw.
   - In the `[+0x898] == 0` branch, `elapsed` is scaled by `[0xe8fb44 + 4*k]`, where
     `k = clamp(([+0x2624] + [+0x3c4] - [+0x2618] - trunc(1.5*[+0x2620]) - elapsed)
     / [+0x2620] + 3, 0, 6)` indexes `{0.25, 0.4, 0.67, 1.0, 1.5, 2.5, 4.0}`. When
     `[+0x2620] == 0` the factor is `1.0`. A stall check on `[+0x42c]`/`[+0x438]`
     can set the scaled value to 0.
   - Unless `[+0x2688]` is set, the real-time budget gains the scaled value:
     `[+0x2618] += elapsed * [+0x2680] / [+0x2684]`, capped at 4000.
   - **Quantum loop:** while `[+0x2618] >= 25`, the loop first pulls (`0x5a9d70`,
     `0x5aaba0`) until `[+0x3c4] >= 25` or nothing is available. It then subtracts 25
     from both `[+0x3c4]` and `[+0x2618]`. In states 4 and 5 (`[+0x3b8]`) it
     computes `N = int(S[[+0x2670]] * M * [+0x2678] + carry)`, where `S` is `.data`
     `0x10dc4fc` with initial entries `{3, 4, 5}`, `M` comes from `0x5237a0`/`0x5237c0`
     (`1` when the former returns 0), and `carry` is the fractional remainder kept in
     `[0x113cb44]`. It posts event `0x40090092` `N` times with `event+0x14 = [0x113cb40]`,
     and adds 5 to `[+0x2038]` and to the ring counter `[+0x268c]` (mod 250) per post.
     The loop exits when `[+0x2680] > 1` and more than 200 `GetTickCount` ms have
     elapsed since entry; in that case the budget `[+0x2618]` is zeroed.
   - After the loop it posts `0x40090093` with `5*sum(N) * 0.001f` and, when raw
     `elapsed != 0`, posts `0x40090094` with `elapsed * 0.001f` (`0xe8df60` holds
     `0.001f`).
9. **Sub-step size.** Static initializer `0x406730` stores `float(5) / float(1000)`
   into `[0x113cb40]` using soft-float divide `0x9a02d0` (`a * reciprocal(b)`),
   nominally `0.005` seconds. The bit-exact value depends on the soft reciprocal
   `0x9a1d00`, which was not evaluated.

## Derived relationship (inference from the chain above)

- The native scheduler drains its budget in fixed **25 source-clock units**. On
  the GetTickCount branch these are milliseconds; the QPC branch needs the
  scale-field producer to establish the same conversion. A quantum
  requires both accumulated real time and at least 25 units in `[+0x3c4]`, which is
  filled by the pull calls (likely network or turn input, not proved).
- Each quantum advances the `AGILE_DATA+0x44` timer queue in **5 ms** (nominal)
  sub-steps, `N` per quantum. With `M = 1` and `[+0x2678] = 1.0`, `N` is 3, 4 or 5
  for speed indices 0, 1 and 2: 15, 20 or 25 game-ms per quantum. Relating that
  quantum to 25 real-ms on the QPC branch requires the conversion above. Which
  index maps to which UI game speed is not proved.
- Queue `+0x48` advances by raw elapsed real time, unquantized and pausable. The
  timer flag bit `0x4` therefore selects real time versus the quantized game clock
  for `TimerGetElapsed`. The later [TimerStart binding](legacy-timer-start.md)
  traces its zero selector to game-time queue `+0x44`; live timing remains open.
- Timer-queue updates can repeat inside one scheduler call. A single call can
  run many quanta, subject to its budget and bailout. This does not establish
  how all simulation work relates to a render frame; the subscribers and render
  dispatcher below remain unknown.

## Unknowns (exact next targets)

1. Who dispatches callback id 5 (`0x8405d0` table at `0x1160640`/`0x1160660`), and
   how often relative to the render/present path. This determines the frame-to-tick
   relationship.
2. Initial values and writers of `[+0x2678]` (float), `[+0x2680]`/`[+0x2684]` (ratio),
   `[+0x2688]`, `[+0x2670]` (speed index), and `0x5237a0`/`0x5237c0` (`M`).
3. Subscribers of event `0x40090092` other than `0x5ea6e0`, and whether unit
   movement, orders and RNG draws run off the agile timer queues, the sub-step event
   or the `0x60ad10` consumer.
4. The bit-exact `[0x113cb40]` after soft reciprocal, and the rounding mode of the
   float-to-int conversion `0xa56bf0` used for `N`.
5. Runtime confirmation (traced `[+0x2618]`, `[+0x3c4]`, `N` and `[0x113cb40]` in a
   live single-player game) remains uncollected.
6. The producer of QPC scale `[0x1140630]`, including its relationship to the
   performance-counter frequency. No calibration/default value is inferred.

## Reproducibility checks

All 24 retained code/data range hashes were independently checked against the
pinned executable, with PE section raw-size bounds enforced. Two central ranges:

| Range | SHA-256 |
| --- | --- |
| `0x5aefd0..0x5af526` scheduler | `0b2d4d5facf000be3b509a1059eb10b26900f16324e6b97c8f6fc4f995dfe5fc` |
| `0x406730..0x406768` sub-step initializer | `aa92491d8deae4b3670822314358300f42d6601806c4cfece78ad9e0469fbc16` |

Range identity supports reproducibility of static observations; it does not
verify live field values or close native tick/frame/gameplay parity.

## Bounded QPC calibration check

An exact little-endian DWORD search over every PE section's raw bytes found
`0x1140628` only at operands `0x796ca4` and `0x796cbd`, `0x114062c` only at
`0x796cb7`, and `0x1140630` only at `0x796ce0`. Disassembly confirms that these
belong to the clock getter above: it reads and writes the first field, tests the
second, and multiplies by the third. This is a bounded negative for those exact
address encodings. It does not exclude indirect writes, aliases, external
mutation or other address forms, and does not establish their live defaults.

The adjacent function `0x796b90..0x796c6f` calls the imported
`QueryPerformanceFrequency` slot `0xe66298`, samples both `RDTSC` through
`0x796c80` and `QueryPerformanceCounter` through `0xe66294`, then stores its
result in the separate pair `0x1140618`/`0x114061c`. It has no observed store to
the clock enable or scale fields. The arithmetic delegates `0xa57590` and
`0xa574e0` were not inspected for this check. Neither that adjacent calibration
nor its numeric result establishes the conversion used by `0x796ca0`.

The adjacent function's 223 raw bytes have SHA-256
`6932d413c94c404154cd1ae8a61296658b025cc5f4efc7987d67fe0662da8609`.
The clock getter's 87 bytes retain SHA-256
`df7ee9f36316d1a2be53eb0a6fe9fc9bb290cff8f340a7a4ddf9f52dc914ff23`.
Local search inventories and range identities are in
`.scratch/qpc-calibration-proof/`. QPC calibration remains unresolved; the next
required evidence is an actual producer or write observation for the enable and
scale fields. No such producer was positively located by this search, and no
simulation constant was changed.
