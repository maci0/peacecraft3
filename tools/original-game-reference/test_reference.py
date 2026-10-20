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
    def invoke(self, *args, env=None, timeout=10):
        return subprocess.run([sys.executable, str(HERE / 'reference.py'), *args],
                              input='', capture_output=True, text=True, timeout=timeout, env=env)

    def test_schema_offline_and_published_pin(self):
        result = self.invoke('--format', 'json', 'schema')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(result.stderr)
        schema = json.loads(result.stdout)
        import jsonschema
        published = json.loads((HERE / 'schema/clispec-v0.3.json').read_text())
        jsonschema.Draft202012Validator(published).validate(schema)
        self.assertEqual(schema['clispec'], '0.3')
        self.assertEqual(len(schema['commands']), 13)
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
        missing = subprocess.CompletedProcess([], 1, b'', b'error: no such object: example')
        info = subprocess.CompletedProcess([], 0, b'24.0.0', b'')
        with patch.object(ref.subprocess, 'run', side_effect=[missing, info]) as run:
            self.assertIsNone(ref.inspect(Path('/private')))
            self.assertEqual(run.call_args_list[1].args[0][:2], ['docker', 'info'])

    def test_status_refuses_inspect_output_over_protocol_bytes(self):
        schema = json.loads((HERE / 'schema.json').read_text())
        self.assertEqual(schema['clispec'], '0.3')
        self.assertEqual(schema['extensions']['collection_limits']['protocol_bytes'], ref.MAX_METADATA)
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            run = root / 'run'
            run.mkdir()
            (run / 'source.json').write_text('{}\n')
            resolved = str(run.resolve())
            bindir = root / 'bin'
            bindir.mkdir()
            docker = bindir / 'docker'
            docker.write_text(
                f'#!{sys.executable}\n'
                'import os, sys\n'
                'from pathlib import Path\n'
                'stream = sys.stderr if os.environ.get("DOCKER_STREAM") == "stderr" else sys.stdout\n'
                'stream.buffer.write(Path(os.environ["DOCKER_INSPECT_OUT"]).read_bytes())\n'
                'sys.exit(int(os.environ.get("DOCKER_EXIT", "0")))\n'
            )
            docker.chmod(0o755)
            oversized = root / 'oversized.json'
            small = root / 'small.json'
            pad = 'p' * (ref.MAX_METADATA + 1)
            document = {'Config': {'Labels': {ref.LABEL: resolved}}, 'State': {'Running': False, 'pad': pad}}
            oversized.write_text(json.dumps([document]))
            self.assertGreater(oversized.stat().st_size, ref.MAX_METADATA)
            small.write_text(json.dumps([{'Config': {'Labels': {ref.LABEL: resolved}}, 'State': {'Running': False}}]))
            env = os.environ.copy()
            env['PATH'] = str(bindir) + os.pathsep + env.get('PATH', '')
            env['DOCKER_INSPECT_OUT'] = str(oversized)
            for stream in ['stdout', 'stderr']:
                for exit_code in ['0', '1']:
                    with self.subTest(stream=stream, exit_code=exit_code):
                        env.update(DOCKER_STREAM=stream, DOCKER_EXIT=exit_code)
                        result = self.invoke('status', '--run-dir', resolved, env=env, timeout=30)
                        self.assertEqual(result.returncode, 2, result.stderr)
                        self.assertFalse(result.stdout)
                        error = json.loads(result.stderr)['error']
                        self.assertEqual(error['kind'], 'reference_failed')
                        self.assertIn('exceeds 2 MiB', error['message'])
                        self.assertFalse(error['retryable'])
                        self.assertNotIn(pad[:64], result.stdout)
                        self.assertNotIn(pad[:64], result.stderr)
            env.update(DOCKER_STREAM='stdout', DOCKER_EXIT='0')
            env['DOCKER_INSPECT_OUT'] = str(small)
            result = self.invoke('status', '--run-dir', resolved, env=env, timeout=30)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertFalse(result.stderr)
            self.assertEqual(json.loads(result.stdout)['state'], {'Running': False})

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

    def replay_run(self, tmp, payload=b'synthetic replay fixture', user='tester', docs='Documents'):
        run = Path(tmp) / 'run'
        run.mkdir()
        (run / 'source.json').write_text(json.dumps({'executable_sha256': 'a' * 64}))
        replay = run / 'prefix/drive_c/users' / user / docs / ref.REPLAY_RELATIVE
        replay.parent.mkdir(parents=True)
        replay.write_bytes(payload)
        return run

    def test_export_replay_copies_the_single_candidate_with_a_receipt(self):
        with tempfile.TemporaryDirectory() as tmp:
            run = self.replay_run(tmp)
            args = argparse.Namespace(run_dir=str(run), label='sample-1')
            with patch.object(ref, 'inspect', return_value=None):
                result = ref.export_replay(args)
                self.assertEqual(set(result), {'replay', 'sha256', 'bytes'})
                self.assertEqual((run / 'replays/sample-1.w3g').read_bytes(), b'synthetic replay fixture')
                self.assertEqual(result['bytes'], len(b'synthetic replay fixture'))
                self.assertEqual(result['sha256'], hashlib.sha256(b'synthetic replay fixture').hexdigest())
                receipt = json.loads((run / 'replays/sample-1.json').read_text())
                self.assertEqual(receipt['executable_sha256'], 'a' * 64)
                self.assertEqual(receipt['source'], 'prefix/drive_c/users/tester/Documents/Warcraft III/Replay/LastReplay.w3g')
                with self.assertRaisesRegex(ref.Failure, 'already exists'):
                    ref.export_replay(args)

    def test_export_replay_refuses_ambiguous_linked_oversized_and_running_sources(self):
        with tempfile.TemporaryDirectory() as tmp:
            run = self.replay_run(tmp)
            args = argparse.Namespace(run_dir=str(run), label='sample-2')
            source = run / 'prefix/drive_c/users/tester/Documents' / ref.REPLAY_RELATIVE
            with patch.object(ref, 'inspect', return_value=None):
                other = run / 'prefix/drive_c/users/other/My Documents' / ref.REPLAY_RELATIVE
                other.parent.mkdir(parents=True)
                other.write_bytes(b'second')
                with self.assertRaisesRegex(ref.Failure, 'found 2'):
                    ref.export_replay(args)
                other.unlink()
                source.unlink()
                source.symlink_to('/etc/passwd')
                with self.assertRaisesRegex(ref.Failure, 'not a regular file'):
                    ref.export_replay(args)
                source.unlink()
                source.write_bytes(b'x' * (ref.MAX_REPLAY + 1))
                with self.assertRaisesRegex(ref.Failure, 'exceeds'):
                    ref.export_replay(args)
                self.assertFalse((run / 'replays').exists())
            with patch.object(ref, 'inspect', return_value={'State': {'Running': True}}), \
                    self.assertRaisesRegex(ref.Failure, 'stop the reference run'):
                ref.export_replay(args)

    def test_export_replay_usage_failure_is_json_stderr_with_exit_two(self):
        with tempfile.TemporaryDirectory() as tmp:
            run = self.replay_run(tmp)
            result = self.invoke('export-replay', '--run-dir', str(run), '--label', 'bad label!')
            self.assertEqual(result.returncode, 2)
            self.assertFalse(result.stdout)
            self.assertEqual(json.loads(result.stderr)['error']['kind'], 'reference_failed')

    def test_export_replay_process_writes_only_json_to_stdout(self):
        with tempfile.TemporaryDirectory() as tmp:
            run = self.replay_run(tmp)
            bindir = Path(tmp) / 'bin'
            bindir.mkdir()
            docker = bindir / 'docker'
            docker.write_text(
                f'#!{sys.executable}\n'
                'import sys\n'
                'if sys.argv[1] == "info":\n'
                '    sys.exit(0)\n'
                'sys.stderr.write("Error: No such object: fixture\\n")\n'
                'sys.exit(1)\n'
            )
            docker.chmod(0o755)
            env = os.environ.copy()
            env['PATH'] = str(bindir) + os.pathsep + env.get('PATH', '')
            result = self.invoke('export-replay', '--run-dir', str(run), '--label', 'process-1', env=env)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertFalse(result.stderr)
            output = json.loads(result.stdout)
            self.assertEqual(output['bytes'], len(b'synthetic replay fixture'))
            self.assertEqual((run / 'replays/process-1.w3g').read_bytes(), b'synthetic replay fixture')

    def test_export_replay_schema_matches_its_arguments_and_output(self):
        schema = json.loads((HERE / 'schema.json').read_text())
        command = next(c for c in schema['commands'] if c['name'] == 'export-replay')
        self.assertEqual([a['name'] for a in command['args']], ['--run-dir', '--label'])
        self.assertEqual(sorted(command['stdout_schema']['required']), ['bytes', 'replay', 'sha256'])


if __name__ == '__main__':
    unittest.main()
