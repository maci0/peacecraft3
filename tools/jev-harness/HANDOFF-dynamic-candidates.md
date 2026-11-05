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
