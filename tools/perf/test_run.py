"""Contract tests for the simulation performance gate runner.

The runner decides whether a profiled run produced usable work: an empty or
unreadable instruction collection must be reported as an error rather than
presented as zero work, which would read like a fast result. Its own argument
handling is checked too. Synthetic Callgrind summaries only: no profiler, no
measurement and no original assets are needed.

    python3 -m unittest discover -s tools/perf -p 'test_*.py'
"""

import importlib.util
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location('perf_run', HERE / 'run.py')
runner = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(runner)


class InstructionCountTests(unittest.TestCase):
    def callgrind(self, text):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        path = Path(directory.name) / 'simulation.callgrind'
        path.write_text(text)
        return runner.instruction_count(path)

    def test_the_instruction_count_is_read_from_the_summary(self):
        self.assertEqual(self.callgrind('events: Ir\nsummary: 12345\n'), 12345)

    def test_the_count_comes_from_the_event_it_names(self):
        self.assertEqual(self.callgrind('events: Ir Dr\nsummary: 12345 7\n'), 12345)

    def test_a_collection_without_instruction_counts_is_an_error(self):
        with self.assertRaisesRegex(RuntimeError, 'did not report instruction counts'):
            self.callgrind('events: \nsummary: \n')

    def test_an_empty_collection_is_not_presented_as_zero_work(self):
        with self.assertRaisesRegex(RuntimeError, 'No workload instructions collected'):
            self.callgrind('events: Ir\nsummary: 0\n')


class CommandLineTests(unittest.TestCase):
    def invoke(self, *args):
        return subprocess.run([sys.executable, str(HERE / 'run.py'), *args],
                              capture_output=True, text=True, timeout=60, check=False)

    def test_the_report_only_options_cannot_be_combined(self):
        result = self.invoke('--timing', '--instructions')
        self.assertEqual(result.returncode, 2)
        self.assertFalse(result.stdout)
        self.assertIn('usage:', result.stderr)


if __name__ == '__main__':
    unittest.main()
