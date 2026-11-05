"""Runnable library example: fake provider drives synthetic backend.

    python3 -m jev_harness.example_fake
"""
from __future__ import annotations
import json
import sys
from pathlib import Path

# Allow running as `python3 tools/jev-harness/jev_harness/example_fake.py`
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from jev_harness import SyntheticBackend, FakeProvider, run_loop, LoopConfig

def main() -> int:
    print("backend: synthetic-smoke-fixture (NOT PeaceCraft gameplay)")
    backend = SyntheticBackend(width=640, height=480)
    provider = FakeProvider()
    result = run_loop(backend, provider, LoopConfig(max_model_calls=3, max_actions=8))
    print(f"stop_reason={result.stop_reason.value} model_calls={result.model_calls} "
          f"actions={result.actions_applied} final_state={result.final_state}")
    # Demonstrated gameplay evidence: unit selected + order issued
    assert result.final_state.get("selected") is True, "unit not selected"
    assert result.final_state.get("order_count", 0) >= 1, "no order issued"
    print("OK: fake screenshot -> action -> changed state (selected + order)")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
