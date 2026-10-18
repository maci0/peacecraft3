import copy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

import rng_fixture as rng

HERE = Path(__file__).resolve().parent


def scenario():
    return {'version': 1, 'nonce': 'synthetic', 'steps': [
        {'op': 'seed', 'value': 7}, {'op': 'integer', 'low': -9, 'high': 13},
        {'op': 'real', 'low': -1, 'high': 1}]}


def observation(plan, values):
    key = rng.digest(rng.encoded(plan))
    lines = [f'PCX_RNG1 {key} begin {len(values)}']
    for index, (step, value) in enumerate(zip(plan['steps'], values, strict=True)):
        a, b = (step['value'], 0) if step['op'] == 'seed' else (step['low'], step['high'])
        lines.append(f'PCX_RNG1 {key} {index} {step["op"]} {a} {b} {value}')
    return ('\n'.join(lines + [f'PCX_RNG1 {key} end {len(values)}']) + '\n').encode()


class RngFixtureTests(unittest.TestCase):
    def invoke(self, *args):
        return subprocess.run([sys.executable, str(HERE / 'reference.py'), *map(str, args)],
                              input='', capture_output=True, text=True, timeout=10)

    def setup_runs(self, root):
        plan = scenario()
        source = root / 'scenario.json'
        source.write_bytes(rng.encoded(plan))
        fixture = root / 'fixture'
        result = self.invoke('rng-plan', '--scenario', source, '--output', fixture)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(result.stderr)
        for name in ['image', 'common', 'execution']:
            (root / name).write_bytes(b'synthetic ' + name.encode())
        (root / 'execution').write_bytes(rng.encoded({'exit_code': 0, 'complete': True,
            'records': 3, 'observation_sha256': rng.digest(observation(plan, [7, -3, -1090519040]))}))
        receipts = []
        for target in ['original', 'recreation']:
            for run in range(2):
                trace = root / f'{target}-{run}.txt'
                trace.write_bytes(observation(plan, [7, -3, -1090519040]))
                paths = {'image': root / 'image', 'common': root / 'common',
                         'probe': fixture / 'probe.j', 'observation': trace,
                         'execution': root / 'execution'}
                record = {'version': 1, 'target': target, 'run_id': f'{target}-{run}',
                          'runtime': 'synthetic fixture only',
                          'files': {name: {'path': str(path), 'sha256': rng.digest(path.read_bytes())}
                                    for name, path in paths.items()}}
                path = root / f'{target}-{run}.json'
                path.write_bytes(rng.encoded(record))
                receipts.append(path)
        return fixture, receipts

    def compare(self, fixture, receipts, output):
        return self.invoke('rng-compare', '--fixture', fixture, '--original', receipts[0],
                           '--original', receipts[1], '--recreation', receipts[2],
                           '--recreation', receipts[3], '--output', output)

    def test_plan_validation_and_canonical_generation(self):
        self.assertEqual(rng.validate(scenario()), scenario())
        first = rng.probe(scenario())
        self.assertEqual(first, rng.probe(json.loads(rng.encoded(scenario()))))
        self.assertIn('call SetRandomSeed(7)', first[1])
        self.assertLess(first[1].index('set PCXRngValues[2]'), first[1].index('call PreloadGenStart'))
        self.assertIn('or exponent < 1', first[1])
        self.assertIn('value != 0.0', first[1])
        mutations = [lambda p: p.update(version=True), lambda p: p.update(unknown=1),
                     lambda p: p.update(nonce='../outside'), lambda p: p.update(steps=[]),
                     lambda p: p['steps'].pop(0), lambda p: p['steps'][0].update(value=True),
                     lambda p: p['steps'][0].update(value=2**31),
                     lambda p: p['steps'][2].update(low=-0.5),
                     lambda p: p['steps'][2].update(high=2),
                     lambda p: p['steps'][2].update(low=1, high=-1),
                     lambda p: p['steps'][2].update(low=0, high=0),
                     lambda p: p.update(steps=p['steps'] * 100)]
        for mutation in mutations:
            plan = copy.deepcopy(scenario()); mutation(plan)
            with self.subTest(plan=plan), self.assertRaises(ValueError):
                rng.validate(plan)

    def test_records_refuse_wrong_order_arguments_seed_and_incomplete_exports(self):
        plan = scenario()
        raw = observation(plan, [7, -3, -1090519040])
        wrapped = b'// native preload export\n' + b'\n'.join(b'call Preload("' + line + b'")' for line in raw.splitlines())
        self.assertEqual(rng.records(wrapped, plan), [7, -3, -1090519040])
        rows = raw.splitlines()
        mutants = [b'\n'.join(rows[:-1]), raw + rows[1] + b'\n',
                   raw.replace(b' 0 seed 7 0 7', b' 0 seed 7 0 8'),
                   raw.replace(b' 1 integer -9 13', b' 1 integer -9 14'),
                   raw.replace(b' 1 integer', b' 2 integer'),
                   raw.replace(b' -3\n', b' -03\n'),
                   raw.replace(b' begin 3', b' begin 4'),
                   raw.replace(rng.digest(rng.encoded(plan)).encode(), b'0' * 64)]
        for mutant in mutants:
            with self.subTest(mutant=mutant), self.assertRaises(ValueError):
                rng.records(mutant, plan)

    def test_process_success_difference_and_safe_retry(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp); fixture, receipts = self.setup_runs(root)
            result = self.compare(fixture, receipts, root / 'match')
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertFalse(result.stderr)
            report = json.loads(result.stdout)
            import jsonschema
            contract = json.loads((HERE / 'schema.json').read_text())
            command = next(c for c in contract['commands'] if c['name'] == 'rng-compare')
            jsonschema.Draft202012Validator(command['stdout_schema']).validate(report)
            self.assertTrue(report['rng_results_match'])
            self.assertFalse(report['deterministic_parity_verified'])
            self.assertEqual(report['runs'], 4)
            before = Path(report['receipt']).read_bytes()
            retried = self.compare(fixture, receipts, root / 'match')
            self.assertEqual(retried.returncode, 2)
            self.assertFalse(retried.stdout)
            self.assertEqual(Path(report['receipt']).read_bytes(), before)
            record = json.loads(receipts[-1].read_text())
            trace = Path(record['files']['observation']['path'])
            trace.write_bytes(trace.read_bytes().replace(b' -1090519040\n', b' -1090519039\n'))
            record['files']['observation']['sha256'] = rng.digest(trace.read_bytes())
            receipts[-1].write_bytes(rng.encoded(record))
            result = self.compare(fixture, receipts, root / 'difference')
            self.assertEqual(result.returncode, 1, result.stderr)
            self.assertFalse(result.stderr)
            self.assertEqual(json.loads(result.stdout)['first_divergence']['step'], 2)

    def test_process_refuses_provenance_and_shape_mutants(self):
        mutations = [lambda r: r.update(target='other'), lambda r: r.update(version=True),
                     lambda r: r.update(run_id='original-0'), lambda r: r.update(runtime=''),
                     lambda r: r['files']['probe'].update(sha256='0' * 64),
                     lambda r: r['files']['image'].update(path=[]),
                     lambda r: r['files'].pop('execution')]
        for index, mutation in enumerate(mutations):
            with tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp); fixture, receipts = self.setup_runs(root)
                record = json.loads(receipts[-1].read_text()); mutation(record)
                receipts[-1].write_bytes(rng.encoded(record))
                result = self.compare(fixture, receipts, root / 'refused')
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertFalse(result.stdout)
                self.assertFalse(json.loads(result.stderr)['error']['retryable'])
                self.assertFalse((root / 'refused').exists())

    def test_input_bound_duplicate_fields_and_existing_plan_refuse(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp); path = root / 'plan.json'
            for raw in [b'{"version":1,"version":1}', b' ' * ((2 << 20) + 1)]:
                path.write_bytes(raw)
                result = self.invoke('rng-plan', '--scenario', path, '--output', root / 'fixture')
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertFalse(result.stdout)
                self.assertFalse((root / 'fixture').exists())
            path.write_bytes(rng.encoded(scenario()))
            created = self.invoke('rng-plan', '--scenario', path, '--output', root / 'fixture')
            self.assertEqual(created.returncode, 0, created.stderr)
            before = (root / 'fixture' / 'fixture.json').read_bytes()
            retried = self.invoke('rng-plan', '--scenario', path, '--output', root / 'fixture')
            self.assertEqual(retried.returncode, 2, retried.stderr)
            self.assertFalse(retried.stdout)
            self.assertEqual((root / 'fixture' / 'fixture.json').read_bytes(), before)

    def test_failed_execution_reused_observation_and_foreign_image_refuse(self):
        for mutation in ['failed', 'reused', 'foreign']:
            with tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp); fixture, receipts = self.setup_runs(root)
                record = json.loads(receipts[-1].read_text())
                if mutation == 'failed':
                    path = root / 'failed-execution.json'
                    path.write_bytes(rng.encoded({'exit_code': 101}))
                    record['files']['execution'] = {'path': str(path), 'sha256': rng.digest(path.read_bytes())}
                elif mutation == 'reused':
                    record['files']['observation'] = json.loads(receipts[0].read_text())['files']['observation']
                else:
                    path = root / 'foreign-image'
                    path.write_bytes(b'other synthetic version')
                    record['files']['image'] = {'path': str(path), 'sha256': rng.digest(path.read_bytes())}
                receipts[-1].write_bytes(rng.encoded(record))
                result = self.compare(fixture, receipts, root / 'refused')
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertFalse(result.stdout)
                self.assertFalse((root / 'refused').exists())


if __name__ == '__main__':
    unittest.main()
