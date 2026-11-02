"""Contract tests for the render CPU budget tool.

The tool sets the render limits that the ignored render fixture then enforces,
so its own rules are checked here: the headroom each counter class gets, how
several observations merge into one limit, and that the tool never raises a
limit that is already in the file. Every case runs on synthetic observed
documents: no render, no GPU and no original assets are needed.

    python3 -m unittest discover -s tools/perf -p 'test_*.py'
"""

import contextlib
import importlib.util
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location('render_budget', HERE / 'render_budget.py')
budgets = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(budgets)


def observed(limits, workload='mounted-campaign'):
    """One observed run, as the render fixture writes it."""
    return {'version': budgets.VERSION, 'workload': workload, 'limits': limits}


class HeadroomTests(unittest.TestCase):
    def test_menu_counters_get_the_menu_headroom(self):
        self.assertEqual(budgets.headroom_percent('menu.allocation_calls'),
                         budgets.MENU_HEADROOM_PERCENT)
        self.assertEqual(budgets.headroom_percent('menu.requested_bytes'),
                         budgets.MENU_HEADROOM_PERCENT)

    def test_other_allocator_counters_get_the_allocator_headroom(self):
        self.assertEqual(budgets.headroom_percent('scene.allocation_calls'),
                         budgets.ALLOCATOR_HEADROOM_PERCENT)
        self.assertEqual(budgets.headroom_percent('scene.requested_bytes'),
                         budgets.ALLOCATOR_HEADROOM_PERCENT)

    def test_other_counters_get_no_headroom(self):
        self.assertEqual(budgets.headroom_percent('scene.frames'), 0)
        self.assertEqual(budgets.headroom_percent('scene.draw_calls'), 0)


class MergeTests(unittest.TestCase):
    def test_the_limit_is_the_largest_observation_plus_its_headroom(self):
        workload, limits = budgets.merged([
            observed({'scene.allocation_calls': 100, 'scene.frames': 10}),
            observed({'scene.allocation_calls': 200, 'scene.frames': 12}),
        ])
        self.assertEqual(workload, 'mounted-campaign')
        self.assertEqual(limits, {'scene.allocation_calls': 202, 'scene.frames': 12})

    def test_runs_that_disagree_on_the_workload_are_refused(self):
        with self.assertRaises(SystemExit) as refusal:
            budgets.merged([observed({'a': 1}, 'one'), observed({'a': 1}, 'two')])
        self.assertIn('workload', str(refusal.exception))

    def test_runs_that_disagree_on_the_counter_set_are_refused(self):
        with self.assertRaises(SystemExit) as refusal:
            budgets.merged([observed({'a': 1}), observed({'b': 1})])
        self.assertIn('counter set', str(refusal.exception))


class CommandTests(unittest.TestCase):
    def invoke(self, *args):
        argv = sys.argv
        sys.argv = ['render_budget.py', *args]
        out, err = io.StringIO(), io.StringIO()
        try:
            with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
                try:
                    code = budgets.main()
                except SystemExit as exit:
                    # A refusal exits with its message on stderr.
                    code = exit.code
                    if isinstance(code, str):
                        err.write(code + '\n')
                        code = 1
            return code, out.getvalue(), err.getvalue()
        finally:
            sys.argv = argv

    def write(self, directory, document):
        path = Path(directory) / 'render.json'
        path.write_text(json.dumps(document))
        return path

    def test_without_write_the_limit_is_printed_and_the_file_is_untouched(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = self.write(tmp, {'version': budgets.VERSION, 'workload': 'mounted-campaign',
                                    'limits': {'scene.allocation_calls': 1000}})
            before = path.read_bytes()
            observed_path = Path(tmp) / 'observed.json'
            observed_path.write_text(json.dumps(observed({'scene.allocation_calls': 100})))
            code, out, err = self.invoke(str(observed_path), '--budget', str(path))
            self.assertEqual(code, 0, err)
            self.assertEqual(json.loads(out)['limits'], {'scene.allocation_calls': 101})
            self.assertEqual(path.read_bytes(), before)

    def test_write_replaces_the_budget_file(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = self.write(tmp, {'version': budgets.VERSION, 'workload': 'mounted-campaign',
                                    'limits': {'scene.allocation_calls': 1000}})
            observed_path = Path(tmp) / 'observed.json'
            observed_path.write_text(json.dumps(observed({'scene.allocation_calls': 100})))
            code, _, err = self.invoke(str(observed_path), '--budget', str(path), '--write')
            self.assertEqual(code, 0, err)
            stored = json.loads(path.read_text())
            self.assertEqual(stored['limits'], {'scene.allocation_calls': 101})
            self.assertEqual(stored['workload'], 'mounted-campaign')

    def test_a_limit_that_would_rise_is_refused_and_nothing_is_written(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = self.write(tmp, {'version': budgets.VERSION, 'workload': 'mounted-campaign',
                                    'limits': {'scene.allocation_calls': 100}})
            before = path.read_bytes()
            observed_path = Path(tmp) / 'observed.json'
            observed_path.write_text(json.dumps(observed({'scene.allocation_calls': 200})))
            code, out, err = self.invoke(str(observed_path), '--budget', str(path), '--write')
            self.assertEqual(code, 1)
            self.assertEqual(out, '')
            self.assertIn('would raise the limit', err)
            self.assertEqual(path.read_bytes(), before)

    def test_a_budget_for_another_workload_is_refused(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = self.write(tmp, {'version': budgets.VERSION, 'workload': 'other',
                                    'limits': {'scene.allocation_calls': 100}})
            observed_path = Path(tmp) / 'observed.json'
            observed_path.write_text(json.dumps(observed({'scene.allocation_calls': 100})))
            code, out, err = self.invoke(str(observed_path), '--budget', str(path))
            self.assertEqual(code, 1)
            self.assertEqual(out, '')
            self.assertIn('workload', err)

    def test_an_unknown_budget_version_is_refused(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = self.write(tmp, {'version': budgets.VERSION + 1, 'workload': 'mounted-campaign',
                                    'limits': {}})
            observed_path = Path(tmp) / 'observed.json'
            observed_path.write_text(json.dumps(observed({'scene.allocation_calls': 100})))
            code, out, err = self.invoke(str(observed_path), '--budget', str(path))
            self.assertEqual(code, 1)
            self.assertEqual(out, '')
            self.assertIn('version', err)


if __name__ == '__main__':
    unittest.main()
