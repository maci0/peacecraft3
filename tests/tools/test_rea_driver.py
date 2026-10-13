"""Exercise driver argument routing without invoking native analysis."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


class DriverArguments(unittest.TestCase):
    def test_target_snapshot_and_options_survive_cold_and_cached_queries(self):
        driver = Path(__file__).resolve().parents[2] / 'tools/rea.sh'
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'tools').mkdir()
            shutil.copy(driver, root / 'tools/rea.sh')
            target = root / 'target with spaces.exe'
            target.write_bytes(b'synthetic target identity')
            snapshot = root / '.rea/snapshots' / (hashlib.sha256(target.read_bytes()).hexdigest() + '.snap')
            log = root / 'calls.jsonl'
            stub = root / 'bunx'
            stub.write_text('''#!/usr/bin/env python3
import json, os, pathlib, sys
args = sys.argv[1:]
with open(os.environ['DRIVER_LOG'], 'a') as output:
    output.write(json.dumps(args) + '\\n')
if args[1] == 'inspect':
    pathlib.Path(args[args.index('--snapshot') + 1]).touch()
''')
            stub.chmod(0o755)
            env = {**os.environ, 'PATH': f'{root}:{os.environ["PATH"]}', 'DRIVER_LOG': str(log)}
            cases = [('search', 'a pattern', ['--kind', 'strings']),
                     ('function', '0x1234', ['--json']),
                     ('trace', 'a query', ['--json']),
                     ('xrefs', '0x4321', ['--json']),
                     ('inspect', None, ['--json'])]
            for command, operand, options in cases:
                for cached in (False, True):
                    with self.subTest(command=command, cached=cached):
                        snapshot.parent.mkdir(parents=True, exist_ok=True)
                        snapshot.unlink(missing_ok=True)
                        if cached:
                            snapshot.touch()
                        log.unlink(missing_ok=True)
                        args = [str(root / 'tools/rea.sh'), command, str(target)]
                        if operand is not None:
                            args.append(operand)
                        subprocess.run(args + options, env=env, check=True, capture_output=True)
                        calls = [json.loads(line) for line in log.read_text().splitlines()]
                        expected = ['rea-agents', command, str(target)]
                        if operand is not None:
                            expected.append(operand)
                        expected += ['--snapshot', str(snapshot), *options]
                        self.assertEqual(calls[-1], expected)
                        self.assertEqual(len(calls), 1 if cached else 2)
                        if not cached:
                            self.assertEqual(calls[0], ['rea-agents', 'inspect', str(target), '--snapshot', str(snapshot), '--json'])


if __name__ == '__main__':
    unittest.main()
