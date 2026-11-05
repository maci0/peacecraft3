"""Canonical gameplay-fixture client (protocol 1) — stdlib only.

Speaks the file protocol defined by ``crates/peacecraft3/src/gameplay_fixture.rs``:
the Python side writes ``fixture-request.json`` (atomically, <=8192 bytes),
the Rust fixture writes ``fixture-response.json`` (<=1 MiB). Requests are
explicit typed canonical orders (move/step/cancel/stop) — no raw shell, no
JS, no mouse/UI, no world teleport.

This module is only the client half. It does NOT launch, spawn, or integrate
with any game process: the caller owns the run directory and guarantees a
fixture worker (real or fake) is polling it. The real fixture launch hook
(process start / wiring the Rust ``run_fixture`` loop to a run dir) is a
remaining step, intentionally not in this first slice.
"""

from __future__ import annotations

import json
import math
import os
import time
from dataclasses import dataclass, field
from pathlib import Path

PROTOCOL_VERSION = 1
REQUEST_FILE = "fixture-request.json"
RESPONSE_FILE = "fixture-response.json"
MAX_REQUEST_BYTES = 8192
MAX_RESPONSE_BYTES = 1 << 20  # 1 MiB
# Mirrors the Rust-side caps so the client fails closed before writing.
MAX_ORDER_UNITS = 64
MAX_STEP_SECONDS = 30.0
MAX_COORDINATE = 100_000.0

_ACTION_TYPES = {"move", "step", "cancel", "stop"}


class FixtureError(Exception):
    """Malformed/unknown/stale response, or a protocol-level failure."""


class FixtureTimeout(FixtureError, TimeoutError):
    """No matching response arrived before the deadline."""


class RequestTooLarge(FixtureError, ValueError):
    """Serialized request exceeds the 8192-byte bound."""


@dataclass(frozen=True)
class FixtureResponse:
    protocol: int
    id: int
    ok: bool
    error: str | None
    observation: dict | None
    state: str | None
    raw: dict = field(repr=False, compare=False)


def _finite_number(value, name: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ValueError(f"{name} must be a number")
    number = float(value)
    if not math.isfinite(number):
        raise ValueError(f"{name} must be finite")
    return number


def _unit_ids(units) -> list[int]:
    if not isinstance(units, (list, tuple)):
        raise ValueError("units must be a list of unit ids")
    ids = list(units)
    if not 1 <= len(ids) <= MAX_ORDER_UNITS:
        raise ValueError(f"units length {len(ids)} out of bounds (1..{MAX_ORDER_UNITS})")
    for unit in ids:
        if isinstance(unit, bool) or not isinstance(unit, int) or unit <= 0:
            raise ValueError(f"unit id {unit!r} must be a positive integer")
    return ids


def _seconds(value) -> float:
    seconds = _finite_number(value, "seconds")
    if not 0.0 < seconds <= MAX_STEP_SECONDS:
        raise ValueError(f"seconds {seconds} out of bounds (0, {MAX_STEP_SECONDS}]")
    return seconds


def _target(value) -> list[float]:
    if not isinstance(value, (list, tuple)) or len(value) != 2:
        raise ValueError("target must be a [x, y] pair")
    point = [_finite_number(v, "target coordinate") for v in value]
    if any(abs(v) > MAX_COORDINATE for v in point):
        raise ValueError("target coordinate out of bounds")
    return point


def build_request(request_id: int, action: dict) -> dict:
    """Validate an explicit typed action and build the request record.

    Raises ValueError on unknown action types, unknown fields, or any
    value violating the Rust-side bounds (fail-closed, mirroring
    ``validate`` in gameplay_fixture.rs).
    """
    if isinstance(request_id, bool) or not isinstance(request_id, int) or request_id < 0:
        raise ValueError("request id must be a non-negative integer")
    if not isinstance(action, dict):
        raise ValueError("action must be an object")
    action_type = action.get("type")
    if action_type not in _ACTION_TYPES:
        raise ValueError(f"unknown action type {action_type!r}")

    allowed = {
        "move": {"type", "units", "target", "seconds"},
        "step": {"type", "seconds"},
        "cancel": {"type", "units", "seconds"},
        "stop": {"type"},
    }[action_type]
    unknown = set(action) - allowed
    if unknown:
        raise ValueError(f"unknown field(s) for action {action_type!r}: {sorted(unknown)}")
    missing = allowed - set(action)
    if missing:
        raise ValueError(f"missing field(s) for action {action_type!r}: {sorted(missing)}")

    if action_type == "move":
        canonical_action = {
            "type": "move",
            "units": _unit_ids(action["units"]),
            "target": _target(action["target"]),
            "seconds": _seconds(action["seconds"]),
        }
    elif action_type == "cancel":
        canonical_action = {
            "type": "cancel",
            "units": _unit_ids(action["units"]),
            "seconds": _seconds(action["seconds"]),
        }
    elif action_type == "step":
        canonical_action = {"type": "step", "seconds": _seconds(action["seconds"])}
    else:
        canonical_action = {"type": "stop"}
    return {"protocol": PROTOCOL_VERSION, "id": request_id, "action": canonical_action}


def _serialize(request: dict) -> bytes:
    payload = json.dumps(request, separators=(",", ":")).encode("utf-8")
    if len(payload) > MAX_REQUEST_BYTES:
        raise RequestTooLarge(
            f"request size {len(payload)} exceeds cap {MAX_REQUEST_BYTES}"
        )
    return payload


def _parse_response(raw: bytes, want_id: int) -> FixtureResponse:
    if len(raw) > MAX_RESPONSE_BYTES:
        raise FixtureError(f"response size {len(raw)} exceeds cap {MAX_RESPONSE_BYTES}")
    try:
        record = json.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise FixtureError(f"malformed response: {exc}") from exc
    if not isinstance(record, dict):
        raise FixtureError("malformed response: expected a JSON object")
    unknown = set(record) - {"protocol", "id", "ok", "error", "observation", "state"}
    if unknown:
        raise FixtureError(f"unknown response field(s): {sorted(unknown)}")
    missing = {"protocol", "id", "ok", "error", "observation", "state"} - set(record)
    if missing:
        raise FixtureError(f"malformed response: missing field(s) {sorted(missing)}")
    if record["protocol"] != PROTOCOL_VERSION:
        raise FixtureError(f"unsupported response protocol {record['protocol']!r}")
    if record["id"] != want_id:
        raise FixtureError(
            f"stale response id {record['id']!r}, expected {want_id!r}"
        )
    if not isinstance(record["ok"], bool):
        raise FixtureError("malformed response: ok must be a boolean")
    for name in ("error", "state"):
        if record[name] is not None and not isinstance(record[name], str):
            raise FixtureError(f"malformed response: {name} must be a string or null")
    if record["observation"] is not None and not isinstance(record["observation"], dict):
        raise FixtureError("malformed response: observation must be an object or null")
    return FixtureResponse(
        protocol=record["protocol"],
        id=record["id"],
        ok=record["ok"],
        error=record["error"],
        observation=record["observation"],
        state=record["state"],
        raw=record,
    )


class FixtureClient:
    """One outstanding request at a time, incrementing ids, deadline wait.

    ``run_dir`` is owned by the caller; paths never escape it. ``stop`` is
    sent and awaited exactly like any other request type — there is no
    separate teardown path.
    """

    def __init__(self, run_dir, *, poll_interval: float = 0.01):
        self.run_dir = Path(run_dir)
        if not self.run_dir.is_dir():
            raise ValueError(f"run_dir {self.run_dir} is not an existing directory")
        if poll_interval <= 0:
            raise ValueError("poll_interval must be positive")
        self.poll_interval = poll_interval
        self._next_id = 1
        self._outstanding = False

    # -- actions (explicit typed canonical orders) -------------------------
    def move(self, units, target, seconds, *, timeout: float = 10.0) -> FixtureResponse:
        return self.send({"type": "move", "units": units, "target": target, "seconds": seconds}, timeout=timeout)

    def step(self, seconds, *, timeout: float = 10.0) -> FixtureResponse:
        return self.send({"type": "step", "seconds": seconds}, timeout=timeout)

    def cancel(self, units, seconds, *, timeout: float = 10.0) -> FixtureResponse:
        return self.send({"type": "cancel", "units": units, "seconds": seconds}, timeout=timeout)

    def stop(self, *, timeout: float = 10.0) -> FixtureResponse:
        return self.send({"type": "stop"}, timeout=timeout)

    # -- core --------------------------------------------------------------
    def send(self, action: dict, *, timeout: float = 10.0) -> FixtureResponse:
        if self._outstanding:
            raise FixtureError("a request is already outstanding")
        if timeout <= 0:
            raise ValueError("timeout must be positive")
        request_id = self._next_id
        request = build_request(request_id, action)
        payload = _serialize(request)
        self._outstanding = True
        try:
            self._write_request(payload)
            return self._wait_response(request_id, timeout)
        finally:
            self._outstanding = False
            self._next_id += 1

    def _write_request(self, payload: bytes) -> None:
        tmp = self.run_dir / (REQUEST_FILE + ".tmp")
        final = self.run_dir / REQUEST_FILE
        # Atomic temp + rename: the worker never sees a partial record.
        fd = os.open(tmp, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o644)
        try:
            os.write(fd, payload)
            os.fsync(fd)
        finally:
            os.close(fd)
        os.replace(tmp, final)

    def _wait_response(self, request_id: int, timeout: float) -> FixtureResponse:
        path = self.run_dir / RESPONSE_FILE
        deadline = time.monotonic() + timeout
        while True:
            if path.is_file():
                try:
                    raw = path.read_bytes()
                except OSError:
                    raw = b""
                # A stale/malformed record is rejected, but only a matching
                # id is accepted; keep waiting until the deadline otherwise.
                try:
                    return _parse_response(raw, request_id)
                except FixtureError as exc:
                    if "stale response id" in str(exc):
                        pass  # stale: ignore and keep waiting
                    else:
                        raise
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise FixtureTimeout(
                    f"no response for request id {request_id} within {timeout:g}s"
                )
            time.sleep(min(self.poll_interval, remaining))
