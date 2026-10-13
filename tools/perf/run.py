#!/usr/bin/env python3
"""Allocation regression gate; optional timing and Callgrind counts are reports."""
import argparse
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[2]
TARGET = ROOT / '.scratch/perf/target'
RESULTS = ROOT / '.scratch/perf/results'


def disable_core_dump():
    import resource
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))


def command(arguments, profiling=False):
    options = {'preexec_fn': disable_core_dump} if profiling and os.name == 'posix' else {}
    return subprocess.run(arguments, cwd=ROOT, check=True, text=True, capture_output=True, **options).stdout.strip()


def instruction_count(path):
    events = summary = None
    for line in path.read_text().splitlines():
        if line.startswith('events:'):
            events = line.split()[1:]
        if line.startswith('summary:'):
            summary = [int(value) for value in line.split()[1:]]
    if events is None or summary is None or 'Ir' not in events:
        raise RuntimeError('Callgrind did not report instruction counts')
    count = summary[events.index('Ir')]
    if count == 0:
        raise RuntimeError('No workload instructions collected; check the pcx_perf_measure symbol')
    return count


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument('--timing', action='store_true', help='report elapsed time; never use it as a speed gate')
    mode.add_argument('--instructions', action='store_true', help='report scoped Callgrind instruction counts')
    args = parser.parse_args()
    RESULTS.mkdir(parents=True, exist_ok=True)
    command(['cargo', 'build', '--locked', '--manifest-path', 'tools/perf/Cargo.toml', '--release', '--target-dir', str(TARGET)])
    binary = TARGET / 'release' / ('peacecraft-perf.exe' if os.name == 'nt' else 'peacecraft-perf')
    invocation = [str(binary)] + (['--timing'] if args.timing else [])
    valgrind_version = None
    if args.instructions:
        valgrind = shutil.which('valgrind')
        if not valgrind:
            raise RuntimeError('Valgrind is not installed; the allocation gate does not require it')
        valgrind_version = command([valgrind, '--version'])
        profile = RESULTS / 'simulation.callgrind'
        invocation = [valgrind, '--tool=callgrind', '--collect-atstart=no', '--toggle-collect=pcx_perf_measure', f'--callgrind-out-file={profile}', *invocation]
    report = json.loads(command(invocation, profiling=args.instructions))
    report.update({'target_arch': platform.machine(), 'os': platform.system(), 'compiler': command(['rustc', '--version']), 'profile': 'release-opt3-cgu1-lto-off', 'rustflags': os.environ.get('RUSTFLAGS', ''), 'valgrind': valgrind_version})
    if args.instructions:
        report['instructions_report_only'] = instruction_count(profile)
    path = RESULTS / ('instructions.json' if args.instructions else 'timing.json' if args.timing else 'gate.json')
    path.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, sort_keys=True))


if __name__ == '__main__':
    try:
        main()
    except subprocess.CalledProcessError as error:
        print(error.stdout or '', end='')
        print(error.stderr or '', end='')
        raise SystemExit(error.returncode)
    except (OSError, ValueError, RuntimeError) as error:
        raise SystemExit(str(error))
