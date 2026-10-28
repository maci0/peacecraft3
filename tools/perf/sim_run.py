#!/usr/bin/env python3
"""Simulation CPU workloads: user-mode instruction counts (perf stat), allocation counters and
output digests. Elapsed time and instruction counts are report-only. Real-asset modes need
PCX_SIM_BENCH_INSTALL and PCX_SIM_BENCH_MAP (the local install and the extracted Human03 map).

--check compares the deterministic counters (allocation calls, reallocations, requested bytes,
fixed-work counters) and the output digest of every run against tools/perf/budgets/sim.json and
exits 1 when a counter exceeds its budget, a digest differs, or a budget was raised above the
committed one. Budgets only go down."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
MANIFEST = ROOT / 'tools/perf/sim_bench/Cargo.toml'
TARGET = ROOT / '.scratch/perf-sim/target'
RESULTS = ROOT / '.scratch/perf-sim/results'
BUDGETS = ROOT / 'tools/perf/budgets/sim.json'
SYNTHETIC = ['script-vm baseline', 'script-vm run']
ASSETS = ['script-front-end load', 'script-front-end parse', 'script-front-end resolve', 'human03 0', 'human03 4096']
# Reported fields that are counters of fixed work. Each must not exceed its budget.
COUNTERS = ('allocations', 'reallocations', 'allocated_bytes', 'vision_cell_visits')
INSTALL_EXE = 'Warcraft III.exe'


def instructions(binary: Path, mode: str) -> dict:
    stat = RESULTS / 'stat.txt'
    run = subprocess.run(['perf', 'stat', '-e', 'instructions:u', '-x,', '-o', str(stat), str(binary), *mode.split()],
                         cwd=ROOT, check=True, text=True, capture_output=True)
    report = json.loads(run.stdout)
    report['instructions_user'] = int(stat.read_text().split(',')[0].strip().splitlines()[-1])
    return report


def sha256_of(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open('rb') as handle:
        for block in iter(lambda: handle.read(1 << 20), b''):
            digest.update(block)
    return digest.hexdigest()


def input_identities() -> dict:
    """Hashes of the local inputs the real-asset budgets were measured on."""
    exe = Path(os.environ['PCX_SIM_BENCH_INSTALL']) / INSTALL_EXE
    return {'install_exe_sha256': sha256_of(exe), 'map_sha256': sha256_of(Path(os.environ['PCX_SIM_BENCH_MAP']))}


def committed_budgets() -> dict | None:
    """The budget file at HEAD, or None when it is not tracked yet."""
    # No in-process git API exists; the committed copy is the only reference a raise can be judged by.
    shown = subprocess.run(['git', 'show', f'HEAD:{BUDGETS.relative_to(ROOT)}'], cwd=ROOT, text=True, capture_output=True)
    return json.loads(shown.stdout) if shown.returncode == 0 else None


def check(reports: list[dict], modes: list[str], with_assets: bool) -> list[str]:
    """Failures of `reports` (one per mode, in order) against the budget file."""
    budgets = json.loads(BUDGETS.read_text())
    failures: list[str] = []
    if with_assets:
        measured = input_identities()
        for name, value in measured.items():
            if budgets['inputs'].get(name) != value:
                failures.append(f'input {name} is {value}, the budgets were measured on {budgets["inputs"].get(name)}')
    for mode, report in zip(modes, reports):
        budget = budgets['workloads'].get(mode)
        if budget is None:
            failures.append(f'{mode}: no budget')
            continue
        if budget['digest'] != report['digest']:
            failures.append(f'{mode}: digest {report["digest"]} differs from {budget["digest"]} (behaviour changed)')
        for counter in COUNTERS:
            if counter not in report:
                continue
            if counter not in budget:
                failures.append(f'{mode}: {counter} has no budget')
            elif report[counter] > budget[counter]:
                failures.append(f'{mode}: {counter} {report[counter]} exceeds the budget {budget[counter]}')
            else:
                print(f'{mode}: {counter} {report[counter]} within the budget {budget[counter]}')
    previous = committed_budgets()
    if previous is not None:
        for mode, old in previous['workloads'].items():
            for counter in COUNTERS:
                new = budgets['workloads'].get(mode, {}).get(counter)
                if counter in old and (new is None or new > old[counter]):
                    failures.append(f'{mode}: {counter} budget {new} is above the committed {old[counter]}; budgets only go down')
    return failures


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--no-assets', action='store_true', help='run only the synthetic script VM workload')
    parser.add_argument('--check', action='store_true', help='fail when a counter exceeds tools/perf/budgets/sim.json')
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
    if args.check:
        failures = check(reports, modes, not args.no_assets)
        for failure in failures:
            print(f'FAIL {failure}', file=sys.stderr)
        raise SystemExit(1 if failures else 0)


if __name__ == '__main__':
    main()
