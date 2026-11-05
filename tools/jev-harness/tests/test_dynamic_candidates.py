"""Tests for per-Observation candidate rebuild (DynamicJevProvider).

Stdlib only, headless, muted, mock transport, no network, no real key/env.
Proves: moving screen coordinate across 2 observations yields a NEW
candidate; missing projection excluded; out-of-bounds/invalid rejected
(never 0,0); exact candidate-id/criteria alignment; key retained
in-process (loaded once, not per step); no secrets in criteria/logs.
Selection only -- no move/attack/goals asserted.
"""
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from jev_harness import Observation, ProviderError
from jev_harness.dynamic_candidates import (
    DynamicJevProvider, build_candidates, NOOP_ID,
)
from jev_harness.jev_provider import MODEL


def _obs(step, units, width=640, height=480):
    return Observation(step=step, png_bytes=b"x", width=width,
                       height=height, state={"units": units})


class MockTransport:
    """Argmax-selects the first non-noop candidate offered, or noop."""
    def __init__(self):
        self.calls = []

    def __call__(self, url, headers, body, timeout_s):
        self.calls.append({"url": url, "headers": dict(headers), "body": body})
        payload = json.loads(body)
        ids = list(payload["questions"]["action"]["criteria"].keys())
        choice = next((k for k in ids if k != NOOP_ID), NOOP_ID)
        probs = {k: (0.9 if k == choice else 0.1 / max(1, len(ids) - 1))
                 for k in ids}
        # normalize to sum 1
        total = sum(probs.values())
        probs = {k: v / total for k, v in probs.items()}
        return 200, json.dumps({
            "model": MODEL,
            "answers": {"action": {"type": "choice", "choice": choice,
                                   "confidence": 0.9, "probabilities": probs}},
            "usage": {"input_tokens": 1, "output_tokens": 1},
        }).encode()


def test_moving_screen_yields_new_candidate():
    t = MockTransport()
    p = DynamicJevProvider(api_key="test-key-not-a-secret", transport=t)
    a0 = p(_obs(0, [{"id": "2a", "screen": [50.0, 60.0], "in_viewport": True}]))
    assert a0 == {"kind": "click", "x": 50, "y": 60, "button": "left"}, a0
    # exact id/criteria alignment on fresh candidates
    assert set(p.last_candidates) == set(p.last_criteria) == {"stop_noop", "select_unit_2a"}
    payload0 = json.loads(t.calls[0]["body"])
    assert set(payload0["questions"]["action"]["criteria"]) == set(p.last_candidates)

    a1 = p(_obs(1, [{"id": "2a", "screen": [300.0, 200.0], "in_viewport": True}]))
    assert a1 == {"kind": "click", "x": 300, "y": 200, "button": "left"}, a1
    assert p.last_candidates["select_unit_2a"] == {"kind": "click", "x": 300, "y": 200, "button": "left"}
    payload1 = json.loads(t.calls[1]["body"])
    assert set(payload1["questions"]["action"]["criteria"]) == {"stop_noop", "select_unit_2a"}
    # stale coords from step 0 must not be offered
    blob = t.calls[1]["body"].decode()
    assert '"x": 50' not in blob


def test_missing_projection_excluded_never_zero_zero():
    for units in [
        [{"id": "a"}],                                       # no screen
        [{"id": "a", "screen": None}],
        [{"id": "a", "screen": [None, 10]}],
        [{"id": "a", "screen": [10]}],                       # wrong arity
        [{"id": "a", "screen": "10,10"}],
    ]:
        cands, crit = build_candidates(_obs(0, units))
        assert set(cands) == {NOOP_ID}, (units, cands)
        assert set(crit) == {NOOP_ID}
        assert not any(v.get("x") == 0 and v.get("y") == 0 for v in cands.values())


def test_out_of_bounds_and_invalid_rejected():
    bad_screens = [
        [640.0, 10.0], [-1.0, 10.0], [10.0, 480.0], [10.0, -0.5],
        [float("nan"), 10.0], [10.0, float("inf")], [float("-inf"), 10.0],
        [True, 10.0], ["10", 10.0], [99999.0, 99999.0],
    ]
    for s in bad_screens:
        cands, _ = build_candidates(_obs(0, [{"id": "a", "screen": s, "in_viewport": True}]))
        assert set(cands) == {NOOP_ID}, (s, cands)
    # in_viewport False excluded even with valid screen
    cands, _ = build_candidates(_obs(0, [{"id": "a", "screen": [10.0, 10.0], "in_viewport": False}]))
    assert set(cands) == {NOOP_ID}
    # hidden flags excluded
    for extra in ({"visible": False}, {"hidden": True}):
        u = {"id": "a", "screen": [10.0, 10.0], "in_viewport": True, **extra}
        cands, _ = build_candidates(_obs(0, [u]))
        assert set(cands) == {NOOP_ID}, extra
    # boundary-valid: 639.9 -> 639 (explicit truncation, not clamp of OOB)
    cands, _ = build_candidates(_obs(0, [{"id": "a", "screen": [639.9, 479.9], "in_viewport": True}]))
    assert cands["select_unit_a"] == {"kind": "click", "x": 639, "y": 479, "button": "left"}


def test_exact_id_criteria_alignment_and_noop_always():
    units = [
        {"id": "u1", "screen": [10.0, 20.0], "in_viewport": True},
        {"id": "u 2!", "screen": [30.0, 40.0], "in_viewport": True},
        {"id": "u1", "screen": [99.0, 99.0], "in_viewport": True},  # dup -> excluded
        {"no_id": True, "screen": [1.0, 1.0]},
    ]
    cands, crit = build_candidates(_obs(0, units))
    assert set(cands) == set(crit)
    assert NOOP_ID in cands
    assert "select_unit_u1" not in cands  # duplicate id ambiguous
    assert "select_unit_u_2" in cands
    assert all(isinstance(v, str) and v for v in crit.values())
    # transport payload criteria exactly match candidates
    t = MockTransport()
    p = DynamicJevProvider(api_key="k", transport=t)
    p(_obs(0, units))
    payload = json.loads(t.calls[0]["body"])
    assert set(payload["questions"]["action"]["criteria"]) == set(p.last_candidates)


def test_viewport_bounds_from_current_observation():
    # Same unit valid in 640x480 but out of bounds in smaller frame.
    unit = [{"id": "a", "screen": [500.0, 400.0], "in_viewport": True}]
    cands, _ = build_candidates(_obs(0, unit, width=640, height=480))
    assert "select_unit_a" in cands
    cands2, _ = build_candidates(_obs(1, unit, width=320, height=240))
    assert set(cands2) == {NOOP_ID}
    # Invalid viewport fails closed, never sentinel bounds.
    try:
        build_candidates(_obs(0, unit, width=0, height=480))
        raise AssertionError("expected ProviderError")
    except ProviderError:
        pass


def test_key_retained_not_reloaded_and_not_in_criteria_or_logs():
    t = MockTransport()
    # environ that would raise if re-read after construction is not used;
    # explicit api_key path: provider built once, per-call providers reuse it.
    p = DynamicJevProvider(api_key="supersecret-xyz", transport=t)
    p(_obs(0, [{"id": "a", "screen": [10.0, 10.0], "in_viewport": True}]))
    p(_obs(1, [{"id": "a", "screen": [20.0, 20.0], "in_viewport": True}]))
    assert len(t.calls) == 2
    for call in t.calls:
        assert call["headers"]["Authorization"] == "Bearer supersecret-xyz"
        assert "supersecret-xyz" not in call["body"].decode()
    assert "supersecret-xyz" not in json.dumps(p.last_criteria)
    assert "supersecret-xyz" not in json.dumps(p.last_summary)
    # transport header is the only place the key appears (JevProvider contract)
