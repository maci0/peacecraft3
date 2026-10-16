import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location('reference', HERE / 'reference.py')
ref = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ref)


class ReferenceTests(unittest.TestCase):
    def invoke(self, *args):
        return subprocess.run([sys.executable, str(HERE / 'reference.py'), *args],
                              input='', capture_output=True, text=True, timeout=10)

    def test_schema_offline_and_published_pin(self):
        result = self.invoke('--format', 'json', 'schema')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(result.stderr)
        schema = json.loads(result.stdout)
        import jsonschema
        published = json.loads((HERE / 'schema/clispec-v0.3.json').read_text())
        jsonschema.Draft202012Validator(published).validate(schema)
        self.assertEqual(schema['clispec'], '0.3')
        self.assertEqual(len(schema['commands']), 10)
        self.assertIn('compare', [command['name'] for command in schema['commands']])
        sha = hashlib.sha256((HERE / 'schema/clispec-v0.3.json').read_bytes()).hexdigest()
        self.assertIn(f'candidate  {sha}  schema/v0.3.json', (HERE / 'schema/CHECKSUMS.txt').read_text())

    def test_usage_failure_is_json_stderr_with_exit_two(self):
        for args in [[], ['unknown'], ['launch', '--width', '0'], ['--format', 'yaml', 'schema']]:
            result = self.invoke(*args)
            self.assertEqual(result.returncode, 2)
            self.assertFalse(result.stdout)
            self.assertEqual(json.loads(result.stderr)['error']['kind'], 'reference_failed')

    def test_prepare_copy_hash_and_refuse_retry_without_overwriting(self):
        with tempfile.TemporaryDirectory() as tmp:
            source, target = Path(tmp) / 'source', Path(tmp) / 'private'
            source.mkdir(); (source / 'Warcraft III.exe').write_bytes(b'synthetic PE fixture')
            (source / 'sample.mpq').write_bytes(b'synthetic archive fixture')
            result = self.invoke('prepare', '--source', str(source), '--run-dir', str(target))
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(json.loads(result.stdout)['file_count'], 2)
            before = (target / 'source.json').read_bytes()
            result = self.invoke('prepare', '--source', str(source), '--run-dir', str(target))
            self.assertEqual(result.returncode, 2)
            self.assertEqual((target / 'source.json').read_bytes(), before)
            self.assertEqual((target / 'game/sample.mpq').read_bytes(), (source / 'sample.mpq').read_bytes())

    def test_prepare_refuses_links_and_nested_destination(self):
        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp) / 'source'; source.mkdir()
            (source / 'Warcraft III.exe').write_bytes(b'synthetic')
            (source / 'link').symlink_to('/etc/passwd')
            result = self.invoke('prepare', '--source', str(source), '--run-dir', str(Path(tmp) / 'private'))
            self.assertEqual(result.returncode, 2)
            self.assertFalse((Path(tmp) / 'private').exists())
            (source / 'link').unlink()
            result = self.invoke('prepare', '--source', str(source), '--run-dir', str(source / 'nested'))
            self.assertEqual(result.returncode, 2)

    def test_process_ownership_refusal_and_missing_container(self):
        completed = subprocess.CompletedProcess([], 0, json.dumps([{'Config': {'Labels': {}}}]).encode(), b'')
        with patch.object(ref.subprocess, 'run', return_value=completed):
            with self.assertRaisesRegex(ref.Failure, 'ownership'): ref.inspect(Path('/private'))
        completed = subprocess.CompletedProcess([], 1, b'', b'error: no such object: example')
        with patch.object(ref.subprocess, 'run', return_value=completed), patch.object(ref, 'command'):
            self.assertIsNone(ref.inspect(Path('/private')))

    def test_command_bound_and_failure(self):
        for completed in [subprocess.CompletedProcess([], 0, b'x' * (ref.MAX_METADATA + 1), b''),
                          subprocess.CompletedProcess([], 3, b'', b'failure')]:
            with patch.object(ref.subprocess, 'run', return_value=completed):
                with self.assertRaises(ref.Failure): ref.command(['synthetic'])

    def test_launch_is_isolated_and_safe_to_repeat_while_running(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp); (path / 'source.json').write_text('{}')
            args = argparse.Namespace(run_dir=tmp, width=640, height=480, dpi=96, game_arg=['-window'])
            with patch.object(ref, 'inspect', return_value=None), patch.object(ref, 'command', side_effect=['sha256:fixture', 'container-fixture']) as call:
                ref.launch(args)
                argv = call.call_args_list[1].args[0]
                self.assertEqual(argv[argv.index('--network') + 1], 'none')
                self.assertIn('--read-only', argv)
                self.assertIn('no-new-privileges', argv)
                self.assertNotIn('--privileged', argv)
                self.assertEqual(argv[-1], '-window')
            state = {'State': {'Running': True}, 'Config': {
                'Env': ['REFERENCE_WIDTH=640', 'REFERENCE_HEIGHT=480', 'REFERENCE_DPI=96'],
                'Cmd': ['sh', '/harness/session.sh', '-window']}}
            with patch.object(ref, 'inspect', return_value=state), patch.object(ref, 'command') as call:
                self.assertTrue(ref.launch(args)['already_running'])
                call.assert_not_called()

    def test_running_launch_refuses_changed_calibration_or_game_arguments(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp); (path / 'source.json').write_text('{}')
            args = argparse.Namespace(run_dir=tmp, width=800, height=600, dpi=96, game_arg=[])
            state = {'State': {'Running': True}, 'Config': {
                'Env': ['REFERENCE_WIDTH=800', 'REFERENCE_HEIGHT=600', 'REFERENCE_DPI=96'],
                'Cmd': ['sh', '/harness/session.sh']}}
            for field, changed in [('width', 1024), ('height', 480), ('dpi', 120), ('game_arg', ['-window'])]:
                original = getattr(args, field)
                setattr(args, field, changed)
                with patch.object(ref, 'inspect', return_value=state), patch.object(ref, 'command') as call:
                    with self.assertRaisesRegex(ref.Failure, 'different'):
                        ref.launch(args)
                    call.assert_not_called()
                setattr(args, field, original)

    def test_failed_launch_receipt_is_not_overwritten_by_retry(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp); (path / 'source.json').write_text('{}')
            prior = b'{"previous":"failed launch attempt"}\n'
            (path / 'launch.json').write_bytes(prior)
            args = argparse.Namespace(run_dir=tmp, width=800, height=600, dpi=96, game_arg=[])
            with patch.object(ref, 'inspect', return_value=None), patch.object(ref, 'command', side_effect=['sha256:fixture', 'container-fixture']) as call:
                with self.assertRaisesRegex(ref.Failure, 'prior launch'):
                    ref.launch(args)
                call.assert_not_called()
            self.assertEqual((path / 'launch.json').read_bytes(), prior)

    def test_report_equal_extent_and_refuse_rescaling(self):
        from PIL import Image
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp); a, b = tmp / 'a.png', tmp / 'b.png'
            Image.new('RGB', (8, 6), 'red').save(a); Image.new('RGB', (8, 6), 'blue').save(b)
            result = self.invoke('report', '--reference', str(a), '--recreation', str(b),
                                 '--output', str(tmp / 'report'), '--note', '<synthetic>')
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn('&lt;synthetic&gt;', (tmp / 'report/index.html').read_text())
            Image.new('RGB', (9, 6)).save(b)
            result = self.invoke('report', '--reference', str(a), '--recreation', str(b),
                                 '--output', str(tmp / 'mismatch'), '--note', 'fixture')
            self.assertEqual(result.returncode, 2)
            self.assertFalse((tmp / 'mismatch').exists())


if __name__ == '__main__':
    unittest.main()
