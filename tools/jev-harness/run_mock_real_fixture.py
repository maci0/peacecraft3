#!/usr/bin/env python3
"""Mock-Jev -> REAL canonical process proof (protocol 1) — stdlib only.

Same isolated launch as ``run_real_fixture.py`` (owned cargo-test child,
private Xvfb, ALSA null, no PULSE_SERVER socket, private XDG dirs, killed
only via its own process group, no live API / env key / secret-file read),
but the typed Move/Stop choices come from ``DecisionAdapter`` with an
injected ``mock_transport`` (fake key, queued typed payloads) instead of
hand-written client calls. Asserts: nonempty real receipts (ids 1..2,
ok, game-time advance), actual unit motion (position delta vs the initial
observation), child exit 0, journal, and a sha256 transcript hash.

Usage:
  python3 tools/jev-harness/run_mock_real_fixture.py [--install DIR]
      [--map 'Maps\\Campaign\\Prologue01.w3m'] [--addon roc]
      [--run-dir .scratch/mock-real-fixture] [--timeout 600]
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import signal
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools" / "jev-harness"))
from jev_harness.canonical_fixture import FixtureClient  # noqa: E402
from jev_harness.decision_adapter import (  # noqa: E402
    CHOICE_MOVE_EAST,
    CHOICE_STOP,
    DecisionAdapter,
    mock_transport,
    typed_payload,
)

TEST_NAME = "campaign_entry_gpu_tests::canonical_fixture_real_interaction"


def _test_binary() -> Path:
    # The main binary target shares the name; only the test harness accepts
    # `--list`. Probe each candidate newest-first and keep the first that
    # actually lists the fixture test.
    deps = ROOT / "target" / "debug" / "deps"
    candidates = [p for p in deps.glob("peacecraft3-*") if p.is_file() and os.access(p, os.X_OK)]
    if not candidates:
        raise SystemExit("no built peacecraft3 test binary; run cargo test --no-run first")
    for cand in sorted(candidates, key=lambda p: p.stat().st_mtime, reverse=True):
        try:
            out = subprocess.run([str(cand), "--list"], capture_output=True, text=True, timeout=10)
        except (OSError, subprocess.TimeoutExpired):
            continue
        if out.returncode == 0 and TEST_NAME in out.stdout:
            return cand
    raise SystemExit("no peacecraft3 test binary lists canonical_fixture_real_interaction; run cargo test --no-run first")


def _private_xvfb() -> str:
    for n in range(90, 120):
        if not Path(f"/tmp/.X11-unix/X{n}").exists():
            return f":{n}"
    raise SystemExit("no free Xvfb display in 90..119")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--install", default=os.environ.get("PCX_NATIVE_CHAPTER_INSTALL"),
                    help="Legacy install directory (default: $PCX_NATIVE_CHAPTER_INSTALL)")
    ap.add_argument("--map", default=r"Maps\Campaign\Prologue01.w3m")
    ap.add_argument("--addon", default="roc")
    ap.add_argument("--run-dir", default=".scratch/mock-real-fixture")
    ap.add_argument("--timeout", type=float, default=600.0)
    args = ap.parse_args()
    if not args.install:
        ap.error("--install or PCX_NATIVE_CHAPTER_INSTALL is required")

    run_dir = (ROOT / args.run_dir).resolve()
    if run_dir.exists():
        raise SystemExit(f"run dir {run_dir} must be new (ignored local rundir)")
    fixture_dir = run_dir / "fixture"
    fixture_dir.mkdir(parents=True)
    xdg_data = run_dir / "xdg-data"
    xdg_config = run_dir / "xdg-config"
    xdg_data.mkdir(parents=True)
    xdg_config.mkdir(parents=True)
    alsa_conf = run_dir / "alsa.conf"
    alsa_conf.write_text("pcm.!default { type null }\nctl.!default { type null }\n")
    pulse_socket = run_dir / "pulse-native"
    display = _private_xvfb()

    env = dict(os.environ)
    env.pop("DISPLAY", None)
    env.pop("WAYLAND_DISPLAY", None)
    # Guard: no live key may leak into the child; mock transport only.
    env.pop("TYPESAFE_API_KEY", None)
    env.pop("JEV_API_KEY", None)
    env["DISPLAY"] = display
    env["ALSA_CONFIG_PATH"] = str(alsa_conf)
    env["PULSE_SERVER"] = f"unix:{pulse_socket}"
    assert not pulse_socket.exists(), "PULSE_SERVER socket must be absent"
    env["XDG_DATA_HOME"] = str(xdg_data)
    env["XDG_CONFIG_HOME"] = str(xdg_config)
    env["PCX_NATIVE_CHAPTER_INSTALL"] = args.install
    env["PCX_NATIVE_CHAPTER_MAP"] = args.map
    env["PCX_NATIVE_CHAPTER_ADDON"] = args.addon
    env["PCX_GAMEPLAY_FIXTURE_DIR"] = str(fixture_dir)
    env.setdefault("VK_DRIVER_FILES", "/usr/share/vulkan/icd.d/lvp_icd.x86_64.json")

    binary = _test_binary()
    cmd = [str(binary), TEST_NAME, "--ignored", "--exact", "--nocapture", "--test-threads=1"]
    print(f"mock-real-fixture: cmd={' '.join(cmd)}", flush=True)

    xvfb = subprocess.Popen(
        ["Xvfb", display, "-screen", "0", "640x480x24", "-r", "-nolisten", "tcp"],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    time.sleep(0.5)
    if xvfb.poll() is not None:
        raise SystemExit(f"private Xvfb {display} failed to start")

    proc = None
    try:
        proc = subprocess.Popen(
            cmd, cwd=str(ROOT), env=env,
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
            text=True, start_new_session=True,
        )
        client = FixtureClient(fixture_dir)
        resp_path = fixture_dir / "fixture-response.json"
        deadline = time.monotonic() + 300.0
        initial = None
        while time.monotonic() < deadline:
            if resp_path.is_file():
                try:
                    rec = json.loads(resp_path.read_text())
                except (OSError, ValueError):
                    rec = None
                if rec and rec.get("id") == 0 and rec.get("ok"):
                    initial = rec
                    break
            if proc.poll() is not None:
                break
            time.sleep(0.05)
        if initial is None:
            out = proc.stdout.read() if proc.stdout else ""
            raise SystemExit(f"no initial fixture response; child exited={proc.poll()} out tail:\n{out[-4000:]}")
        obs0 = initial["observation"]
        assert obs0 and obs0["units"], "initial observation must be NONEMPTY"
        # Ownership gate: only owned+controlled units may be ordered.
        owned = [u for u in obs0["units"]
                 if u.get("position") and u.get("life")
                 and u.get("owned") is True and u.get("control") is True
                 and u.get("owner") == obs0.get("local_player")]
        assert owned, "no owned+controlled unit in initial observation"
        unit = max(owned, key=lambda u: u["life"] or 0.0)
        t0 = obs0["game_time"]
        pos0 = unit["position"]
        print(f"mock-real-fixture: initial id=0 units={len(obs0['units'])} owned={len(owned)} game_time={t0} unit={unit['id']} pos={pos0}", flush=True)

        post = mock_transport([
            typed_payload(CHOICE_MOVE_EAST, 0.9),
            typed_payload(CHOICE_STOP, 0.95),
        ])
        adapter = DecisionAdapter(
            client, goal="move east toward the objective", key="fake-injected-key", post=post
        )
        result = adapter.run(obs0)

        assert [d.choice for d in result.model_decisions] == [CHOICE_MOVE_EAST, CHOICE_STOP]
        assert result.stopped
        assert len(result.canonical_receipts) == 2
        move_receipt, stop_receipt = result.canonical_receipts
        assert move_receipt["id"] == 1 and move_receipt["ok"]
        assert stop_receipt["id"] == 2 and stop_receipt["state"] == "stopped"
        t1 = move_receipt["observation"]["game_time"]
        assert t1 is not None and t0 is not None and t1 > t0, f"game time must advance: {t0}->{t1}"
        pos_after = next(
            (u["position"] for u in move_receipt["observation"]["units"] if u["id"] == unit["id"]), None
        )
        moved = None
        if pos_after:
            moved = (pos_after[0] - pos0[0]) ** 2 + (pos_after[1] - pos0[1]) ** 2
        assert moved is not None and moved > 1.0, f"unit must move: before={pos0} after={pos_after} moved_sq={moved}"
        assert adapter.model == "jev-1.14.0"
        assert len(post.calls) == 2
        assert post.calls[0]["url"].endswith("/v1/systemone")
        assert post.calls[0]["headers"]["Authorization"] == "Bearer fake-injected-key"
        assert "fake-injected-key" not in json.dumps(post.calls[0]["body"])

        rc = proc.wait(timeout=120.0)
        out = proc.stdout.read() if proc.stdout else ""
        assert rc == 0, f"owned cargo-test process exited {rc}\n{out[-4000:]}"
        journal = json.loads((fixture_dir / "fixture-journal.json").read_text())
        assert journal == [1, 2], f"journal must be [1,2], got {journal}"
        transcript = {"initial": initial, "receipts": result.canonical_receipts,
                      "decisions": [{"choice": d.choice, "confidence": d.confidence, "action": d.action} for d in result.model_decisions]}
        digest = hashlib.sha256(json.dumps(transcript, sort_keys=True).encode()).hexdigest()
        summary = {
            "cmd": cmd, "install": args.install, "map": args.map, "addon": args.addon,
            "display": display, "game_time": [t0, t1],
            "unit": unit["id"], "moved_squared": moved, "journal": journal,
            "transcript_sha256": digest, "requests": 2, "responses": 3,
            "model": adapter.model, "child_exit": rc,
        }
        (run_dir / "mock-real-fixture-summary.json").write_text(json.dumps(summary, indent=2) + "\n")
        print(f"mock-real-fixture: OK game_time={t0}->{t1} moved_sq={moved:.1} journal={journal} sha256={digest}", flush=True)
        return 0
    except (AssertionError, Exception) as exc:  # noqa: BLE001 - report, kill owned group
        print(f"mock-real-fixture: FAILED {exc}", file=sys.stderr)
        if proc is not None and proc.poll() is None:
            try:
                os.killpg(os.getpgid(proc.pid), signal.SIGKILL)
            except ProcessLookupError:
                pass
        return 1
    finally:
        if proc is not None and proc.poll() is None:
            try:
                os.killpg(os.getpgid(proc.pid), signal.SIGKILL)
            except ProcessLookupError:
                pass
        if xvfb.poll() is None:
            xvfb.terminate()
            try:
                xvfb.wait(timeout=5.0)
            except subprocess.TimeoutExpired:
                xvfb.kill()


if __name__ == "__main__":
    raise SystemExit(main())
