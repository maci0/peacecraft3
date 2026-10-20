#!/usr/bin/env python3
"""Local, isolated Wine reference captures. Original files never enter the image."""
import argparse
import hashlib
import html
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import time

HERE = Path(__file__).resolve().parent
LABEL = 'org.peacecraft.original-reference'
IMAGE = 'peacecraft-original-reference:local'
MAX_METADATA = 2 * 1024 * 1024


class Failure(Exception):
    pass


def command(argv, *, timeout=30, check=True):
    result = subprocess.run(argv, capture_output=True, timeout=timeout)
    # The metadata cap applies even when the caller classifies a non-zero exit.
    if len(result.stdout) > MAX_METADATA or len(result.stderr) > MAX_METADATA:
        raise Failure('command metadata exceeds 2 MiB; inspect local logs')
    if not check:
        return result
    if result.returncode:
        diagnostic = result.stderr.decode(errors='replace')[-4000:]
        raise Failure(f'{argv[0]} exited {result.returncode}: {diagnostic}')
    return result.stdout.decode(errors='replace').strip()


def digest(path):
    with path.open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2) + '\n')


def local_run(value):
    path = Path(value).resolve()
    if not (path / 'source.json').is_file():
        raise Failure(f'not a prepared reference directory: {path}')
    if not (path / 'source.json').stat().st_size <= MAX_METADATA:
        raise Failure('source manifest exceeds metadata bound')
    return path


def name(path):
    return 'pcx-reference-' + hashlib.sha256(str(path).encode()).hexdigest()[:16]


def inspect(path):
    result = command(['docker', 'inspect', name(path)], check=False)
    if result.returncode:
        # A daemon failure must not be mistaken for an absent/stopped container.
        command(['docker', 'info', '--format', '{{.ServerVersion}}'])
        if b'no such object' in result.stderr.lower() or b'no such container' in result.stderr.lower():
            return None
        raise Failure(result.stderr.decode(errors='replace')[-4000:])
    state = json.loads(result.stdout)[0]
    if state.get('Config', {}).get('Labels', {}).get(LABEL) != str(path):
        raise Failure('container ownership label does not match run directory')
    return state


def execute(path, argv):
    state = inspect(path)
    if not state or not state['State']['Running']:
        raise Failure('reference container is not running')
    return command(['docker', 'exec', name(path), *argv])


def mark(path, action, details):
    stamp = {'utc': time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime()),
             'monotonic_ns': time.monotonic_ns(), 'action': action, **details}
    with (path / 'actions.ndjson').open('a') as output:
        output.write(json.dumps(stamp) + '\n')
    return stamp


def prepare(args):
    source = Path(args.source).resolve(strict=True)
    target = Path(args.run_dir).resolve()
    if target.exists():
        raise Failure('run directory exists; choose a new path (prepare never overwrites)')
    executable = source / 'Warcraft III.exe'
    if not executable.is_file():
        raise Failure('source must contain Warcraft III.exe')
    if source == target or source in target.parents or target in source.parents:
        raise Failure('source and private run directory must be disjoint')
    paths = sorted(source.rglob('*'))
    if any(p.is_symlink() or not (p.is_file() or p.is_dir()) for p in paths):
        raise Failure('source contains a symlink or special file; refusing ambiguous copy')
    files = [{'path': str(p.relative_to(source)), 'bytes': p.stat().st_size,
              'sha256': digest(p)} for p in paths if p.is_file()]
    record = {'source': str(source), 'executable_sha256': digest(executable), 'files': files}
    if len(json.dumps(record).encode()) > MAX_METADATA:
        raise Failure('source manifest exceeds 2 MiB metadata bound')
    target.mkdir(parents=True, mode=0o700)
    shutil.copytree(source, target / 'game')
    for entry in files:
        if digest(target / 'game' / entry['path']) != entry['sha256']:
            raise Failure('source changed during copy; private run not prepared')
    write_json(target / 'source.json', record)
    return {'run_dir': str(target), 'file_count': len(files), 'executable_sha256': record['executable_sha256']}


def launch(args):
    if os.getuid() == 0:
        raise Failure('run this harness as a non-root user')
    path = local_run(args.run_dir)
    state = inspect(path)
    if state and state['State']['Running']:
        config = state.get('Config', {})
        environment = dict(item.split('=', 1) for item in config.get('Env', []) if '=' in item)
        expected = {'REFERENCE_WIDTH': str(args.width), 'REFERENCE_HEIGHT': str(args.height),
                    'REFERENCE_DPI': str(args.dpi)}
        if (any(environment.get(key) != value for key, value in expected.items())
                or config.get('Cmd') != ['sh', '/harness/session.sh', *args.game_arg]):
            raise Failure('running reference has different display, DPI or game arguments; prepare a new run')
        return {'running': True, 'already_running': True, 'container': name(path)}
    if state:
        raise Failure('prior container exists; use a new prepared run directory to preserve evidence')
    if (path / 'launch.json').exists():
        raise Failure('prior launch receipt exists; use a new prepared run directory to preserve evidence')
    image_id = command(['docker', 'image', 'inspect', IMAGE, '--format', '{{.Id}}'])
    argv = ['docker', 'run', '--detach', '--name', name(path), '--label', f'{LABEL}={path}',
            '--network', 'none', '--cap-drop', 'ALL', '--security-opt', 'no-new-privileges',
            '--pids-limit', '1024', '--shm-size', '256m', '--read-only',
            '--tmpfs', '/tmp:rw,nosuid,nodev,size=512m', '--user', f'{os.getuid()}:{os.getgid()}',
            '--mount', f'type=bind,src={path},dst=/run/reference',
            '--mount', f'type=bind,src={HERE},dst=/harness,readonly',
            '-e', f'REFERENCE_WIDTH={args.width}', '-e', f'REFERENCE_HEIGHT={args.height}',
            '-e', f'REFERENCE_DPI={args.dpi}', IMAGE, 'sh', '/harness/session.sh', *args.game_arg]
    # argv has no original file contents or host credentials.
    write_json(path / 'launch.json', {'image_id': image_id, 'argv': argv,
               'framebuffer': [args.width, args.height], 'x_dpi': args.dpi,
               'runtime': 'Wine/Xvfb/software Mesa; not native Windows ground truth',
               'harness_sha256': {p.name: digest(p) for p in [HERE / 'reference.py', HERE / 'session.sh', HERE / 'Dockerfile']},
               'keyboard_autorepeat': 'disabled by Xvfb -r; replay supplies explicit down/up events',
               'movie_decoder': 'winegstreamer disabled: unsupported DirectShow playback'})
    container = command(argv)
    mark(path, 'launch', {'container': container})
    return {'container': container, 'run_dir': str(path), 'initializing': True}


def status(args):
    path = local_run(args.run_dir)
    state = inspect(path)
    return {'container': name(path), 'state': state['State'] if state else None,
            'ready': (path / 'ready').exists(),
            'launcher_exit_code': (path / 'launcher-exit-code.txt').read_text().strip()
                if (path / 'launcher-exit-code.txt').exists() else None,
            'runtime': 'Wine reference; boot or process existence does not prove rendered fidelity'}


def capture(args):
    path = local_run(args.run_dir)
    if not re.fullmatch(r'[a-zA-Z0-9_-]{1,80}', args.label):
        raise Failure('label must be 1..80 letters, digits, underscores or hyphens')
    (path / 'captures').mkdir(exist_ok=True)
    target = path / 'captures' / f'{args.label}.png'
    if target.exists():
        raise Failure('capture label already exists; choose a new label')
    started_ns = time.monotonic_ns()
    execute(path, ['import', '-window', 'root', f'/run/reference/captures/{args.label}.png'])
    finished_ns = time.monotonic_ns()
    windows = execute(path, ['xwininfo', '-root', '-tree'])
    display = execute(path, ['xdpyinfo'])
    settings = execute(path, ['sh', '-c', "wine reg query 'HKCU\\Software\\Blizzard Entertainment\\Warcraft III\\Video' 2>&1 || true"])
    record = mark(path, 'capture', {'label': args.label, 'sha256': digest(target),
                   'capture_started_ns': started_ns, 'capture_finished_ns': finished_ns})
    write_json(target.with_suffix('.json'), {**record, 'windows': windows, 'display': display,
               'video_registry': settings, 'source': json.loads((path / 'source.json').read_text())['executable_sha256']})
    return {'capture': str(target), 'sha256': record['sha256']}


def action(args):
    path = local_run(args.run_dir)
    if args.command == 'key':
        if not re.fullmatch(r'[a-zA-Z0-9_+]{1,80}', args.key):
            raise Failure('unsupported X keysym syntax')
        argv = ['xdotool', 'key', '--clearmodifiers', '--delay', '150', args.key]
        details = {'key': args.key}
    else:
        argv = ['xdotool', 'mousemove', str(args.x), str(args.y), 'sleep', '0.3', 'mousedown', '1', 'sleep', '0.3', 'mouseup', '1']
        details = {'x': args.x, 'y': args.y}
    details['action_started_ns'] = time.monotonic_ns()
    execute(path, argv)
    return mark(path, args.command, details)


def stop(args):
    path = local_run(args.run_dir)
    state = inspect(path)
    if state and state['State']['Running']:
        command(['docker', 'stop', '--time', '15', name(path)])
        mark(path, 'stop', {})
    return {'running': False, 'container_preserved': bool(state)}


REPLAY_RELATIVE = Path('Warcraft III/Replay/LastReplay.w3g')
REPLAY_DOCUMENTS = ('Documents', 'My Documents')
MAX_REPLAY = 32 * 1024 * 1024


def replay_sources(path):
    users = path / 'prefix/drive_c/users'
    if not users.is_dir():
        return []
    found = []
    for user in users.iterdir():
        if user.is_symlink():
            continue
        for docs in REPLAY_DOCUMENTS:
            candidate = user / docs / REPLAY_RELATIVE
            if candidate.is_symlink() or candidate.exists():
                found.append(candidate)
    return found


def export_replay(args):
    path = local_run(args.run_dir)
    if not re.fullmatch(r'[a-zA-Z0-9_-]{1,80}', args.label):
        raise Failure('label must be 1..80 letters, digits, underscores or hyphens')
    state = inspect(path)
    if state and state['State']['Running']:
        raise Failure('stop the reference run before export; the game writes its replay at match end')
    found = replay_sources(path)
    if len(found) != 1:
        raise Failure(f'expected exactly one {REPLAY_RELATIVE} under the Wine prefix, found {len(found)}')
    source = found[0]
    if source.is_symlink() or not source.is_file():
        raise Failure('replay is not a regular file; refusing to follow it')
    with source.open('rb') as handle:
        data = handle.read(MAX_REPLAY + 1)
    if len(data) > MAX_REPLAY:
        raise Failure(f'replay exceeds the {MAX_REPLAY // (1024 * 1024)} MiB bound')
    target_dir = path / 'replays'
    target_dir.mkdir(exist_ok=True)
    target, receipt = target_dir / f'{args.label}.w3g', target_dir / f'{args.label}.json'
    if target.exists() or receipt.exists():
        raise Failure('replay label already exists; choose a new label')
    with target.open('xb') as output:
        output.write(data)
    executable = json.loads((path / 'source.json').read_text())['executable_sha256']
    record = mark(path, 'export-replay', {'label': args.label, 'sha256': hashlib.sha256(data).hexdigest(),
                  'bytes': len(data), 'source': str(source.relative_to(path)), 'executable_sha256': executable})
    write_json(receipt, record)
    return {'replay': str(target), 'sha256': record['sha256'], 'bytes': record['bytes']}


PIXEL_POLICY = {
    'comparison': 'exact decoded RGB equality',
    'channel_tolerance': 0,
    'rescaling': False,
    'permitted_exception': 'bottom-right version string only',
    'applied_exclusions': [],
    'version_text_footprint': 'unqualified; full-frame comparison remains active',
}


def report(args):
    from PIL import Image
    reference = Path(args.reference).resolve(strict=True)
    recreation = Path(args.recreation).resolve(strict=True)
    native = Path(args.native).resolve(strict=True) if getattr(args, 'native', None) else None
    images = [('Original WC3 / Wine', reference, 'reference.png')]
    if native:
        images.append(('PC3 native app', native, 'native.png'))
    images.append(('PC3 WASM app' if native else 'PeaceCraft III', recreation, 'recreation.png'))
    size = None
    for _, path, _ in images:
        if path.stat().st_size > 64 << 20:
            raise Failure('comparison image exceeds 64 MiB')
        with Image.open(path) as image:
            if not all(1 <= axis <= 4096 for axis in image.size):
                raise Failure('comparison image extent exceeds 4096 pixels')
            if size is not None and size != image.size:
                raise Failure(f'comparison extents differ: {size} and {image.size}; no rescaling performed')
            size = image.size
    target = Path(args.output).resolve()
    if target.exists():
        raise Failure('report destination exists; choose a new path')
    target.mkdir(parents=True, mode=0o700)
    for _, path, name in images:
        shutil.copyfile(path, target / name)
    receipt = {'reference': str(reference), 'reference_sha256': digest(reference),
               'recreation': str(recreation), 'recreation_sha256': digest(recreation),
               'extent': size, 'note': args.note, 'pixel_policy': PIXEL_POLICY,
               'claim': 'Wine reference comparison; no parity claim'}
    if native:
        receipt.update(native=str(native), native_sha256=digest(native),
                       targets=['original-wc3', 'pc3-native', 'pc3-wasm'],
                       provenance='caller-supplied PNGs; application execution identities require the replay receipt')
    write_json(target / 'receipt.json', receipt)
    metadata = html.escape(json.dumps(receipt, indent=2))
    (target / 'index.html').write_text('<!doctype html><meta charset="utf-8"><title>Local original comparison</title>'
        '<style>body{font-family:system-ui;background:#17191c;color:#eee;margin:24px}main{display:flex;gap:16px}'
        'figure{margin:0;flex:1;min-width:0}img{width:100%;height:auto}pre{white-space:pre-wrap}</style>'
        '<h1>Local original comparison</h1><main>' +
        ''.join('<figure><figcaption>' + label + '</figcaption><img src="' + name + '"></figure>'
                for label, _, name in images) + '</main><pre>' + metadata + '</pre>')
    return {'report': str(target / 'index.html'), 'extent': size}


def compare(args):
    from replay import compare as replay
    return replay(args, sys.modules[__name__])


def rng_plan(args):
    from rng_fixture import prepare
    return prepare(args, sys.modules[__name__])


def rng_compare(args):
    from rng_fixture import compare
    return compare(args, sys.modules[__name__])


class Parser(argparse.ArgumentParser):
    def error(self, message):
        raise Failure(message)


def positive(value):
    number = int(value)
    if not 1 <= number <= 16384:
        raise argparse.ArgumentTypeError('must be 1..16384')
    return number


def parser():
    p = Parser(description=__doc__)
    p.add_argument('--format', choices=['json', 'text'], default='json')
    s = p.add_subparsers(dest='command', required=True)
    s.add_parser('schema', help='offline command/effect/output contract')
    a = s.add_parser('prepare'); a.add_argument('--source', required=True); a.add_argument('--run-dir', required=True)
    a = s.add_parser('launch'); a.add_argument('--run-dir', required=True)
    for arg in ['width', 'height', 'dpi']: a.add_argument('--' + arg, type=positive, required=True)
    a.add_argument('--game-arg', action='append', default=[])
    for sub in ['status', 'stop', 'capture', 'export-replay', 'key', 'click']:
        a = s.add_parser(sub); a.add_argument('--run-dir', required=True)
        if sub in ('capture', 'export-replay'): a.add_argument('--label', required=True)
        if sub == 'key': a.add_argument('--key', required=True)
        if sub == 'click':
            a.add_argument('--x', type=int, required=True); a.add_argument('--y', type=int, required=True)
    a = s.add_parser('report')
    for arg in ['reference', 'recreation', 'output', 'note']: a.add_argument('--' + arg, required=True)
    a.add_argument('--native', help='native application PNG; produces WC3 | PC3 native | PC3 WASM')
    a = s.add_parser('compare', help='replay one shared input journal and compare paired captures')
    for arg in ['run-dir', 'scenario', 'url', 'manifest', 'iso', 'playwright-module', 'output']:
        a.add_argument('--' + arg, required=True)
    a.add_argument('--chromium', default='/usr/bin/chromium')
    a.add_argument('--native-config', help='fingerprinted live native-app X11 configuration; replay all three targets')
    a = s.add_parser('rng-plan', help='generate one bounded, explicitly seeded serial JASS RNG fixture')
    for arg in ['scenario', 'output']:
        a.add_argument('--' + arg, required=True)
    a = s.add_parser('rng-compare', help='compare exact ordered RNG results and repeated run receipts')
    for arg in ['fixture', 'output']:
        a.add_argument('--' + arg, required=True)
    for arg in ['original', 'recreation']:
        a.add_argument('--' + arg, required=True, action='append')
    return p


def main():
    try:
        args = parser().parse_args()
        if args.command == 'schema':
            result = json.loads((HERE / 'schema.json').read_text())
        else:
            handler = {'prepare': prepare, 'launch': launch, 'status': status, 'stop': stop,
                       'capture': capture, 'export-replay': export_replay, 'key': action, 'click': action, 'report': report,
                       'compare': compare, 'rng-plan': rng_plan, 'rng-compare': rng_compare}[args.command]
            result = handler(args)
        print(json.dumps(result) if args.format == 'json' else json.dumps(result, indent=2))
        if args.command == 'compare' and not result['images_equal']:
            return 1
        return 1 if args.command == 'rng-compare' and not result['rng_results_match'] else 0
    except KeyboardInterrupt:
        print(json.dumps({'error': {'kind': 'reference_interrupted', 'message': 'Interrupted; inspect retained receipts before starting a new run', 'retryable': False}}), file=sys.stderr)
        return 130
    except (Failure, OSError, ValueError, ImportError, subprocess.TimeoutExpired) as error:
        print(json.dumps({'error': {'kind': 'reference_failed', 'message': str(error), 'retryable': False}}), file=sys.stderr)
        return 2


if __name__ == '__main__':
    sys.exit(main())
