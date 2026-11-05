"""Live Jev (TypeSafe AI) provider adapter — structured state first.

Official contract (verified in .scratch/jev-research/report.md from
https://api.typesafe.ai/openapi.json and https://docs.typesafe.ai):

- POST https://api.typesafe.ai/v1/systemone
- Auth: Authorization: Bearer $TYPESAFE_API_KEY (key loaded in-process
  only via secrets.load_key_in_process(); never logged, never sent as
  model text, never sourced as shell)
- Request: {model, state, questions} (+ optional assets). No
  messages/tools/chat-completions — that schema does not exist here.
- questions: map name -> {type:"choice", instructions?, criteria:map<id,text>}
- Response: {model, answers:{name:{type:"choice", choice, confidence,
  probabilities}}, usage:{input_tokens, output_tokens}}
- Jev NEVER returns actions directly. Code owns a small finite candidate
  action map; Jev selects a candidate ID; the strict action validator
  (actions.validate_action via run_loop, and re-checked here) gates it.
- Fail closed: any schema/type/model/probability/ID violation ->
  ProviderError, no action returned, no retry/replay.
- Argmax policy (code-owned, not a contract claim): the official
  OpenAPI ChoiceAnswer documents ``choice`` as "The name of the choice
  with the highest probability among the question's criteria", and one
  community Doom demo (junyeong-nero/jev-doom) additionally validated
  choice==argmax. The OpenAPI does not state that a non-argmax choice
  is a protocol violation, and another surveyed demo (mikespins/doom-jev)
  samples from probabilities instead of always taking argmax. This
  adapter therefore makes an explicit code-owned policy choice: take
  argmax deterministically. If a response's ``choice`` is in the offered
  set and probabilities are otherwise valid but are not argmax, the
  adapter does NOT reject it as a contract violation; it instead selects
  the argmax candidate itself (documented here, covered by tests).
- First mode is STRUCTURED state only (no images). Optional image mode
  (`include_image=True`) sends the viewport PNG as an opaque asset ref
  that appears in state, with format/data_base64 (no data: prefix) and
  header TypeSafe-Danger-Zone-Images: true. Box answers (0..1 fractions)
  are validated/converted by parse_box_answer/box_to_pixels helpers;
  image mode is experimental and never blocks structured smoke.
"""
from __future__ import annotations

import base64
import json
import math
import urllib.error
import urllib.request
from typing import Callable, Optional

from .actions import Action, validate_action, ValidationError
from .provider import Observation, Provider, ProviderError
from .redact import redact_text

API_URL = "https://api.typesafe.ai/v1/systemone"
MODEL = "jev-1.14.0"
QUESTION_NAME = "action"
# Doom-derived validation tolerance: probabilities must sum to ~1.
PROB_SUM_TOLERANCE = 0.02
# Bounded transport (no retry/replay of billable requests).
MAX_REQUEST_BYTES = 2_000_000
MAX_RESPONSE_BYTES = 1_000_000
DEFAULT_TIMEOUT_S = 30.0

# Transport signature: (url, headers, body_bytes, timeout_s)
#   -> (status:int, body_bytes:bytes). Injectable for mock tests.
Transport = Callable[[str, dict, bytes, float], "tuple[int, bytes]"]


class _NoRedirect(urllib.request.HTTPRedirectHandler):
    """Refuse every redirect: Authorization must never be forwarded
    across hosts, and there is no documented redirect flow anyway."""

    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None  # urllib raises HTTPError for the 3xx


def _urllib_transport(url, headers, body_bytes, timeout_s):
    req = urllib.request.Request(url, data=body_bytes, headers=headers,
                                 method="POST")
    opener = urllib.request.build_opener(_NoRedirect())
    try:
        with opener.open(req, timeout=timeout_s) as resp:
            return resp.status, resp.read(MAX_RESPONSE_BYTES + 1)
    except urllib.error.HTTPError as e:
        # Safe error: status only, never the raw body (may echo state).
        raise ProviderError(f"jev HTTP error status={e.code}") from None
    except urllib.error.URLError as e:
        raise ProviderError(f"jev transport error: {type(e).__name__}") from None
    except TimeoutError:
        raise ProviderError("jev request timeout") from None


def _is_finite_number(x) -> bool:
    return type(x) in (int, float) and math.isfinite(float(x))


def default_candidates() -> dict:
    """Fail-closed default: safe no-op only. Fixed synthetic clicks
    (100,100)/(200,150) key 'a' were tuned to SyntheticBackend
    (x<150,y<150); silently usable as real targets vs lane03 live
    unit `screen` positions. Real candidates must be integrator-
    supplied per-step (see report). IDs are criteria keys; values
    are strict action dicts (validated before apply)."""
    return {
        "stop_noop": {"kind": "wait", "wait_ms": 100},
    }


def default_criteria() -> dict:
    return {
        "stop_noop": "Do nothing further this step; safest when state is unclear or done.",
    }


def parse_box_answer(answer: dict) -> dict:
    """Validate a Jev box answer: normalized 0..1 fractions only.
    Returns {top,bottom,left,right} floats. Fail closed."""
    if not isinstance(answer, dict) or answer.get("type") != "box":
        raise ProviderError("box answer must be type=box object")
    box = answer.get("box")
    if not isinstance(box, dict):
        raise ProviderError("box answer missing box object")
    out = {}
    for k in ("top", "bottom", "left", "right"):
        v = box.get(k)
        if not _is_finite_number(v) or not (0.0 <= float(v) <= 1.0):
            raise ProviderError(f"box.{k} must be a finite number in 0..1")
        out[k] = float(v)
    if out["left"] > out["right"] or out["top"] > out["bottom"]:
        raise ProviderError("box edges inverted")
    return out


def box_to_pixels(box: dict, width: int, height: int) -> dict:
    """Convert validated 0..1 box to integer pixels, clamped to viewport."""
    if type(width) is not int or width <= 0 or type(height) is not int or height <= 0:
        raise ProviderError("viewport extent must be positive ints")
    b = parse_box_answer({"type": "box", "box": box})
    return {
        "left": min(width - 1, max(0, int(b["left"] * width))),
        "top": min(height - 1, max(0, int(b["top"] * height))),
        "right": min(width - 1, max(0, int(b["right"] * width))),
        "bottom": min(height - 1, max(0, int(b["bottom"] * height))),
    }


class JevProvider(Provider):
    """Provider callable matching the harness Provider interface.

    api_key: explicit key (tests) — otherwise loaded in-process from the
      authorized .secrets path via secrets.load_key_in_process().
    candidates: finite map id -> action dict (subset of criteria keys).
    transport: injectable HTTP transport (mock tests); default stdlib
      urllib POST with redirects refused, timeout + size limits.
    include_image: False = STRUCTURED state only (first/default mode).
      True = also send obs.png_bytes as opaque image asset (experimental).
    """
    name = "jev"

    def __init__(self, api_key: Optional[str] = None, *,
                 model: str = MODEL, url: str = API_URL,
                 candidates: Optional[dict] = None,
                 criteria: Optional[dict] = None,
                 transport: Optional[Transport] = None,
                 timeout_s: float = DEFAULT_TIMEOUT_S,
                 max_request_bytes: int = MAX_REQUEST_BYTES,
                 max_response_bytes: int = MAX_RESPONSE_BYTES,
                 include_image: bool = False,
                 image_ref: str = "FRAME_REF"):
        if api_key is None:
            from .secrets import load_key_in_process
            api_key = load_key_in_process()  # SecretsError propagates
        if not isinstance(api_key, str) or not api_key.strip():
            raise ProviderError("api key must be a non-empty string (not logged)")
        self._api_key = api_key.strip()
        self.model = model
        self.url = url
        self.candidates = dict(candidates) if candidates is not None else default_candidates()
        if not self.candidates:
            raise ProviderError("candidate map must be non-empty")
        for _cid, _act in self.candidates.items():
            try:
                validate_action(Action.from_dict(_act), 100000, 100000)
            except ValidationError as e:
                raise ProviderError(f"candidate {_cid!r} invalid: {redact_text(str(e))}") from None
        self.criteria = dict(criteria) if criteria is not None else {
            k: default_criteria().get(k, f"Take candidate action {k}.")
            for k in self.candidates
        }
        if set(self.criteria) != set(self.candidates):
            raise ProviderError("criteria keys must exactly match candidate ids")
        if len(self.criteria) > 255:
            raise ProviderError("choice criteria max 255 options")
        self.transport = transport or _urllib_transport
        self.timeout_s = float(timeout_s)
        self.max_request_bytes = int(max_request_bytes)
        self.max_response_bytes = int(max_response_bytes)
        self.include_image = bool(include_image)
        self.image_ref = image_ref
        self.calls = 0
        self.last_summary: dict = {}

    def build_request(self, obs: Observation) -> tuple[dict, dict]:
        """Return (payload, headers) without the Authorization value
        exposed here — headers contain only non-secret entries; auth is
        added at send time. (Auth header itself is set in __call__.)"""
        state = dict(obs.state) if isinstance(obs.state, dict) else {"note": str(obs.state)}
        if "hidden_units" in state or "unfiltered_units" in state:
            raise ProviderError("state must not contain hidden/unfiltered units")
        _units = state.get("units")
        if isinstance(_units, list):
            for _u in _units:
                if isinstance(_u, dict) and (_u.get("visible") is False or _u.get("hidden") is True):
                    raise ProviderError("state contains hidden unit (fail closed)")
        state["step"] = obs.step
        state["viewport"] = {"width": obs.width, "height": obs.height}
        payload = {
            "model": self.model,
            "state": state,
            "questions": {
                QUESTION_NAME: {
                    "type": "choice",
                    "instructions": "Select exactly one candidate action id for the next step, or the safe no-op if unclear.",
                    "criteria": dict(self.criteria),
                }
            },
        }
        headers = {"Content-Type": "application/json"}
        if self.include_image:
            if not obs.png_bytes:
                raise ProviderError("image mode requires viewport PNG bytes")
            b64 = base64.b64encode(obs.png_bytes).decode("ascii")
            if "data:" in b64:  # never emit a data: URI prefix
                raise ProviderError("image encoding must be bare base64")
            payload["assets"] = {self.image_ref: {
                "type": "image", "format": "png", "data_base64": b64}}
            # Opaque ref must appear in state (whole-value match first).
            state["frame_ref"] = self.image_ref
            headers["TypeSafe-Danger-Zone-Images"] = "true"
        return payload, headers

    def _validate_response(self, data) -> dict:
        """Strict fail-closed validation. Returns sanitized summary."""
        if not isinstance(data, dict):
            raise ProviderError("jev response must be a JSON object")
        if data.get("model") != self.model:
            raise ProviderError("jev response model mismatch (fail closed)")
        answers = data.get("answers")
        if not isinstance(answers, dict) or QUESTION_NAME not in answers:
            raise ProviderError("jev response missing answers[action]")
        ans = answers[QUESTION_NAME]
        if not isinstance(ans, dict) or ans.get("type") != "choice":
            raise ProviderError("answers[action] must be type=choice")
        choice = ans.get("choice")
        if choice not in self.candidates:
            raise ProviderError(f"illegal choice id: {choice!r}")
        confidence = ans.get("confidence")
        if not _is_finite_number(confidence) or not (0.0 <= float(confidence) <= 1.0):
            raise ProviderError("confidence must be finite in 0..1")
        probs = ans.get("probabilities")
        if not isinstance(probs, dict) or set(probs) != set(self.candidates):
            raise ProviderError("probabilities must cover exactly the candidate ids")
        total = 0.0
        for k, v in probs.items():
            if not _is_finite_number(v) or not (0.0 <= float(v) <= 1.0):
                raise ProviderError(f"probability[{k}] must be finite in 0..1")
            total += float(v)
        if abs(total - 1.0) > PROB_SUM_TOLERANCE:
            raise ProviderError("probabilities must sum to ~1")
        # Code-owned argmax policy (see module docstring): OpenAPI
        # documents choice as the max-probability option but does not
        # define a non-argmax choice as a protocol violation. Do not
        # reject a valid offered-set choice merely for not being argmax;
        # explicitly select argmax here instead.
        argmax_choice = max(probs, key=lambda k: float(probs[k]))
        # Use argmax as the authoritative selection. Ties resolved by
        # max() first-encountered order (deterministic for a given dict).
        choice = argmax_choice
        usage = data.get("usage")
        if not isinstance(usage, dict):
            raise ProviderError("response missing usage object")
        for k in ("input_tokens", "output_tokens"):
            v = usage.get(k)
            if type(v) is not int or v < 0:
                raise ProviderError(f"usage.{k} must be a non-negative int")
        return {
            "model": data["model"], "choice": choice,
            "confidence": float(confidence),
            "probabilities": {k: float(v) for k, v in probs.items()},
            "usage": {"input_tokens": usage["input_tokens"],
                      "output_tokens": usage["output_tokens"]},
        }

    def __call__(self, obs: Observation) -> dict:
        # No retry/replay: exactly one transport attempt per call.
        self.calls += 1
        payload, headers = self.build_request(obs)
        body = json.dumps(payload).encode("utf-8")
        if len(body) > self.max_request_bytes:
            raise ProviderError("request exceeds size limit")
        headers = dict(headers)
        headers["Authorization"] = f"Bearer {self._api_key}"
        try:
            status, resp_body = self.transport(self.url, headers, body, self.timeout_s)
        except ProviderError:
            raise
        except Exception as e:  # transport raised unexpectedly — safe msg
            raise ProviderError(f"jev transport error: {type(e).__name__}") from None
        if status != 200:
            raise ProviderError(f"jev HTTP error status={status}")
        if len(resp_body) > self.max_response_bytes:
            raise ProviderError("response exceeds size limit")
        try:
            data = json.loads(resp_body.decode("utf-8"))
        except Exception:
            raise ProviderError("jev response is not valid JSON") from None
        summary = self._validate_response(data)
        self.last_summary = summary
        action_dict = dict(self.candidates[summary["choice"]])
        # Reuse the strict action validator before returning (loop
        # re-validates); fail closed here too.
        try:
            validate_action(Action.from_dict(action_dict), obs.width, obs.height)
        except ValidationError as e:
            raise ProviderError(f"candidate action invalid: {redact_text(str(e))}") from None
        return action_dict
