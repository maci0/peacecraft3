"""Contract tests for the asset performance budget tool.

The tool decides whether a change is a performance regression, so its own
decision logic is checked here: what counts as a budget violation, what
`lower` may and may not change, what `compare` calls a regression, and how the
command refuses bad input. Every case is driven by synthetic result documents:
no original assets, no `perf` and no measurement are needed.

    python3 -m unittest discover -s tools/perf -p 'test_*.py'
"""

import contextlib
import importlib.util
import io
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location('perf_assets', HERE / 'assets.py')
assets = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(assets)


def report(workload='demo', **overrides):
    """One measured workload, as `run` writes it."""
    base = {
        'workload': workload,
        'input_digest': 'in' * 16,
        'output_digest': 'out' * 16,
        'work': 100,
        'allocations': 10,
        'reallocations': 5,
        'allocated_bytes': 1000,
        'instructions_user': 1234,
    }
    base.update(overrides)
    return base


def budget(**overrides):
    """One budgeted workload, as `budgets/assets.json` stores it."""
    base = {
        'input_digest': 'in' * 16,
        'output_digest': 'out' * 16,
        'work': 100,
        'allocation_calls': 15,
        'allocated_bytes': 1000,
    }
    base.update(overrides)
    return base


def document(*reports):
    return {'results': list(reports), 'environment': {'rustc': 'rustc 1.99.1'}}


def capture(function, *args):
    """Run a command handler and return its exit code with what it printed."""
    out, err = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
        code = function(*args)
    return code, out.getvalue(), err.getvalue()


class BudgetLogicTests(unittest.TestCase):
    def test_a_matching_workload_has_no_violation(self):
        self.assertEqual(assets.violations('demo', budget(), report()), [])

    def test_a_changed_digest_or_work_is_a_violation(self):
        for field, value in (('input_digest', 'other'), ('output_digest', 'other'), ('work', 99)):
            with self.subTest(field=field):
                found = assets.violations('demo', budget(), report(**{field: value}))
                self.assertEqual(len(found), 1, found)
                self.assertIn(field, found[0])

    def test_more_allocations_are_a_violation_and_fewer_are_not(self):
        more = assets.violations('demo', budget(), report(allocations=11))
        self.assertEqual(len(more), 1, more)
        self.assertIn('allocation_calls', more[0])
        self.assertEqual(assets.violations('demo', budget(), report(allocations=1)), [])


class LowerTests(unittest.TestCase):
    def lower(self, budgets, result_document):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'assets.json'
            path.write_text(json.dumps(budgets))
            result = Path(tmp) / 'result.json'
            result.write_text(json.dumps(result_document))
            with patch.object(assets, 'BUDGETS', path):
                code, out, err = capture(assets.command_lower,
                                         type('Args', (), {'result': str(result)})())
            return code, out, err, json.loads(path.read_text())

    def test_lower_refuses_to_raise_a_counter(self):
        code, _, err, stored = self.lower(
            {'version': assets.BUDGET_VERSION, 'workloads': {'demo': budget()}},
            document(report(allocations=11)),
        )
        self.assertEqual(code, 1)
        self.assertIn('would rise', err)
        self.assertEqual(stored['workloads']['demo']['allocation_calls'], 15)

    def test_lower_refuses_a_changed_digest_or_work(self):
        for field, value in (('input_digest', 'other'), ('output_digest', 'other'), ('work', 99)):
            with self.subTest(field=field):
                code, _, err, stored = self.lower(
                    {'version': assets.BUDGET_VERSION, 'workloads': {'demo': budget()}},
                    document(report(**{field: value})),
                )
                self.assertEqual(code, 1)
                self.assertIn(field, err)
                self.assertEqual(stored['workloads']['demo'][field], budget()[field])

    def test_lower_lowers_a_counter_and_records_the_environment(self):
        code, out, _, stored = self.lower(
            {'version': assets.BUDGET_VERSION, 'workloads': {'demo': budget()}},
            document(report(allocations=1, reallocations=0)),
        )
        self.assertEqual(code, 0, out)
        self.assertEqual(stored['workloads']['demo']['allocation_calls'], 1)
        self.assertEqual(stored['rustc'], 'rustc 1.99.1')


class CompareTests(unittest.TestCase):
    def compare(self, before, after, tolerance=2.0):
        with tempfile.TemporaryDirectory() as tmp:
            left = Path(tmp) / 'before.json'
            right = Path(tmp) / 'after.json'
            left.write_text(json.dumps(before))
            right.write_text(json.dumps(after))
            args = type('Args', (), {'before': str(left), 'after': str(right),
                                     'tolerance': tolerance})()
            return capture(assets.command_compare, args)

    def test_an_equal_pair_is_unchanged(self):
        code, _, err = self.compare(document(report()), document(report()))
        self.assertEqual(code, 0, err)
        self.assertEqual(err, '')

    def test_a_changed_output_digest_or_work_is_a_regression(self):
        for field, message in (('output_digest', 'output digest changed'), ('work', 'work')):
            with self.subTest(field=field):
                code, _, err = self.compare(document(report()),
                                            document(report(**{field: 'other' if field == 'output_digest' else 99})))
                self.assertEqual(code, 1)
                self.assertIn(message, err)

    def test_more_allocation_calls_are_a_regression(self):
        code, _, err = self.compare(document(report()), document(report(allocations=11)))
        self.assertEqual(code, 1)
        self.assertIn('allocation calls increased', err)

    def test_different_inputs_are_not_a_comparison(self):
        code, _, err = self.compare(document(report()),
                                    document(report(input_digest='other')))
        self.assertEqual(code, 1)
        self.assertIn('inputs differ', err)

    def test_grown_instructions_are_a_regression_past_the_tolerance(self):
        code, _, err = self.compare(document(report()),
                                    document(report(instructions_user=2000)))
        self.assertEqual(code, 1)
        self.assertIn('instructions grew', err)


class CommandLineTests(unittest.TestCase):
    def invoke(self, *args):
        return subprocess.run([sys.executable, str(HERE / 'assets.py'), *args],
                              capture_output=True, text=True, timeout=60, check=False)

    def test_a_missing_or_unknown_command_is_a_usage_error_on_stderr(self):
        for args in ([], ['bogus']):
            with self.subTest(args=args):
                result = self.invoke(*args)
                self.assertEqual(result.returncode, 2)
                self.assertFalse(result.stdout)
                self.assertIn('usage:', result.stderr)


if __name__ == '__main__':
    unittest.main()
