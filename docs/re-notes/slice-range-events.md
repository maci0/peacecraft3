# TriggerRegisterUnitInRange

No code change. Registration and deferred firing are already in this crate.

`ObjectHost::register_unit_in_range` stores the trigger, the `Entry::Unit`, the executed finite real, and an optional filter, then returns an event. The call does not move the unit or run the filter. A missing `unit_in_range` budget leaves the native unsupported. Negative finite ranges are retained and later refused.

`ObjectHost::poll_unit_ranges` samples `ExecutedUnit::position` (the accepted position, not the `CreateUnit` request) and the candidate catalog collision already stored on that record. The tested radius is the script real plus that candidate collision. The anchor collision is not added. Hidden candidates are omitted. A hidden anchor still delivers a shown candidate. An outside-to-inside edge emits one `UnitRangeEntry`. A later poll of the same pair does not. `begin_unit_range_entry` and `begin_unit_range_trigger` then run the filter, then the trigger conditions and actions. `MapScriptSession::advance_unit_ranges` is the session caller. Startup does not flush at registration.

## Evidence

Code at this commit: `range_event_host.rs`, `unit_range_runtime_host.rs`, and the existing tests in `range_event_tests.rs` and `unit_range_runtime_tests.rs`. Positions are the public `ExecutedUnit::position` field written by placement. No `unit_host` edit is required.

The comparison with one instrumented Human01 run is the 2026-10-18 probe in [script-unit-world.md](script-unit-world.md#one-bounded-range-probe-2026-10-18): deferred delivery, final-position sampling, candidate-only collision versus `IsUnitInRange`, and filter versus condition context. That note is a prior runtime observation. This pass did not re-read the logs or launch the game. The inclusive equality, registration order, and per-tick cadence in the poll are labeled shortcuts there, not a decoded native formula.

## Refused

Not invented, and not changed: an equality rule, a poll interval, spatial order, dead or sleeping or paused or locust eligibility, re-testing a false filter that already joined membership, destroy-time registration cleanup, adding the anchor collision, or falling back to `CreateUnit` x/y when `position` is absent. `GetUnitAcquireRange` stays unsupported. The host test expects that, and combat does not consume `SetUnitAcquireRange`.
