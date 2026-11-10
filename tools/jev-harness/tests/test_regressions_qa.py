"""QA regressions for bool/NaN/unknown-field, delayed-provider, sentinel redaction.

Stdlib + pytest only, headless, muted, no network, no real key, no live
backend: SyntheticBackend (fake) only — never OriginalReferenceBackend,
never a native client, never tools/original-game-reference.
"""
import math
import sys
import tempfile
import threading
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from jev_harness import (
    Action, validate_action, ValidationError,
    SyntheticBackend, FakeProvider, run_loop, LoopConfig, StopReason,
)
from jev_harness.provider import Provider

# Fake sentinel, built dynamically; never a real credential.
S = "FAKE" + "SENTINEL" + "_7x9q"


def _rejects(d, width=640, height=480):
    try:
        validate_action(Action.from_dict(d), width, height)
    except ValidationError:
        return
    raise AssertionError(f"expected ValidationError: {d!r} {width}x{height}")


def test_bool_coordinates_rejected():
    for d in [{"kind": "click", "x": True, "y": 10},
              {"kind": "click", "x": 10, "y": False},
              {"kind": "click", "x": True, "y": True}]:
        _rejects(d)


def test_bool_wait_rejected():
    _rejects({"kind": "wait", "wait_ms": True})
    _rejects({"kind": "wait", "wait_ms": False})


def test_bool_and_bad_dimensions_rejected():
    good = {"kind": "click", "x": 1, "y": 1}
    for w, h in [(True, 480), (640, True), (0, 480), (640, 0), (640, -1),
                 (-1, 480), (640.0, 480), (640, 480.0), ("640", 480),
                 (None, 480), (640, None)]:
        _rejects(good, w, h)


def test_nan_infinity_rejected():
    for v in [math.nan, math.inf, -math.inf]:
        _rejects({"kind": "click", "x": v, "y": 10})
        _rejects({"kind": "click", "x": 10, "y": v})
        _rejects({"kind": "wait", "wait_ms": v})
        _rejects({"kind": "click", "x": 1, "y": 1}, v, 480)
        _rejects({"kind": "click", "x": 1, "y": 1}, 640, v)


def test_non_int_and_unknown_fields_rejected():
    for d in [{"kind": "click", "x": 10.5, "y": 10},
              {"kind": "click", "x": "10", "y": 10},
              {"kind": "click", "x": None, "y": 10},
              {"kind": "click", "x": 10, "y": 10, "extra": 1},
              {"kind": "click", "x": 10, "y": 10, "note": "x"},
              {"kind": "wait", "wait_ms": 0},
              {"kind": "wait", "wait_ms": 2001},
              {"kind": "wait", "wait_ms": -5},
              {"kind": "wait", "wait_ms": 1.5},
              {"kind": "js", "x": 1, "y": 1},
              {"x": 1, "y": 1}]:
        _rejects(d)
    for bad in [None, [], "click", 42]:
        try:
            Action.from_dict(bad)
            raise AssertionError(f"expected rejection: {bad!r}")
        except ValidationError:
            pass


def test_delayed_provider_result_never_applied_after_timeout():
    release = threading.Event()
    finished = threading.Event()

    class Delayed(Provider):
        name = "delayed"
        def __call__(self, obs):
            # Held until run_loop has returned, so the call always outlives
            # the per-call timeout.
            release.wait(timeout=5)
            finished.set()
            return {"kind": "click", "x": 10, "y": 10}

    backend = SyntheticBackend(640, 480)
    result = run_loop(backend, Delayed(),
                      LoopConfig(per_call_timeout_s=0.1, max_model_calls=3))
    assert result.stop_reason == StopReason.PROVIDER_ERROR
    assert result.actions_applied == 0
    assert backend.applied == 0
    # Let the late provider finish; its result must still never apply.
    release.set()
    assert finished.wait(timeout=5)
    assert backend.applied == 0
    assert result.actions_applied == 0


def test_cancelled_slow_provider_result_never_applied():
    cancel = threading.Event()
    called = threading.Event()

    class Slow(Provider):
        name = "slow"
        def __call__(self, obs):
            called.set()
            # Return only after the canceller has fired, so the result always
            # arrives after cancellation.
            assert cancel.wait(timeout=5), "cancel never arrived"
            return {"kind": "click", "x": 10, "y": 10}

    backend = SyntheticBackend(640, 480)
    threading.Thread(target=lambda: called.wait(timeout=5) and cancel.set(),
                     daemon=True).start()
    result = run_loop(backend, Slow(),
                      LoopConfig(per_call_timeout_s=10), cancel=cancel)
    assert result.stop_reason == StopReason.CANCELLED
    assert result.actions_applied == 0
    assert backend.applied == 0


def _run_with_trace(backend, provider):
    with tempfile.TemporaryDirectory() as d:
        tp = Path(d) / "trace.json"
        result = run_loop(backend, provider, LoopConfig(), trace_path=tp)
        return result, tp.read_text()


def test_sentinel_not_in_capture_error_trace():
    class BadCapture(SyntheticBackend):
        def capture(self):
            raise RuntimeError("capture failed Authorization: Bearer " + S)

    result, txt = _run_with_trace(BadCapture(), FakeProvider())
    assert result.stop_reason == StopReason.BACKEND_ERROR
    assert S not in txt
    assert S not in " ".join(e.error for e in result.entries)


def test_sentinel_not_in_apply_error_trace():
    class BadApply(SyntheticBackend):
        def apply(self, action):
            raise RuntimeError("apply failed api-key=" + S)

    provider = FakeProvider(script=[{"kind": "click", "x": 10, "y": 10}])
    result, txt = _run_with_trace(BadApply(), provider)
    assert result.stop_reason == StopReason.BACKEND_ERROR
    assert S not in txt
    assert S not in " ".join(e.error for e in result.entries)


def test_sentinel_not_in_provider_error_trace():
    class BadProvider(Provider):
        name = "bad"
        def __call__(self, obs):
            raise RuntimeError("provider failed Bearer " + S)

    result, txt = _run_with_trace(SyntheticBackend(), BadProvider())
    assert result.stop_reason == StopReason.PROVIDER_ERROR
    assert S not in txt
    assert S not in " ".join(e.error for e in result.entries)


def test_sentinel_not_in_validation_error_trace():
    # Bad key value reaches the allowlist error; unknown-field value reaches raw.
    for script in [[{"kind": "key", "key": "Bearer " + S}],
                   [{"kind": "click", "x": 1, "y": 1, "note": "Bearer " + S}]]:
        result, txt = _run_with_trace(
            SyntheticBackend(), FakeProvider(script=script))
        assert result.stop_reason == StopReason.VALIDATION_ERROR
        assert S not in txt
        assert S not in " ".join(e.error for e in result.entries)
        assert S not in " ".join(str(e.raw_action) for e in result.entries)


def test_sentinel_not_in_sk_pattern_trace():
    class BadProvider(Provider):
        name = "bad2"
        def __call__(self, obs):
            raise RuntimeError("failed sk-" + S + "12345678 done")

    result, txt = _run_with_trace(SyntheticBackend(), BadProvider())
    assert result.stop_reason == StopReason.PROVIDER_ERROR
    assert S not in txt
    assert "sk-" not in txt


def test_sentinel_bare_unknown_field_and_key_values_never_in_run_loop_trace():
    # run_loop-level (not redact()-unit): a bare sentinel with no Bearer/sk-
    # shape, smuggled as an unknown-field value, a bad key value, or a
    # non-dict provider return, must not appear in entries or trace file.
    for script in [[{"kind": "click", "x": 1, "y": 1, "note": S}],
                   [{"kind": "key", "key": S}]]:
        result, txt = _run_with_trace(
            SyntheticBackend(), FakeProvider(script=script))
        assert result.stop_reason == StopReason.VALIDATION_ERROR
        assert S not in txt
        assert S not in " ".join(e.error for e in result.entries)
        assert S not in " ".join(str(e.raw_action) for e in result.entries)

    class NonDict(Provider):
        name = "nondict"

        def __call__(self, obs):
            return S

    result, txt = _run_with_trace(SyntheticBackend(), NonDict())
    assert result.stop_reason == StopReason.VALIDATION_ERROR
    assert S not in txt
    assert S not in " ".join(str(e.raw_action) for e in result.entries)
