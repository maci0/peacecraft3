import copy
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from types import SimpleNamespace
from unittest.mock import patch

import replay
import reference

HERE = Path(__file__).resolve().parent


def scenario():
    return {'version': 1, 'edition': 'roc', 'extent': [80, 60], 'dpi': 96,
            'window_title': 'Synthetic reference', 'seed': None, 'events': [
                {'kind': 'checkpoint', 'after_ms': 0, 'name': 'start'},
                {'kind': 'pointer', 'phase': 'move', 'x': 17, 'y': 23, 'after_ms': 0},
                {'kind': 'pointer', 'phase': 'down', 'x': 17, 'y': 23, 'button': 'left', 'after_ms': 0},
                {'kind': 'pointer', 'phase': 'up', 'x': 17, 'y': 23, 'button': 'left', 'after_ms': 150},
                {'kind': 'key', 'phase': 'down', 'key': 'Tab', 'after_ms': 0},
                {'kind': 'key', 'phase': 'up', 'key': 'Tab', 'after_ms': 150},
                {'kind': 'checkpoint', 'after_ms': 200, 'name': 'end'}]}


class ReplayTests(unittest.TestCase):
    def test_native_move_refuses_clamped_pointer_before_marking_delivery(self):
        event = {'kind': 'pointer', 'phase': 'move', 'x': 2385, 'y': 92, 'after_ms': 0}
        for observed, succeeds in [('X=2385\nY=92\nSCREEN=0', True),
                                   ('X=1279\nY=92\nSCREEN=0', False),
                                   ('X=2385\nY=92\nSCREEN=1', False), ('', False)]:
            with self.subTest(observed=observed), patch.object(reference, 'execute',
                    side_effect=['', observed]) as send, patch.object(reference, 'mark') as mark:
                if succeeds:
                    replay.native_input(reference, 'synthetic', event)
                    mark.assert_called_once()
                else:
                    with self.assertRaisesRegex(reference.Failure, 'actual pointer'):
                        replay.native_input(reference, 'synthetic', event)
                    mark.assert_not_called()
                self.assertEqual(send.call_args.args[1], ['xdotool', 'getmouselocation', '--shell'])

    def test_native_grid_matches_the_only_default_screen_and_window(self):
        display = ('default screen number:    0\n'
                   'screen #0:\n'
                   '  dimensions:    1920x1080 pixels (508x286 millimeters)\n'
                   '  resolution:    96x96 dots per inch\n')
        randr = ('Screen 0: minimum 1 x 1, current 1920 x 1080, maximum 1920 x 1080\n'
                 'screen connected primary 1920x1080+0+0 (normal left inverted right x axis y axis)\n'
                 '   1920x1080 60.00*\n')

        def check(text=display, extent=(1920, 1080), screen='0', outputs=randr):
            calls = []

            def execute(unused_run, argv):
                calls.append(argv)
                if argv == ['xdpyinfo']:
                    return text
                if argv == ['xrandr', '--current']:
                    return outputs
                if argv[:2] == ['xdotool', 'search']:
                    return '42'
                if argv[:2] == ['xdotool', 'getwindowgeometry']:
                    return (f'WINDOW=42\nX=0\nY=0\nWIDTH={extent[0]}\n'
                            f'HEIGHT={extent[1]}\nSCREEN={screen}')
                if argv == ['xdotool', 'windowfocus', '--sync', '42']:
                    return ''
                if argv == ['xdotool', 'getwindowfocus']:
                    return '42'
                self.fail(f'unexpected command: {argv}')

            ref = SimpleNamespace(execute=execute, Failure=reference.Failure,
                                  MAX_METADATA=reference.MAX_METADATA)
            value = {'extent': list(extent), 'dpi': 96, 'window_title': 'Synthetic reference'}
            return replay.native_window(ref, 'synthetic-run', value), calls

        found, calls = check()
        self.assertEqual(found['window'], '42')
        self.assertIn(['xdotool', 'windowfocus', '--sync', '42'], calls)
        invalid = [
            (display, (920, 1080)),
            (display.replace('96x96', '196x96'), (1920, 1080)),
            (display.replace('screen #0:', 'screen #1:'), (1920, 1080)),
            (display.replace('number:    0', 'number:    1'), (1920, 1080)),
            (display + 'screen #1:\n  dimensions: 80x60 pixels\n'
                       '  resolution: 96x96 dots per inch\n', (1920, 1080)),
            (display + '  resolution: 120x120 dots per inch\n', (1920, 1080)),
            ('default screen number: 0\n', (1920, 1080)),
        ]
        for text, extent in invalid:
            with self.subTest(text=text, extent=extent), self.assertRaisesRegex(
                    reference.Failure, 'framebuffer/DPI'):
                check(text, extent)
        for screen in ['1', '']:
            with self.subTest(screen=screen), self.assertRaisesRegex(reference.Failure, 'framebuffer'):
                check(screen=screen)
        for outputs in [
            randr.replace('1920x1080+0+0', '1280x1024+0+0'),
            randr.replace('1920x1080+0+0', '1920x1080+1+0'),
            randr.replace('current 1920 x 1080', 'current 1280 x 1024'),
            randr + 'other connected 1920x1080+0+0 (normal)\n',
            randr.replace(' connected primary 1920x1080+0+0', ' disconnected'),
            '',
        ]:
            with self.subTest(outputs=outputs), self.assertRaisesRegex(reference.Failure, 'RandR'):
                check(outputs=outputs)

    def test_offline_input_schema_describes_the_supported_journal(self):
        import jsonschema
        schema = json.loads((HERE / 'schema.json').read_text())['extensions']['comparison_scenario']
        jsonschema.Draft202012Validator.check_schema(schema)
        validator = jsonschema.Draft202012Validator(schema)
        validator.validate(scenario())
        for change in [{'edition': 'unknown'}, {'extent': [0, 60]}, {'dpi': 192}, {'unknown': 1}]:
            with self.subTest(change=change), self.assertRaises(jsonschema.ValidationError):
                validator.validate({**scenario(), **change})

    def test_protocol_refuses_malformed_envelopes_without_traceback(self):
        for payload in [[], True, {}, {'ok': 'yes'}, {'ok': True, 'data': None}]:
            read_fd, write_fd = os.pipe()
            with os.fdopen(read_fd, 'rb') as reader, os.fdopen(write_fd, 'wb') as writer:
                writer.write((json.dumps(payload) + '\n').encode()); writer.flush()
                browser = replay.Browser.__new__(replay.Browser)
                browser.ref = reference; browser.buffer = b''
                browser.process = SimpleNamespace(stdout=reader, poll=lambda: 0)
                with self.subTest(payload=payload), self.assertRaisesRegex(reference.Failure, 'malformed'):
                    browser.receive(1)

    def test_abort_and_interrupt_release_inputs_and_retain_failure_receipt(self):
        for error in [reference.Failure('synthetic adapter failure'), KeyboardInterrupt()]:
            with self.subTest(error=type(error).__name__), tempfile.TemporaryDirectory() as tmp:
                tmp = Path(tmp); run = tmp / 'run'; (run / 'game').mkdir(parents=True)
                (run / 'game/Warcraft III.exe').write_bytes(b'synthetic executable')
                reference.write_json(run / 'source.json', {'executable_sha256': reference.digest(run / 'game/Warcraft III.exe')})
                reference.write_json(run / 'launch.json', {'framebuffer': [80, 60], 'x_dpi': 96})
                manifest = tmp / 'code.json'; reference.write_json(manifest, {'files': {'synthetic.js': {'bytes': 1, 'sha256': 'f' * 64}}})
                iso = tmp / 'fixture.iso'; iso.write_bytes(b'synthetic source')
                module = tmp / 'fixture.mjs'; module.write_text('')
                path = tmp / 'scenario.json'; value = scenario()
                value['events'] = [{'kind': 'key', 'phase': 'down', 'key': 'Tab', 'after_ms': 0},
                                   {'kind': 'key', 'phase': 'up', 'key': 'Tab', 'after_ms': 0},
                                   {'kind': 'checkpoint', 'name': 'end', 'after_ms': 0}]
                reference.write_json(path, value)
                args = SimpleNamespace(scenario=str(path), run_dir=str(run), url='http://127.0.0.1:1/index.html',
                    manifest=str(manifest), iso=str(iso), playwright_module=str(module), chromium='synthetic', output=str(tmp / 'output'))
                commands, closed = [], []
                class SyntheticBrowser:
                    def __init__(self, *unused): pass
                    def receive(self, *unused): return {'data': {'state': {'screen': 'Synthetic'}}}
                    def request(self, *unused, **arguments): raise error
                    def close(self): closed.append(True)
                def execute(unused_run, argv):
                    commands.append(argv)
                    return 'Xvfb :99 -screen 0 80x60x24 -r -nolisten tcp'
                with patch.object(reference, 'inspect', return_value={'State': {'Running': True}}), \
                     patch.object(reference, 'execute', side_effect=execute), \
                     patch.object(replay, 'native_window', return_value={}), \
                     patch.object(replay, 'Browser', SyntheticBrowser):
                    with self.assertRaises(type(error)):
                        replay.compare(args, reference)
                    self.assertIn(['xdotool', 'keydown', 'Tab'], commands)
                    self.assertIn(['xdotool', 'keyup', 'Tab'], commands)
                    self.assertEqual(closed, [True])
                    receipt = json.loads((tmp / 'output/comparison.json').read_text())
                    self.assertIn('failure', receipt)
                    self.assertEqual(receipt['abort_cleanup']['keys'], ['Tab'])
                    before = (tmp / 'output/comparison.json').read_bytes()
                    with self.assertRaisesRegex(reference.Failure, 'output exists'):
                        replay.compare(args, reference)
                    self.assertEqual((tmp / 'output/comparison.json').read_bytes(), before)

    def test_capture_cost_cannot_collapse_a_later_key_hold(self):
        now = [10_000_000_000]
        sleeps = []
        def sleep(seconds):
            sleeps.append(seconds)
            now[0] += round(seconds * 1e9)
        dispatch, late = replay.wait_after(now[0], 150, lambda: now[0], sleep)
        self.assertEqual(dispatch, 10_150_000_000)
        self.assertEqual(sleeps, [0.15])
        self.assertEqual(late, 0)
        now[0] += 8_000_000_000  # A screenshot operation takes longer than an input delay.
        previous = now[0]
        dispatch, late = replay.wait_after(previous, 150, lambda: now[0], sleep)
        self.assertEqual(dispatch - previous, 150_000_000)
        self.assertEqual(sleeps, [0.15, 0.15])

    def test_physical_events_preserve_order_and_caller_values(self):
        value = scenario()
        before = copy.deepcopy(value)
        self.assertEqual(replay.validate(value), before)
        self.assertEqual(value, before)

    def test_seed_is_never_silently_treated_as_applied(self):
        for seed in [0, 1, -(2**31), 2**31 - 1]:
            with self.subTest(seed=seed):
                value = scenario(); value['seed'] = seed
                with self.assertRaisesRegex(ValueError, 'SEED_CONTROL_UNAVAILABLE'):
                    replay.validate(value)
        for seed in [2**64 - 1, -2**31 - 1, 2**31, True, '1']:
            value = scenario(); value['seed'] = seed
            with self.assertRaisesRegex(ValueError, 'signed i32'):
                replay.validate(value)

    def test_unbalanced_and_implicit_input_events_refuse(self):
        mutations = [
            lambda v: v['events'].pop(1),
            lambda v: v['events'][2].update(x=18),
            lambda v: v['events'][2].update(phase='up'),
            lambda v: v['events'][3].update(phase='down'),
            lambda v: v['events'].pop(5),
            lambda v: v['events'][4].update(key=[]),
            lambda v: v['events'][4].update(key='Unknown'),
            lambda v: v['events'][2].update(button={}),
            lambda v: v['events'][1].update(x=80),
            lambda v: v['events'][1].update(y=-1),
            lambda v: v['events'][1].update(x=0.5),
            lambda v: v['events'].insert(3, {'kind': 'checkpoint', 'after_ms': 0, 'name': 'held'}),
        ]
        for mutation in mutations:
            value = scenario(); mutation(value)
            with self.subTest(value=value), self.assertRaises(ValueError): replay.validate(value)

    def test_work_bounds_unknown_fields_and_duplicate_checkpoints(self):
        mutations = [
            lambda v: v.update(edition='unknown'),
            lambda v: v.update(dpi=192),
            lambda v: v.update(extent=[0, 60]),
            lambda v: v.update(extent=[4097, 60]),
            lambda v: v.update(version=True),
            lambda v: v.update(ignored='unrecognized'),
            lambda v: v['events'][1].update(ignored=True),
            lambda v: v['events'][1].update(after_ms=True),
            lambda v: v['events'][1].update(after_ms=30001),
            lambda v: v['events'][-1].update(name='../escape'),
            lambda v: v['events'][-1].update(name='start'),
            lambda v: v.update(events=v['events'] * 100),
            lambda v: v.update(events=[{'kind': 'checkpoint', 'after_ms': 30000, 'name': str(n)} for n in range(11)]),
        ]
        for mutation in mutations:
            value = scenario(); mutation(value)
            with self.subTest(value=value), self.assertRaises(ValueError): replay.validate(value)

    def test_duplicate_json_fields_refuse(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'scenario.json'
            path.write_text('{"version":1,"version":1}')
            with self.assertRaisesRegex(ValueError, 'duplicate'): replay.load_scenario(path)

    def test_seeded_cli_refuses_before_any_runtime_or_output_effect(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp); path = tmp / 'scenario.json'; output = tmp / 'output'
            value = scenario(); value['seed'] = 173
            path.write_text(json.dumps(value))
            argv = [sys.executable, str(HERE / 'reference.py'), 'compare', '--scenario', str(path)]
            for flag, text in [('run-dir', '/nonexistent'), ('url', 'http://127.0.0.1:1/index.html'),
                               ('manifest', '/nonexistent'), ('iso', '/nonexistent'),
                               ('playwright-module', '/nonexistent'), ('output', str(output))]:
                argv.extend(['--' + flag, text])
            result = subprocess.run(argv, input='', capture_output=True, text=True, timeout=10)
            self.assertEqual(result.returncode, 2)
            self.assertFalse(result.stdout)
            self.assertIn('SEED_CONTROL_UNAVAILABLE', json.loads(result.stderr)['error']['message'])
            self.assertFalse(output.exists())

    def test_image_difference_is_not_a_header_or_file_hash_comparison(self):
        from PIL import Image
        with tempfile.TemporaryDirectory() as tmp:
            a, b = Path(tmp) / 'a.png', Path(tmp) / 'b.png'
            Image.new('RGB', (8, 6), (10, 20, 30)).save(a, compress_level=0)
            Image.new('RGB', (8, 6), (10, 20, 30)).save(b, compress_level=9)
            self.assertNotEqual(a.read_bytes(), b.read_bytes())
            self.assertTrue(replay.image_difference(a, b, [8, 6])['equal'])
            with Image.open(b) as image:
                image.putpixel((3, 2), (11, 20, 30)); image.save(b)
            difference = replay.image_difference(a, b, [8, 6])
            self.assertFalse(difference['equal'])
            self.assertEqual(difference['changed_pixels'], 1)
            self.assertEqual(difference['compared_pixels'], 48)
            self.assertEqual(difference['changed_bounds'], (3, 2, 4, 3))
            self.assertEqual(difference['max_channel_error'], 1)
            with self.assertRaisesRegex(ValueError, 'extent'): replay.image_difference(a, b, [9, 6])

    def test_one_channel_one_pixel_mismatch_never_passes_or_crops_the_corner(self):
        from PIL import Image
        with tempfile.TemporaryDirectory() as tmp:
            a, b = Path(tmp) / 'a.png', Path(tmp) / 'b.png'
            Image.new('RGB', (8, 6), (10, 20, 30)).save(a)
            for point in [(0, 0), (4, 3), (7, 5)]:
                for channel in range(3):
                    image = Image.new('RGB', (8, 6), (10, 20, 30))
                    color = [10, 20, 30]; color[channel] += 1
                    image.putpixel(point, tuple(color)); image.save(b)
                    difference = replay.image_difference(a, b, [8, 6])
                    self.assertFalse(difference['equal'])
                    self.assertEqual(difference['changed_pixels'], 1)
                    self.assertEqual(difference['max_channel_error'], 1)
            self.assertEqual(reference.PIXEL_POLICY['channel_tolerance'], 0)
            self.assertEqual(reference.PIXEL_POLICY['applied_exclusions'], [])
            contract = json.loads((HERE / 'schema.json').read_text())
            self.assertEqual(contract['extensions']['three_target_comparison']['pixel_policy'],
                             reference.PIXEL_POLICY)

    def test_unstable_capture_is_not_a_stable_pixel_mismatch(self):
        from PIL import Image
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp); run = tmp / 'run'; (run / 'game').mkdir(parents=True)
            (run / 'game/Warcraft III.exe').write_bytes(b'synthetic executable')
            reference.write_json(run / 'source.json', {'executable_sha256': reference.digest(run / 'game/Warcraft III.exe')})
            reference.write_json(run / 'launch.json', {'framebuffer': [8, 6], 'x_dpi': 96})
            manifest = tmp / 'code.json'; reference.write_json(manifest, {'files': {'synthetic.js': {'bytes': 1, 'sha256': 'f' * 64}}})
            iso = tmp / 'fixture.iso'; iso.write_bytes(b'synthetic source')
            module = tmp / 'fixture.mjs'; module.write_text('')
            path = tmp / 'scenario.json'; value = scenario()
            value['extent'] = [8, 6]
            value['events'] = [{'kind': 'checkpoint', 'after_ms': 0, 'name': 'stable'},
                               {'kind': 'checkpoint', 'after_ms': 0, 'name': 'moved'}]
            reference.write_json(path, value)
            args = SimpleNamespace(scenario=str(path), run_dir=str(run), url='http://127.0.0.1:1/index.html',
                manifest=str(manifest), iso=str(iso), playwright_module=str(module), chromium='synthetic', output=str(tmp / 'output'))

            def write_delta(target):
                image = Image.new('RGB', (8, 6), (10, 20, 30))
                image.putpixel((3, 2), (11, 20, 30))
                image.save(target)

            def capture(capture_args):
                target = tmp / f'{capture_args.label}.png'
                Image.new('RGB', (8, 6), (10, 20, 30)).save(target)
                return {'capture': str(target), 'sha256': 'synthetic-reference'}

            class SyntheticBrowser:
                def __init__(self, *unused): pass
                def receive(self, *unused): return {'data': {'state': {'screen': 'Synthetic'}}}
                def request(self, op, **arguments):
                    if op == 'summary': return {'finished': True}
                    path = Path(arguments['path'])
                    write_delta(path)
                    changed = path.name.startswith('moved-')
                    return {'path': str(path), 'sha256': 'synthetic-recreation',
                            'geometry': {'clip': {'width': 8, 'height': 6, 'scale': 1}},
                            'state_before': {'screen': 'Synthetic'},
                            'state_after': {'screen': 'Synthetic-next'} if changed else {'screen': 'Synthetic'},
                            'state_changed_during_capture': changed}
                def close(self): pass

            def execute(unused_run, argv):
                return 'Xvfb :99 -screen 0 8x6x24 -r -nolisten tcp'

            with patch.object(reference, 'inspect', return_value={'State': {'Running': True}}), \
                 patch.object(reference, 'execute', side_effect=execute), \
                 patch.object(replay, 'native_window', return_value={'window': '1'}), \
                 patch.object(replay, 'Browser', SyntheticBrowser), \
                 patch.object(reference, 'capture', side_effect=capture):
                result = replay.compare(args, reference)
            receipt = json.loads((tmp / 'output/comparison.json').read_text())
            checkpoints = {item['name']: item for item in receipt['checkpoints']}
            self.assertEqual(checkpoints['stable']['difference'], checkpoints['moved']['difference'])
            self.assertFalse(checkpoints['stable']['difference']['equal'])
            self.assertEqual(checkpoints['stable']['difference']['changed_bounds'], [3, 2, 4, 3])
            self.assertEqual(checkpoints['stable']['difference']['max_channel_error'], 1)
            self.assertFalse(checkpoints['stable']['state_changed_during_capture'])
            self.assertTrue(checkpoints['moved']['state_changed_during_capture'])
            self.assertEqual(checkpoints['stable']['kind'], 'pixel_mismatch')
            self.assertEqual(checkpoints['moved']['kind'], 'frame_time')
            self.assertNotEqual(checkpoints['moved']['kind'], 'pixel_mismatch')
            self.assertFalse(receipt['observed_images_equal'])
            self.assertFalse(result['images_equal'])
            self.assertFalse(result['deterministic_parity_verified'])
            page = (tmp / 'output/index.html').read_text()
            self.assertIn('pixel_mismatch', page)
            self.assertIn('frame_time', page)
            self.assertIn('state_changed_during_capture=false', page)
            self.assertIn('state_changed_during_capture=true', page)
            self.assertIn('unqualified for stable comparison', page)
            stable_note = json.loads((tmp / 'output/stable/receipt.json').read_text())['note']
            moved_note = json.loads((tmp / 'output/moved/receipt.json').read_text())['note']
            self.assertIn('state_changed_during_capture is false', stable_note)
            self.assertNotIn('unqualified for stable comparison', stable_note)
            self.assertIn('state_changed_during_capture is true', moved_note)
            self.assertIn('unqualified for stable comparison', moved_note)
            self.assertNotIn('state_before', checkpoints['moved'])


if __name__ == '__main__':
    unittest.main()
