#!/usr/bin/env python3
"""Simulation CPU workloads: user-mode instruction counts (perf stat), allocation counters and
output digests. Elapsed time is report-only. Real-asset modes need PCX_SIM_BENCH_INSTALL and
PCX_SIM_BENCH_MAP (the local install and the extracted Human03 map)."""
import argparse
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]
MANIFEST = ROOT / 'tools/perf/sim_bench/Cargo.toml'
TARGET = ROOT / '.scratch/perf-sim/target'
RESULTS = ROOT / '.scratch/perf-sim/results'
SYNTHETIC = ['script-vm baseline', 'script-vm run']
ASSETS = ['script-front-end load', 'script-front-end parse', 'script-front-end resolve', 'human03 0', 'human03 4096']


def instructions(binary, mode):
    stat = RESULTS / 'stat.txt'
    run = subprocess.run(['perf', 'stat', '-e', 'instructions:u', '-x,', '-o', str(stat), str(binary), *mode.split()],
                         cwd=ROOT, check=True, text=True, capture_output=True)
    report = json.loads(run.stdout)
    report['instructions_user'] = int(stat.read_text().split(',')[0].strip().splitlines()[-1])
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--no-assets', action='store_true', help='run only the synthetic script VM workload')
    args = parser.parse_args()
    RESULTS.mkdir(parents=True, exist_ok=True)
    subprocess.run(['cargo', 'build', '--locked', '--release', '--manifest-path', str(MANIFEST), '--target-dir', str(TARGET)],
                   cwd=ROOT, check=True)
    binary = TARGET / 'release/peacecraft-sim-bench'
    modes = SYNTHETIC + ([] if args.no_assets else ASSETS)
    if not args.no_assets:
        missing = [name for name in ('PCX_SIM_BENCH_INSTALL', 'PCX_SIM_BENCH_MAP') if name not in os.environ]
        if missing:
            raise SystemExit(f'set {", ".join(missing)} or pass --no-assets')
    reports = [instructions(binary, mode) for mode in modes]
    (RESULTS / 'sim.json').write_text(json.dumps(reports, indent=2) + '\n')
    for report in reports:
        print(json.dumps(report, sort_keys=True))


if __name__ == '__main__':
    main()
