"""Mock-transport proof: typed model decisions drive REAL canonical receipts.

Fake injected key + mock transport only: no live API, no secret-file
reads (env_key refusal is asserted). A fake file worker stands in for
the Rust ``run_fixture`` loop exactly as in ``test_canonical_fixture``;
the adapter under test is real code and the canonical receipts come
from the real ``FixtureClient`` serialization/validation path.
"""

import json
import sys
import tempfile
import threading
import time
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from jev_harness.canonical_fixture import FixtureClient
from jev_harness.decision_adapter import (
    CHOICE_MOVE_EAST,
    CHOICE_STOP,
    DecisionAdapter,
    DecisionError,
    build_choices,
    choice_to_action,
    env_key,
    mock_transport,
    typed_payload,
)

REQUEST = "fixture-request.json"
RESPONSE = "fixture-response.json"

OBSERVATION = {
    "scope": "structured-state sensors (not a vision benchmark)",
    "game_time": 10.0,
    "local_player": 1,
    "selection": [],
    "units": [
        {
            "id": "a",
            "rawcode": "hfoo",
            "position": [100.0, 200.0],
            "position_source": "live",
            "life": 400.0,
            "max_life": 400.0,
            "owner": 1,
            "owned": True,
            "control": True,
            "current_order": 0,
            "screen": None,
            "in_viewport": True,
        },
        {
            "id": "b",
            "rawcode": "hfoo",
            "position": [120.0, 220.0],
            "position_source": "live",
            "life": 300.0,
            "max_life": 300.0,
            "owner": 2,
            "owned": False,
            "control": False,
            "current_order": 0,
            "screen": None,
            "in_viewport": True,
        },
    ],
    "camera_bounds": [0.0, 0.0, 500.0, 0.0, 500.0, 500.0, 0.0, 500.0],
}


class CanonicalWorker(threading.Thread):
    """Fake worker with canonical receipts: echoes action + observation."""

    def __init__(self, run_dir):
        super().__init__(daemon=True)
        self.run_dir = Path(run_dir)
        self.seen = []
        self._stop = threading.Event()
        self._game_time = OBSERVATION["game_time"]

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
                action = request["action"]
                if action["type"] in ("move", "step", "cancel"):
                    self._game_time += float(action.get("seconds", 0.0))
                observation = json.loads(json.dumps(OBSERVATION))
                observation["game_time"] = self._game_time
                payload = {
                    "protocol": 1,
                    "id": request["id"],
                    "ok": True,
                    "error": None,
                    "observation": observation,
                    "state": "stopped" if action["type"] == "stop" else "stepped",
                }
                tmp = self.run_dir / (RESPONSE + ".tmp")
                tmp.write_text(json.dumps(payload), encoding="utf-8")
                tmp.replace(self.run_dir / RESPONSE)
                path.unlink(missing_ok=True)
            else:
                time.sleep(0.005)


class DecisionAdapterTests(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.run_dir = Path(self._tmp.name)
        self.worker = CanonicalWorker(self.run_dir)
        self.worker.start()
        self.addCleanup(self.worker.stop)
        self.addCleanup(self._tmp.cleanup)
        self.client = FixtureClient(self.run_dir, poll_interval=0.002)

    def test_mock_model_move_then_stop_produces_canonical_receipts(self):
        post = mock_transport(
            [typed_payload(CHOICE_MOVE_EAST, 0.9), typed_payload(CHOICE_STOP, 0.95)]
        )
        adapter = DecisionAdapter(
            self.client, goal="move east toward the objective", key="fake-injected-key", post=post
        )
        result = adapter.run(OBSERVATION)

        # NONEMPTY actual mock-model -> real canonical order proof.
        self.assertEqual([d["choice"] for d in []] + [r.choice for r in result.model_decisions], [CHOICE_MOVE_EAST, CHOICE_STOP])
        self.assertTrue(result.stopped)
        self.assertEqual(len(result.canonical_receipts), 2)
        move_receipt, stop_receipt = result.canonical_receipts
        # Model decision vs actual canonical receipt recorded, no raw keys.
        self.assertEqual(result.model_decisions[0].action["type"], "move")
        self.assertEqual(move_receipt["id"], 1)
        self.assertTrue(move_receipt["ok"])
        self.assertEqual(move_receipt["observation"]["game_time"], 15.0)
        self.assertEqual(stop_receipt["id"], 2)
        self.assertEqual(stop_receipt["state"], "stopped")
        # Worker saw only typed canonical actions, with derived targets.
        self.assertEqual(self.worker.seen[0]["action"]["type"], "move")
        self.assertEqual(self.worker.seen[0]["action"]["units"], [10])
        self.assertEqual(self.worker.seen[0]["action"]["target"], [300.0, 200.0])
        self.assertEqual(self.worker.seen[1]["action"], {"type": "stop"})
        # Transport used the Jev endpoint + Bearer auth, no key in body.
        self.assertEqual(len(post.calls), 2)
        self.assertTrue(post.calls[0]["url"].endswith("/v1/systemone"))
        self.assertEqual(post.calls[0]["headers"]["Authorization"], "Bearer fake-injected-key")
        self.assertNotIn("fake-injected-key", json.dumps(post.calls[0]["body"]))
        self.assertLessEqual(post.calls[0]["timeout"], 30.0)

    def test_choices_derive_from_current_observed_position(self):
        choices = build_choices(OBSERVATION)
        self.assertEqual(choices.unit_uid, 10)  # most life observed unit
        self.assertEqual(choices.target_east, [300.0, 200.0])
        self.assertEqual(choices.target_north, [100.0, 1.0])  # clamped by camera_bounds
        action = choice_to_action(CHOICE_MOVE_EAST, choices)
        self.assertEqual(action["type"], "move")
        self.assertEqual(action["units"], [10])

    def test_visible_enemy_never_ordered(self):
        # Visible but not owned/controlled: excluded even with most life.
        obs = json.loads(json.dumps(OBSERVATION))
        obs["units"].append({
            "id": "ff", "rawcode": "hfoo", "position": [50.0, 50.0],
            "position_source": "live", "life": 9999.0, "max_life": 9999.0,
            "owner": 2, "owned": False, "control": False,
            "current_order": 0, "screen": None, "in_viewport": True,
        })
        choices = build_choices(obs)
        self.assertEqual(choices.unit_uid, 0xA)
        # Legacy schema without owner fields must fail closed, not assume own.
        legacy = {"units": [{"id": "a", "position": [1.0, 1.0], "life": 10.0}]}
        with self.assertRaises(DecisionError):
            build_choices(legacy)

    def test_unknown_choice_fails_closed(self):
        with self.assertRaises(DecisionError):
            choice_to_action("teleport", build_choices(OBSERVATION))
        post = mock_transport([typed_payload("teleport")])
        adapter = DecisionAdapter(
            self.client, goal="move east", key="fake-injected-key", post=post
        )
        with self.assertRaises(DecisionError):
            adapter.run(OBSERVATION)
        self.assertEqual(self.worker.seen, [])  # no canonical action issued

    def test_env_key_refuses_secret_files(self):
        with self.assertRaises(DecisionError) as ctx:
            env_key(environ={})
        self.assertIn("secret-file reads are refused", str(ctx.exception))
        self.assertEqual(env_key(environ={"TYPESAFE_API_KEY": "env-key-123"}), "env-key-123")

    def test_budget_refusal_triggers_code_owned_stop(self):
        post = mock_transport([typed_payload(CHOICE_MOVE_EAST)])
        adapter = DecisionAdapter(
            self.client, goal="move east", key="fake-injected-key", post=post,
            clock=lambda: 10_000.0,
        )
        # Pretend the total budget is already consumed before any call.
        adapter._started = 0.0
        result = adapter.run(OBSERVATION)
        self.assertTrue(result.stopped)
        self.assertEqual(result.model_decisions, [])
        self.assertEqual(len(result.canonical_receipts), 1)
        self.assertEqual(result.canonical_receipts[0]["state"], "stopped")
        self.assertEqual(self.worker.seen[0]["action"], {"type": "stop"})


if __name__ == "__main__":
    unittest.main()
