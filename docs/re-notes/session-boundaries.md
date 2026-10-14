# Session trust boundaries

Scope: `crates/pcx-net/src/session.rs`, boundary tests in `crates/pcx-net/tests/session_boundaries.rs`.

Authority, tick, peer and packet checks in `Session::receive` run before any
input is buffered. Duplicate submission is rejected before buffering, so no
unauthorized command aliases another player's batch. `leave` removes the
disconnecting peer's pending orders from every tick and prunes emptied buckets
before inserting its terminal `Stop` for the current tick.

Rejected cases are covered by `session_boundaries`: unauthorized peer, spoofed
player, rewound/beyond-horizon tick, oversized/malformed/unknown-kind packet,
coordinate and command-count overflow, duplicate submission, non-rewinding resync,
post-leave use, duplicate join identity, and two-player authoritative advance.

No transport, codec or lib.rs changes were made.
