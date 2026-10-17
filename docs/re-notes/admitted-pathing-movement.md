# Caller-admitted point movement

`pcx_sim::step` can clamp its existing fixed-point movement segment against an
explicitly admitted WPM raster. `World::empty` retains the previous unconstrained
harness behavior. This does not establish original unit speed, walkability,
collision radius or path search.

`admit_movement` derives a uniform integer grid scale from the supplied terrain
and pathing dimensions. `admit_aligned_movement` can reuse an existing checked
`GridAlignment`. The caller supplies the fixed-point origin, terrain-tile extent,
blocking mask and retained-byte ceiling. Empty masks, mismatched dimensions,
nonintegral cell extents, overflows and unsupported step granularity refuse before
copying the raster. No original mask, cell size or origin is copied into a default.
The caller must keep the selected map's source/edition identity with its match.

The immutable admitted profile owns one raster copy. World snapshots share the
profile through `Arc`. The codec retains its prior payload: it does not transmit
the raster. `Client::reconcile_detailed` restores the client's existing admission
onto a compatible decoded snapshot before subsequent steps. A different explicit
admission returns `Correction::AdmissionMismatch` without changing the client.
Compatible snapshots keep the client's own `Arc`; the usual shared-identity
comparison does not scan the raster. Match setup must bind the same selected map
and profile on both peers; this is not a new network map handshake.

The traversal policy treats a cell as blocked when its byte intersects the
caller's mask. Cells own half-open rectangles. A blocked or exterior starting
point stays put. A segment stops before its first blocked or exterior point;
it does not slide or search around an obstacle. Traversal has explicit work caps
and refuses unsupported granularity. These are reconstruction policies for the
point-movement harness, not measured original unit collision behavior.

Synthetic tests exercise scale/origin conversion, boundaries, diagonal and
multi-cell crossings, fixed-point extremes, stopping, shared snapshots, actual
wire encode/decode and server rewind. Cell-check counts are deterministic work
gates rather than elapsed-time thresholds. Native and actual WASI checks are
recorded under `.scratch/pathing-root-20261017/`.

Original WPM blocking meanings and world-axis placement/orientation, per-unit
movement classes and footprints,
sliding/path search, runtime pathing changes, application session ownership and
original-game movement comparisons remain open. The application still needs the
source-bound unit world and session clock before this becomes gameplay movement.
