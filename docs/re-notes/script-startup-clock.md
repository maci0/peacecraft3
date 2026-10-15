# Source-bound startup time of day

`StartupClock::read` and `ObjectHost::with_startup_clock` provide an opt-in,
immutable snapshot for script startup. They do not implement a running game clock,
saved-game restoration, time advancement, clock suspension or `SetFloatGameState`.
The default launcher does not admit this adapter automatically.

The caller supplies one complete PE, its source identity and SHA-256, a reviewed
initializer window and explicit input budgets. The existing checked PE reader
requires initialized, readable executable bytes and a matching range SHA-256.
The supported x86 shape is `push imm8; push address; call rel32; add esp,8; ret`.
The signed operand is converted to an exactly representable real. No measured
hour, image address or source fingerprint is embedded in production code. A
matching shape/fingerprint establishes bytes, not that the caller selected the
right semantic role; that review remains explicit.

Admission also selects a named constant from the *same retained Program*.
Its scalar `fgamestate` type, admitted conversion, declaration source and bounded
literal arithmetic determine the selector. There is no built-in selector number.
Only that typed symbolic handle is accepted by the exact constant
`GetFloatGameState(fgamestate)->real` contract. Other selectors, nulls, unrelated
families, incompatible declarations, missing source and exhausted budgets refuse.
No new handle arena or substitute script program is created.

## Static evidence and reconstruction choice

The local target is the Legacy executable with SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`,
whose own VERSIONINFO is `1.29.2.9231`. No compatibility DLL offsets are used.
The first named registration of `GetFloatGameState` selects
`0x492530..0x492579` (73 bytes, SHA-256
`2a00a728f2dd90e36be1404c0d5580f49eb7ff89b4955dd29e2dbf627ffcccc7`).
Its selected context property comes through `0x4e8d00` and is read by `0x800d80`.
This is static registration evidence, not a measurement of the active runtime family.

The context's observed virtual method `0x4e81e0..0x4e82f1` passes a default-value
address to the clock proxy's initializer at `0x800cd0`. That initializer copies
the pointed value into its property descriptor. The address lies in runtime
storage without file-backed bytes. Reading unrelated file bytes at that virtual
offset would be wrong.

The address is instead the destination of `0x401300..0x401310`, a complete
16-byte initializer (SHA-256
`8a7e430bec84d134e19ed79cb08733466aed5b6252a006ea034fcf9a505dc482`).
Its signed integer argument is zero. The adjacent, independently selected scale
initializer `0x401460..0x401470` supplies one to the same converter.
Both have observed pointers in initialized `.rdata`. The converter at
`0x9a1620..0x9a167c` explicitly writes zero for a zero argument and constructs
IEEE single-precision bits otherwise. Capstone and independent objdump instruction
boundaries agree for these complete bodies. No executable or DLL was loaded.

Using the selected constructor default for a new startup snapshot is an explicit
recreation policy. Cold-start lifecycle ordering, saved/loaded state and live
native time have not been measured. A caller must not use this immutable adapter
as the simulation's running clock. The selected configuration's day length/hours
were located separately, but their rate/speed/suspension integration is deferred.
The melee helper's starting-hour constant is not substituted for this map's state.

## Verification and current boundary

Seven synthetic test groups vary the hour, selector and source locations; cover
signature/type/budget failures, range hashes, executable permissions, runtime-only
storage, malformed instruction shapes and retained-VM execution. All seven also
execute in actual 32-bit WASM, mask 127, with no imports. The full play suite,
strict native/WASM all-target/all-feature Clippy and library builds pass.

The same retained Human01 Program/admission/localization completes globals and
`config`, creates the admitted day sound, then successfully reads the selected
startup hour through Blizzard.j's `GetTimeOfDay`. The authored control flow now
requests the night ambient label. The deliberately day-only sound bank refuses
that missing resource; no alias or playback is invented. This is Host/data evidence,
not campaign completion, native clock parity or audible behavior.

Actual source inspection, complete command logs, failed review-harness attempt,
corrected gate, fingerprints and WASM results remain local under
`.scratch/game-clock-root/`. Original PE, table, script and audio contents are not
committed or uploaded.
