"""One validated input journal, two targets; wall-clock observation is not lockstep."""
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
import hashlib
import html
import json
import os
import re
import select
import subprocess
import time
from types import SimpleNamespace
from urllib.parse import urlparse

KEYS = {**{c: c for c in 'abcdefghijklmnopqrstuvwxyz0123456789'},
        'Enter': 'Return', 'Escape': 'Escape', 'Tab': 'Tab', 'Backspace': 'BackSpace',
        'Space': 'space', 'ArrowLeft': 'Left', 'ArrowRight': 'Right',
        'ArrowUp': 'Up', 'ArrowDown': 'Down', 'Delete': 'Delete',
        'Shift': 'Shift_L', 'Control': 'Control_L', 'Alt': 'Alt_L'}
BUTTONS = {'left': '1', 'middle': '2', 'right': '3'}


def validate(value):
    def require(ok, message):
        if not ok:
            raise ValueError(message)
    require(isinstance(value, dict), 'scenario must be an object')
    require(set(value) == {'version', 'edition', 'extent', 'dpi', 'window_title', 'seed', 'events'},
            'scenario fields must be version/edition/extent/dpi/window_title/seed/events')
    require(value['version'] == 1 and type(value['version']) is int, 'unsupported scenario version')
    require(value['edition'] in ('roc', 'tft'), 'edition must be explicit roc or tft')
    extent = value['extent']
    require(isinstance(extent, list) and len(extent) == 2 and
            all(type(n) is int and 1 <= n <= 4096 for n in extent), 'extent must be 1..4096 physical pixels')
    require(value['dpi'] == 96 and type(value['dpi']) is int, 'browser pairing currently requires 96 DPI / actual DPR1')
    require(isinstance(value['window_title'], str) and 0 < len(value['window_title']) <= 128
            and all(ord(c) >= 32 for c in value['window_title']),
            'a bounded exact original window title is required')
    seed = value['seed']
    require(seed is None or (type(seed) is int and -(2**31) <= seed < 2**31),
            'script seed must be null or signed i32, never silently converted from u64')
    events = value['events']
    require(isinstance(events, list) and 1 <= len(events) <= 256, 'scenario needs 1..256 events')
    held_keys, held_buttons, checkpoints = set(), set(), set()
    pointer = None
    duration = 0
    for event in events:
        require(isinstance(event, dict), 'event must be an object')
        require(type(event.get('after_ms')) is int and 0 <= event['after_ms'] <= 30000,
                'event after_ms must be 0..30000')
        duration += event['after_ms']
        require(duration <= 300000, 'scenario exceeds five-minute scheduled input budget')
        kind = event.get('kind')
        if kind == 'checkpoint':
            require(set(event) == {'kind', 'after_ms', 'name'}, 'invalid checkpoint fields')
            label = event['name']
            require(isinstance(label, str) and re.fullmatch(r'[a-zA-Z0-9_-]{1,64}', label)
                    and label not in checkpoints, 'checkpoint names must be safe and unique')
            require(not held_keys and not held_buttons, 'checkpoint cannot extend a held input while capturing')
            checkpoints.add(label)
            require(len(checkpoints) <= 32, 'scenario exceeds 32 checkpoints')
        elif kind == 'key':
            require(set(event) == {'kind', 'after_ms', 'phase', 'key'}, 'invalid key fields')
            require(isinstance(event['key'], str) and event['key'] in KEYS and event['phase'] in ('down', 'up'), 'unsupported key/phase')
            held, item = held_keys, event['key']
            require((item in held) == (event['phase'] == 'up'), 'key down/up sequence is unbalanced')
            (held.add if event['phase'] == 'down' else held.remove)(item)
        elif kind == 'pointer':
            phase = event.get('phase')
            fields = {'kind', 'after_ms', 'phase', 'x', 'y'} | ({'button'} if phase != 'move' else set())
            require(set(event) == fields and phase in ('move', 'down', 'up'), 'invalid pointer fields/phase')
            require(all(type(event[axis]) is int and 0 <= event[axis] < limit
                        for axis, limit in zip(('x', 'y'), extent)), 'pointer outside framebuffer')
            point = (event['x'], event['y'])
            if phase == 'move':
                pointer = point
            else:
                require(pointer == point, 'pointer down/up needs a preceding explicit move to that point')
                require(isinstance(event['button'], str) and event['button'] in BUTTONS, 'unsupported pointer button')
                item = event['button']
                require((item in held_buttons) == (phase == 'up'), 'pointer down/up sequence is unbalanced')
                (held_buttons.add if phase == 'down' else held_buttons.remove)(item)
        else:
            raise ValueError('unsupported event kind')
    require(checkpoints and not held_keys and not held_buttons, 'scenario needs checkpoints and released inputs')
    if seed is not None:
        raise ValueError('SEED_CONTROL_UNAVAILABLE: neither original launcher nor recreation gameplay Host '
                         'has an acknowledged paired seed setter; no input will be replayed')
    return value


def load_scenario(path):
    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError(f'duplicate scenario field: {key}')
            result[key] = value
        return result
    return validate(json.loads(path.read_text(), object_pairs_hook=unique))


class Browser:
    def __init__(self, ref, config, output):
        self.ref, self.serial, self.buffer = ref, 0, b''
        self.log = (output / 'browser-stderr.log').open('wb')
        self.process = subprocess.Popen(['node', str(ref.HERE / 'browser-worker.mjs'), str(config)],
                                        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=self.log)

    def receive(self, timeout):
        deadline = time.monotonic() + timeout
        while b'\n' not in self.buffer:
            remaining = deadline - time.monotonic()
            if remaining <= 0 or not select.select([self.process.stdout], [], [], max(0, remaining))[0]:
                raise self.ref.Failure('browser adapter response timed out; replay is stopped, never retried')
            chunk = os.read(self.process.stdout.fileno(), 65536)
            if not chunk:
                raise self.ref.Failure(f'browser adapter exited {self.process.poll()}; inspect browser-stderr.log')
            self.buffer += chunk
            if len(self.buffer) > self.ref.MAX_METADATA:
                raise self.ref.Failure('browser adapter response exceeds metadata bound')
        line, self.buffer = self.buffer.split(b'\n', 1)
        result = json.loads(line)
        if not isinstance(result, dict) or type(result.get('ok')) is not bool or (result['ok'] and not isinstance(result.get('data'), dict)):
            raise self.ref.Failure('malformed browser protocol response')
        if not result.get('ok'):
            raise self.ref.Failure(f'browser adapter refused: {result.get("error")}')
        return result

    def request(self, op, **arguments):
        self.serial += 1
        message = {'id': self.serial, 'op': op, **arguments}
        self.process.stdin.write((json.dumps(message) + '\n').encode())
        self.process.stdin.flush()
        result = self.receive(90)
        if type(result.get('id')) is not int or result.get('id') != self.serial:
            raise self.ref.Failure('browser adapter response order mismatch')
        return result.get('data')

    def close(self):
        try:
            self.process.stdin.close()
            self.process.wait(timeout=10)
        except (OSError, subprocess.TimeoutExpired):
            self.process.kill()
            self.process.wait(timeout=10)
        self.log.close()
        self.process.stdout.close()


def image_difference(reference, recreation, extent):
    from PIL import Image, ImageChops
    with Image.open(reference) as a, Image.open(recreation) as b:
        if list(a.size) != extent or list(b.size) != extent:
            raise ValueError('checkpoint extent differs; no rescaling is allowed')
        difference = ImageChops.difference(a.convert('RGB'), b.convert('RGB'))
        histogram = difference.histogram()
        absolute = sum((i % 256) * count for i, count in enumerate(histogram))
        maximum = max((i % 256 for i, count in enumerate(histogram) if count), default=0)
        return {'equal': difference.getbbox() is None, 'changed_bounds': difference.getbbox(),
                'max_channel_error': maximum, 'mean_absolute_channel_error': absolute / (extent[0] * extent[1] * 3)}


def native_window(ref, run, scenario):
    display = ref.execute(run, ['xdpyinfo'])
    width, height = scenario['extent']
    if f'{width}x{height} pixels' not in display or '96x96 dots per inch' not in display:
        raise ref.Failure('actual X framebuffer/DPI differs from the scenario')
    pattern = '^' + re.escape(scenario['window_title']) + '$'
    ids = ref.execute(run, ['xdotool', 'search', '--onlyvisible', '--name', pattern]).splitlines()
    if len(ids) != 1 or not ids[0].isdigit():
        raise ref.Failure('expected exactly one visible original window with the supplied title')
    window = ids[0]
    geometry = ref.execute(run, ['xdotool', 'getwindowgeometry', '--shell', window])
    fields = dict(line.split('=', 1) for line in geometry.splitlines() if '=' in line)
    expected = dict(zip(('WIDTH', 'HEIGHT'), map(str, scenario['extent'])))
    if any(fields.get(key) != value for key, value in {**expected, 'X': '0', 'Y': '0'}.items()):
        raise ref.Failure('original client must occupy the full requested framebuffer at origin')
    ref.execute(run, ['xdotool', 'windowfocus', '--sync', window])
    if ref.execute(run, ['xdotool', 'getwindowfocus']).strip() != window:
        raise ref.Failure('original window focus could not be verified')
    return {'window': window, 'geometry': fields}


def native_input(ref, run, event):
    if event['kind'] == 'key':
        argv = ['xdotool', 'key' + event['phase'], KEYS[event['key']]]
    elif event['phase'] == 'move':
        argv = ['xdotool', 'mousemove', '--sync', str(event['x']), str(event['y'])]
    else:
        argv = ['xdotool', 'mouse' + event['phase'], BUTTONS[event['button']]]
    started = time.monotonic_ns()
    ref.execute(run, argv)
    return ref.mark(run, 'paired_input', {'event': event, 'started_ns': started, 'finished_ns': time.monotonic_ns()})


def wait_after(previous_finished, delay_ms, clock=time.monotonic_ns, sleep=time.sleep):
    deadline = previous_finished + delay_ms * 1000000
    remaining = (deadline - clock()) / 1e9
    if remaining > 0:
        sleep(remaining)
    dispatched = clock()
    return dispatched, max(0, (dispatched - deadline) / 1e6)


def compare(args, ref):
    scenario_path = Path(args.scenario).resolve(strict=True)
    if scenario_path.stat().st_size > ref.MAX_METADATA:
        raise ref.Failure('scenario exceeds metadata bound')
    scenario = load_scenario(scenario_path)
    run = ref.local_run(args.run_dir)
    launch = json.loads((run / 'launch.json').read_text())
    if not isinstance(launch, dict) or launch.get('framebuffer') != scenario['extent'] or launch.get('x_dpi') != scenario['dpi']:
        raise ref.Failure('scenario does not match the original run calibration')
    state = ref.inspect(run)
    if not state or not state['State']['Running']:
        raise ref.Failure('start a fresh isolated original run before comparison')
    server_args = ref.execute(run, ['sh', '-c', 'for p in /proc/[0-9]*/cmdline; do tr "\\000" " " < "$p" 2>/dev/null; printf "\\n"; done'])
    if not any(re.match(r'(?:\S*/)?Xvfb\s', line) and re.search(r'\s-r\s', line) for line in server_args.splitlines()):
        raise ref.Failure('original Xvfb must disable keyboard auto-repeat (-r); start a fresh isolated run')
    source = json.loads((run / 'source.json').read_text())
    if not isinstance(source, dict) or not isinstance(source.get('executable_sha256'), str):
        raise ref.Failure('malformed original source manifest')
    if ref.digest(run / 'game' / 'Warcraft III.exe') != source['executable_sha256']:
        raise ref.Failure('original executable changed since the private copy was prepared')
    url = urlparse(args.url)
    if url.scheme != 'http' or url.hostname not in ('127.0.0.1', 'localhost', '::1') or url.username or url.password or url.query or url.fragment:
        raise ref.Failure('recreation URL must be an explicit code-only loopback HTTP page')
    manifest = Path(args.manifest).resolve(strict=True)
    if manifest.stat().st_size > ref.MAX_METADATA:
        raise ref.Failure('code manifest exceeds metadata bound')
    code = json.loads(manifest.read_text())
    if not isinstance(code, dict) or not isinstance(code.get('files'), dict) or not 1 <= len(code['files']) <= 256:
        raise ref.Failure('bounded code build receipt with files is required')
    iso, module = Path(args.iso).resolve(strict=True), Path(args.playwright_module).resolve(strict=True)
    if not iso.is_file() or not module.is_file():
        raise ref.Failure('ISO and installed Playwright module must be local files')
    output = Path(args.output).resolve()
    local_roots = [ref.HERE.parents[1] / '.scratch', Path('/tmp')]
    if not any(output.is_relative_to(base.resolve()) and output != base.resolve() for base in local_roots):
        raise ref.Failure('original comparison artifacts must stay in local .scratch or /tmp')
    if output.exists():
        raise ref.Failure('comparison output exists; use a new directory, never retry inputs blindly')
    # No game input is sent until all scenario/seed/calibration validation passed.
    output.mkdir(parents=True, mode=0o700)
    browser = None
    result = {'version': 1, 'scenario_sha256': ref.digest(scenario_path), 'scenario': scenario,
              'source_manifest_sha256': ref.digest(run / 'source.json'),
              'harness_sha256': {name: ref.digest(ref.HERE / name) for name in
                  ['reference.py', 'replay.py', 'browser-worker.mjs', 'browser-adapter.mjs']},
              'reference_launch': launch, 'reference_start_state': 'caller selected; screenshot retained for review',
              'iso_sha256': ref.digest(iso), 'code_manifest_sha256': ref.digest(manifest),
              'rng': {'requested_seed': None, 'original': 'uncontrolled', 'recreation': 'uncontrolled'},
              'clock': 'barrier-relative wall-clock delays after both preceding operations finish; no simulation-tick or animation-phase control',
              'deterministic_parity_verified': False, 'events': [], 'checkpoints': []}
    try:
        result['native'] = native_window(ref, run, scenario)
        config = {'url': args.url, 'manifest': str(manifest), 'iso': str(iso),
                  'playwrightModule': str(module), 'chromium': args.chromium,
                  'extent': scenario['extent'], 'edition': scenario['edition']}
        ref.write_json(output / 'browser-config.json', config)
        browser = Browser(ref, output / 'browser-config.json', output)
        result['browser_start'] = browser.receive(180)['data']
        start = time.monotonic_ns()
        previous_finished = start
        scheduled = 0
        with ThreadPoolExecutor(max_workers=2) as pair:
            for index, event in enumerate(scenario['events']):
                scheduled += event['after_ms']
                dispatch, lateness = wait_after(previous_finished, event['after_ms'])
                journal = {'index': index, 'input': event, 'scheduled_ms': scheduled,
                           'previous_finished_ns': previous_finished, 'dispatch_ns': dispatch,
                           'relative_ms': (dispatch - start) / 1e6, 'lateness_ms': lateness}
                result['events'].append(journal)
                if event['kind'] == 'checkpoint':
                    label = f'paired-{hashlib.sha256(str(output).encode()).hexdigest()[:12]}-{index}'
                    native = pair.submit(ref.capture, SimpleNamespace(run_dir=str(run), label=label))
                    recreation = pair.submit(browser.request, 'capture', path=str(output / f'{event["name"]}-recreation.png'))
                    journal['original'], journal['recreation'] = native.result(), recreation.result()
                    target = output / event['name']
                    report = ref.report(SimpleNamespace(reference=journal['original']['capture'],
                        recreation=journal['recreation']['path'], output=str(target),
                        note='Same declared inputs; random streams and animation phases uncontrolled. Wine reference, not native Windows parity.'))
                    ref.write_json(target / 'browser-capture.json', journal['recreation'])
                    journal['recreation'] = {key: value for key, value in journal['recreation'].items() if key not in ('state_before', 'state_after')}
                    metrics = image_difference(target / 'reference.png', target / 'recreation.png', scenario['extent'])
                    result['checkpoints'].append({'name': event['name'], 'report': report, 'difference': metrics})
                else:
                    native = pair.submit(native_input, ref, run, event)
                    recreation = pair.submit(browser.request, 'input', event=event)
                    journal['original'], journal['recreation'] = native.result(), recreation.result()
                journal['finished_ns'] = time.monotonic_ns()
                previous_finished = journal['finished_ns']
                ref.write_json(output / 'comparison.json', result)
        result['browser_end'] = browser.request('summary')
        result['observed_images_equal'] = all(c['difference']['equal'] for c in result['checkpoints'])
        links = ''.join(f'<li><a href="{html.escape(c["name"])}/index.html">{html.escape(c["name"])}</a>: '
                        f'{html.escape(json.dumps(c["difference"]))}</li>' for c in result['checkpoints'])
        (output / 'index.html').write_text('<!doctype html><meta charset="utf-8"><title>Paired input comparison</title>'
            '<h1>Paired input comparison</h1><p>Uncontrolled RNG and animation clocks. '
            'These are observed outputs, not verified deterministic parity.</p><ul>' + links + '</ul>')
        return {'report': str(output / 'index.html'), 'receipt': str(output / 'comparison.json'),
                'checkpoints': len(result['checkpoints']), 'images_equal': result['observed_images_equal'],
                'deterministic_parity_verified': False}
    except BaseException as error:
        result['failure'] = str(error) or type(error).__name__
        keys = {event['input']['key'] for event in result['events']
                if event['input']['kind'] == 'key' and event['input']['phase'] == 'down'}
        buttons = {event['input']['button'] for event in result['events']
                   if event['input']['kind'] == 'pointer' and event['input']['phase'] == 'down'}
        try:
            for key in sorted(keys):
                ref.execute(run, ['xdotool', 'keyup', KEYS[key]])
            for button in sorted(buttons):
                ref.execute(run, ['xdotool', 'mouseup', BUTTONS[button]])
            result['abort_cleanup'] = ref.mark(run, 'paired_abort_release', {'keys': sorted(keys), 'buttons': sorted(buttons)})
        except Exception as cleanup_error:
            result['abort_cleanup_error'] = str(cleanup_error)
        raise
    finally:
        if browser:
            browser.close()
        ref.write_json(output / 'comparison.json', result)
