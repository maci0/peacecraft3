"""Game backends: isolated viewport only. Never desktop.

- SyntheticBackend: in-memory state machine for smoke tests.
- OriginalReferenceBackend: wraps tools/original-game-reference/reference.py
  (Docker/Wine/Xvfb). This is the ORIGINAL WC3 client, explicitly NOT
  PeaceCraft gameplay. Label it as such in every trace.
- PeaceCraftBrowserBackend: drives the REAL PeaceCraft III WASM app in
  headless muted Chromium through jev-browser.mjs, which reuses the existing
  browser-adapter.mjs capture/input path (checkedGeometry, physicalPoint,
  actual clipped canvas, screen-geometry verification). This is ACTUAL
  PeaceCraft gameplay, not a substitute.
"""
from __future__ import annotations
import hashlib
import json
import os
import select
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


_PNG_SIG = b"\x89PNG\r\n\x1a\n"

def _png_dimensions(data: bytes) -> tuple[int, int]:
    if len(data) < 24 or data[:8] != _PNG_SIG or data[12:16] != b"IHDR":
        raise BackendError("capture is not a PNG with IHDR")
    w = int.from_bytes(data[16:20], "big")
    h = int.from_bytes(data[20:24], "big")
    if not (1 <= w <= 16384 and 1 <= h <= 16384):
        raise BackendError(f"capture PNG extent {w}x{h} outside 1..16384")
    return w, h

def _verify_original_geometry(metadata: dict, width: int, height: int) -> dict:
    """Fail-closed verify full-root capture actually IS viewport.

    Reuses metadata types reference.py capture already emits: xdpyinfo
    display + xwininfo -root -tree windows (sidecar JSON). Does NOT call
    replay.native_window (needs live docker-exec + scenario title/DPI,
    unavailable here).
    Requires: single screen dims == declared, exactly one game window at
    origin with WIDTH/HEIGHT == declared, no other window covering root.
    Never assume matching total dimensions == viewport.
    """
    if not isinstance(metadata, dict):
        raise BackendError("reference capture metadata missing; verification unsupported, refusing live use")
    display = metadata.get("display")
    windows = metadata.get("windows")
    if not isinstance(display, str) or not isinstance(windows, str) or not display or not windows:
        raise BackendError("reference capture metadata missing display/windows; verification unsupported, refusing live use")
    import re
    # xdpyinfo: single screen #0 dims must match declared
    screens = re.split(r"(?m)^screen #(\d+):[ \t]*$", display)
    dims = re.findall(r"(?m)^[ \t]*dimensions:[ \t]*(\d+)x(\d+) pixels\b", display)
    if len(screens) != 3 or screens[1] != "0":
        raise BackendError("reference display is not a single screen #0; refusing live use")
    if dims != [(str(width), str(height))]:
        raise BackendError(f"reference X framebuffer {dims} differs from declared {width}x{height}; refusing")
    # xwininfo -root -tree: parse windows. Format lines like:
    #   0x... "title": ("...") 1024x768+0+0  +10+10 +0+0
    # We require exactly one leaf game window at +0+0 sized WxH, and root size WxH.
    # If parsing yields no windows or ambiguous, refuse.
    win_re = re.compile(r'(?m)^\s+(0x[0-9a-fA-F]+)\s+"([^"]*)":.*?\s(\d+)x(\d+)\+(\d+)\+(\d+)')
    wins = win_re.findall(windows)
    if not wins:
        raise BackendError("reference xwininfo tree has no parseable windows; verification unsupported, refusing live use")
    root_match = re.search(r'(?m)^\s+Root window:.*?\s(\d+)x(\d+)\+(\d+)\+(\d+)', windows)
    # Fallback: first window line is typically root; if root line not matched, require first win dims
    if root_match:
        rw, rh, rx, ry = map(int, root_match.groups())
        if (rw, rh, rx, ry) != (width, height, 0, 0):
            raise BackendError(f"reference root window {rw}x{rh}+{rx}+{ry} differs from declared {width}x{height} at origin; refusing")
    else:
        # Use first parsed window as root candidate; must match.
        fw, fh, fx, fy = map(int, wins[0][2:])
        if (fw, fh, fx, fy) != (width, height, 0, 0):
            raise BackendError(f"reference root window {fw}x{fh}+{fx}+{fy} differs from declared {width}x{height}; refusing")
    # Game window: exactly one window (excluding root) sized WxH at origin.
    # If xwininfo lists only root (no child), that itself is suspicious but we
    # treat root-only as unverified unless a second window exists. Require >=1 child.
    root_id = None
    if root_match:
        m = re.search(r'(?m)^\s+Root window:\s+(0x[0-9a-fA-F]+)', windows)
        if m:
            root_id = m.group(1)
    candidates = []
    for wid, title, w, h, x, y in wins:
        wi, hi, xi, yi = map(int, (w, h, x, y))
        if (wi, hi, xi, yi) == (width, height, 0, 0) and wid != root_id:
            candidates.append((wid, title))
    if len(candidates) != 1:
        raise BackendError(
            f"expected exactly one game window {width}x{height} at origin, found {len(candidates)} "
            f"(total windows {len(wins)}); full-root capture not verified as viewport, refusing live use"
        )
    return {"window": candidates[0][0], "window_title": candidates[0][1],
            "window_extent": [width, height], "window_position": [0, 0],
            "framebuffer_extent": [width, height], "root_extent": [width, height]}

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
        self._verified_geometry: dict | None = None

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
        # Any failed/stale capture must clear verified flag so apply cannot
        # run on unverified geometry (TOCTOU: live geometry may change
        # after this capture; see apply() TOCTOU note).
        self._verified_geometry = None
        receipt = self._run("capture", "--label", label)
        p = self.run_dir / "captures" / f"{label}.png"
        data = p.read_bytes()
        # PNG IHDR must match declared viewport BEFORE return/actions
        try:
            w, h = _png_dimensions(data)
        except BackendError:
            self._verified_geometry = None
            raise
        if (w, h) != (self.width, self.height):
            self._verified_geometry = None
            raise BackendError(f"capture PNG extent {w}x{h} differs from declared viewport {self.width}x{self.height}; no rescaling; refusing")
        # Sidecar JSON carries display/windows from reference.py capture; reuse it.
        meta_path = p.with_suffix(".json")
        try:
            metadata = json.loads(meta_path.read_text())
        except (OSError, json.JSONDecodeError) as e:
            self._verified_geometry = None
            raise BackendError(f"reference capture metadata unreadable ({e}); verification unsupported, refusing live use") from e
        try:
            geom = _verify_original_geometry(metadata, self.width, self.height)
        except BackendError:
            self._verified_geometry = None
            raise
        self._verified_geometry = geom
        return data, {"receipt": receipt, "label": label,
                      "sha256": hashlib.sha256(data).hexdigest(),
                      "geometry": geom, "png_extent": [w, h],
                      "verified_viewport": True}

    def apply(self, action: Action) -> dict:
        # TOCTOU: geometry verified at last successful capture may change in
        # live client before apply runs; no API to re-verify atomically with
        # input exists in reference.py (capture is sidecar side-effect, not
        # a lock). Minimum mitigation: require a fresh successful capture
        # immediately before each apply path in the harness loop; backend
        # cannot close the window itself.
        if self._verified_geometry is None:
            raise BackendError("original backend not verified: capture must succeed with geometry verification before any action; refusing")
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

class PeaceCraftBrowserBackend:
    """ACTUAL PeaceCraft III WASM app in headless muted Chromium.

    Drives jev-browser.mjs, which reuses the existing browser-adapter.mjs
    capture/input path against the real PeaceCraft browser build
    (web/pkg/peacecraft3.wasm). Capture uses checkedGeometry + clipped
    canvas; input uses physicalPoint/physicalKey with explicit move first.
    No None to 0,0 fallback; no OS desktop input or audio.
    """
    label = "PeaceCraft III browser WASM (actual PeaceCraft gameplay)"

    def __init__(self, *, url: str, playwright_module: str | Path,
                 chromium: str | Path, width: int = 1280, height: int = 960,
                 manifest: str | Path | None = None,
                 iso: str | Path | None = None,
                 edition: str | None = None,
                 import_mode: str | None = None,
                 out_dir: str | Path | None = None,
                 startup_timeout_s: float = 300.0,
                 driver: str | Path | None = None):
        self.width = width
        self.height = height
        self.url = url
        self.startup_timeout_s = startup_timeout_s
        self._cancel = threading.Event()
        self._serial = 0
        self._n = 0
        self._proc = None
        self._buf = b""
        self._start_state = None
        self._stripped_env = {k: v for k, v in os.environ.items()
                              if k not in ("DISPLAY", "WAYLAND_DISPLAY")}
        from urllib.parse import urlparse
        parsed = urlparse(url)
        if parsed.scheme != "http" or parsed.hostname not in ("127.0.0.1", "localhost", "::1"):
            raise BackendError(f"PeaceCraft browser URL must be loopback http, got {url!r}")
        if parsed.username or parsed.password or parsed.query or parsed.fragment:
            raise BackendError("PeaceCraft browser URL must not contain auth or query data")
        self._playwright = Path(playwright_module)
        self._chromium = Path(chromium)
        if not self._playwright.is_file():
            raise BackendError(f"playwright module not found: {self._playwright}")
        if not self._chromium.is_file():
            raise BackendError(f"chromium not found: {self._chromium}")
        if not (isinstance(width, int) and isinstance(height, int) and 1 <= width <= 4096 and 1 <= height <= 4096):
            raise BackendError("extent must be 1..4096 physical pixels")
        self._manifest = Path(manifest) if manifest else None
        if self._manifest and not self._manifest.is_file():
            raise BackendError(f"manifest not found: {self._manifest}")
        self._iso = Path(iso) if iso else None
        if self._iso and not self._iso.is_file():
            raise BackendError(f"iso not found: {self._iso}")
        if edition is not None and edition not in ("roc", "tft"):
            raise BackendError("edition must be roc or tft")
        if import_mode is None:
            import_mode = "iso" if self._iso else "none"
        if import_mode not in ("iso", "assets", "none"):
            raise BackendError("import_mode must be iso/assets/none")
        if import_mode == "iso" and not self._iso:
            raise BackendError("iso import_mode requires iso path")
        if import_mode == "iso" and edition is None:
            raise BackendError("iso import_mode requires edition")
        self._edition = edition
        self._import_mode = import_mode
        if out_dir is None:
            raise BackendError("out_dir is required (local .scratch or /tmp)")
        self.out_dir = Path(out_dir).resolve()
        self.out_dir.mkdir(parents=True, exist_ok=True)
        self._driver = Path(driver) if driver else (
            Path(__file__).resolve().parent / "jev-browser.mjs"
        )
        if not self._driver.is_file():
            raise BackendError(f"jev-browser.mjs driver not found: {self._driver}")
        import shutil
        if shutil.which("node") is None:
            raise BackendError("node is required for PeaceCraft browser backend")
        self._start()

    def _start(self):
        import tempfile
        config = {
            "url": self.url,
            "manifest": str(self._manifest) if self._manifest else None,
            "iso": str(self._iso) if self._iso else None,
            "playwrightModule": str(self._playwright),
            "chromium": str(self._chromium),
            "extent": [self.width, self.height],
            "edition": self._edition,
            "importMode": self._import_mode,
        }
        tmp = tempfile.NamedTemporaryFile("w", suffix=".json", delete=False)
        json.dump(config, tmp)
        tmp.close()
        self._proc = subprocess.Popen(
            ["node", str(self._driver), tmp.name],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=subprocess.PIPE, env=self._stripped_env,
        )
        # Config must outlive node startup (node reads it after fork). Remove
        # only after the startup response, never before.
        try:
            resp = self._receive(self.startup_timeout_s)
        except Exception:
            self._terminate()
            raise
        finally:
            try:
                Path(tmp.name).unlink(missing_ok=True)
            except Exception:
                pass
        if not resp.get("ok"):
            err = resp.get("error", {})
            self._terminate()
            from .redact import redact_text
            raise BackendError(redact_text(
                f"PeaceCraft browser startup refused [{err.get('code')}] "
                f"stage={err.get('stage')}: {str(err.get('message'))[:500]}"
            ))
        self._start_state = resp.get("data", {})

    def _receive(self, timeout: float) -> dict:
        deadline = time.monotonic() + timeout
        buf = self._buf
        while b"\n" not in buf:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise BackendError("PeaceCraft browser response timed out; backend stopped, never retried")
            if not select.select([self._proc.stdout], [], [], max(0, remaining))[0]:
                raise BackendError("PeaceCraft browser response timed out; backend stopped, never retried")
            chunk = os.read(self._proc.stdout.fileno(), 65536)
            if not chunk:
                raise BackendError(f"PeaceCraft browser exited {self._proc.poll()}; inspect backend stderr")
            buf += chunk
        line, buf = buf.split(b"\n", 1)
        self._buf = buf
        if len(line) > 2 * 1024 * 1024:
            raise BackendError("PeaceCraft browser response exceeds metadata bound")
        result = json.loads(line)
        if not isinstance(result, dict) or type(result.get("ok")) is not bool:
            raise BackendError("malformed PeaceCraft browser protocol response")
        return result

    def _request(self, op: str, timeout: float = 90, **arguments) -> dict:
        if self._cancel.is_set():
            raise BackendError("backend cancelled")
        if self._proc is None or self._proc.poll() is not None:
            raise BackendError("PeaceCraft browser not running")
        self._serial += 1
        msg = {"id": self._serial, "op": op, **arguments}
        self._proc.stdin.write((json.dumps(msg) + "\n").encode())
        self._proc.stdin.flush()
        result = self._receive(timeout)
        if type(result.get("id")) is not int or result.get("id") != self._serial:
            raise BackendError("PeaceCraft browser response order mismatch")
        if not result.get("ok"):
            err = result.get("error", {})
            from .redact import redact_text
            raise BackendError(redact_text(
                f"PeaceCraft browser refused [{err.get('code')}] "
                f"stage={err.get('stage')}: {str(err.get('message'))[:500]}"
            ))
        return result.get("data") or {}

    def _terminate(self):
        proc, self._proc = self._proc, None
        if proc is None:
            return
        try:
            proc.stdin.close()
        except Exception:
            pass
        try:
            proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            proc.terminate()
            try:
                proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait(timeout=10)

    def cancel(self):
        self._cancel.set()

    def cancelled(self) -> bool:
        return self._cancel.is_set()

    def capture(self) -> tuple[bytes, dict]:
        self._n += 1
        path = self.out_dir / f"jev-{self._n}.png"
        if path.exists():
            raise BackendError(f"capture path exists; choose a new out_dir: {path}")
        data = self._request("capture", timeout=150, path=str(path))
        png = path.read_bytes()
        if len(png) < 24 or png[:8] != b"\x89PNG\r\n\x1a\n":
            raise BackendError("capture is not a PNG")
        w = int.from_bytes(png[16:20], "big")
        h = int.from_bytes(png[20:24], "big")
        if [w, h] != [self.width, self.height]:
            raise BackendError(f"capture extent {w}x{h} differs from requested {self.width}x{self.height}; no rescaling")
        state = {
            "backend": self.label,
            "sha256": data.get("sha256") or hashlib.sha256(png).hexdigest(),
            "geometry": data.get("geometry"),
            "state_before": data.get("state_before"),
            "state_after": data.get("state_after"),
            "state_changed_during_capture": data.get("state_changed_during_capture"),
            "import_mode": self._import_mode,
            "startup_state": self._start_state,
        }
        return png, state

    def apply(self, action: Action) -> dict:
        if action.kind == "click":
            return self._request("input", timeout=90,
                                 event={"kind": "click", "x": action.x, "y": action.y, "button": action.button})
        if action.kind == "key":
            return self._request("input", timeout=90,
                                 event={"kind": "key", "key": action.key})
        if action.kind == "wait":
            time.sleep(min(action.wait_ms, 2000) / 1000.0)
            return {"kind": "wait", "wait_ms": action.wait_ms}
        raise BackendError(f"unsupported action: {action.kind!r}")

    def close(self) -> None:
        self._cancel.set()
        self._terminate()
