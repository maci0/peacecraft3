"""Tests for JevProvider. Stdlib only, headless, mock transport, no network,
no key. Exact official-schema fixtures (OpenAPI SystemOne request/response)."""
import base64
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from jev_harness import (
    Action, JevProvider, Observation, ProviderError,
    box_to_pixels, parse_box_answer, redact_text,
)
from jev_harness.jev_provider import API_URL, MODEL


def _obs():
    return Observation(step=0, png_bytes=b"\x89PNGfake", width=640, height=480,
                       state={"selected": False, "order_count": 0})


DEFAULT_IDS = ("stop_noop", "select_unit", "move_selected", "order_attack")
def _good_response(choice="select_unit", ids=DEFAULT_IDS):
    probs = {k: 0.1 for k in ids}
    probs[choice] = 1.0 - 0.1 * (len(ids) - 1)
    return {
        "model": MODEL,
        "answers": {"action": {"type": "choice", "choice": choice,
                               "confidence": 0.7, "probabilities": probs}},
        "usage": {"input_tokens": 120, "output_tokens": 12},
    }


class MockTransport:
    def __init__(self, status=200, body=None, body_bytes=None):
        self.status = status
        self.body = body
        self.body_bytes = body_bytes
        self.calls = []
    def __call__(self, url, headers, body, timeout_s):
        self.calls.append({"url": url, "headers": dict(headers),
                           "body": body, "timeout": timeout_s})
        if self.body_bytes is not None:
            return self.status, self.body_bytes
        return self.status, json.dumps(self.body).encode()


def _provider(transport, **kw):
    return JevProvider(api_key="test-key-not-a-secret", transport=transport, **kw)

def _provider4(transport, **kw):
    return _provider(transport, candidates={
        "stop_noop": {"kind": "wait", "wait_ms": 100},
        "select_unit": {"kind": "click", "x": 100, "y": 100},
        "move_selected": {"kind": "click", "x": 200, "y": 150},
        "order_attack": {"kind": "key", "key": "a"}}, **kw)

def test_request_exact_official_schema():
    t = MockTransport(body=_good_response())
    p = _provider(t, candidates={
        "stop_noop": {"kind": "wait", "wait_ms": 100},
        "select_unit": {"kind": "click", "x": 100, "y": 100},
        "move_selected": {"kind": "click", "x": 200, "y": 150},
        "order_attack": {"kind": "key", "key": "a"}})
    action = p(_obs())
    assert action == {"kind": "click", "x": 100, "y": 100}
    assert len(t.calls) == 1
    call = t.calls[0]
    assert call["url"] == "https://api.typesafe.ai/v1/systemone"
    assert call["headers"]["Authorization"] == "Bearer test-key-not-a-secret"
    assert call["headers"]["Content-Type"] == "application/json"
    assert "TypeSafe-Danger-Zone-Images" not in call["headers"]  # structured only
    payload = json.loads(call["body"])
    assert payload["model"] == "jev-1.14.0"
    assert isinstance(payload["state"], dict) and payload["state"]["selected"] is False
    assert "assets" not in payload
    q = payload["questions"]["action"]
    assert q["type"] == "choice"
    assert set(q["criteria"]) == {"stop_noop", "select_unit", "move_selected", "order_attack"}
    # No OpenAI/chat schema anywhere in the request
    blob = call["body"].decode()
    assert "messages" not in blob and "chat" not in blob and "tools" not in blob
    assert p.last_summary["model"] == MODEL
    assert p.last_summary["usage"] == {"input_tokens": 120, "output_tokens": 12}


def test_no_retry_on_transport_error():
    t = MockTransport(status=429, body={"error": "rate limited"})
    p = _provider(t)
    try:
        p(_obs()); raise AssertionError("expected ProviderError")
    except ProviderError as e:
        assert "429" in str(e)
    assert len(t.calls) == 1  # no retry/replay


def test_safe_http_error_no_body_leak():
    t = MockTransport(status=400, body_bytes=b'{"detail":"echoed state secret-123"}')
    p = _provider(t)
    try:
        p(_obs()); raise AssertionError("expected ProviderError")
    except ProviderError as e:
        assert "secret-123" not in str(e) and "400" in str(e)


def test_fail_closed_illegal_id():
    body = _good_response(choice="stop_noop")
    # default provider now only stop_noop: add others for illegal-id case
    body = _good_response(choice="stop_noop")
    body["answers"]["action"]["choice"] = "nuke_everything"
    body["answers"]["action"]["probabilities"] = {
        k: (0.7 if k == "stop_noop" else 0.1)
        for k in ("stop_noop", "select_unit", "move_selected", "order_attack")}
    t = MockTransport(body=body); p = _provider(t)
    try:
        p(_obs()); raise AssertionError("expected ProviderError")
    except ProviderError:
        pass


def test_fail_closed_probability_violations():
    for mutate in [
        lambda a: a.update(confidence=1.5),
        lambda a: a.update(confidence=float("nan")),
        lambda a: a.update(probabilities={k: True for k in a["probabilities"]}),
        lambda a: a.update(probabilities={"stop_noop": 1.0}),
        lambda a: a.update(probabilities={k: 0.5 for k in a["probabilities"]}),  # sum 2
    ]:
        body = _good_response(); a = body["answers"]["action"]; mutate(a)
        t = MockTransport(body=body); p = _provider(t)
        try:
            p(_obs()); raise AssertionError(f"expected rejection: {mutate}")
        except ProviderError:
            pass


def test_fail_closed_model_and_type_mismatch():
    body = _good_response(); body["model"] = "jev-1.13.0"
    t = MockTransport(body=body); p = _provider(t)
    try:
        p(_obs()); raise AssertionError("expected model mismatch rejection")
    except ProviderError:
        pass
    body = _good_response(); body["answers"]["action"]["type"] = "score"
    t = MockTransport(body=body); p = _provider(t)
    try:
        p(_obs()); raise AssertionError("expected type rejection")
    except ProviderError:
        pass


def test_candidate_revalidated_against_viewport():
    cands = {"stop_noop": {"kind": "wait", "wait_ms": 100},
             "bad_click": {"kind": "click", "x": 9999, "y": 0}}
    t = MockTransport(body={
        "model": MODEL,
        "answers": {"action": {"type": "choice", "choice": "bad_click",
                               "confidence": 0.9,
                               "probabilities": {"stop_noop": 0.1, "bad_click": 0.9}}},
        "usage": {"input_tokens": 10, "output_tokens": 5}})
    p = _provider(t, candidates=cands,
                  criteria={"stop_noop": "wait", "bad_click": "click far"})
    try:
        p(_obs()); raise AssertionError("expected candidate validation rejection")
    except ProviderError:
        pass


def test_image_mode_opaque_ref_and_header():
    t = MockTransport(body=_good_response())
    p = _provider4(t, include_image=True)
    p(_obs())
    call = t.calls[0]
    assert call["headers"]["TypeSafe-Danger-Zone-Images"] == "true"
    payload = json.loads(call["body"])
    ref = "FRAME_REF"
    assert payload["assets"][ref]["type"] == "image"
    assert payload["assets"][ref]["format"] == "png"
    b64 = payload["assets"][ref]["data_base64"]
    assert "data:" not in b64
    assert base64.b64decode(b64) == b"\x89PNGfake"
    assert payload["state"]["frame_ref"] == ref  # opaque ref in state


def test_box_parse_and_convert():
    box = parse_box_answer({"type": "box", "box": {"top": 0.1, "bottom": 0.5,
                                                   "left": 0.25, "right": 0.75},
                            "confidence": 0.9})
    px = box_to_pixels(box, 640, 480)
    assert px == {"left": 160, "top": 48, "right": 480, "bottom": 240}
    for bad in [{"top": 2, "bottom": 0.5, "left": 0, "right": 1},
                {"top": 0.9, "bottom": 0.1, "left": 0, "right": 1},
                {"top": 0, "bottom": 1, "left": True, "right": 1}]:
        try:
            parse_box_answer({"type": "box", "box": bad})
            raise AssertionError(f"expected rejection {bad}")
        except ProviderError:
            pass


def test_request_size_limit():
    t = MockTransport(body=_good_response())
    p = _provider(t, max_request_bytes=10)
    try:
        p(_obs()); raise AssertionError("expected size rejection")
    except ProviderError:
        pass
    assert len(t.calls) == 0


def test_response_size_limit():
    t = MockTransport(body_bytes=b" " * 2000)
    p = _provider(t, max_response_bytes=100)
    try:
        p(_obs()); raise AssertionError("expected size rejection")
    except ProviderError:
        pass


def test_key_never_in_summary_or_errors():
    t = MockTransport(status=401, body={"error": "unauthorized"})
    p = JevProvider(api_key="supersecret-xyz", transport=t)
    try:
        p(_obs())
    except ProviderError as e:
        assert "supersecret-xyz" not in str(e)
    assert "supersecret-xyz" not in json.dumps(p.last_summary)
    assert "supersecret-xyz" not in redact_text("Authorization: Bearer supersecret-xyz")


def test_nonargmax_choice_uses_codeowned_argmax():
    """Non-argmax offered choice is not a contract violation.

    OpenAPI documents choice as max-prob but does not define a
    non-argmax response as a protocol violation (and a second Doom demo
    samples from probabilities). Adapter explicitly picks argmax."""
    body = _good_response()  # argmax is select_unit (0.7)
    body["answers"]["action"]["choice"] = "stop_noop"
    t = MockTransport(body=body); p = _provider4(t)
    action = p(_obs())
    assert action == {"kind": "click", "x": 100, "y": 100}  # select_unit
    assert p.last_summary["choice"] == "select_unit"
