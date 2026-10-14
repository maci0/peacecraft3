# Cinematic/dialog FDF boundary audit and hardening

This slice remains checked FDF declaration ingestion plus explicit caller binding.
It is not a native controller and claims no rendering, game-handle, or event
semantics.

Hardened areas:
- `FrameId` values are bounds-checked in `relationships` and `Target::Frame`.
- inheritance template resolution still requires unique globally named declarations
  and refuses self/cyclic inheritance.
- resource accounting uses checked subtraction/addition for aggregate token budgets.
- public `frame(FrameId)` provides bounds-checked immutable record access.

Tests: `crates/pcx-play/tests/cinematic_boundaries.rs`.
