"""Explicit bounded action schema. No shell/JS/URL/file/credential actions."""
from __future__ import annotations
from dataclasses import dataclass

# Small explicit key allowlist: single printable chars + a few named keys.
# No modifier chords, no arbitrary sequences.
ALLOWED_KEYS = frozenset(
    list("abcdefghijklmnopqrstuvwxyz0123456789") +
    ["Space", "Enter", "Escape", "Tab", "Backspace", "Delete",
     "Up", "Down", "Left", "Right", "Home", "End", "PageUp", "PageDown"]
)

MAX_WAIT_MS = 2000
MAX_CLICK_PER_ACTION = 1  # one click per action; sequences need separate actions

class ValidationError(ValueError):
    pass

@dataclass(frozen=True)
class Action:
    kind: str  # "click" | "key" | "wait"
    x: int = 0
    y: int = 0
    key: str = ""
    wait_ms: int = 0
    button: str = "left"

    @classmethod
    def from_dict(cls, d):
        if not isinstance(d, dict):
            raise ValidationError("action must be an object")
        if "kind" not in d:
            raise ValidationError("missing kind")
        kind = d["kind"]
        if kind not in ("click", "key", "wait"):
            # No raw value embedded: callers redact, but bare values must
            # never reach logs/traces even before redaction.
            raise ValidationError("unsupported kind")
        # Reject unknown fields strictly (count only, never names/values:
        # an unknown field name or value may carry credential material)
        allowed = {"kind", "x", "y", "key", "wait_ms", "button"}
        extra = set(d) - allowed
        if extra:
            raise ValidationError(f"unknown fields present: {len(extra)}")
        return cls(
            kind=kind,
            x=d.get("x", 0), y=d.get("y", 0),
            key=d.get("key", ""), wait_ms=d.get("wait_ms", 0),
            button=d.get("button", "left"),
        )

    def to_dict(self):
        if self.kind == "click":
            return {"kind": "click", "x": self.x, "y": self.y, "button": self.button}
        if self.kind == "key":
            return {"kind": "key", "key": self.key}
        return {"kind": "wait", "wait_ms": self.wait_ms}


def validate_action(action: Action, width: int, height: int) -> Action:
    """Validate against viewport bounds and schema. Raises ValidationError."""
    if type(width) is not int or width <= 0 or type(height) is not int or height <= 0:
        raise ValidationError("viewport extent must be positive ints")
    if action.kind == "click":
        if action.button not in ("left", "middle", "right"):
            raise ValidationError("invalid button")
        if type(action.x) is not int or type(action.y) is not int:
            raise ValidationError("click coordinates must be integers")
        if not (0 <= action.x < width and 0 <= action.y < height):
            raise ValidationError(
                f"click out of bounds: ({action.x},{action.y}) not in {width}x{height}"
            )
    elif action.kind == "key":
        if action.key not in ALLOWED_KEYS:
            raise ValidationError("key not in allowlist")
    elif action.kind == "wait":
        if type(action.wait_ms) is not int or not (0 < action.wait_ms <= MAX_WAIT_MS):
            raise ValidationError(f"wait_ms must be 1..{MAX_WAIT_MS}")
    else:
        raise ValidationError("unsupported kind")
    return action
