"""Check that the 32-bit WebAssembly test gate reports a failing test.

`cargo test --target wasm32-unknown-unknown` cannot report a result: the
binaries export `main` rather than the WASI command `_start`, so `wasmtime
run` only instantiates them, and the target has no console and no process to
exit with. It exits 0 whatever the tests do.

The path that does report is `wasm32-wasip1` with `wasmtime run --dir .`. This
builds a fixture with a failing test and requires that path to fail, so a
silent gate cannot be mistaken for a passing one.

    python3 -m unittest discover -s tools -t tools -p 'test_*.py'
"""

import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

FIXTURE = """\
[package]
name = "wasm-gate-probe"
version = "0.0.0"
edition = "2021"

[[test]]
name = "probe"
path = "probe.rs"
"""

PROBE = """\
#[test]
fn a_failing_test() {
    assert_eq!(1 + 1, 3, "deliberate failure");
}
"""

TARGET = "wasm32-wasip1"


def repository_root():
    here = Path(__file__).resolve()
    for parent in [here, *here.parents]:
        if (parent / "Cargo.toml").is_file() and (parent / "AGENTS.md").is_file():
            return parent
    raise RuntimeError("repository root not found above " + str(here))


def installed_targets():
    """The targets the active toolchain can build, from its sysroot."""
    try:
        sysroot = subprocess.run(["rustc", "--print", "sysroot"], capture_output=True,
                                 text=True, check=False)
    except OSError:
        return set()
    directory = Path(sysroot.stdout.strip()) / "lib" / "rustlib"
    if not directory.is_dir():
        return set()
    return {entry.name for entry in directory.iterdir() if entry.is_dir()}


def toolchain_available():
    if shutil.which("wasmtime") is None:
        return False
    return TARGET in installed_targets()


@unittest.skipUnless(toolchain_available(),
                     "wasmtime and the wasm32-wasip1 target are required")
class WasmGateTests(unittest.TestCase):
    def test_a_failing_test_is_reported_as_a_failure(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "Cargo.toml").write_text(FIXTURE)
            (root / "probe.rs").write_text(PROBE)
            environment = os.environ.copy()
            environment["CARGO_TARGET_WASM32_WASIP1_RUNNER"] = "wasmtime run --dir ."
            result = subprocess.run(
                ["cargo", "test", "--target", TARGET, "--test", "probe"],
                cwd=root, capture_output=True, text=True, timeout=600,
                env=environment, check=False,
            )
            self.assertNotEqual(result.returncode, 0,
                                "the gate must report a failing test")
            self.assertIn("a_failing_test", result.stdout + result.stderr)

    def test_the_harness_cases_run_as_32_bit_webassembly(self):
        """The harness's own 32-bit cases, run as actual 32-bit WebAssembly.

        The simulation and the play-test harness must hold their determinism
        and capacity properties at a 32-bit pointer width, not only at 64.
        """
        root = repository_root()
        cases = [
            ('pcx-sim', []),
            ('pcx-play', ['--test', 'trace_capacity']),
            # The generator's CLI case spawns a process, which the 32-bit
            # WebAssembly path cannot do; the rest of that target is the
            # complete signed-coordinate domain, the exact budgets and 100
            # byte-identical fresh runs.
            ('pcx-play', ['--test', 'generator', '--', '--skip',
                          'cli_emits_executable_rpl_and_refusals_do_not_emit_partial_output']),
        ]
        for package, extra in cases:
            with self.subTest(package=package, extra=extra):
                result = subprocess.run(
                    ["cargo", "test", "--release", "--target", TARGET, "-p", package, *extra],
                    cwd=root, capture_output=True, text=True, timeout=3600, check=False,
                )
                self.assertEqual(result.returncode, 0,
                                 (result.stdout[-2000:] + result.stderr[-2000:]))


if __name__ == '__main__':
    unittest.main()
