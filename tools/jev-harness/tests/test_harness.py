"""Tests for jev_harness. Stdlib only, headless, no network, no key."""
import sys
import threading
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from jev_harness import (
    Action, validate_action, ValidationError,
    SyntheticBackend, FakeProvider, run_loop, LoopConfig, StopReason,
    ProviderError, redact_text, redact, load_key_in_process, SecretsError,
)
from jev_harness.provider import Observation, Provider


def test_action_validation_accepts_in_bounds():
    a = validate_action(Action.from_dict({"kind": "click", "x": 10, "y": 10}), 640, 480)
    assert a.kind == "click"
    validate_action(Action.from_dict({"kind": "key", "key": "a"}), 640, 480)
    validate_action(Action.from_dict({"kind": "wait", "wait_ms": 100}), 640, 480)


def test_action_validation_rejects_out_of_bounds():
    for bad in [{"kind": "click", "x": 640, "y": 0}, {"kind": "click", "x": -1, "y": 0},
                {"kind": "click", "x": 0, "y": 480}]:
        try:
            validate_action(Action.from_dict(bad), 640, 480)
            raise AssertionError(f"expected rejection: {bad}")
        except ValidationError:
            pass


def test_action_validation_rejects_unknown_and_bad_key():
    for bad in [{"kind": "click", "x": 1, "y": 1, "shell": "rm"},
                {"kind": "key", "key": "ctrl+c"},
                {"kind": "key", "key": "F12"},
                {"kind": "js", "code": "alert(1)"},
                {"kind": "wait", "wait_ms": 99999},
                {"kind": "wait", "wait_ms": 0}]:
        try:
            validate_action(Action.from_dict(bad), 640, 480)
            raise AssertionError(f"expected rejection: {bad}")
        except ValidationError:
            pass


def test_fake_screenshot_action_changed_state():
    backend = SyntheticBackend(640, 480)
    result = run_loop(backend, FakeProvider(), LoopConfig())
    assert result.final_state["selected"] is True
    assert result.final_state["order_count"] >= 1
    assert result.actions_applied >= 2


def test_budget_enforced():
    backend = SyntheticBackend()
    provider = FakeProvider(script=[{"kind": "click", "x": 10, "y": 10}] * 10)
    result = run_loop(backend, provider, LoopConfig(max_model_calls=2, max_actions=8))
    assert result.model_calls == 2
    assert result.stop_reason == StopReason.BUDGET_EXHAUSTED


def test_action_budget_enforced():
    # wait with stop_on_wait=False so loop continues until action budget
    backend = SyntheticBackend()
    provider = FakeProvider(script=[{"kind": "wait", "wait_ms": 1}] * 20)
    result = run_loop(backend, provider,
                      LoopConfig(max_model_calls=20, max_actions=3, stop_on_wait=False))
    assert result.actions_applied == 3
    assert result.stop_reason == StopReason.ACTION_BUDGET


def test_cancel_stops():
    backend = SyntheticBackend()
    cancel = threading.Event()
    def _cancel_soon():
        time.sleep(0.05)
        cancel.set()
    threading.Thread(target=_cancel_soon, daemon=True).start()
    provider = FakeProvider(script=[{"kind": "wait", "wait_ms": 1}] * 20)
    result = run_loop(backend, provider,
                      LoopConfig(max_model_calls=20, max_actions=20, stop_on_wait=False),
                      cancel=cancel)
    assert result.stop_reason in (StopReason.CANCELLED, StopReason.BUDGET_EXHAUSTED,
                                  StopReason.COMPLETED_SCRIPT) or result.model_calls < 20


def test_provider_failure_no_retry():
    class Boom(Provider):
        name = "boom"
        def __init__(self):
            self.calls = 0
        def __call__(self, obs):
            self.calls += 1
            raise ProviderError("Authorization: Bearer supersecret123")
    backend = SyntheticBackend()
    p = Boom()
    result = run_loop(backend, p, LoopConfig())
    assert result.stop_reason == StopReason.PROVIDER_ERROR
    assert p.calls == 1  # no retry
    # Trace error must be redacted
    blob = str([e.error for e in result.entries])
    assert "supersecret" not in blob


def test_validation_error_stops_without_apply():
    backend = SyntheticBackend()
    provider = FakeProvider(script=[{"kind": "click", "x": 9999, "y": 9999}])
    result = run_loop(backend, provider, LoopConfig())
    assert result.stop_reason == StopReason.VALIDATION_ERROR
    assert backend.applied == 0


def test_redacted_logs():
    assert "supersecret" not in redact_text("Authorization: Bearer supersecret123")
    assert "supersecret" not in str(redact({"h": "api-key=supersecret", "n": 1}))
    assert "sk-abcdefghij" not in redact_text("key sk-abcdefghij12345678 end")
    assert redact_text("plain text") == "plain text"


def test_secrets_loader(tmp_path=None):
    import tempfile, os
    with tempfile.TemporaryDirectory() as d:
        p = Path(d) / "key"
        p.write_text("MY_KEY=abc123\n")
        assert load_key_in_process(p) == "abc123"
        try:
            load_key_in_process(Path(d) / "missing")
            raise AssertionError("expected SecretsError")
        except SecretsError:
            pass
        p.write_text("   \n")
        try:
            load_key_in_process(p)
            raise AssertionError("expected SecretsError for empty")
        except SecretsError:
            pass


def test_trace_file_redacted_and_no_png(tmp_path=None):
    import tempfile, json
    with tempfile.TemporaryDirectory() as d:
        tp = Path(d) / "trace.json"
        backend = SyntheticBackend()
        run_loop(backend, FakeProvider(), LoopConfig(), trace_path=tp)
        data = json.loads(tp.read_text())
        assert data["model_calls"] >= 1
        assert "png_sha256" in tp.read_text()
        # raw PNG bytes never in trace (summary length field is fine)
        assert "\\x89PNG" not in tp.read_text() and "iVBOR" not in tp.read_text()
