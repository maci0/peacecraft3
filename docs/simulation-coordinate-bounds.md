# Fixed-point coordinate bounds

The movement harness accepts signed 32-bit coordinates. The difference between
two such coordinates can require 33 bits: subtracting before widening can panic
or wrap. Taking the absolute value of `i32::MIN` also exceeds that type.

Movement and selection now compute differences as `i64`. Movement clamps each
axis to the existing harness speed, then converts the bounded step back to `Fx`.
The next position lies between the current position and the target, both valid
`Fx` values. Selection compares the widened absolute distance with its unchanged
radius. This preserves the existing rules for distances that previously fit.

Four regressions cover opposite coordinate extremes, a distance equal to
`i32::MIN`, exact arrival at both boundaries, and far selections that previously
wrapped into the selection radius. Before the fix, three native cases panicked;
on actual 32-bit release WASM, three cases trapped at assertions detecting wrong
results. All four pass after the fix on native and actual 32-bit release WASM.
Existing simulation, networking and canonical play-trace tests also pass.

These are project movement rules. Original coordinate conversion, speed,
selection, pathing, tick rate and RNG parity remain unverified. This change
does not supply those game rules or change the wire representation of positions.
