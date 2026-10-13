#!/usr/bin/env python3
"""Local profiling commands. Measurements are reports, never performance gates."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / '.scratch/profiles'


def locate(name):
    found = shutil.which(name)
    local = ROOT / '.tools/profiling' / name
    rocm = Path('/opt/rocm/bin') / name
    return found or (str(local) if local.is_file() else None) or (str(rocm) if rocm.is_file() else None)


def probe(command):
    try:
        result = subprocess.run(command, capture_output=True, text=True, timeout=15)
        return {'available': result.returncode == 0, 'exit_code': result.returncode,
                'output': (result.stdout + result.stderr).strip()[:4000]}
    except (OSError, subprocess.TimeoutExpired) as error:
        return {'available': False, 'error': str(error)}


def doctor():
    tools = {name: locate(name) for name in
             ['perf', 'valgrind', 'rocprofv3', 'rocpd', 'vulkaninfo', 'RadeonGPUProfiler', 'RGP']}
    report = {'tools': tools, 'timing_is_report_only': True}
    if tools['perf']:
        report['perf_self_instructions'] = probe([tools['perf'], 'stat', '-e', 'instructions:u', '--', 'true'])
    if tools['rocprofv3']:
        report['rocprofv3_version'] = probe([tools['rocprofv3'], '--version'])
    if tools['vulkaninfo']:
        report['vulkan'] = probe([tools['vulkaninfo'], '--summary'])
    report['gpu_route'] = 'wgpu Vulkan graphics: RADV RGP or Bevy render diagnostics; rocprofv3 is for HIP/HSA workloads'
    print(json.dumps(report, indent=2))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=['doctor', 'cpu', 'gpu', 'rocprof', 'trigger'])
    parser.add_argument('command', nargs=argparse.REMAINDER)
    args = parser.parse_args()
    if args.mode == 'doctor':
        doctor()
        return 0
    OUTPUT.mkdir(parents=True, exist_ok=True)
    if args.mode == 'trigger':
        (OUTPUT / 'rgp-trigger').touch()
        print('Requested RADV capture; the driver writes .rgp files to /tmp.')
        return 0
    command = args.command[1:] if args.command[:1] == ['--'] else args.command
    if not command:
        parser.error('provide an application after --')
    environment = os.environ.copy()
    if args.mode == 'cpu':
        tool = locate('perf')
        if not tool:
            parser.error('perf is not installed; use the profile-cpu Bevy trace feature instead')
        command = [tool, 'record', '-e', 'cycles:u', '-F', '199', '--call-graph', 'dwarf',
                   '-o', str(OUTPUT / 'cpu.data'), '--', *command]
    elif args.mode == 'gpu':
        # Mesa RADV's documented graphics capture path; no global driver changes.
        trigger = OUTPUT / 'rgp-trigger'
        trigger.unlink(missing_ok=True)
        environment.update({'WGPU_BACKEND': 'vulkan', 'MESA_VK_TRACE': 'rgp',
                            'MESA_VK_TRACE_TRIGGER': str(trigger)})
        print('Launch uses RADV graphics tracing. Run tools/profile.py trigger at the scene to capture.', flush=True)
    elif args.mode == 'rocprof':
        tool = locate('rocprofv3')
        if not tool:
            parser.error('rocprofv3 is not installed')
        command = [tool, '--sys-trace', '--output-format', 'rocpd',
                   '--output-directory', str(OUTPUT / 'rocm'), '--', *command]
        print('HIP/HSA compute trace, not a wgpu Vulkan graphics trace.', flush=True)
    return subprocess.run(command, env=environment, cwd=ROOT).returncode


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except OSError as error:
        raise SystemExit(str(error))
