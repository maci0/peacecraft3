#!/usr/bin/env python3
"""Local handoff for lane 09-ai (T0885): read a step-driver journal.

Consumes `observation-journal.json` written by the ignored Rust test
`step_driver_two_steps_with_one_real_order` in
`crates/peacecraft3/src/campaign_entry_gpu_tests.rs`. Pure stdlib, no
network, no assets: validates the explicit structured state (StepState,
order, moved_squared) and prints a one-line verdict. Exit 0 = journal
proves two bounded steps with one real legal order; exit 1 = refused.

Usage:
  python3 tools/step_driver_handoff.py <PCX_CAMPAIGN_CAPTURE_DIR>
"""
import json, sys
from pathlib import Path

def fail(msg):
    print(f"step-driver handoff: REFUSED {msg}", file=sys.stderr)
    return 1

def main():
    if len(sys.argv) != 2:
        return fail("usage: step_driver_handoff.py <capture-dir>")
    journal = Path(sys.argv[1]) / "observation-journal.json"
    if not journal.is_file():
        return fail(f"missing {journal}")
    data = json.loads(journal.read_text())
    if "structured-state sensors" not in data.get("scope", ""):
        return fail("journal scope is not structured-state sensors")
    for key in ("step1_state", "step2_state"):
        if not str(data.get(key, "")).startswith("Advanced { ticks: 20 }"):
            return fail(f"{key}={data.get(key)!r} is not Advanced 20")
    order = data.get("order", {})
    if order.get("kind") != "move" or not order.get("unit"):
        return fail(f"order is not a legal move: {order!r}")
    moved = data.get("moved_squared")
    if not isinstance(moved, (int, float)) or moved <= 1.0:
        return fail(f"moved_squared={moved!r} does not prove movement")
    before, after = data.get("before", {}), data.get("after", {})
    print(f"step-driver handoff: OK step1={data['step1_state']} step2={data['step2_state']} "
          f"units {len(before.get('units', []))}->{len(after.get('units', []))} moved_sq={moved:.1}")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
