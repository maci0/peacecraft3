"""Typed canonical decision adapter over the Jev provider — stdlib only.

Explicit structured canonical order mode (not mouse/vision benchmark):
the model returns only a typed choice id; code owns the legal canonical
actions derived from the CURRENT observed owned units / visible positions
plus the caller goal. Choice ids map to validated protocol Move/Step/
Cancel/Stop via ``FixtureClient`` — never disguised keys/clicks.

Provider reuse: endpoint/auth/schema/transport/response validation are
reused from the verified ``campaign_agent`` Jev provider
(``jev_request``/``post_json``/``answers_of``/``load_key`` env path,
redaction, no-retry, size caps, redirect/security behaviour of
``post_json``). No duplicate API wrapper/framework/chat messages.

Bounds: <=3 model calls, <=8 canonical actions, 30s per model request,
120s total, code-owned stop on budget/session refusal. No live API now:
key comes only from the supported worker env ``TYPESAFE_API_KEY`` (or
``JEV_API_KEY``); secret-file reads are REFUSED. Fake injected key /
mock transport until a confirmed env key is ready.
"""

from __future__ import annotations

import json
import math
import time
from dataclasses import dataclass, field

from jev_harness.canonical_fixture import FixtureClient, FixtureError, FixtureTimeout

# Reuse verified Jev provider constants/transport from campaign_agent.
# Imported lazily so mock-transport tests never touch the network module
# path at import time beyond definitions.
_PROVIDER = None


def _provider():
    global _PROVIDER
    if _PROVIDER is None:
        import importlib.util
        from pathlib import Path

        path = Path(__file__).resolve().parents[2] / "original-game-reference" / "campaign_agent.py"
        spec = importlib.util.spec_from_file_location("campaign_agent_provider", path)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        _PROVIDER = module
    return _PROVIDER


JEV_URL = "https://api.typesafe.ai/v1/systemone"
JEV_MODEL = "jev-1.14.0"
MAX_MODEL_CALLS = 3
MAX_CANONICAL_ACTIONS = 8
MODEL_REQUEST_TIMEOUT_S = 30.0
TOTAL_BUDGET_S = 120.0
MAX_RESPONSE_BYTES = 2 * 1024 * 1024
MAX_STEP_SECONDS = 30.0
MOVE_STEP_SECONDS = 5.0
MAX_COORDINATE = 100_000.0
MODEL_RE = __import__("re").compile(r"^[A-Za-z0-9_.:/+-]{1,160}$")

# Finite code-owned legal canonical action catalog. Choice ids are stable,
# typed, and map 1:1 to protocol actions. No hidden enemy, teleport, or
# invented asset values: targets derive only from observed positions.
CHOICE_MOVE_EAST = "move_east"
CHOICE_MOVE_NORTH = "move_north"
CHOICE_STEP = "step"
CHOICE_CANCEL = "cancel"
CHOICE_STOP = "stop"
CHOICE_IDS = (CHOICE_MOVE_EAST, CHOICE_MOVE_NORTH, CHOICE_STEP, CHOICE_CANCEL, CHOICE_STOP)
CHOICE_CRITERIA = {
    CHOICE_MOVE_EAST: "move the selected owned unit east (positive x) toward the caller goal",
    CHOICE_MOVE_NORTH: "move the selected owned unit north (negative y in fixture world) toward the caller goal",
    CHOICE_STEP: "advance simulation without issuing an order",
    CHOICE_CANCEL: "cancel the selected owned unit order then step",
    CHOICE_STOP: "stop the canonical session gracefully",
}


class DecisionError(Exception):
    """Malformed model answer, budget/session refusal, or adapter failure."""


def env_key(environ=None):
    """Supported worker env only. Secret-file reads are refused by design."""
    import os

    env = os.environ if environ is None else environ
    for name in ("TYPESAFE_API_KEY", "JEV_API_KEY"):
        value = env.get(name, "")
        if isinstance(value, str) and value.strip():
            return value.strip()
    raise DecisionError(
        "TYPESAFE_API_KEY not present in worker env; secret-file reads are refused. "
        "Supply the key via env or use the mock transport."
    )


def _check_model(model: str) -> str:
    if not MODEL_RE.fullmatch(model or ""):
        raise DecisionError("model id contains unsupported characters")
    return model


def _finite(value, name: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise DecisionError(f"{name} must be a number")
    number = float(value)
    if not math.isfinite(number):
        raise DecisionError(f"{name} must be finite")
    return number


def observed_owned_units(observation: dict) -> list[dict]:
    """CURRENT observed owned units: position + life present, hex id > 0.

    ``gameplay_observation::observe`` already limits to owned/visible
    units; this adapter re-checks position/life so no hidden unit can
    be ordered. Enemy/undead-hidden units never reach the catalog.
    """
    if not isinstance(observation, dict):
        raise DecisionError("observation must be an object")
    units = observation.get("units")
    if not isinstance(units, list) or not units:
        raise DecisionError("observation has no units; refusing to invent targets")
    owned = []
    for unit in units:
        if not isinstance(unit, dict):
            continue
        position = unit.get("position")
        life = unit.get("life")
        uid_raw = unit.get("id")
        if position is None or life is None or uid_raw is None:
            continue
        try:
            uid = int(str(uid_raw), 16)
        except ValueError:
            continue
        if uid <= 0:
            continue
        if not (isinstance(position, (list, tuple)) and len(position) == 2):
            continue
        owned.append(
            {
                "id": str(uid_raw),
                "uid": uid,
                "position": [
                    _finite(position[0], "unit x"),
                    _finite(position[1], "unit y"),
                ],
                "life": _finite(life, "unit life"),
            }
        )
    if not owned:
        raise DecisionError("no observed owned unit with position/life; refusing")
    return owned


def _clamp_target(target: list[float], observation: dict) -> list[float]:
    for axis in range(2):
        value = target[axis]
        if not math.isfinite(value) or abs(value) > MAX_COORDINATE:
            raise DecisionError("derived target out of bounds")
    bounds = observation.get("camera_bounds") if isinstance(observation, dict) else None
    if isinstance(bounds, (list, tuple)) and len(bounds) == 8:
        try:
            xs = [float(bounds[i]) for i in (0, 2, 4, 6)]
            ys = [float(bounds[i]) for i in (1, 3, 5, 7)]
        except (TypeError, ValueError):
            xs, ys = [], []
        if xs and ys and all(math.isfinite(v) for v in (*xs, *ys)):
            target[0] = min(max(target[0], min(xs) + 1.0), max(xs) - 1.0)
            target[1] = min(max(target[1], min(ys) + 1.0), max(ys) - 1.0)
    return target


@dataclass(frozen=True)
class CanonicalChoices:
    """Code-owned legal actions for one observation + caller goal."""

    unit_uid: int
    unit_id: str
    position: list[float]
    target_east: list[float]
    target_north: list[float]

    def criteria(self) -> dict:
        return dict(CHOICE_CRITERIA)


def build_choices(observation: dict) -> CanonicalChoices:
    owned = observed_owned_units(observation)
    # Deterministic: most life, then lowest uid. Observed only.
    unit = max(owned, key=lambda u: (u["life"], -u["uid"]))
    pos = unit["position"]
    # Bounded offsets (<=200 world units, same scale as the real driver).
    east = _clamp_target([pos[0] + 200.0, pos[1]], observation)
    north = _clamp_target([pos[0], pos[1] - 200.0], observation)
    if east == pos and north == pos:
        raise DecisionError("derived targets equal current position; refusing no-op move")
    return CanonicalChoices(
        unit_uid=unit["uid"],
        unit_id=unit["id"],
        position=list(pos),
        target_east=east,
        target_north=north,
    )


def choice_questions(choices: CanonicalChoices) -> dict:
    """One typed choice question — the only model question asked."""
    return {
        "canonical_order": {
            "type": "choice",
            "instructions": (
                "Choose the single next canonical order for the observed owned unit "
                f"{choices.unit_id} at {choices.position}. "
                "Targets are code-derived from its current position; "
                "pick the id whose direction best advances the caller goal."
            ),
            "criteria": choices.criteria(),
        }
    }


def canonical_request_body(key: str, model: str, goal: str, step: int, choices: CanonicalChoices) -> tuple[dict, dict]:
    """Minimal Jev body reusing the verified provider schema.

    Same endpoint/auth/transport shape as ``campaign_agent.jev_request``
    (model/state/assets/questions, Bearer auth, TypeSafe danger-zone
    header) but with a text-only structured state: no image asset, no
    vision benchmark, no chat messages. The model sees only the typed
    choice catalog + caller goal; coordinates stay code-owned.
    """
    if not isinstance(goal, str) or not goal.strip():
        raise DecisionError("caller goal must be a non-empty string")
    body = {
        "model": _check_model(model),
        "state": {
            "goal": goal.strip(),
            "step": step,
            "unit_id": choices.unit_id,
            "unit_position": list(choices.position),
            "target_east": list(choices.target_east),
            "target_north": list(choices.target_north),
        },
        "questions": choice_questions(choices),
    }
    headers = {
        "Authorization": f"Bearer {key}",
        "Content-Type": "application/json",
        "TypeSafe-Danger-Zone-Images": "true",
    }
    return body, headers


def answers_of(payload) -> dict:
    """Response validation reused from the verified provider."""
    return _provider().answers_of(payload)


def _choice_answer(answers: dict) -> tuple[str, float]:
    if not isinstance(answers, dict):
        raise DecisionError("decision response answers must be an object")
    answer = answers.get("canonical_order")
    if not isinstance(answer, dict):
        raise DecisionError("decision is missing canonical_order answer")
    choice = answer.get("choice")
    if choice not in CHOICE_IDS:
        raise DecisionError(f"choice {choice!r} is outside the canonical catalog")
    confidence = answer.get("confidence")
    if confidence is None:
        probabilities = answer.get("probabilities")
        if isinstance(probabilities, dict) and choice in probabilities:
            confidence = probabilities[choice]
        else:
            confidence = 0.0
    try:
        confidence = float(confidence)
    except (TypeError, ValueError):
        raise DecisionError("choice confidence is not a number") from None
    if not math.isfinite(confidence) or not 0.0 <= confidence <= 1.0:
        raise DecisionError("choice confidence out of bounds")
    return choice, confidence


def choice_to_action(choice: str, choices: CanonicalChoices) -> dict:
    """Map a validated typed choice id to a protocol action dict."""
    if choice == CHOICE_MOVE_EAST:
        return {"type": "move", "units": [choices.unit_uid], "target": list(choices.target_east), "seconds": MOVE_STEP_SECONDS}
    if choice == CHOICE_MOVE_NORTH:
        return {"type": "move", "units": [choices.unit_uid], "target": list(choices.target_north), "seconds": MOVE_STEP_SECONDS}
    if choice == CHOICE_STEP:
        return {"type": "step", "seconds": MOVE_STEP_SECONDS}
    if choice == CHOICE_CANCEL:
        return {"type": "cancel", "units": [choices.unit_uid], "seconds": MOVE_STEP_SECONDS}
    if choice == CHOICE_STOP:
        return {"type": "stop"}
    raise DecisionError(f"choice {choice!r} is outside the canonical catalog")


@dataclass
class DecisionRecord:
    model_call: int
    choice: str
    confidence: float
    action: dict
    receipt: dict = field(default_factory=dict)


@dataclass
class AdapterResult:
    model_decisions: list[DecisionRecord]
    canonical_receipts: list[dict]
    stopped: bool


class DecisionAdapter:
    """Bounded model->canonical adapter. Code owns stop on budget refusal.

    ``post`` is the transport (default: verified provider ``post_json``
    against the Jev endpoint). Tests inject a mock transport; no live
    API is called unless a real env key + real transport are supplied.
    """

    def __init__(
        self,
        client: FixtureClient,
        *,
        goal: str,
        key: str | None = None,
        model: str = JEV_MODEL,
        post=None,
        url: str = JEV_URL,
        clock=time.monotonic,
    ):
        if not isinstance(goal, str) or not goal.strip():
            raise DecisionError("caller goal must be a non-empty string")
        self.client = client
        self.goal = goal.strip()
        self.model = _check_model(model)
        self.url = url
        self.clock = clock
        self._started = clock()
        if post is not None:
            self.post = post
            self.key = key or "mock-injected-key"
        else:
            self.key = key if key is not None else env_key()
            self.post = _provider().post_json

    def _budget_hit(self) -> bool:
        return (self.clock() - self._started) > TOTAL_BUDGET_S

    def _call_model(self, choices: CanonicalChoices, step: int) -> tuple[str, float]:
        body, headers = canonical_request_body(self.key, self.model, self.goal, step, choices)
        remaining = TOTAL_BUDGET_S - (self.clock() - self._started)
        timeout = min(MODEL_REQUEST_TIMEOUT_S, max(0.1, remaining))
        try:
            payload = self.post(self.url, headers, body, timeout=timeout)
        except DecisionError:
            raise
        except Exception as error:  # transport failure: fail closed, redact key
            detail = str(error).replace(self.key, "[redacted]") if self.key else str(error)
            raise DecisionError(f"decision request failed: {detail[:500]}") from None
        answers = answers_of(payload)
        return _choice_answer(answers)

    def run(self, initial_observation: dict, *, max_actions: int = MAX_CANONICAL_ACTIONS) -> AdapterResult:
        if not 1 <= max_actions <= MAX_CANONICAL_ACTIONS:
            raise DecisionError(f"max_actions must be 1..{MAX_CANONICAL_ACTIONS}")
        observation = initial_observation
        decisions: list[DecisionRecord] = []
        receipts: list[dict] = []
        stopped = False
        for call in range(1, MAX_MODEL_CALLS + 1):
            if self._budget_hit():
                # Code-owned stop on budget refusal.
                receipt = self.client.stop(timeout=10.0)
                receipts.append(receipt.raw)
                stopped = True
                break
            choices = build_choices(observation)
            choice, confidence = self._call_model(choices, call)
            action = choice_to_action(choice, choices)
            record = DecisionRecord(model_call=call, choice=choice, confidence=confidence, action=action)
            decisions.append(record)
            if len(decisions) > MAX_CANONICAL_ACTIONS:
                raise DecisionError("canonical action budget exceeded")
            if action["type"] == "stop":
                receipt = self.client.stop(timeout=10.0)
                record.receipt = receipt.raw
                receipts.append(receipt.raw)
                stopped = True
                break
            receipt = self.client.send(action, timeout=10.0)
            record.receipt = receipt.raw
            receipts.append(receipt.raw)
            if receipt.observation is None:
                raise DecisionError(f"canonical receipt missing observation for {action['type']}")
            observation = receipt.observation
            if len(receipts) >= max_actions:
                # Budget of canonical actions reached: code-owned stop.
                stop = self.client.stop(timeout=10.0)
                receipts.append(stop.raw)
                stopped = True
                break
        else:
            # Model-call budget exhausted without a model stop: code stop.
            stop = self.client.stop(timeout=10.0)
            receipts.append(stop.raw)
            stopped = True
        return AdapterResult(model_decisions=decisions, canonical_receipts=receipts, stopped=stopped)


def mock_transport(payloads):
    """Fake injected transport: returns queued typed payloads in order."""
    queue = list(payloads)
    calls = []

    def _post(url, headers, body, timeout=MODEL_REQUEST_TIMEOUT_S):
        calls.append({"url": url, "headers": dict(headers), "body": body, "timeout": timeout})
        if not queue:
            raise DecisionError("mock transport exhausted")
        return queue.pop(0)

    _post.calls = calls
    return _post


def typed_payload(choice: str, confidence: float = 0.9, model: str = JEV_MODEL) -> dict:
    return {
        "model": model,
        "answers": {"canonical_order": {"choice": choice, "confidence": confidence}},
    }
