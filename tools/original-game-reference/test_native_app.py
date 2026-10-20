import os
from pathlib import Path
import re
import struct
import tempfile
import unittest
import zlib
from unittest.mock import patch

import native_app
import reference
import replay


def png(width, height):
    body = struct.pack('>IIBBBBB', width, height, 8, 2, 0, 0, 0)
    def chunk(tag, data):
        return struct.pack('>I', len(data)) + tag + data + struct.pack('>I', zlib.crc32(tag + data) & 0xffffffff)
    return b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', body) + chunk(b'IEND', b'')


def scenario():
    return {'version': 1, 'edition': 'roc', 'extent': [80, 60], 'dpi': 96,
            'window_title': 'Synthetic native', 'seed': None, 'events': [
                {'kind': 'checkpoint', 'after_ms': 0, 'name': 'start'}]}


def fingerprint(directory, kind, name):
    path = directory / name
    reference.write_json(path, {'version': 1, 'kind': kind, 'files': [
        {'path': 'build/pc3', 'sha256': 'a' * 64}]})
    return {'path': str(path), 'sha256': reference.digest(path)}


class NativeAppTests(unittest.TestCase):
    def test_live_process_window_and_partitions_of_input_use_only_its_display(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            exe = root / 'pc3'
            exe.write_bytes(b'native-executable')
            auth = root / 'xauth'
            auth.write_bytes(b'authority')
            output = root / 'out'
            output.mkdir()
            config = root / 'native.json'
            reference.write_json(config, {
                'version': 1, 'target': 'pc3-native', 'pid': 4242, 'start_ticks': 1000,
                'executable': {'path': str(exe), 'sha256': reference.digest(exe)},
                'display': ':2', 'window_title': 'Synthetic native',
                'build_receipt': fingerprint(root, 'build-receipt', 'build.json'),
                'content_manifest': fingerprint(root, 'content-manifest', 'content.json')})
            state = {'focus': '42', 'owner': '4242', 'title': 'Synthetic native',
                     'exe': str(exe), 'ticks': 1000, 'display': ':2', 'authority': str(auth),
                     'png': (80, 60), 'alive': True}
            calls = []

            def command(argv, **_unused):
                calls.append(argv)
                self.assertEqual(argv[:3], ['env', '-i', 'DISPLAY=:2'])
                self.assertIn(f'XAUTHORITY={auth}', argv)
                self.assertNotIn('sh', argv)
                self.assertNotIn('-c', argv)
                self.assertNotIn('-resize', argv)
                self.assertFalse(any('should-not-leak' in part for part in argv))
                tool = next(part for part in argv if part in {'xdpyinfo', 'xdotool', 'import'})
                args = argv[argv.index(tool):]
                if args == ['xdpyinfo']:
                    return ('default screen number:    0\n'
                            'screen #0:\n'
                            '  dimensions:    80x60 pixels (21x16 millimeters)\n'
                            '  resolution:    96x96 dots per inch\n')
                if args[:4] == ['xdotool', 'search', '--onlyvisible', '--name']:
                    self.assertEqual(args[4], '^' + re.escape('Synthetic native') + '$')
                    return '42'
                if args[:3] == ['xdotool', 'getwindowgeometry', '--shell']:
                    return 'WINDOW=42\nX=0\nY=0\nWIDTH=80\nHEIGHT=60\nSCREEN=0'
                if args[:2] == ['xdotool', 'getwindowpid']:
                    return state['owner']
                if args[:2] == ['xdotool', 'getwindowname']:
                    return state['title']
                if args[:3] == ['xdotool', 'windowfocus', '--sync']:
                    self.assertEqual(args[3], '42')
                    return ''
                if args == ['xdotool', 'getwindowfocus']:
                    return state['focus']
                if args[0] == 'import':
                    self.assertEqual(args[:3], ['import', '-window', '42'])
                    self.assertTrue(args[3].startswith('png:'))
                    Path(args[3].removeprefix('png:')).write_bytes(png(*state['png']))
                    return ''
                if args[1] in {'keydown', 'keyup', 'mousedown', 'mouseup', 'mousemove'}:
                    return ''
                self.fail(f'unexpected command: {args}')

            def read_link(path):
                self.assertEqual(path, '/proc/4242/exe')
                if not state['alive']:
                    raise FileNotFoundError(path)
                return state['exe']

            def read_text(path):
                self.assertEqual(path, '/proc/4242/stat')
                if not state['alive']:
                    raise FileNotFoundError(path)
                return f"4242 (pc3 native) S 1 1 1 0 -1 0 0 0 0 0 0 0 0 0 20 0 1 0 {state['ticks']} 99\n"

            def read_proc_bytes(path):
                self.assertEqual(path, '/proc/4242/environ')
                if not state['alive']:
                    raise FileNotFoundError(path)
                return (f"DISPLAY={state['display']}\0XAUTHORITY={state['authority']}\0"
                        'PATH=/should-not-leak\0').encode()

            saved = os.environ.get('XAUTHORITY'), os.environ.get('PATH')
            os.environ['XAUTHORITY'] = '/tmp/host-authority'
            os.environ['PATH'] = '/should-not-leak'
            try:
                with patch.object(native_app, 'read_link', read_link), \
                     patch.object(native_app, 'read_text', read_text), \
                     patch.object(native_app, 'read_bytes', read_proc_bytes), \
                     patch.object(reference, 'command', command), \
                     patch('subprocess.Popen', side_effect=AssertionError('adapter launched a process')):
                    app = native_app.NativeApp(reference, config, output, scenario())
                    self.assertEqual(app.start['target'], 'pc3-native')
                    self.assertEqual(app.start['pid'], 4242)
                    self.assertEqual(app.start['window'], '42')
                    self.assertEqual(app.start['executable_sha256'], reference.digest(exe))
                    self.assertEqual(list(output.iterdir()), [])
                    event = {'kind': 'key', 'phase': 'down', 'key': 'Tab', 'after_ms': 0}
                    meta = app.input(dict(event))
                    self.assertEqual(meta['event'], event)
                    self.assertEqual(meta['window'], '42')
                    tools = [call[call.index('xdotool'):] for call in calls if 'xdotool' in call]
                    self.assertIn(['xdotool', 'keydown', 'Tab'], tools)
                    state['focus'] = '99'
                    before = len(calls)
                    with self.assertRaisesRegex(reference.Failure, 'focus'):
                        app.input({'kind': 'key', 'phase': 'up', 'key': 'Tab', 'after_ms': 0})
                    later = [call[call.index('xdotool'):] for call in calls[before:] if 'xdotool' in call]
                    self.assertFalse(any(args[1] in {'keydown', 'keyup'} for args in later))
                    state['focus'] = '42'
                    state['ticks'] = 1001
                    with self.assertRaisesRegex(reference.Failure, 'start ticks'):
                        app.input({'kind': 'key', 'phase': 'up', 'key': 'Tab', 'after_ms': 0})
                    state['ticks'] = 1000
                    shot = output / 'frame.png'
                    record = app.capture(shot)
                    self.assertEqual(record['sha256'], reference.digest(shot))
                    self.assertIsNone(record['state_changed_during_capture'])
                    self.assertEqual(record['qualification'], 'native frame state unobserved')
                    self.assertEqual(record['path'], str(shot.resolve()))
                    state['png'] = (81, 60)
                    with self.assertRaisesRegex(reference.Failure, 'no rescaling'):
                        app.capture(output / 'wrong.png')
                    exe.write_bytes(b'changed')
                    with self.assertRaisesRegex(reference.Failure, 'hash'):
                        app.capture(output / 'after-change.png')
                    exe.write_bytes(b'native-executable')
                    state['alive'] = False
                    with self.assertRaisesRegex(reference.Failure, 'absent'):
                        app.close()
                    state['alive'] = True
                    app.close()
                    released = [call[call.index('xdotool'):] for call in calls if 'xdotool' in call]
                    self.assertIn(['xdotool', 'keyup', 'Tab'], released)
                    self.assertFalse(any('kill' in call or call[0] in {'kill', 'docker'} for call in calls))
            finally:
                if saved[0] is None:
                    os.environ.pop('XAUTHORITY', None)
                else:
                    os.environ['XAUTHORITY'] = saved[0]
                if saved[1] is None:
                    os.environ.pop('PATH', None)
                else:
                    os.environ['PATH'] = saved[1]

    def test_config_and_process_mismatches_fail_closed(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            exe = root / 'pc3'
            exe.write_bytes(b'native-executable')
            output = root / 'out'
            output.mkdir()
            base = {
                'version': 1, 'target': 'pc3-native', 'pid': 4242, 'start_ticks': 1000,
                'executable': {'path': str(exe), 'sha256': reference.digest(exe)},
                'display': ':2', 'window_title': 'Synthetic native',
                'build_receipt': fingerprint(root, 'build-receipt', 'build.json'),
                'content_manifest': fingerprint(root, 'content-manifest', 'content.json')}
            def write(value):
                path = root / 'native.json'
                reference.write_json(path, value)
                return path
            for value, message in [
                ({**base, 'target': 'pc3-wasm'}, 'pc3-native'),
                ({**base, 'target': 'rust-vm'}, 'pc3-native'),
                ({**base, 'version': 2}, 'version'),
                ({**base, 'pid': True}, 'pid'),
                ({**base, 'display': 'localhost:2'}, 'display'),
                ({**base, 'window_title': 'Other'}, 'title'),
            ]:
                with self.subTest(value=value['target'] if 'target' in value else value), \
                     self.assertRaisesRegex(reference.Failure, message):
                    native_app.NativeApp(reference, write(value), output, scenario())
            bad = root / 'blob.json'
            bad.write_bytes(b'not-a-fingerprint')
            broken = {**base, 'build_receipt': {'path': str(bad), 'sha256': reference.digest(bad)}}
            with self.assertRaisesRegex(reference.Failure, 'not JSON'):
                native_app.NativeApp(reference, write(broken), output, scenario())
            text = (root / 'build.json').read_text().replace('build-receipt', 'content-manifest')
            (root / 'build.json').write_text(text)
            base['build_receipt']['sha256'] = reference.digest(root / 'build.json')
            with self.assertRaisesRegex(reference.Failure, 'build-receipt'):
                native_app.NativeApp(reference, write(base), output, scenario())

    def test_owner_is_checked_before_focus_and_deleted_exe_is_refused(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            exe = root / 'pc3'
            exe.write_bytes(b'native-executable')
            output = root / 'out'
            output.mkdir()
            config = {
                'version': 1, 'target': 'pc3-native', 'pid': 7, 'start_ticks': 5,
                'executable': {'path': str(exe), 'sha256': reference.digest(exe)},
                'display': ':9', 'window_title': 'Synthetic native',
                'build_receipt': fingerprint(root, 'build-receipt', 'build.json'),
                'content_manifest': fingerprint(root, 'content-manifest', 'content.json')}
            path = root / 'native.json'
            reference.write_json(path, config)
            calls = []
            def command(argv, **_unused):
                calls.append(argv)
                tool = argv[argv.index('xdotool'):] if 'xdotool' in argv else argv[-1:]
                if 'xdpyinfo' in argv:
                    return ('default screen number:    0\nscreen #0:\n'
                            '  dimensions:    80x60 pixels\n  resolution:    96x96 dots per inch\n')
                if 'search' in argv:
                    return '9'
                if 'getwindowgeometry' in argv:
                    return 'X=0\nY=0\nWIDTH=80\nHEIGHT=60\nSCREEN=0'
                if 'getwindowpid' in argv:
                    return '8'
                self.fail(f'focused or queried past the owner check: {tool}')
            with patch.object(native_app, 'read_link', return_value=str(exe)), \
                 patch.object(native_app, 'read_text', return_value='7 (pc3) S ' + '0 ' * 18 + '5 1\n'), \
                 patch.object(native_app, 'read_bytes', return_value=b'DISPLAY=:9\0'), \
                 patch.object(reference, 'command', command):
                with self.assertRaisesRegex(reference.Failure, 'owner PID'):
                    native_app.NativeApp(reference, path, output, scenario())
            self.assertFalse(any('windowfocus' in call for call in calls))
            with patch.object(native_app, 'read_link', return_value=str(exe) + ' (deleted)'), \
                 patch.object(native_app, 'read_text', return_value='7 (pc3) S ' + '0 ' * 18 + '5 1\n'), \
                 patch.object(native_app, 'read_bytes', return_value=b'DISPLAY=:9\0'), \
                 patch.object(reference, 'command', command):
                with self.assertRaisesRegex(reference.Failure, 'executable'):
                    native_app.NativeApp(reference, path, output, scenario())

    def test_parse_start_ticks_uses_the_field_after_comm(self):
        self.assertEqual(native_app.parse_start_ticks('9 (na)me) S 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 77 1'), 77)
        with self.assertRaises(ValueError):
            native_app.parse_start_ticks('9 (name) Z 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 77 1')


if __name__ == '__main__':
    unittest.main()
