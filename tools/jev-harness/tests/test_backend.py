"""Tests for PeaceCraftBrowserBackend using a fake node driver (mock).

Stdlib only, headless, no network, no real key, no real Chromium.
The fake driver speaks the jev-browser.mjs line protocol and writes a
minimal valid PNG so capture extent checks are real.
"""
import json
import struct
import sys
import tempfile
import zlib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from jev_harness import Action, PeaceCraftBrowserBackend, BackendError
from jev_harness.backend import OriginalReferenceBackend, _verify_original_geometry, _png_dimensions


def _fake_driver(tmp_path: Path, mode: str = "ok") -> Path:
    """Write a fake jev-browser driver node script and return its path."""
    script = tmp_path / "fake-driver.mjs"
    playwright = tmp_path / "playwright.mjs"
    playwright.write_text("export const chromium = {};\n")
    chromium = tmp_path / "chromium"
    chromium.write_text("#!/bin/sh\n")
    chromium.chmod(0o755)
    if mode == "refuse_startup":
        body = """
import {readFile} from 'node:fs/promises';
import {createInterface} from 'node:readline';
const config = JSON.parse(await readFile(process.argv[2], 'utf8'));
process.stdout.write(JSON.stringify({ok: false, error: {
  message: 'CANVAS_MISSING: Launcher canvas is absent', code: 'CANVAS_MISSING',
  stage: 'canvas-ready', message_truncated: false, code_truncated: false,
  errors: [], sockets: [], requests: [], messages: [],
  omitted: {errors: 0, sockets: 0, requests: 0, messages: 0}}}) + '\\n');
process.exitCode = 2;
"""
    elif mode == "bad_png":
        body = """
import {readFile, writeFile} from 'node:fs/promises';
import {createInterface} from 'node:readline';
import {createHash} from 'node:crypto';
const config = JSON.parse(await readFile(process.argv[2], 'utf8'));
const [w, h] = config.extent;
process.stdout.write(JSON.stringify({ok: true, data: {state: {screen: 'Main'}, extent: config.extent, dpr: 1}}) + '\\n');
const respond = m => process.stdout.write(JSON.stringify(m) + '\\n');
for await (const line of createInterface({input: process.stdin, crlfDelay: Infinity})) {
  const req = JSON.parse(line);
  if (req.op === 'capture') {
    // Write a PNG with WRONG extent to exercise the backend's extent guard.
    const bytes = Buffer.alloc(24);
    bytes.set([137,80,78,71,13,10,26,10]);
    bytes.writeUInt32BE(1, 16); bytes.writeUInt32BE(1, 20);
    await writeFile(req.path, bytes);
    respond({id: req.id, ok: true, data: {path: req.path, sha256: 'x', geometry: {}, state_before: {}, state_after: {}, state_changed_during_capture: false}});
  } else if (req.op === 'input') {
    respond({id: req.id, ok: true, data: {applied: true}});
  } else respond({id: req.id, ok: true, data: {}});
}
"""
    else:  # ok
        body = """
import {readFile, writeFile} from 'node:fs/promises';
import {createInterface} from 'node:readline';
import {createHash} from 'node:crypto';
const config = JSON.parse(await readFile(process.argv[2], 'utf8'));
const [w, h] = config.extent;
// Build a minimal valid PNG of the requested extent in JS (1-color).
function png(width, height) {
  const sig = Buffer.from([137,80,78,71,13,10,26,10]);
  const chunk = (tag, data) => { const len = Buffer.alloc(4); len.writeUInt32BE(data.length); const td = Buffer.concat([tag, data]); const crc = Buffer.alloc(4); crc.writeUInt32BE(0); return Buffer.concat([len, td, crc]); };
  return Buffer.concat([sig, chunk(Buffer.from('IHDR'), Buffer.from([0,0,0,width,0,0,0,height,8,2,0,0,0]))]);
}
process.stdout.write(JSON.stringify({ok: true, data: {state: {screen: 'Main', menuVisible: true}, extent: config.extent, dpr: 1, importMode: config.importMode}}) + '\\n');
const respond = m => process.stdout.write(JSON.stringify(m) + '\\n');
for await (const line of createInterface({input: process.stdin, crlfDelay: Infinity})) {
  const req = JSON.parse(line);
  if (req.op === 'capture') {
    const bytes = png(w, h);
    await writeFile(req.path, bytes);
    respond({id: req.id, ok: true, data: {path: req.path, sha256: 'abc', geometry: {clip: {x: 0, y: 0, width: w, height: h, scale: 1}}, state_before: {screen: 'Main'}, state_after: {screen: 'Main'}, state_changed_during_capture: false}});
  } else if (req.op === 'input') {
    respond({id: req.id, ok: true, data: {applied: true, event: req.event}});
  } else respond({id: req.id, ok: true, data: {}});
}
"""
    script.write_text(body)
    return script, playwright, chromium


def _backend(tmp_path: Path, mode: str = "ok", **kw):
    driver, playwright, chromium = _fake_driver(tmp_path, mode)
    out = tmp_path / "out"
    out.mkdir(exist_ok=True)
    args = dict(
        url=kw.pop("url", "http://127.0.0.1:8765/index.html"),
        playwright_module=str(playwright),
        chromium=str(chromium),
        width=kw.pop("width", 80),
        height=kw.pop("height", 60),
        out_dir=str(out),
        driver=str(driver),
        startup_timeout_s=10,
    )
    args.update(kw)
    return PeaceCraftBrowserBackend(**args), out, playwright, chromium


def test_backend_rejects_non_loopback_url(tmp_path=None):
    import tempfile
    with tempfile.TemporaryDirectory() as d:
        try:
            _backend(Path(d), url="http://example.com/index.html")
            raise AssertionError("expected BackendError for non-loopback URL")
        except BackendError as e:
            assert "loopback" in str(e)
        try:
            _backend(Path(d), url="https://127.0.0.1/x")
            raise AssertionError("expected BackendError for https")
        except BackendError:
            pass


def test_backend_rejects_missing_playwright(tmp_path=None):
    import tempfile
    with tempfile.TemporaryDirectory() as d:
        driver, playwright, chromium = _fake_driver(Path(d))
        try:
            PeaceCraftBrowserBackend(url="http://127.0.0.1:8765/index.html",
                                     playwright_module="/nonexistent/pw.mjs",
                                     chromium=str(chromium), out_dir=str(Path(d) / "o"),
                                     driver=str(driver))
            raise AssertionError("expected BackendError for missing playwright")
        except BackendError as e:
            assert "playwright" in str(e)


def test_backend_rejects_iso_without_edition(tmp_path=None):
    import tempfile
    with tempfile.TemporaryDirectory() as d:
        iso = Path(d) / "game.iso"
        iso.write_bytes(b"fake iso bytes")
        try:
            _backend(Path(d), iso=str(iso), edition=None)
            raise AssertionError("expected BackendError for iso without edition")
        except BackendError as e:
            assert "edition" in str(e)


def test_backend_startup_refusal_surfaces_code(tmp_path=None):
    import tempfile
    with tempfile.TemporaryDirectory() as d:
        try:
            _backend(Path(d), mode="refuse_startup")
            raise AssertionError("expected BackendError at startup")
        except BackendError as e:
            assert "CANVAS_MISSING" in str(e)
            assert "canvas-ready" in str(e)


def test_backend_capture_input_apply_roundtrip(tmp_path=None):
    import tempfile
    with tempfile.TemporaryDirectory() as d:
        backend, out, _, _ = _backend(Path(d), mode="ok")
        try:
            assert backend.width == 80 and backend.height == 60
            assert "PeaceCraft" in backend.label
            assert backend._start_state.get("dpr") == 1
            png, state = backend.capture()
            assert png[:8] == b"\x89PNG\r\n\x1a\n"
            assert int.from_bytes(png[16:20], "big") == 80
            assert int.from_bytes(png[20:24], "big") == 60
            assert state["state_changed_during_capture"] is False
            assert state["geometry"]["clip"]["width"] == 80
            # apply click -> driver echoes event
            receipt = backend.apply(Action.from_dict({"kind": "click", "x": 5, "y": 7}))
            assert receipt.get("applied") is True
            assert receipt["event"]["x"] == 5
            # apply key
            receipt2 = backend.apply(Action.from_dict({"kind": "key", "key": "a"}))
            assert receipt2.get("applied") is True
            # cancelled flag + close terminate process
            assert backend.cancelled() is False
            backend.cancel()
            assert backend.cancelled() is True
            try:
                backend.apply(Action.from_dict({"kind": "click", "x": 1, "y": 1}))
                raise AssertionError("expected BackendError after cancel")
            except BackendError:
                pass
        finally:
            backend.close()
            assert backend._proc is None


def test_backend_capture_extent_guard(tmp_path=None):
    import tempfile
    with tempfile.TemporaryDirectory() as d:
        backend, out, _, _ = _backend(Path(d), mode="bad_png")
        try:
            try:
                backend.capture()
                raise AssertionError("expected BackendError for wrong PNG extent")
            except BackendError as e:
                assert "extent" in str(e) or "differs" in str(e)
        finally:
            backend.close()


def test_backend_wait_action(tmp_path=None):
    import tempfile
    with tempfile.TemporaryDirectory() as d:
        backend, out, _, _ = _backend(Path(d), mode="ok")
        try:
            receipt = backend.apply(Action.from_dict({"kind": "wait", "wait_ms": 10}))
            assert receipt["kind"] == "wait"
        finally:
            backend.close()

# --- OriginalReferenceBackend fail-closed verification (mock JSON/PNG, no client) ---

def _good_meta(w=1024, h=768):
    return {
        "display": f"default screen number:    0\nscreen #0:\n  dimensions:    {w}x{h} pixels (271x203 millimeters)\n  resolution:    96x96 dots per inch\n",
        "windows": (
            f'xwininfo: Window id: 0x100000 "root"\n\n'
            f'  Root window: 0x100000 "root"  {w}x{h}+0+0  +0+0 +0+0\n'
            f'  0x200000 "Warcraft III": ("Warcraft III.exe")  {w}x{h}+0+0  +0+0 +0+0\n'
            f'    0x200001 "child": ("child")  100x100+10+10  +10+10 +0+0\n'
        ),
    }

def _png_bytes(w, h):
    import zlib, struct
    sig = b"\x89PNG\r\n\x1a\n"
    ihdr_data = struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)
    ihdr = struct.pack(">I", 13) + b"IHDR" + ihdr_data + struct.pack(">I", zlib.crc32(b"IHDR" + ihdr_data) & 0xFFFFFFFF)
    return sig + ihdr + b"\x00" * 32

def _mock_backend(tmp, png_w=1024, png_h=768, meta=None):
    run = Path(tmp) / "run"
    (run / "captures").mkdir(parents=True)
    meta = _good_meta() if meta is None else meta
    runner = Path(tmp) / "fake-reference.py"
    lines = [
        "import json, sys, pathlib",
        "run = pathlib.Path(sys.argv[sys.argv.index('--run-dir')+1])",
        "label = sys.argv[sys.argv.index('--label')+1] if '--label' in sys.argv else 'jev-1'",
        f"png = {_png_bytes(png_w, png_h)!r}",
        "(run/'captures'/(label+'.png')).write_bytes(png)",
        f"(run/'captures'/(label+'.json')).write_text(json.dumps({meta!r}))",
        "print(json.dumps({'capture': str(run/'captures'/(label+'.png')), 'sha256': 'x'}))",
    ]
    runner.write_text("\n".join(lines) + "\n")
    return OriginalReferenceBackend(run, width=1024, height=768, runner=runner)

def test_verify_good_geometry():
    g = _verify_original_geometry(_good_meta(), 1024, 768)
    assert g["window_extent"] == [1024, 768]
    assert g["window"] == "0x200000"

def test_verify_rejects_missing_metadata():
    for bad in [{}, {"display": "x"}, {"display": _good_meta()["display"], "windows": ""}]:
        try:
            _verify_original_geometry(bad, 1024, 768)
            raise AssertionError("expected BackendError missing metadata")
        except BackendError as e:
            assert "verification unsupported" in str(e)
    try:
        _verify_original_geometry(None, 1024, 768)
        raise AssertionError("expected BackendError None metadata")
    except BackendError:
        pass

def test_verify_rejects_wrong_framebuffer():
    m = _good_meta()
    m["display"] = m["display"].replace("1024x768", "800x600")
    try:
        _verify_original_geometry(m, 1024, 768)
        raise AssertionError("expected framebuffer mismatch")
    except BackendError as e:
        assert "framebuffer" in str(e)

def test_verify_rejects_window_not_full_viewport():
    m = _good_meta()
    m["windows"] = m["windows"].replace('0x200000 "Warcraft III": ("Warcraft III.exe")  1024x768+0+0', '0x200000 "Warcraft III": ("Warcraft III.exe")  800x600+10+10')
    try:
        _verify_original_geometry(m, 1024, 768)
        raise AssertionError("expected game window mismatch")
    except BackendError as e:
        assert "game window" in str(e)

def test_verify_rejects_root_only():
    m = _good_meta()
    m["windows"] = 'xwininfo: Window id: 0x100000 "root"\n  Root window: 0x100000 "root"  1024x768+0+0  +0+0 +0+0\n'
    try:
        _verify_original_geometry(m, 1024, 768)
        raise AssertionError("expected root-only refusal")
    except BackendError as e:
        assert "verification" in str(e) or "game window" in str(e)

def test_verify_rejects_two_full_windows():
    m = _good_meta()
    m["windows"] += '  0x300000 "other": ("other")  1024x768+0+0  +0+0 +0+0\n'
    try:
        _verify_original_geometry(m, 1024, 768)
        raise AssertionError("expected ambiguous window refusal")
    except BackendError as e:
        assert "game window" in str(e)

def test_png_dimensions():
    assert _png_dimensions(_png_bytes(1024, 768)) == (1024, 768)
    try:
        _png_dimensions(b"not a png at all........")
        raise AssertionError("expected BackendError non-PNG")
    except BackendError:
        pass

def test_original_capture_ok_then_apply():
    import tempfile
    with tempfile.TemporaryDirectory() as d:
        b = _mock_backend(d)
        png, state = b.capture()
        assert png[:8] == b"\x89PNG\r\n\x1a\n"
        assert state["verified_viewport"] is True
        assert state["geometry"]["window_extent"] == [1024, 768]
        assert state["png_extent"] == [1024, 768]
        try:
            b.apply(Action.from_dict({"kind": "click", "x": 1, "y": 1}))
        except BackendError as e:
            assert "not verified" not in str(e)

def test_original_capture_rejects_wrong_png_extent():
    import tempfile
    with tempfile.TemporaryDirectory() as d:
        b = _mock_backend(d, png_w=800, png_h=600)
        try:
            b.capture()
            raise AssertionError("expected PNG extent BackendError")
        except BackendError as e:
            assert "PNG extent" in str(e) or "differs" in str(e)

def test_original_capture_rejects_bad_metadata():
    import tempfile
    with tempfile.TemporaryDirectory() as d:
        b = _mock_backend(d, meta={"display": "x", "windows": "y"})
        try:
            b.capture()
            raise AssertionError("expected metadata BackendError")
        except BackendError as e:
            assert "refusing" in str(e) or "verification" in str(e) or "framebuffer" in str(e) or "unsupported" in str(e)

def test_original_apply_refused_before_verify():
    import tempfile
    with tempfile.TemporaryDirectory() as d:
        b = _mock_backend(d)
        try:
            b.apply(Action.from_dict({"kind": "click", "x": 1, "y": 1}))
            raise AssertionError("expected unverified BackendError")
        except BackendError as e:
            assert "not verified" in str(e)
