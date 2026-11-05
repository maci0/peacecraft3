"""Bounded capture -> provider -> validate -> apply -> capture loop.

Defaults: max 3 model calls, 8 game actions, per-call timeout, total budget.
No automatic replay/retry of state-changing or billable requests.
Redacted local trace only (PNG bytes hashed, never logged raw).
"""
from __future__ import annotations
import hashlib
import json
import threading
import time
from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path

from .actions import Action, validate_action, ValidationError
from .backend import GameBackend
from .provider import Provider, Observation, ProviderError
from .redact import redact

class StopReason(str, Enum):
    BUDGET_EXHAUSTED = "budget_exhausted"
    ACTION_BUDGET = "action_budget_exhausted"
    CANCELLED = "cancelled"
    PROVIDER_ERROR = "provider_error"
    VALIDATION_ERROR = "validation_error"
    BACKEND_ERROR = "backend_error"
    COMPLETED_SCRIPT = "completed_script"

@dataclass
class LoopConfig:
    max_model_calls: int = 3
    max_actions: int = 8
    per_call_timeout_s: float = 30.0
    total_timeout_s: float = 120.0
    # If True, stop when provider returns wait (script exhausted)
    stop_on_wait: bool = True

@dataclass
class TraceEntry:
    step: int
    observation_summary: dict
    raw_action: dict
    validated_action: dict | None
    receipt: dict | None = None
    png_sha256: str = ""
    error: str = ""

@dataclass
class LoopResult:
    entries: list[TraceEntry] = field(default_factory=list)
    stop_reason: StopReason = StopReason.COMPLETED_SCRIPT
    model_calls: int = 0
    actions_applied: int = 0
    final_state: dict = field(default_factory=dict)
    trace_path: str = ""

def run_loop(backend: GameBackend, provider: Provider,
             config: LoopConfig | None = None,
             cancel: threading.Event | None = None,
             trace_path: str | Path | None = None) -> LoopResult:
    cfg = config or LoopConfig()
    cancel = cancel or threading.Event()
    # Also honour backend-level cancel
    result = LoopResult()
    deadline = time.monotonic() + cfg.total_timeout_s
    step = 0

    def _cancelled() -> bool:
        return cancel.is_set() or backend.cancelled()

    while True:
        if _cancelled():
            result.stop_reason = StopReason.CANCELLED
            break
        if time.monotonic() > deadline:
            result.stop_reason = StopReason.BUDGET_EXHAUSTED
            break
        if result.model_calls >= cfg.max_model_calls:
            result.stop_reason = StopReason.BUDGET_EXHAUSTED
            break
        if result.actions_applied >= cfg.max_actions:
            result.stop_reason = StopReason.ACTION_BUDGET
            break

        # Capture isolated viewport only
        try:
            png, state = backend.capture()
        except Exception as e:
            result.stop_reason = StopReason.BACKEND_ERROR
            result.entries.append(TraceEntry(step, {}, {}, None, error=redact(str(e))[:300]))
            break
        result.final_state = state
        obs = Observation(step=step, png_bytes=png,
                          width=backend.width, height=backend.height, state=state)

        # Provider call with timeout (thread; no retry on failure)
        if _cancelled():
            result.stop_reason = StopReason.CANCELLED
            break
        raw: dict | None = None
        err: Exception | None = None
        def _call():
            nonlocal raw, err
            try:
                raw = provider(obs)
            except Exception as e:  # noqa
                err = e
        t = threading.Thread(target=_call, daemon=True)
        t.start()
        t.join(timeout=cfg.per_call_timeout_s)
        if t.is_alive():
            result.stop_reason = StopReason.PROVIDER_ERROR
            result.entries.append(TraceEntry(step, obs.summary(), {}, None,
                                             error="provider timeout"))
            break
        result.model_calls += 1
        if err is not None:
            result.stop_reason = StopReason.PROVIDER_ERROR
            result.entries.append(TraceEntry(step, obs.summary(), {}, None,
                                             error=redact(str(err))[:300]))
            break
        if not isinstance(raw, dict):
            # Never embed the raw provider value: it may carry key material
            # and redact() only catches known credential shapes.
            result.stop_reason = StopReason.VALIDATION_ERROR
            result.entries.append(TraceEntry(step, obs.summary(), {"raw": "[REDACTED: non-object]"}, None,
                                             error="provider returned non-object"))
            break

        # Validate
        try:
            action = Action.from_dict(raw)
            validate_action(action, backend.width, backend.height)
        except ValidationError as e:
            # redact() alone cannot catch a bare unknown-field/key value
            # (no Bearer/sk- shape). Drop unknown-field values and key/
            # button/kind values on validation failure; keep only kind
            # structure plus numeric shape so coordinates stay debuggable
            # without carrying credential-shaped strings.
            safe_raw = {"kind": raw.get("kind") if isinstance(raw.get("kind"), str) and raw.get("kind") in ("click", "key", "wait") else "[REDACTED]",
                        "x": raw.get("x") if type(raw.get("x")) is int else "[REDACTED]",
                        "y": raw.get("y") if type(raw.get("y")) is int else "[REDACTED]",
                        "wait_ms": raw.get("wait_ms") if type(raw.get("wait_ms")) is int else "[REDACTED]",
                        "unknown_fields": len(set(raw) - {"kind", "x", "y", "key", "wait_ms", "button"}),
                        "key": "[REDACTED]", "button": "[REDACTED]"}
            result.stop_reason = StopReason.VALIDATION_ERROR
            result.entries.append(TraceEntry(step, obs.summary(), redact(safe_raw), None,
                                             png_sha256=hashlib.sha256(png).hexdigest(),
                                             error="validation failed"))
            break

        if _cancelled():
            result.stop_reason = StopReason.CANCELLED
            break

        # Apply ONLY to this backend surface
        try:
            receipt = backend.apply(action)
        except Exception as e:
            result.stop_reason = StopReason.BACKEND_ERROR
            result.entries.append(TraceEntry(step, obs.summary(), redact(raw),
                                             action.to_dict(), error=redact(str(e))[:300]))
            break
        result.actions_applied += 1
        result.entries.append(TraceEntry(
            step, obs.summary(), redact(raw), action.to_dict(),
            receipt=redact(receipt) if isinstance(receipt, dict) else {"receipt": str(receipt)[:200]},
            png_sha256=hashlib.sha256(png).hexdigest(),
        ))

        if action.kind == "wait" and cfg.stop_on_wait:
            result.stop_reason = StopReason.COMPLETED_SCRIPT
            break
        step += 1

    # Final capture so final_state reflects post-action state (before/after evidence)
    if not _cancelled() and result.actions_applied > 0:
        try:
            _, final = backend.capture()
            result.final_state = final
        except Exception:
            pass

    if trace_path:
        p = Path(trace_path)
        p.parent.mkdir(parents=True, exist_ok=True)
        payload = {
            "backend": backend.label,
            "stop_reason": result.stop_reason.value,
            "model_calls": result.model_calls,
            "actions_applied": result.actions_applied,
            "final_state": redact(result.final_state),
            "entries": [redact({
                "step": e.step, "observation": e.observation_summary,
                "raw_action": e.raw_action, "validated_action": e.validated_action,
                "receipt": e.receipt, "png_sha256": e.png_sha256, "error": e.error,
            }) for e in result.entries],
        }
        p.write_text(json.dumps(payload, indent=2))
        result.trace_path = str(p)
    return result
