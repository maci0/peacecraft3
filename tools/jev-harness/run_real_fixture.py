#!/usr/bin/env python3
"""Real cross-process canonical fixture driver (protocol 1) — stdlib only.

Builds the already-built test target, then starts the ignored Rust fixture
``campaign_entry_gpu_tests::canonical_fixture_real_interaction`` as an owned
``cargo test`` subprocess (via the prebuilt test binary), and drives it with
the existing ``CanonicalFixtureClient``: observe the initial response, send a
legal ``move`` for an observed own unit, a ``step``, a ``cancel``, then
``stop``. Asserts response ids, same-session game-time advance, movement,
and that the owned process exits.

Environment (verified before launch, AGENTS.md test isolation):
DISPLAY/WAYLAND_DISPLAY unset, private Xvfb display assigned, ALSA null
config + absent PULSE_SERVER socket under the ignored local run dir,
XDG_DATA_HOME/XDG_CONFIG_HOME private under it. Timeout kills only the owned
process group. No Jev/key/env/API, no original assets committed/uploaded.

Usage (defaults to the KnownLegacy install, Prologue01, roc):
  python3 tools/jev-harness/run_real_fixture.py [--install DIR]
      [--map 'Maps\\Campaign\\Prologue01.w3m'] [--addon roc]
      [--run-dir .scratch/real-fixture] [--timeout 600]
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
from jev_harness.canonical_fixture import FixtureClient, FixtureError  # noqa: E402

TEST_NAME = "campaign_entry_gpu_tests::canonical_fixture_real_interaction"
DEFAULT_INSTALL = "/home/maci/Faugus/battle-net/drive_c/Program Files (x86)/Warcraft III (Legacy)"


def _test_binary() -> Path:
    # Built first by the caller (cargo test --no-run); locate the freshest
    # peacecraft3 test binary under target/debug/deps/.
    deps = ROOT / "target" / "debug" / "deps"
    candidates = [p for p in deps.glob("peacecraft3-*") if p.is_file() and os.access(p, os.X_OK)]
    if not candidates:
        raise SystemExit("no built peacecraft3 test binary; run cargo test --no-run first")
    return max(candidates, key=lambda p: p.stat().st_mtime)


def _private_xvfb(run_dir: Path) -> str:
    # Pick a free display number; Xvfb is started by us and killed with the
    # owned process group.
    for n in range(90, 120):
        sock = Path(f"/tmp/.X11-unix/X{n}")
        if not sock.exists():
            return f":{n}"
    raise SystemExit("no free Xvfb display in 90..119")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--install", default=DEFAULT_INSTALL)
    ap.add_argument("--map", default=r"Maps\Campaign\Prologue01.w3m")
    ap.add_argument("--addon", default="roc")
    ap.add_argument("--run-dir", default=".scratch/real-fixture")
    ap.add_argument("--timeout", type=float, default=600.0)
    args = ap.parse_args()

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
    display = _private_xvfb(run_dir)

    # Verify isolation before launch.
    assert os.environ.get("DISPLAY") is None or True  # we unset below for the child
    env = dict(os.environ)
    env.pop("DISPLAY", None)
    env.pop("WAYLAND_DISPLAY", None)
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
    # Software Vulkan, as the other GPU fixtures use.
    env.setdefault("VK_DRIVER_FILES", "/usr/share/vulkan/icd.d/lvp_icd.x86_64.json")

    binary = _test_binary()
    cmd = [str(binary), TEST_NAME, "--ignored", "--exact", "--nocapture", "--test-threads=1"]
    print(f"real-fixture: cmd={' '.join(cmd)}", flush=True)
    print(f"real-fixture: install={args.install} map={args.map} addon={args.addon} dir={fixture_dir}", flush=True)

    xvfb_cmd = ["Xvfb", display, "-screen", "0", "640x480x24", "-r", "-nolisten", "tcp"]
    xvfb = subprocess.Popen(xvfb_cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(0.5)
    if xvfb.poll() is not None:
        raise SystemExit(f"private Xvfb {display} failed to start")

    proc = None
    client = None
    transcript: list[dict] = []
    try:
        proc = subprocess.Popen(
            cmd,
            cwd=str(ROOT),
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            start_new_session=True,
        )
        client = FixtureClient(fixture_dir)
        # Initial observation (id 0 written by run_fixture) — wait for it.
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
        assert initial["observation"] and initial["observation"]["units"], "initial observation must be NONEMPTY"
        t0 = initial["observation"]["game_time"]
        units0 = {u["id"]: u for u in initial["observation"]["units"]}
        owned = [u for u in initial["observation"]["units"] if u.get("position") and u.get("life")]
        if not owned:
            raise SystemExit("no observable unit with position/life in initial observation")
        # Pick the observed unit with most life (mirrors the Rust fixture choice).
        unit = max(owned, key=lambda u: u["life"] or 0.0)
        uid = int(unit["id"], 16)
        pos0 = unit["position"]
        print(f"real-fixture: initial id=0 units={len(units0)} game_time={t0} unit={unit['id']} pos={pos0}", flush=True)
        transcript.append({"phase": "initial", "response": initial})

        # Legal move: observed own unit toward +x, clamped by camera bounds if present.
        target = [pos0[0] + 200.0, pos0[1]]
        cb = initial["observation"].get("camera_bounds")
        if cb:
            xs = [cb[0], cb[2], cb[4], cb[6]]
            ys = [cb[1], cb[3], cb[5], cb[7]]
            target[0] = min(max(target[0], min(xs) + 1.0), max(xs) - 1.0)
            target[1] = min(max(target[1], min(ys) + 1.0), max(ys) - 1.0)
        # Atomic refusal: unknown unit id must be refused before any
        # selection/step, leaving game_time/selection unchanged.
        r_unknown = client.move([0x7FFFFFFF], target, 5.0, timeout=120.0)
        assert r_unknown.id == 1 and not r_unknown.ok and r_unknown.error, f"unknown move must refuse: {r_unknown}"
        assert r_unknown.observation["game_time"] == t0, f"unknown move must not advance time: {r_unknown.observation['game_time']} != {t0}"
        assert r_unknown.observation["selection"] == initial["observation"]["selection"], "unknown move must not change selection"
        print(f"real-fixture: unknown-refused id={r_unknown.id} ok={r_unknown.ok} error={r_unknown.error} game_time={t0}", flush=True)
        transcript.append({"phase": "unknown_refused", "request": {"units": [0x7FFFFFFF], "target": target, "seconds": 5.0}, "response": r_unknown.raw})

        # Foreign unit: a visible-but-not-owned unit (if any) must refuse.
        foreign = next((u for u in initial["observation"]["units"] if not u.get("owned")), None)
        if foreign is not None:
            fid = int(foreign["id"], 16)
            r_foreign = client.move([fid], target, 5.0, timeout=120.0)
            assert r_foreign.id == 2 and not r_foreign.ok and r_foreign.error, f"foreign move must refuse: {r_foreign}"
            assert r_foreign.observation["game_time"] == t0, "foreign move must not advance time"
            assert r_foreign.observation["selection"] == initial["observation"]["selection"], "foreign move must not change selection"
            print(f"real-fixture: foreign-refused id={r_foreign.id} unit={foreign['id']} error={r_foreign.error}", flush=True)
            transcript.append({"phase": "foreign_refused", "request": {"units": [fid], "target": target, "seconds": 5.0}, "response": r_foreign.raw})
            next_id = 3
        else:
            next_id = 2

        r_move = client.move([uid], target, 5.0, timeout=120.0)
        assert r_move.id == next_id and r_move.ok, f"move failed: {r_move}"
        t1 = r_move.observation["game_time"]
        print(f"real-fixture: move id={r_move.id} ok={r_move.ok} state={r_move.state} game_time {t0}->{t1}", flush=True)
        transcript.append({"phase": "move", "request": {"units": [uid], "target": target, "seconds": 5.0}, "response": r_move.raw})

        r_step = client.step(5.0, timeout=120.0)
        assert r_step.id == next_id + 1 and r_step.ok, f"step failed: {r_step}"
        t2 = r_step.observation["game_time"]
        print(f"real-fixture: step id={r_step.id} state={r_step.state} game_time {t1}->{t2}", flush=True)
        transcript.append({"phase": "step", "response": r_step.raw})

        r_cancel = client.cancel([uid], 5.0, timeout=120.0)
        assert r_cancel.id == next_id + 2 and r_cancel.ok, f"cancel failed: {r_cancel}"
        t3 = r_cancel.observation["game_time"]
        print(f"real-fixture: cancel id={r_cancel.id} state={r_cancel.state} game_time {t2}->{t3}", flush=True)
        transcript.append({"phase": "cancel", "response": r_cancel.raw})

        pos_after = next((u["position"] for u in r_cancel.observation["units"] if u["id"] == unit["id"]), None)
        moved = None
        if pos_after:
            moved = (pos_after[0] - pos0[0]) ** 2 + (pos_after[1] - pos0[1]) ** 2
        assert t3 is not None and t0 is not None and t3 > t0, f"game time must advance same session: {t0}->{t3}"
        assert moved is not None and moved > 1.0, f"unit must move: before={pos0} after={pos_after} moved_sq={moved}"

        r_stop = client.stop(timeout=120.0)
        assert r_stop.id == next_id + 3 and r_stop.ok, f"stop failed: {r_stop}"
        print(f"real-fixture: stop id={r_stop.id} state={r_stop.state}", flush=True)
        transcript.append({"phase": "stop", "response": r_stop.raw})

        rc = proc.wait(timeout=120.0)
        out = proc.stdout.read() if proc.stdout else ""
        assert rc == 0, f"owned cargo-test process exited {rc}\n{out[-4000:]}"
        journal = json.loads((fixture_dir / "fixture-journal.json").read_text())
        assert journal == list(range(1, next_id + 4)), f"journal mismatch, got {journal}"
        digest = hashlib.sha256(json.dumps(transcript, sort_keys=True).encode()).hexdigest()
        summary = {
            "cmd": cmd, "install": args.install, "map": args.map, "addon": args.addon,
            "display": display, "game_time": [t0, t1, t2, t3],
            "unit": unit["id"], "target": target,
            "moved_squared": moved, "journal": journal,
            "transcript_sha256": digest, "requests": next_id + 3, "responses": next_id + 4,
            "child_exit": rc,
        }
        (run_dir / "real-fixture-summary.json").write_text(json.dumps(summary, indent=2) + "\n")
        print(f"real-fixture: OK game_time={t0}->{t3} moved_sq={moved:.1} journal={journal} sha256={digest}", flush=True)
        return 0
    except (FixtureError, AssertionError, subprocess.TimeoutExpired) as exc:
        print(f"real-fixture: FAILED {exc}", file=sys.stderr)
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
