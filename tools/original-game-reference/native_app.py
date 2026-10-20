"""Caller-owned PC3 native window on its own display. Does not launch or stop that process."""
from pathlib import Path
import json
import os
import re

import replay

_SHA = re.compile(r'^[0-9a-f]{64}$')
_DISPLAY = re.compile(r'^:[0-9]{1,4}$')
_TOOLS = {'xdpyinfo', 'xdotool', 'import'}
_XDOTOOL = {'search', 'getwindowgeometry', 'getwindowpid', 'getwindowname',
            'windowfocus', 'getwindowfocus', 'keydown', 'keyup', 'mousemove', 'mousedown', 'mouseup'}


def read_link(path):
    return os.readlink(path)


def read_text(path):
    return Path(path).read_text()


def read_bytes(path):
    return Path(path).read_bytes()


def parse_start_ticks(stat):
    end = stat.rfind(')')
    if end < 0:
        raise ValueError('unreadable process stat')
    tail = stat[end + 1:].split()
    if len(tail) < 20 or tail[0] == 'Z' or not tail[19].isdigit():
        raise ValueError('unreadable process stat')
    return int(tail[19])


def _json(path, ref):
    try:
        path = Path(path).resolve(strict=True)
    except OSError as error:
        raise ref.Failure('fingerprint file is absent') from error
    if path.stat().st_size > ref.MAX_METADATA:
        raise ref.Failure(f'{path.name} exceeds metadata bound')
    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ref.Failure(f'duplicate field: {key}')
            result[key] = value
        return result
    try:
        return json.loads(path.read_text(), object_pairs_hook=unique)
    except json.JSONDecodeError as error:
        raise ref.Failure(f'{path.name} is not JSON') from error


def _sha(value, label, ref):
    if type(value) is not str or not _SHA.fullmatch(value):
        raise ref.Failure(f'{label} sha256 must be 64 lowercase hex characters')
    return value


def _fingerprint(spec, kind, ref):
    if type(spec) is not dict or set(spec) != {'path', 'sha256'}:
        raise ref.Failure(f'{kind} must contain path and sha256')
    _sha(spec['sha256'], kind, ref)
    path = Path(spec['path'])
    if not path.is_absolute():
        raise ref.Failure(f'{kind} path must be absolute')
    if ref.digest(path) != spec['sha256']:
        raise ref.Failure(f'{kind} hash does not match the file')
    document = _json(path, ref)
    if (type(document) is not dict or set(document) != {'version', 'kind', 'files'}
            or type(document['version']) is not int or document['version'] != 1
            or document['kind'] != kind):
        raise ref.Failure(f'{kind} must be an explicit fingerprint document')
    files = document['files']
    if type(files) is not list or not 1 <= len(files) <= 256:
        raise ref.Failure(f'{kind} must list 1..256 file fingerprints')
    seen = set()
    for entry in files:
        if type(entry) is not dict or set(entry) != {'path', 'sha256'}:
            raise ref.Failure(f'{kind} entries must be path and sha256')
        name = entry['path']
        if (type(name) is not str or not 1 <= len(name) <= 4096 or '\x00' in name or '\n' in name
                or '..' in Path(name).parts):
            raise ref.Failure(f'{kind} contains a path that is not an explicit fingerprint')
        _sha(entry['sha256'], kind, ref)
        if name in seen:
            raise ref.Failure(f'{kind} repeats a path')
        seen.add(name)
    return spec['sha256']


class _Display:
    """Runs xdpyinfo/xdotool on the process display. Refuses focus of another PID."""

    def __init__(self, app):
        self.app = app
        self.Failure = app.ref.Failure

    def execute(self, _run, argv):
        if argv[:3] == ['xdotool', 'windowfocus', '--sync']:
            self.app._require_owner(argv[3])
        return self.app._x(argv)


class NativeApp:
    def __init__(self, ref, config_path, output, scenario):
        self.ref = ref
        self.output = Path(output).resolve()
        if not self.output.is_dir():
            raise ref.Failure('output directory is absent')
        try:
            self.scenario = replay.validate(scenario)
        except ValueError as error:
            raise ref.Failure(str(error)) from error
        config = _json(config_path, ref)
        fields = {'version', 'target', 'pid', 'start_ticks', 'executable', 'display',
                  'window_title', 'build_receipt', 'content_manifest'}
        if type(config) is not dict or set(config) != fields:
            raise ref.Failure('native app config fields are version/target/pid/start_ticks/'
                              'executable/display/window_title/build_receipt/content_manifest')
        if type(config['version']) is not int or config['version'] != 1:
            raise ref.Failure('unsupported native app config version')
        if config['target'] != 'pc3-native':
            raise ref.Failure('target must be pc3-native, the caller-owned Linux process')
        pid, ticks = config['pid'], config['start_ticks']
        if type(pid) is not int or not 1 <= pid <= 2**22:
            raise ref.Failure('pid must be a live process id')
        if type(ticks) is not int or not 0 <= ticks <= 2**64 - 1:
            raise ref.Failure('start_ticks must be the process start tick count')
        if type(config['display']) is not str or not _DISPLAY.fullmatch(config['display']):
            raise ref.Failure('display must be a local :N')
        title = config['window_title']
        if title != self.scenario['window_title']:
            raise ref.Failure('config window title must match the scenario')
        executable = config['executable']
        if type(executable) is not dict or set(executable) != {'path', 'sha256'}:
            raise ref.Failure('executable must contain path and sha256')
        _sha(executable['sha256'], 'executable', ref)
        exe = Path(executable['path'])
        try:
            exe_hash = ref.digest(exe) if exe.is_absolute() else None
        except OSError as error:
            raise ref.Failure('executable is not a readable file') from error
        if exe_hash != executable['sha256']:
            raise ref.Failure('executable path must be absolute and match sha256')
        self.pid = pid
        self.start_ticks = ticks
        self.display = config['display']
        self.executable = exe.resolve()
        self.executable_sha256 = executable['sha256']
        self.build_sha256 = _fingerprint(config['build_receipt'], 'build-receipt', ref)
        self.content_sha256 = _fingerprint(config['content_manifest'], 'content-manifest', ref)
        self.xauthority = None
        self.window = None
        self.held_keys, self.held_buttons = set(), set()
        self.pointer = None
        self._live()
        found = replay.native_window(_Display(self), None, self.scenario)
        self.window = found['window']
        self.start = {'target': 'pc3-native', 'pid': pid, 'start_ticks': ticks,
                      'display': self.display, 'window': self.window,
                      'executable_sha256': self.executable_sha256,
                      'build_receipt_sha256': self.build_sha256,
                      'content_manifest_sha256': self.content_sha256}

    def _x(self, argv):
        if not argv or argv[0] not in _TOOLS or any(type(part) is not str or '\x00' in part for part in argv):
            raise self.ref.Failure('refusing an unbounded display command')
        if argv[0] == 'xdotool' and (len(argv) < 2 or argv[1] not in _XDOTOOL):
            raise self.ref.Failure('refusing an unbounded xdotool command')
        if any(part in {'-resize', '-scale', '-thumbnail', '-c'} or part.startswith('png:-') for part in argv):
            raise self.ref.Failure('refusing a rescaled or shell display command')
        prefix = ['env', '-i', f'DISPLAY={self.display}']
        if self.xauthority is not None:
            prefix.append(f'XAUTHORITY={self.xauthority}')
        return self.ref.command([*prefix, *argv])

    def _require_owner(self, window):
        if not str(window).isdigit():
            raise self.ref.Failure('window id is not numeric')
        owner = self._x(['xdotool', 'getwindowpid', str(window)])
        if owner != str(self.pid):
            raise self.ref.Failure('window owner PID does not match the configured process')
        if self._x(['xdotool', 'getwindowname', str(window)]) != self.scenario['window_title']:
            raise self.ref.Failure('window title does not match the scenario')

    def _live(self):
        proc = f'/proc/{self.pid}'
        try:
            exe = read_link(proc + '/exe')
            stat = read_text(proc + '/stat')
            environ = read_bytes(proc + '/environ')
        except OSError as error:
            raise self.ref.Failure('configured process is absent or unreadable') from error
        if exe.endswith(' (deleted)') or os.path.realpath(exe) != str(self.executable):
            raise self.ref.Failure('live executable does not match the configured path')
        if self.ref.digest(self.executable) != self.executable_sha256:
            raise self.ref.Failure('live executable hash changed')
        if not stat.startswith(f'{self.pid} '):
            raise self.ref.Failure('process stat does not belong to the configured pid')
        try:
            ticks = parse_start_ticks(stat)
        except ValueError as error:
            raise self.ref.Failure(str(error)) from error
        if ticks != self.start_ticks:
            raise self.ref.Failure('process start ticks changed')
        fields = {}
        for item in environ.split(b'\0'):
            if not item:
                continue
            if b'=' not in item:
                raise self.ref.Failure('process environment is unreadable')
            key, value = item.split(b'=', 1)
            name = key.decode('ascii', 'strict') if key.isascii() else ''
            if name in fields:
                raise self.ref.Failure(f'process environment repeats {name}')
            if name in {'DISPLAY', 'XAUTHORITY'}:
                fields[name] = value.decode('ascii', 'strict')
        if fields.get('DISPLAY') != self.display:
            raise self.ref.Failure('process DISPLAY does not match the configured local display')
        authority = fields.get('XAUTHORITY')
        if authority is not None:
            auth = Path(authority)
            if not auth.is_absolute() or not auth.is_file() or '\n' in authority or '\x00' in authority:
                raise self.ref.Failure('process XAUTHORITY is not an absolute file')
            authority = str(auth)
        if self.window is None:
            self.xauthority = authority
        elif authority != self.xauthority:
            raise self.ref.Failure('process XAUTHORITY changed')

    def _ready(self):
        self._live()
        found = replay.native_window(_Display(self), None, self.scenario)
        if found['window'] != self.window:
            raise self.ref.Failure('window id changed')
        self._require_owner(self.window)

    def input(self, event):
        if type(event) is not dict or event.get('kind') not in {'key', 'pointer'}:
            raise self.ref.Failure('input must be one key or pointer event')
        delay = event.get('after_ms')
        if type(delay) is not int or not 0 <= delay <= 30000:
            raise self.ref.Failure('event after_ms must be 0..30000')
        if event['kind'] == 'key':
            if set(event) != {'kind', 'after_ms', 'phase', 'key'}:
                raise self.ref.Failure('invalid key fields')
            if event.get('phase') not in {'down', 'up'} or event.get('key') not in replay.KEYS:
                raise self.ref.Failure('unsupported key/phase')
            held, item = self.held_keys, event['key']
            if (item in held) != (event['phase'] == 'up'):
                raise self.ref.Failure('key down/up sequence is unbalanced')
            argv = ['xdotool', 'key' + event['phase'], replay.KEYS[item]]
        else:
            phase = event.get('phase')
            fields = {'kind', 'after_ms', 'phase', 'x', 'y'} | ({'button'} if phase != 'move' else set())
            if set(event) != fields or phase not in {'move', 'down', 'up'}:
                raise self.ref.Failure('invalid pointer fields/phase')
            extent = self.scenario['extent']
            if any(type(event.get(axis)) is not int or not 0 <= event[axis] < limit
                   for axis, limit in zip(('x', 'y'), extent)):
                raise self.ref.Failure('pointer outside framebuffer')
            point = (event['x'], event['y'])
            if phase == 'move':
                self._ready()
                self._x(['xdotool', 'mousemove', '--sync', str(point[0]), str(point[1])])
                self.pointer = point
                return {**self.start, 'event': dict(event)}
            if self.pointer != point or event.get('button') not in replay.BUTTONS:
                raise self.ref.Failure('pointer down/up needs the current point and a supported button')
            item = event['button']
            if (item in self.held_buttons) != (phase == 'up'):
                raise self.ref.Failure('pointer down/up sequence is unbalanced')
            argv = ['xdotool', 'mouse' + phase, replay.BUTTONS[item]]
            held = self.held_buttons
        self._ready()
        self._x(argv)
        (held.add if event['phase'] == 'down' else held.remove)(item)
        return {**self.start, 'event': dict(event)}

    def capture(self, path):
        target = Path(path)
        if target.is_symlink() or target.exists():
            raise self.ref.Failure('capture path exists; no overwrite and no rescale')
        resolved = target.resolve()
        if self.output not in resolved.parents:
            raise self.ref.Failure('capture must stay inside the output directory')
        self._ready()
        self._x(['import', '-window', self.window, 'png:' + str(resolved)])
        try:
            data = Path(resolved).read_bytes()
        except OSError as error:
            raise self.ref.Failure('capture was not produced') from error
        if len(data) < 24 or data[:8] != b'\x89PNG\r\n\x1a\n' or data[12:16] != b'IHDR':
            raise self.ref.Failure('capture is not a PNG at the scenario extent')
        width, height = int.from_bytes(data[16:20], 'big'), int.from_bytes(data[20:24], 'big')
        if [width, height] != self.scenario['extent']:
            raise self.ref.Failure('capture extent differs; no rescaling is allowed')
        return {'path': str(resolved), 'sha256': self.ref.digest(resolved),
                'state_changed_during_capture': None,
                'qualification': 'native frame state unobserved'}

    def close(self):
        keys, buttons = sorted(self.held_keys), sorted(self.held_buttons)
        if keys or buttons:
            self._ready()
            for key in keys:
                self._x(['xdotool', 'keyup', replay.KEYS[key]])
            for button in buttons:
                self._x(['xdotool', 'mouseup', replay.BUTTONS[button]])
        self.held_keys.clear()
        self.held_buttons.clear()
