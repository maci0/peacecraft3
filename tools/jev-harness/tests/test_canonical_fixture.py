"""Fake-protocol tests for the canonical fixture client.

Clearly a FAKE protocol test: a worker thread stands in for the Rust
fixture. No real game process is launched, no process launcher/model
integration exists in this slice.
"""

import json
import sys
import tempfile
import threading
import time
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from jev_harness.canonical_fixture import (
    MAX_REQUEST_BYTES,
    FixtureClient,
    FixtureError,
    FixtureTimeout,
    RequestTooLarge,
    build_request,
)

REQUEST = "fixture-request.json"
RESPONSE = "fixture-response.json"


def _response(request_id, *, ok=True, state="stepped", observation=None):
    return {
        "protocol": 1,
        "id": request_id,
        "ok": ok,
        "error": None if ok else "fake error",
        "observation": observation if observation is not None else {"units": []},
        "state": state,
    }


class FakeWorker(threading.Thread):
    """Reads fixture-request.json, writes fixture-response.json.

    ``mode`` controls the reply: "matching" echoes the request id,
    "stale" replies with id+100, "silent" never replies, "oversized"
    replies with a >1 MiB payload, "malformed" writes non-JSON.
    """

    def __init__(self, run_dir, mode="matching", delay=0.0):
        super().__init__(daemon=True)
        self.run_dir = Path(run_dir)
        self.mode = mode
        self.delay = delay
        self.seen = []
        self._stop = threading.Event()

    def stop(self):
        self._stop.set()

    def run(self):
        while not self._stop.is_set():
            path = self.run_dir / REQUEST
            if path.is_file():
                try:
                    request = json.loads(path.read_text(encoding="utf-8"))
                except (OSError, ValueError):
                    time.sleep(0.005)
                    continue
                self.seen.append(request)
                if self.delay:
                    time.sleep(self.delay)
                if self.mode == "silent":
                    path.unlink(missing_ok=True)
                    continue
                if self.mode == "stale":
                    payload = _response(request["id"] + 100)
                elif self.mode == "oversized":
                    payload = _response(request["id"])
                    payload["observation"] = {"pad": "x" * (1 << 20)}
                elif self.mode == "malformed":
                    (self.run_dir / RESPONSE).write_text("{not json", encoding="utf-8")
                    path.unlink(missing_ok=True)
                    continue
                else:
                    payload = _response(request["id"])
                tmp = self.run_dir / (RESPONSE + ".tmp")
                tmp.write_text(json.dumps(payload), encoding="utf-8")
                tmp.replace(self.run_dir / RESPONSE)
                path.unlink(missing_ok=True)
            else:
                time.sleep(0.005)


class CanonicalFixtureClientTests(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.run_dir = Path(self._tmp.name)
        self.client = FixtureClient(self.run_dir, poll_interval=0.002)

    def tearDown(self):
        self._tmp.cleanup()

    def _start(self, mode="matching", delay=0.0):
        worker = FakeWorker(self.run_dir, mode=mode, delay=delay)
        worker.start()
        self.addCleanup(worker.stop)
        return worker

    def test_two_different_requests_get_matching_ids(self):
        worker = self._start()
        first = self.client.move([1, 2], [10.0, 20.0], 2.5)
        second = self.client.step(1.0)
        self.assertEqual([r["id"] for r in worker.seen], [1, 2])
        self.assertEqual(first.id, 1)
        self.assertEqual(second.id, 2)
        self.assertTrue(first.ok and second.ok)
        self.assertEqual(worker.seen[0]["action"]["type"], "move")
        self.assertEqual(worker.seen[1]["action"]["type"], "step")

    def test_stale_id_is_not_accepted_and_times_out(self):
        self._start(mode="stale")
        with self.assertRaises(FixtureTimeout):
            self.client.step(1.0, timeout=0.3)

    def test_timeout_when_worker_silent(self):
        self._start(mode="silent")
        started = time.monotonic()
        with self.assertRaises(FixtureTimeout):
            self.client.step(1.0, timeout=0.3)
        self.assertGreaterEqual(time.monotonic() - started, 0.29)

    def test_oversized_response_rejected(self):
        self._start(mode="oversized")
        with self.assertRaises(FixtureError) as ctx:
            self.client.step(1.0, timeout=2.0)
        self.assertIn("exceeds cap", str(ctx.exception))

    def test_malformed_response_rejected(self):
        self._start(mode="malformed")
        with self.assertRaises(FixtureError) as ctx:
            self.client.step(1.0, timeout=2.0)
        self.assertIn("malformed", str(ctx.exception))

    def test_stop_uses_same_request_path(self):
        worker = self._start()
        response = self.client.stop(timeout=2.0)
        self.assertEqual(worker.seen[0]["action"], {"type": "stop"})
        self.assertEqual(response.id, worker.seen[0]["id"])

    def test_request_bound_and_unknown_fields_fail_closed(self):
        with self.assertRaises(ValueError):
            build_request(1, {"type": "teleport", "target": [0, 0]})
        with self.assertRaises(ValueError):
            build_request(1, {"type": "stop", "teleport": [0, 0]})
        with self.assertRaises(ValueError):
            build_request(1, {"type": "move", "units": [], "target": [0, 0], "seconds": 1.0})
        big_units = list(range(1, 200))
        with self.assertRaises((ValueError, RequestTooLarge)):
            self.client.move(big_units, [0, 0], 1.0, timeout=0.1)
        # Sanity: a maximally padded (but valid) request's serialized size
        # is what the 8192 cap guards; oversized payloads raise directly.
        self.assertLessEqual(MAX_REQUEST_BYTES, 8192)


if __name__ == "__main__":
    unittest.main()
