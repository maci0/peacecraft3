import json
from pathlib import Path
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

from PIL import Image
import reference as ref
import replay


class ThreeTargetTests(unittest.TestCase):
    def test_browser_timeout_allows_process_cleanup_before_forced_kill(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            marker = root / 'released'
            script = ('import signal,sys,threading\nfrom pathlib import Path\n'
                      'def finish(*unused):\n Path(sys.argv[1]).write_text("released")\n sys.exit(0)\n'
                      'signal.signal(signal.SIGTERM,finish)\nprint("ready",flush=True)\n'
                      'threading.Event().wait()\n')
            browser = replay.Browser.__new__(replay.Browser)
            browser.log = (root / 'stderr').open('wb')
            browser.process = subprocess.Popen([sys.executable, '-c', script, str(marker)],
                stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=browser.log)
            process = browser.process
            try:
                self.assertTrue(replay.select.select([process.stdout], [], [], 5)[0])
                self.assertEqual(process.stdout.readline(), b'ready\n')
                real_wait, limits = process.wait, iter([0.01, 10])
                with patch.object(process, 'wait', side_effect=lambda timeout: real_wait(timeout=next(limits))):
                    browser.close()
                self.assertEqual(marker.read_text(), 'released')
                self.assertEqual(process.returncode, 0)
            finally:
                if process.poll() is None:
                    process.kill(); process.wait(timeout=10)
                for stream in [process.stdin, process.stdout, browser.log]:
                    stream.close()

    def test_report_cli_accepts_native_and_preserves_stdout_contract(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for name in ['wc3', 'native', 'wasm']:
                Image.new('RGB', (8, 6)).save(root / (name + '.png'))
            argv = [sys.executable, str(Path(ref.__file__)), 'report', '--reference', str(root / 'wc3.png'),
                    '--native', str(root / 'native.png'), '--recreation', str(root / 'wasm.png'),
                    '--output', str(root / 'report'), '--note', 'synthetic process control']
            result = subprocess.run(argv, capture_output=True, text=True, timeout=10)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertFalse(result.stderr)
            import jsonschema
            contract = next(c for c in json.loads((Path(ref.__file__).parent / 'schema.json').read_text())['commands'] if c['name'] == 'report')
            jsonschema.Draft202012Validator(contract['stdout_schema']).validate(json.loads(result.stdout))
            before = (root / 'report/receipt.json').read_bytes()
            retried = subprocess.run(argv, capture_output=True, text=True, timeout=10)
            self.assertEqual(retried.returncode, 2)
            self.assertFalse(retried.stdout)
            self.assertEqual((root / 'report/receipt.json').read_bytes(), before)

    def test_three_columns_keep_exact_pixels_and_refuse_different_extents(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            images = []
            for name, color in [('wc3', (10, 20, 30)), ('native', (11, 20, 30)), ('wasm', (10, 20, 30))]:
                path = root / (name + '.png'); Image.new('RGB', (8, 6), color).save(path); images.append(path)
            args = SimpleNamespace(reference=images[0], native=images[1], recreation=images[2],
                                   output=root / 'report', note='synthetic controls only')
            ref.report(args)
            report = (root / 'report/index.html').read_text()
            positions = [report.index(label) for label in ['Original WC3 / Wine', 'PC3 native app', 'PC3 WASM app']]
            self.assertEqual(positions, sorted(positions))
            self.assertEqual(report.count('<figure>'), 3)
            for source, name in zip(images, ['reference.png', 'native.png', 'recreation.png'], strict=True):
                self.assertEqual(source.read_bytes(), (root / 'report' / name).read_bytes())
            receipt = json.loads((root / 'report/receipt.json').read_text())
            self.assertEqual(receipt['targets'], ['original-wc3', 'pc3-native', 'pc3-wasm'])
            Image.new('RGB', (9, 6)).save(images[1]); args.output = root / 'refused'
            with self.assertRaisesRegex(ref.Failure, 'extents differ'):
                ref.report(args)
            self.assertFalse(args.output.exists())

    def test_replay_sends_the_same_journal_to_all_apps_and_checks_every_pair(self):
        self.run_three()

    def test_native_failure_releases_inputs_and_closes_both_adapters(self):
        self.run_three(fail=True)

    def test_mismatched_application_builds_refuse_before_input(self):
        self.run_three(mismatch=True)

    def test_oversized_code_refuses_before_adapters_launch(self):
        self.run_three(oversize=True)

    def run_three(self, fail=False, mismatch=False, oversize=False):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp); run = root / 'original'; (run / 'game').mkdir(parents=True)
            executable = run / 'game/Warcraft III.exe'; executable.write_bytes(b'synthetic executable')
            ref.write_json(run / 'source.json', {'executable_sha256': ref.digest(executable)})
            ref.write_json(run / 'launch.json', {'framebuffer': [80, 60], 'x_dpi': 96})
            manifest = root / 'code.json'
            ref.write_json(manifest, {'files': {'fixture.js': {'bytes': replay.MAX_CODE_BYTES + 1 if oversize else 1, 'sha256': 'f' * 64}}, 'source_head': 'a' * 40, 'source_tree_sha256': 'b' * 64})
            build = root / 'native-build.json'; ref.write_json(build, {'source_head': 'a' * 40, 'source_tree_sha256': ('c' if mismatch else 'b') * 64})
            ref.write_json(root / 'native.json', {'build_receipt': {'path': str(build)}})
            iso = root / 'fixture.iso'; iso.write_bytes(b'synthetic source')
            module = root / 'fixture.mjs'; module.write_text('')
            events = [{'kind': 'checkpoint', 'after_ms': 0, 'name': 'start'},
                      {'kind': 'key', 'phase': 'down', 'key': 'Tab', 'after_ms': 0},
                      {'kind': 'key', 'phase': 'up', 'key': 'Tab', 'after_ms': 0},
                      {'kind': 'checkpoint', 'after_ms': 0, 'name': 'end'}]
            scenario = root / 'scenario.json'; ref.write_json(scenario, {'version': 1, 'edition': 'roc',
                'extent': [80, 60], 'dpi': 96, 'window_title': 'Synthetic WC3', 'seed': None, 'events': events})
            args = SimpleNamespace(scenario=scenario, run_dir=run, url='http://127.0.0.1:1/index.html',
                manifest=manifest, iso=iso, playwright_module=module, chromium='synthetic',
                native_config=root / 'native.json', output=root / 'report')
            native_inputs, wasm_inputs, original_commands, closed = [], [], [], []
            class Browser:
                def __init__(self, *unused): pass
                def receive(self, *unused): return {'data': {'target': 'pc3-wasm'}}
                def request(self, op, **kw):
                    if op == 'input': wasm_inputs.append(kw['event']); return {}
                    if op == 'capture':
                        path = Path(kw['path']); Image.new('RGB', (80, 60), (10, 20, 30)).save(path)
                        return {'path': str(path), 'state_changed_during_capture': False}
                    return {}
                def close(self): closed.append('wasm')
            class Native:
                def __init__(self, *unused): self.start = {'target': 'pc3-native', 'process_checked': True}
                def input(self, event):
                    native_inputs.append(event)
                    if fail: raise ref.Failure('synthetic native input failure')
                    return {}
                def capture(self, path):
                    Image.new('RGB', (80, 60), (11, 20, 30)).save(path)
                    return {'path': str(path), 'sha256': ref.digest(Path(path)), 'state_changed_during_capture': None}
                def close(self): closed.append('native')
            def capture(unused):
                path = root / 'original.png'; Image.new('RGB', (80, 60), (10, 20, 30)).save(path)
                return {'capture': str(path), 'sha256': ref.digest(path)}
            def execute(unused, argv):
                original_commands.append(argv)
                return 'Xvfb :99 -screen 0 80x60x24 -r -nolisten tcp'
            with patch.dict(sys.modules, {'native_app': SimpleNamespace(NativeApp=Native)}), \
                 patch.object(ref, 'inspect', return_value={'State': {'Running': True}}), \
                 patch.object(ref, 'execute', side_effect=execute), patch.object(ref, 'capture', side_effect=capture), \
                 patch.object(replay, 'native_window', return_value={}), patch.object(replay, 'Browser', Browser):
                if oversize:
                    with self.assertRaisesRegex(ref.Failure, '0..72 MiB'):
                        replay.compare(args, ref)
                    self.assertFalse(native_inputs or wasm_inputs)
                    self.assertFalse(args.output.exists())
                elif mismatch:
                    with self.assertRaisesRegex(ref.Failure, 'source head and tree'):
                        replay.compare(args, ref)
                    self.assertFalse(native_inputs or wasm_inputs)
                    self.assertFalse(any(command[0] == 'xdotool' for command in original_commands))
                elif fail:
                    with self.assertRaisesRegex(ref.Failure, 'native input failure'):
                        replay.compare(args, ref)
                    self.assertIn(['xdotool', 'keyup', 'Tab'], original_commands)
                else:
                    result = replay.compare(args, ref)
                    self.assertFalse(result['images_equal'])  # WC3 and WASM match; native alone differs.
                    self.assertFalse(result['deterministic_parity_verified'])
                    self.assertEqual(native_inputs, [e for e in events if e['kind'] == 'key'])
                    self.assertEqual(native_inputs, wasm_inputs)
                    receipt = json.loads((root / 'report/comparison.json').read_text())
                    self.assertEqual(receipt['targets'], ['original-wc3', 'pc3-native', 'pc3-wasm'])
                    for checkpoint in receipt['checkpoints']:
                        pairs = checkpoint['pairwise_difference']
                        self.assertFalse(pairs['wc3_vs_pc3_native']['equal'])
                        self.assertTrue(pairs['wc3_vs_pc3_wasm']['equal'])
                        self.assertFalse(pairs['pc3_native_vs_pc3_wasm']['equal'])
                        self.assertIsNone(checkpoint['native_state_changed_during_capture'])
            self.assertCountEqual(closed, [] if oversize else ['native'] if mismatch else ['native', 'wasm'])


if __name__ == '__main__':
    unittest.main()
