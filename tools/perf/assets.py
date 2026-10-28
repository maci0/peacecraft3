#!/usr/bin/env python3
"""Fixed-work asset loading and decode workloads, measured with perf stat.

The gate metrics are user-mode instructions (`perf stat -e instructions:u`),
allocation calls and requested bytes, plus an output digest that proves decoded
results are unchanged. Wall time is report-only and never decides the exit code.

    python3 tools/perf/assets.py list
    python3 tools/perf/assets.py run [--label before] [--workload NAME]... [--repeat 3]
    python3 tools/perf/assets.py compare BEFORE.json AFTER.json
    python3 tools/perf/assets.py check [--workload NAME]...
    python3 tools/perf/assets.py lower RESULT.json

The install comes from --install or PEACECRAFT3_LEGACY_DIR; assets are read at
run time and never copied. Results go to .scratch/perf-assets/results/.

`check` measures the workloads and fails when an allocation counter exceeds the
versioned budget in tools/perf/budgets/assets.json, or a fixed-work counter or
digest differs. Instruction counts are printed and never decide the exit code.
`lower` rewrites the budget from a result file and only ever lowers it.
"""

from __future__ import annotations

import argparse
import json
import os
import statistics
import subprocess
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
MANIFEST = ROOT / "tools/perf/assets/Cargo.toml"
BUDGETS = ROOT / "tools/perf/budgets/assets.json"
SCRATCH = ROOT / ".scratch/perf-assets"
TARGET = SCRATCH / "target"
RESULTS = SCRATCH / "results"
BINARY = TARGET / "release/peacecraft-assets-perf"

INSTRUCTION_EVENT = "instructions:u"
# Allowed instruction growth before `compare` calls a change a regression. The
# counts are deterministic for one binary and input, so this only absorbs
# unrelated code-layout shifts from edits elsewhere in the dependency graph.
DEFAULT_TOLERANCE_PERCENT = 2.0
# jpeg-decoder uses rayon; one thread keeps instruction counts independent of scheduling.
DEFAULT_THREADS = 1
BUDGET_VERSION = 1

Report = dict[str, Any]


def build() -> None:
    subprocess.run(
        [
            "cargo",
            "build",
            "--release",
            "--locked",
            "--offline",
            "--manifest-path",
            str(MANIFEST),
            "--target-dir",
            str(TARGET),
        ],
        cwd=ROOT,
        check=True,
    )


def workload_names(binary: Path) -> list[str]:
    listing = subprocess.run(
        [str(binary), "list"], check=True, capture_output=True, text=True
    )
    return [entry["name"] for entry in json.loads(listing.stdout)["workloads"]]


def instructions_from(stat_csv: str) -> int:
    for line in stat_csv.splitlines():
        fields = line.split(",")
        if len(fields) > 2 and fields[2].startswith(INSTRUCTION_EVENT):
            if not fields[0].isdigit():
                raise RuntimeError(f"perf did not count instructions: {line!r}")
            return int(fields[0])
    raise RuntimeError(f"no {INSTRUCTION_EVENT} line in perf output: {stat_csv!r}")


def run_once(binary: Path, name: str, install: Path, threads: int) -> Report:
    """One process under `perf stat`; the program enables the counter around its window."""
    RESULTS.mkdir(parents=True, exist_ok=True)
    control = SCRATCH / f"control-{os.getpid()}.fifo"
    ack = SCRATCH / f"ack-{os.getpid()}.fifo"
    stat = SCRATCH / f"stat-{os.getpid()}.csv"
    for fifo in (control, ack):
        fifo.unlink(missing_ok=True)
        os.mkfifo(fifo)
    try:
        completed = subprocess.run(
            [
                "perf",
                "stat",
                "-x,",
                "-e",
                INSTRUCTION_EVENT,
                "-D",
                "-1",
                "--control",
                f"fifo:{control},{ack}",
                "-o",
                str(stat),
                "--",
                str(binary),
                "run",
                name,
                "--install",
                str(install),
                "--control",
                str(control),
                "--ack",
                str(ack),
                "--timing",
            ],
            cwd=ROOT,
            capture_output=True,
            text=True,
            env={**os.environ, "RAYON_NUM_THREADS": str(threads)},
            timeout=600,
            check=False,
        )
        if completed.returncode != 0:
            raise RuntimeError(
                f"{name} exited {completed.returncode}: {completed.stderr.strip()}"
            )
        report: Report = json.loads(completed.stdout)
        report["instructions_user"] = instructions_from(stat.read_text())
        return report
    finally:
        for path in (control, ack, stat):
            path.unlink(missing_ok=True)


STABLE_FIELDS = (
    "input_digest",
    "output_digest",
    "work",
    "allocations",
    "reallocations",
    "allocated_bytes",
)


def run_workload(
    binary: Path, name: str, install: Path, repeat: int, threads: int
) -> Report:
    runs = [run_once(binary, name, install, threads) for _ in range(repeat)]
    for field in STABLE_FIELDS:
        if any(run[field] != runs[0][field] for run in runs):
            raise RuntimeError(
                f"{name}: {field} differs between identical runs: {[r[field] for r in runs]}"
            )
    counts = [run["instructions_user"] for run in runs]
    report = {field: runs[0][field] for field in STABLE_FIELDS}
    report.update(
        workload=name,
        instructions_user=int(statistics.median(counts)),
        instructions_user_min=min(counts),
        instructions_user_max=max(counts),
        elapsed_ns_report_only=int(
            statistics.median(run["elapsed_ns_report_only"] for run in runs)
        ),
    )
    return report


def command_list(args: argparse.Namespace) -> int:
    if not args.no_build:
        build()
    print("\n".join(workload_names(BINARY)))
    return 0


def resolve_install(args: argparse.Namespace) -> Path:
    value = args.install or os.environ.get("PEACECRAFT3_LEGACY_DIR")
    if not value:
        raise SystemExit(
            "set --install or PEACECRAFT3_LEGACY_DIR to the Legacy install"
        )
    return Path(value)


def toolchain() -> dict[str, str]:
    version = subprocess.run(
        ["rustc", "--version"], check=True, capture_output=True, text=True
    )
    perf = subprocess.run(
        ["perf", "--version"], check=True, capture_output=True, text=True
    )
    return {
        "rustc": version.stdout.strip(),
        "perf": perf.stdout.strip(),
        "profile": "release-o3-thin-lto",
    }


def measure(args: argparse.Namespace, label: str) -> Report:
    """Runs the selected workloads and returns the result document."""
    install = resolve_install(args)
    binary = Path(args.binary) if args.binary else BINARY
    if not (args.no_build or args.binary):
        build()
    names = args.workload or workload_names(binary)
    results = []
    for name in names:
        report = run_workload(binary, name, install, args.repeat, args.threads)
        results.append(report)
        print(
            f"{name:28} instr={report['instructions_user']:>15,} "
            f"spread={report['instructions_user_max'] - report['instructions_user_min']:>9,} "
            f"allocs={report['allocations']:>9,} realloc={report['reallocations']:>7,} "
            f"bytes={report['allocated_bytes']:>12,} digest={report['output_digest']}"
        )
    return {
        "label": label,
        "environment": toolchain(),
        "repeat": args.repeat,
        "threads": args.threads,
        "results": results,
    }


def command_run(args: argparse.Namespace) -> int:
    document = measure(args, args.label)
    RESULTS.mkdir(parents=True, exist_ok=True)
    path = Path(args.output) if args.output else RESULTS / f"{args.label}.json"
    path.write_text(json.dumps(document, indent=2) + "\n")
    print(f"wrote {path}")
    return 0


def budget_entry(report: Report) -> Report:
    return {
        "input_digest": report["input_digest"],
        "output_digest": report["output_digest"],
        "work": report["work"],
        "allocation_calls": report["allocations"] + report["reallocations"],
        "allocated_bytes": report["allocated_bytes"],
    }


def load_budgets() -> Report:
    document: Report = json.loads(BUDGETS.read_text())
    if document.get("version") != BUDGET_VERSION:
        raise RuntimeError(
            f"{BUDGETS}: budget version {document.get('version')!r}, expected {BUDGET_VERSION}"
        )
    return document


def violations(name: str, budget: Report, report: Report) -> list[str]:
    """What a measured workload breaks in its budget; empty when it passes."""
    found = []
    for field in ("input_digest", "output_digest", "work"):
        if budget[field] != report[field]:
            found.append(f"{name}: {field} {report[field]} != budget {budget[field]}")
    calls = report["allocations"] + report["reallocations"]
    for field, measured in (
        ("allocation_calls", calls),
        ("allocated_bytes", report["allocated_bytes"]),
    ):
        if measured > budget[field]:
            found.append(
                f"{name}: {field} {measured:,} exceeds budget {budget[field]:,}"
            )
    return found


def command_check(args: argparse.Namespace) -> int:
    budgets = load_budgets()["workloads"]
    document = measure(args, "check")
    failures: list[str] = []
    measured = {report["workload"]: report for report in document["results"]}
    for name, report in measured.items():
        if name not in budgets:
            failures.append(
                f"{name}: no budget; run `lower` on a result file to add it"
            )
        else:
            failures.extend(violations(name, budgets[name], report))
    if not args.workload:
        failures.extend(
            f"{name}: budgeted but not a workload"
            for name in budgets.keys() - measured.keys()
        )
    for failure in failures:
        print(f"FAIL {failure}", file=sys.stderr)
    print(
        "instruction counts are report-only; budgets cover allocations and fixed work"
    )
    return 1 if failures else 0


def command_lower(args: argparse.Namespace) -> int:
    """Lowers budgets to a result file; refuses to raise a counter or change work."""
    document = json.loads(Path(args.result).read_text())
    budgets: Report = (
        load_budgets()
        if BUDGETS.exists()
        else {"version": BUDGET_VERSION, "workloads": {}}
    )
    refused: list[str] = []
    for report in document["results"]:
        name = report["workload"]
        entry = budget_entry(report)
        old = budgets["workloads"].get(name)
        if old is None:
            budgets["workloads"][name] = entry
            continue
        for field in ("input_digest", "output_digest", "work"):
            if old[field] != entry[field]:
                refused.append(
                    f"{name}: {field} changed; edit the budget by hand with the reason"
                )
        for field in ("allocation_calls", "allocated_bytes"):
            if entry[field] > old[field]:
                refused.append(
                    f"{name}: {field} would rise {old[field]:,} -> {entry[field]:,}"
                )
            else:
                old[field] = entry[field]
    for failure in refused:
        print(f"FAIL {failure}", file=sys.stderr)
    if refused:
        return 1
    budgets["rustc"] = document["environment"]["rustc"]
    BUDGETS.parent.mkdir(parents=True, exist_ok=True)
    BUDGETS.write_text(json.dumps(budgets, indent=2, sort_keys=True) + "\n")
    print(f"wrote {BUDGETS}")
    return 0


def percent(before: int, after: int) -> float:
    return 0.0 if before == 0 else (after - before) * 100.0 / before


def command_compare(args: argparse.Namespace) -> int:
    before = {
        r["workload"]: r for r in json.loads(Path(args.before).read_text())["results"]
    }
    after = {
        r["workload"]: r for r in json.loads(Path(args.after).read_text())["results"]
    }
    failures: list[str] = []
    print(
        f"{'workload':28} {'instr before':>15} {'instr after':>15} {'delta':>8} {'allocs before':>14} {'after':>10}"
    )
    for name in sorted(before.keys() & after.keys()):
        old, new = before[name], after[name]
        delta = percent(old["instructions_user"], new["instructions_user"])
        print(
            f"{name:28} {old['instructions_user']:>15,} {new['instructions_user']:>15,} {delta:>+7.1f}% "
            f"{old['allocations']:>14,} {new['allocations']:>10,}"
        )
        if old["input_digest"] != new["input_digest"]:
            failures.append(f"{name}: inputs differ, not a comparison")
        if old["output_digest"] != new["output_digest"]:
            failures.append(f"{name}: output digest changed")
        if old["work"] != new["work"]:
            failures.append(f"{name}: fixed-work counters changed")
        if (
            new["allocations"] + new["reallocations"]
            > old["allocations"] + old["reallocations"]
        ):
            failures.append(f"{name}: allocation calls increased")
        if delta > args.tolerance:
            failures.append(
                f"{name}: instructions grew {delta:.1f}% (tolerance {args.tolerance}%)"
            )
    for name in sorted(before.keys() ^ after.keys()):
        failures.append(f"{name}: present in only one result file")
    for failure in failures:
        print(f"FAIL {failure}", file=sys.stderr)
    return 1 if failures else 0


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    commands = parser.add_subparsers(dest="command", required=True)
    listing = commands.add_parser("list", help="print the workload names")
    listing.add_argument(
        "--no-build", action="store_true", help="use the already built binary"
    )
    listing.set_defaults(handler=command_list)
    run = commands.add_parser("run", help="measure workloads and write a result file")
    run.add_argument("--install", help="Legacy install directory")
    run.add_argument(
        "--label",
        default="latest",
        help="result file name under .scratch/perf-assets/results",
    )
    run.add_argument("--output", help="explicit result path")
    run.add_argument(
        "--workload", action="append", help="limit to this workload (repeatable)"
    )
    run.add_argument(
        "--repeat", type=int, default=3, help="process runs per workload (default 3)"
    )
    run.add_argument(
        "--no-build", action="store_true", help="use the already built binary"
    )
    run.add_argument("--binary", help="measure this binary (implies --no-build)")
    run.add_argument(
        "--threads",
        type=int,
        default=DEFAULT_THREADS,
        help="RAYON_NUM_THREADS for jpeg-decoder (default 1; counts include all threads)",
    )
    run.set_defaults(handler=command_run)
    check = commands.add_parser(
        "check", help="measure workloads and fail on a broken budget"
    )
    check.add_argument("--install", help="Legacy install directory")
    check.add_argument(
        "--workload", action="append", help="limit to this workload (repeatable)"
    )
    check.add_argument(
        "--repeat", type=int, default=1, help="process runs per workload (default 1)"
    )
    check.add_argument(
        "--no-build", action="store_true", help="use the already built binary"
    )
    check.add_argument("--binary", help="measure this binary (implies --no-build)")
    check.set_defaults(handler=command_check, threads=DEFAULT_THREADS)
    lower = commands.add_parser(
        "lower", help="lower the budgets to a result file (never raises them)"
    )
    lower.add_argument("result")
    lower.set_defaults(handler=command_lower)
    compare = commands.add_parser(
        "compare", help="compare two result files; exit 1 on a regression"
    )
    compare.add_argument("before")
    compare.add_argument("after")
    compare.add_argument(
        "--tolerance",
        type=float,
        default=DEFAULT_TOLERANCE_PERCENT,
        help="allowed instruction growth in percent",
    )
    compare.set_defaults(handler=command_compare)
    args = parser.parse_args()
    return int(args.handler(args))


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except subprocess.CalledProcessError as error:
        raise SystemExit(f"{error.cmd[0]} failed with exit code {error.returncode}")
    except (OSError, RuntimeError, ValueError, KeyError) as error:
        raise SystemExit(str(error))
