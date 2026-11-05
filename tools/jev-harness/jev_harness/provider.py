"""Provider-neutral callable. FakeProvider is the only implementation until
the official JEV contract is verified by lane05. No invented endpoint/auth."""
from __future__ import annotations
from dataclasses import dataclass, field
from typing import Callable, Optional

class ProviderError(RuntimeError):
    pass

@dataclass
class Observation:
    step: int
    png_bytes: bytes  # isolated viewport capture only
    width: int
    height: int
    state: dict = field(default_factory=dict)  # device-state hint, e.g. selection
    note: str = ""

    def summary(self) -> dict:
        return {
            "step": self.step, "width": self.width, "height": self.height,
            "png_bytes": len(self.png_bytes), "state": self.state, "note": self.note,
        }

class Provider:
    name = "abstract"
    def __call__(self, obs: Observation) -> dict:
        """Return a raw action dict (unvalidated). Must not perform I/O beyond
        the approved endpoint after contract verification."""
        raise NotImplementedError

class FakeProvider(Provider):
    """Deterministic scripted provider for smoke tests. No network, no key."""
    name = "fake"

    def __init__(self, script: Optional[list] = None):
        # script: list of dicts or callables(obs, step) -> dict
        self.script = script if script is not None else [
            {"kind": "click", "x": 100, "y": 100},   # select unit (synthetic)
            {"kind": "key", "key": "a"},              # attack/move order key
            {"kind": "click", "x": 200, "y": 150},   # target point
        ]
        self.calls = 0

    def __call__(self, obs: Observation) -> dict:
        self.calls += 1
        idx = obs.step
        if idx < len(self.script):
            item = self.script[idx]
            if callable(item):
                return item(obs, idx)
            return dict(item)
        return {"kind": "wait", "wait_ms": 100}
