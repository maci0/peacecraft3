# Handoff: per-Observation dynamic candidates (lane 09/10 integration)

Status: helper + tests committed as new files only. Integration 10 owns
existing provider/actions/loop/tests files — do not merge this by editing
shared README/__init__ here. This note is the handoff, not a shared edit.

## What exists

- `jev_harness/dynamic_candidates.py` — ONE new helper module:
  - `build_candidates(obs) -> (candidates, criteria)`: rebuilds finite
    candidates from EACH Observation. One left-click selection candidate
    per visible/owned unit with valid `screen` projection, plus
    `stop_noop` always. No move/attack candidates in this slice.
  - `DynamicJevProvider`: Provider-callable. Builds a template
    `JevProvider` once (key loaded in-process only, retained; never
    reloaded per step, never in criteria/logs), then per call builds a
    fresh `JevProvider(api_key=<retained>, candidates, criteria)` and
    delegates. No framework, no validation bypass — construction +
    per-call viewport validation in `JevProvider` still gate everything.
- `tests/test_dynamic_candidates.py` — dedicated tests (mock transport,
  no network/key/env): moving coords across 2 observations yields new
  candidate; missing projection excluded; OOB/NaN/inf/bool/str rejected
  (never 0,0); `in_viewport=False`/hidden excluded; bounds from current
  `obs.width/height`; exact ID/criteria alignment in transport payload;
  key retained once, absent from criteria/logs/summary.

## Safety rules in build_candidates

- `screen` None/missing/not 2-list/non-numeric/bool/NaN/inf/outside
  `0 <= x < width`, `0 <= y < height` → unit excluded, never (0,0).
- Conversion: bounds-check float first, then explicit `int()` truncation,
  then re-check int range. `639.9` in 640-wide → 639; `640.0` → excluded.
- Duplicate unit ids excluded (ambiguous); sanitisation collisions skipped.
- Viewport must be positive ints from current Observation, else
  `ProviderError`. No sentinel viewport.
- Candidate ids derived only from visible/owned observation unit `id`s.
  No guessed world targets, no attack semantics, no preferred-unit policy.
  Target ordering belongs to backend/gameplay policy (Integration 10+).

## 09/10 integration recipe (for Integration 10 owner)

```python
from jev_harness.dynamic_candidates import DynamicJevProvider

provider = DynamicJevProvider()  # key from authorized .secrets / env, in-process only
# per step, with a FRESH Observation whose state["units"] are the
# upstream own-or-visible filtered units (lane03 shape: id + screen +
# in_viewport), call the same provider object from run_loop:
action = provider(obs)  # candidates rebuilt inside, every call
```

- Pass the SAME `DynamicJevProvider` instance to `run_loop` each step;
  do not construct per step (that would reload the key) and do not pass
  frozen `JevProvider(candidates=...)` (that reintroduces stale clicks,
  reproduced: unit moved 50,60 → 300,200 still clicked 50,50).
- Export wiring (Integration 10, in shared `__init__.py`):
  `from .dynamic_candidates import DynamicJevProvider, build_candidates`
  and add both to `__all__`.
- Selection-only slice: Jev choosing `select_unit_<id>` selects a unit.
  It does NOT move, attack, or complete gameplay. Do not claim complete
  gameplay from selection alone.

## Not done / next

- Move/attack target candidates: need trustworthy screen→world target
  derivation from the backend; intentionally absent here.
- Live API call: still blocked on key location (see README Live status);
  no live call made, no key/network used in tests.

## Integration record (pc3-menu-10, 2026-11-05)

- Cherry-picked 2abf0b0 cleanly (3 new files only, no conflicts) as c795d2a atop 92189c1 (50-test state) + f7eea78 Help/Tips + observer/CustomKeys already in tree. No backend.py edits (09-owned), no dirty-WIP imports.
- Minimum reconciliation: exported DynamicJevProvider, build_candidates from package init (+ __all__) per recipe. No other file edits.
- API compatibility checked against 92189c1 JevProvider: constructor forwarding matches; candidate schema (click int/non-neg, wait) matches, per-call viewport re-gate in JevProvider applies to rebuilt candidates; hidden-flag guard in build_request matches builder explicit visible/hidden exclusion (hidden units would fail closed at provider too).
- Review: adapter filters invalid projection (missing/None/non-numeric/bool/NaN/inf/OOB never 0,0, in_viewport False, hidden flags, dup ids, sanitisation collisions, 255 cap), bounds from actual obs extent (ProviderError on non-positive-int viewport), choice criteria IDs exactly aligned, retained key loaded once, never in criteria/logs/body beyond provider auth header, no move/attack invented (selection left-click + stop_noop only).
- Tests: pytest tools/jev-harness/tests headless muted, no real key/env/network: 56 passed (50 pre-existing + 6 dynamic_candidates).
- Remaining blockers: follow-up run_loop 2-frame regression owned by Lane05; driver/backend wiring (PeaceCraft backend not wired, Synthetic not gameplay), live credential location unconfirmed (no live call). No broad Rust retest triggered for Python-only change.

## Integration record (pc3-menu-10, 2026-11-05 follow-up)

- Integrated 9a11f72 (model-first budget expectation: run_loop second-click fresh-coords + missing-projection noop) and 27caaab (OverflowError on huge-int coords) as working-tree changes atop 883a6fd. Scope: only `jev_harness/dynamic_candidates.py` (_finite_number try/except OverflowError) and `tests/test_dynamic_candidates.py` (+149 lines, 3 new tests). No shared backend/loop/provider edits, no lane08 dirty producer import, no texture/backend HOLD items touched.
- Reproduced before fix: `_finite_number(10**400)` raised `OverflowError: int too large to convert to float`. After fix returns False; huge coords excluded like NaN/inf.
- UnitCheckBox reconciliation (739c887 pending): working tree already carried menu_sound_preference + campaign_ack gate + menu_settings/gameplay reconciliation; completed the missing preview step by removing stale `UnitCheckBox` join from `original_options_preview::join` and its retained-disabled entry (now interactive via sound_preference, skipped by `supported()` guard like CustomKeys). CustomKeys gameplay path untouched.
- Tests headless muted, no key/env/network: `pytest tools/jev-harness/tests` 59 passed (56 + 3 new). `cargo test --bin peacecraft3` 696 passed, 0 failed, 76 ignored. `cargo check -p peacecraft3` and `--target wasm32-unknown-unknown` clean. Pre-existing `cargo clippy` failure in `pcx-play` (collapsible_if, verified present on clean HEAD via stash) is unrelated debt, not touched.

## Sound preference + lint closeout (2026-11-05)

- Sound integration (UnitCheckBox/unit): `menu_sound_preference.rs` (new), `campaign_ack.rs` (only `unit_voices` producer gated by applied `Navigation`; draft never gates), `menu_screens.rs` (`supported()` includes sound_preference), `menu_settings.rs` (reconciliation), `original_options_preview.rs` + tests (stale `UnitCheckBox` join removed, now interactive via sound_preference). CustomKeys gameplay path untouched.
- Lint: `pcx-play` collapsible_if fixed by collapsing nested if in `escape_help_tips.rs` (let-chain, no behavior change); `peacecraft3` too-many-arguments on `unit_voices` (8 params Bevy system) resolved with targeted `#[allow(clippy::too_many_arguments)]` matching existing crate pattern (7 other files).
- Evidence: `cargo clippy -p peacecraft3 --bin peacecraft3 -D warnings` clean, `-p pcx-play` clean, wasm32 target clean. `cargo test --bin peacecraft3` 696 passed/0 failed/76 ignored. `pytest tools/jev-harness/tests` 59 passed. All headless muted, no key/env/network.
- Backend chain (10aec0c/e5069b6/54ddad5) closed with driver/exports/mock 31 tests, live still blocked; not imported in this slice per parent instruction.
