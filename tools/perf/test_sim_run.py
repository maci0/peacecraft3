"""Contract tests for the simulation performance budget tool.

The tool decides whether a change made the simulation do more work, so its own
rules are checked here: what counts as a budget violation, that a budget may
only go down against the committed one, and that a budget measured on other
inputs is not a comparison. Every case runs on synthetic reports: no original
assets, no `perf` and no measurement are needed.

    python3 -m unittest discover -s tools/perf -p 'test_*.py'
"""

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location('perf_sim_run', HERE / 'sim_run.py')
sim_run = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(sim_run)


def report(**overrides):
    """One measured workload, as the bench binary reports it."""
    base = {
        'digest': 'cbf29ce484225242',
        'allocations': 1,
        'reallocations': 0,
        'allocated_bytes': 42,
    }
    base.update(overrides)
    return base


class BudgetTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)

    def committed(self):
        """The budgets as they are committed in the repository."""
        return json.loads(sim_run.BUDGETS.read_text())

    def write(self, document):
        path = self.root / 'sim.json'
        path.write_text(json.dumps(document))
        return path

    def check(self, reports, modes, budgets=None, committed=None, with_assets=False):
        """Run the gate against `budgets`, comparing with the `committed` copy."""
        budgets = self.committed() if budgets is None else budgets
        committed = self.committed() if committed is None else committed
        with patch.object(sim_run, 'BUDGETS', self.write(budgets)), \
                patch.object(sim_run, 'committed_budgets', lambda: committed):
            return sim_run.check(reports, modes, with_assets)

    def test_a_workload_inside_its_budget_has_no_failure(self):
        budget = self.committed()['workloads']['script-vm baseline']
        failures = self.check([report(digest=budget['digest'],
                                      allocations=budget['allocations'],
                                      reallocations=budget['reallocations'],
                                      allocated_bytes=budget['allocated_bytes'])],
                               ['script-vm baseline'])
        self.assertEqual(failures, [])

    def test_a_changed_digest_is_a_failure(self):
        failures = self.check([report(digest='0' * 16)], ['script-vm baseline'])
        self.assertEqual(len(failures), 1, failures)
        self.assertIn('digest', failures[0])

    def test_a_counter_over_its_budget_is_a_failure(self):
        budget = self.committed()['workloads']['script-vm baseline']
        failures = self.check([report(digest=budget['digest'],
                                      allocations=budget['allocations'] + 1,
                                      reallocations=budget['reallocations'],
                                      allocated_bytes=budget['allocated_bytes'])],
                               ['script-vm baseline'])
        self.assertEqual(len(failures), 1, failures)
        self.assertIn('exceeds the budget', failures[0])

    def test_a_workload_without_a_budget_is_a_failure(self):
        failures = self.check([report()], ['not-a-workload'])
        self.assertEqual(len(failures), 1, failures)
        self.assertIn('no budget', failures[0])

    def test_a_budget_above_the_committed_one_is_a_failure(self):
        # The committed budgets only go down; a file that raised one is refused
        # even when the measurement is inside it.
        committed = self.committed()
        raised = json.loads(json.dumps(committed))
        raised['workloads']['script-vm baseline']['allocations'] += 1
        failures = self.check([report()], ['script-vm baseline'],
                               budgets=raised, committed=committed)
        self.assertEqual(len(failures), 1, failures)
        self.assertIn('budgets only go down', failures[0])

    def test_budgets_measured_on_other_inputs_are_not_a_comparison(self):
        measured = {'install_exe_sha256': 'a' * 64, 'map_sha256': 'b' * 64}
        with patch.object(sim_run, 'input_identities', lambda: measured):
            failures = self.check([report()], ['script-vm baseline'], with_assets=True)
        self.assertEqual(len(failures), 2, failures)
        self.assertTrue(all('input' in failure for failure in failures), failures)


if __name__ == '__main__':
    unittest.main()
