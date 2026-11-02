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


if __name__ == '__main__':
    unittest.main()
