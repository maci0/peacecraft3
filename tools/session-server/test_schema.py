"""Validate `session_server schema` against the pinned published CLI Spec v0.3 bytes.

Run from the repository root after building the binary:

    cargo build -p pcx-net --features session-server --bin session_server --locked --offline
    uv run --no-project --with jsonschema python -m unittest tools/session-server/test_schema.py

The pinned bytes and their recorded hash live in tools/original-game-reference/schema, the
repository's one pin of clispec v0.3. This test does not claim full CLI Spec conformance.
"""

import hashlib
import json
import os
import subprocess
import unittest
from pathlib import Path


def repository_root() -> Path:
    here = Path(__file__).resolve()
    for parent in [here, *here.parents]:
        if (parent / "Cargo.toml").is_file() and (parent / "AGENTS.md").is_file():
            return parent
    raise RuntimeError("repository root not found above " + str(here))


ROOT = repository_root()
PIN = ROOT / "tools/original-game-reference/schema/clispec-v0.3.json"
CHECKSUMS = ROOT / "tools/original-game-reference/schema/CHECKSUMS.txt"
BINARY = Path(os.environ.get("SESSION_SERVER_BIN", ROOT / "target/debug/session_server"))


class SchemaPinTests(unittest.TestCase):
    def test_pinned_bytes_match_the_recorded_candidate_hash(self):
        digest = hashlib.sha256(PIN.read_bytes()).hexdigest()
        recorded = [line for line in CHECKSUMS.read_text().splitlines() if line.endswith("schema/v0.3.json")]
        self.assertEqual(recorded, [f"candidate  {digest}  schema/v0.3.json"])

    def test_schema_output_validates_against_the_pinned_spec(self):
        import jsonschema

        result = subprocess.run([str(BINARY), "schema"], capture_output=True, text=True, timeout=30)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stderr, "")
        document = json.loads(result.stdout)
        jsonschema.Draft202012Validator(json.loads(PIN.read_text())).validate(document)
        self.assertEqual(document["clispec"], "0.3")

    def test_every_command_error_resolves_to_a_declared_kind(self):
        result = subprocess.run([str(BINARY), "schema"], capture_output=True, text=True, timeout=30)
        document = json.loads(result.stdout)
        declared = {error["kind"] for error in document["errors"]}
        for command in document["commands"]:
            self.assertLessEqual(set(command.get("errors", [])), declared, command["name"])

    def test_error_exit_codes_are_distinct_and_nonzero(self):
        result = subprocess.run([str(BINARY), "schema"], capture_output=True, text=True, timeout=30)
        document = json.loads(result.stdout)
        codes = [error["exit_code"] for error in document["errors"]]
        self.assertEqual(len(codes), len(set(codes)))
        self.assertNotIn(0, codes)


if __name__ == "__main__":
    unittest.main()
