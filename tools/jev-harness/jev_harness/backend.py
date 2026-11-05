"""Game backends: isolated viewport only. Never desktop.

- SyntheticBackend: in-memory state machine for smoke tests.
- OriginalReferenceBackend: wraps tools/original-game-reference/reference.py
  (Docker/Wine/Xvfb). This is the ORIGINAL WC3 client, explicitly NOT
  PeaceCraft gameplay. Label it as such in every trace.
- PeaceCraft browser backend: NOT implemented here — requires integrator
  wiring through the existing browser-adapter (see README). The Protocol
  below is what that backend must satisfy.
"""
from __future__ import annotations
import hashlib
import struct
import subprocess
import threading
import time
import zlib
from pathlib import Path
from typing import Protocol, runtime_checkable

from .actions import Action

class BackendError(RuntimeError):
    pass

@runtime_checkable
class GameBackend(Protocol):
    label: str
    width: int
    height: int
    def capture(self) -> tuple[bytes, dict]:
        """Return (png_bytes, state_dict) for isolated viewport only."""
        ...
    def apply(self, action: Action) -> dict:
        """Apply validated action to this surface only. Returns receipt."""
        ...
    def close(self) -> None:
        ...
    def cancelled(self) -> bool:
        ...

def _png(width: int, height: int, rgb: tuple[int, int, int]) -> bytes:
    """Minimal valid 1-color PNG, stdlib only."""
    def chunk(tag: bytes, data: bytes) -> bytes:
        c = struct.pack(">I", len(data)) + tag + data
        return c + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
    sig = b"\x89PNG\r\n\x1a\n"
    ihdr = chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
    raw = b"".join(b"\x00" + bytes(rgb) * width for _ in range(height))
    idat = chunk(b"IDAT", zlib.compress(raw))
    iend = chunk(b"IEND", b"")
    return sig + ihdr + idat + iend


class SyntheticBackend:
    """Honest smoke-test fixture. NOT a substitute for game trial.

    State machine: cursor, selected (bool), orders (list).
    Click on unit region (x<150,y<150) selects; subsequent click adds order.
    """
    label = "synthetic-smoke-fixture (NOT PeaceCraft gameplay)"

    def __init__(self, width: int = 640, height: int = 480):
        self.width = width
        self.height = height
        self.cursor = (0, 0)
        self.selected = False
        self.orders: list[tuple[int, int]] = []
        self._cancel = threading.Event()
        self.applied = 0

    def cancel(self):
        self._cancel.set()

    def cancelled(self) -> bool:
        return self._cancel.is_set()

    def capture(self) -> tuple[bytes, dict]:
        # Color encodes state: dark=unselected, mid=selected, bright=orders>0
        if self.orders:
            rgb = (40, 120, 40)
        elif self.selected:
            rgb = (40, 60, 120)
        else:
            rgb = (24, 24, 24)
        png = _png(self.width, self.height, rgb)
        state = {
            "cursor": list(self.cursor), "selected": self.selected,
            "order_count": len(self.orders), "applied": self.applied,
        }
        return png, state

    def apply(self, action: Action) -> dict:
        if self._cancel.is_set():
            raise BackendError("backend cancelled")
        self.applied += 1
        if action.kind == "click":
            self.cursor = (action.x, action.y)
            if action.x < 150 and action.y < 150 and not self.selected:
                self.selected = True
            elif self.selected:
                self.orders.append((action.x, action.y))
        elif action.kind == "key":
            pass  # recorded in receipt; synthetic has no key effect beyond count
        elif action.kind == "wait":
            time.sleep(min(action.wait_ms, 50) / 1000.0)  # capped for tests
        return {"applied": self.applied, "kind": action.kind}

    def close(self) -> None:
        self._cancel.set()


class OriginalReferenceBackend:
    """Original WC3 via Docker/Wine/Xvfb — explicitly NOT PeaceCraft gameplay.

    Never opens an unowned client: caller must supply a run_dir already
    prepared+launched by reference.py, and this backend only sends
    capture/click/key to that owned container. No network from container.
    """
    label = "original WC3 reference backend (NOT PeaceCraft gameplay)"

    def __init__(self, run_dir: str | Path, width: int = 1024, height: int = 768,
                 runner: Path | None = None):
        self.run_dir = Path(run_dir)
        self.width = width
        self.height = height
        self._runner = Path(runner) if runner else (
            Path(__file__).resolve().parents[2] / "original-game-reference" / "reference.py"
        )
        self._n = 0
        self._cancel = threading.Event()

    def cancel(self):
        self._cancel.set()

    def cancelled(self) -> bool:
        return self._cancel.is_set()

    def _run(self, *args: str) -> dict:
        if self._cancel.is_set():
            raise BackendError("backend cancelled")
        proc = subprocess.run(
            ["python3", str(self._runner), *args, "--run-dir", str(self.run_dir)],
            capture_output=True, text=True, timeout=60,
        )
        if proc.returncode != 0:
            # Redact any accidental credential-ish content (shouldn't exist)
            from .redact import redact_text
            raise BackendError(redact_text(f"reference.py failed: {proc.stderr[:500]}"))
        import json
        return json.loads(proc.stdout or "{}")

    def capture(self) -> tuple[bytes, dict]:
        self._n += 1
        label = f"jev-{self._n}"
        receipt = self._run("capture", "--label", label)
        p = self.run_dir / "captures" / f"{label}.png"
        data = p.read_bytes()
        return data, {"receipt": receipt, "label": label,
                      "sha256": hashlib.sha256(data).hexdigest()}

    def apply(self, action: Action) -> dict:
        if action.kind == "click":
            return self._run("click", "--x", str(action.x), "--y", str(action.y))
        if action.kind == "key":
            # reference.py only accepts [a-zA-Z0-9_+]{1,80}; allowlist keys pass
            if action.key not in ("Space", "Enter", "Escape", "Tab", "Backspace",
                                  "Delete", "Up", "Down", "Left", "Right",
                                  "Home", "End", "PageUp", "PageDown") \
               and not (len(action.key) == 1 and action.key.isalnum()):
                raise BackendError(f"key not supported by original backend: {action.key!r}")
            return self._run("key", "--key", action.key)
        if action.kind == "wait":
            time.sleep(min(action.wait_ms, 2000) / 1000.0)
            return {"kind": "wait"}
        raise BackendError(f"unsupported action: {action.kind!r}")

    def close(self) -> None:
        self._cancel.set()
