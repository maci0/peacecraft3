# Step driver handoff (lane 09-ai, T0885)

Bounded repeated-step driver over the canonical game update. `step_once` in
`crates/peacecraft3/src/gameplay_observation.rs` (test-fixtures only) advances
at most 64 ticks of 0.25s via `advance_timers` + `advance_queued_events`,
returning an explicit `StepState` (`Advanced { ticks }`, `Paused`,
`Cinematic`, `NoSession`, `Refused`). It refuses when the session witness
changes, when `game_paused() == Some(true)`, or when control is off
(`presentation().control_enabled == Some(false)`). No teleport, no
`SetUnitX`, no hidden state.

Proof: ignored test `step_driver_two_steps_with_one_real_order` in
`crates/peacecraft3/src/campaign_entry_gpu_tests.rs` issues one legal
`standard_target` Move order, then two `step_once(20)` steps, asserting
movement across both. Use a local original install, a map member and a fresh capture
directory. Verify offscreen display and null audio before launch.

```bash
export PCX_NATIVE_CHAPTER_INSTALL='/path/to/Warcraft III (Legacy)'
export PCX_NATIVE_CHAPTER_MAP='path/within/selected/archive.w3m'
export PCX_NATIVE_CHAPTER_ADDON=roc
export PCX_CAMPAIGN_CAPTURE_DIR='.scratch/step-driver-new'

cargo test --locked -p peacecraft3 --bin peacecraft3 --features test-fixtures -- \
  --exact campaign_entry_gpu_tests::step_driver_two_steps_with_one_real_order \
  --ignored --nocapture --test-threads=1
python3 tools/step_driver_handoff.py "$PCX_CAMPAIGN_CAPTURE_DIR"
```

`tools/step_driver_handoff.py` is pure stdlib and validates `observation-journal.json` for
`StepState`, legal order and `moved_squared > 1.0`. This is a harness proof,
not campaign-AI completion: T0885's acceptance gate (one selected campaign
AI entry, next missing interface recorded) remains open.
