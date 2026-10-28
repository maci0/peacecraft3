#!/usr/bin/env python3
"""Lowers tools/perf/budgets/render.json from observed render-cpu runs; never raises a limit.

Each run of the ignored fixture `mounted_campaign_render_cpu_workload` writes
`render-budget-observed.json` next to its summary. This tool merges several of them:

    python3 tools/perf/render_budget.py OBSERVED.json [OBSERVED.json ...] [--write]

The new limit of a counter is its largest observed value, raised by a headroom for the
counters that repeat only approximately: ALLOCATOR_HEADROOM_PERCENT for allocator calls and
bytes, MENU_HEADROOM_PERCENT for every counter of the menu phase. A counter whose new limit would exceed
the limit already in the file is an error: raise nothing, find the regression. Without
`--write` the result is printed and the file is left alone. The enforcing check is the fixture
itself (`PCX_RENDER_BUDGET=tools/perf/budgets/render.json`).
"""
import argparse
import json
import math
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BUDGET = ROOT / 'tools/perf/budgets/render.json'
VERSION = 1
# Allocator totals include background threads (spread under 0.1% between identical runs). The
# menu animation phase at capture depends on asset-load timing (spread up to about 2%).
ALLOCATOR_HEADROOM_PERCENT = 1
MENU_HEADROOM_PERCENT = 5
ALLOCATOR_SUFFIXES = ('.allocation_calls', '.requested_bytes')
MENU_PREFIX = 'menu.'


def headroom_percent(name):
    if name.startswith(MENU_PREFIX):
        return MENU_HEADROOM_PERCENT
    return ALLOCATOR_HEADROOM_PERCENT if name.endswith(ALLOCATOR_SUFFIXES) else 0


def load(path):
    data = json.loads(Path(path).read_text())
    if data.get('version') != VERSION:
        raise SystemExit(f'{path}: version {data.get("version")} is not {VERSION}')
    return data


def merged(runs):
    workloads = {run['workload'] for run in runs}
    if len(workloads) != 1:
        raise SystemExit(f'runs disagree on the workload: {sorted(workloads)}')
    names = [set(run['limits']) for run in runs]
    if any(found != names[0] for found in names):
        raise SystemExit('runs disagree on the counter set')
    limits = {}
    for name in sorted(names[0]):
        highest = max(run['limits'][name] for run in runs)
        limits[name] = math.ceil(highest * (100 + headroom_percent(name)) / 100)
    return workloads.pop(), limits


def main():
    parser = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    parser.add_argument('observed', nargs='+', help='render-budget-observed.json files')
    parser.add_argument('--budget', default=str(BUDGET))
    parser.add_argument('--write', action='store_true', help='replace the budget file')
    args = parser.parse_args()
    workload, limits = merged([load(path) for path in args.observed])
    path = Path(args.budget)
    if path.exists():
        current = load(path)
        if current['workload'] != workload:
            raise SystemExit(f'budget workload {current["workload"]} is not {workload}')
        raised = {n: (current['limits'][n], v) for n, v in limits.items()
                  if n in current['limits'] and v > current['limits'][n]}
        for name, (old, new) in raised.items():
            print(f'{name}: {new} would raise the limit {old}', file=sys.stderr)
        if raised:
            return 1
    text = json.dumps({'version': VERSION, 'workload': workload, 'limits': limits}, indent=2) + '\n'
    if args.write:
        path.write_text(text)
    else:
        sys.stdout.write(text)
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
