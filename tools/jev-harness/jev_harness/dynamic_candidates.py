"""Per-Observation candidate builder for JevProvider.

Rebuilds finite candidate map from EACH fresh Observation using only
visible/owned units already present in `obs.state["units"]` (upstream
observe() filters to own-or-visible; this module does not re-decide
visibility, does not guess world targets, does not invent attack/move
semantics, and does not choose a preferred unit — target ordering is
backend/gameplay policy, not this slice).

Slice scope: selection only. For each unit with a valid projected
`screen` coordinate inside the current viewport, emit one left-click
candidate `select_unit_<id>`. Always also emit `stop_noop` (wait).
No move/attack candidates: there is no trustworthy screen->world target
in this slice, so none are offered.

Safety rules (fail closed / exclude, never fabricate):
- `screen` missing / None / not a 2-list / non-numeric / bool / NaN /
  inf / outside `0 <= x < obs.width`, `0 <= y < obs.height` -> unit
  excluded. Never coerced to (0,0).
- `in_viewport is False` -> excluded even if screen present.
- Explicit hidden flags (`visible is False` / `hidden is True`) ->
  excluded (mirrors JevProvider.build_request guard; upstream should
  already have filtered).
- Coordinate conversion is explicit safe int: float must be finite;
  `int(v)` truncates toward zero only after bounds check on the float,
  then re-checked as int. No rounding of out-of-bounds values into range.
- Unit `id` must be non-empty str/int (not bool); ids that would
  collide after sanitisation, or duplicate ids, are excluded after the
  first? No — duplicates are ambiguous: all duplicates excluded.
- Criteria keys exactly match candidate ids (JevProvider enforces at
  construction; builder guarantees it by construction).
- Viewport itself must be positive ints (from current Observation), else
  ProviderError — bounds come from current Observation, never a sentinel.

Key handling: factory functions accept every JevProvider kwarg except
candidates/criteria (which are rebuilt per call). api_key / key_path /
environ are forwarded untouched to JevProvider, which loads in-process
via secrets.load_key_in_process; this module never logs/prints the key,
never puts it in criteria/logs, and retains the constructed provider
so the key is loaded once per process, not reloaded per step.
"""
from __future__ import annotations

import math
from typing import Optional

from .jev_provider import JevProvider
from .provider import Observation, Provider, ProviderError

NOOP_ID = "stop_noop"
NOOP_ACTION = {"kind": "wait", "wait_ms": 100}
NOOP_CRITERION = (
    "Do nothing further this step; safest when state is unclear or done."
)
MAX_CRITERIA = 255  # JevProvider / SystemOne choice cap


def _finite_number(v) -> bool:
    if type(v) not in (int, float):
        return False
    try:
        return math.isfinite(float(v))
    except OverflowError:
        return False  # e.g. huge int coord 10**400: not finite-representable


def _safe_int_coord(v, extent: int):
    """Return int coordinate or None. Bounds checked on float first so
    e.g. 639.9 in a 640-wide viewport -> 639 (valid), while 640.0,
    -0.1, NaN, inf, bool, str -> None (never clamped/fabricated)."""
    if not _finite_number(v):
        return None
    f = float(v)
    if not (0.0 <= f < extent):
        return None
    iv = int(f)  # truncation toward zero; f >= 0 so this is floor
    if not (0 <= iv < extent):
        return None
    return iv


def _unit_id(u) -> Optional[str]:
    uid = u.get("id")
    if type(uid) is bool:
        return None
    if isinstance(uid, str) and uid.strip():
        return uid.strip()
    if type(uid) is int:
        return str(uid)
    return None


def _candidate_id(uid: str) -> str:
    # Keep ids readable for criteria/logs; sanitize only characters
    # that would make ids ambiguous. Original uid stays in criterion text.
    safe = "".join(c if (c.isalnum() or c in ("_", "-")) else "_" for c in uid)
    safe = safe.strip("_-") or "unit"
    return f"select_unit_{safe}"


def build_candidates(obs: Observation) -> tuple[dict, dict]:
    """Return (candidates, criteria) rebuilt from this Observation only.

    candidates: id -> strict action dict; criteria: id -> text.
    Keys exactly aligned. Always contains stop_noop.
    """
    if type(obs.width) is not int or obs.width <= 0 or type(obs.height) is not int or obs.height <= 0:
        raise ProviderError("viewport extent must be positive ints (from current Observation)")
    candidates: dict = {NOOP_ID: dict(NOOP_ACTION)}
    criteria: dict = {NOOP_ID: NOOP_CRITERION}

    state = obs.state if isinstance(obs.state, dict) else {}
    units = state.get("units")
    if not isinstance(units, list):
        return candidates, criteria

    # First pass: collect valid (uid, x, y) preserving observation order;
    # duplicates (same uid twice) are ambiguous -> exclude all of that uid.
    seen: dict[str, int] = {}
    valid: list[tuple[str, int, int]] = []
    for u in units:
        if not isinstance(u, dict):
            continue
        if u.get("visible") is False or u.get("hidden") is True:
            continue
        if u.get("in_viewport") is False:
            continue
        uid = _unit_id(u)
        if uid is None:
            continue
        screen = u.get("screen")
        if not isinstance(screen, (list, tuple)) or len(screen) != 2:
            continue
        x = _safe_int_coord(screen[0], obs.width)
        y = _safe_int_coord(screen[1], obs.height)
        if x is None or y is None:
            continue
        seen[uid] = seen.get(uid, 0) + 1
        valid.append((uid, x, y))

    for uid, x, y in valid:
        if seen[uid] != 1:
            continue  # duplicate id: ambiguous, excluded
        cid = _candidate_id(uid)
        if cid in candidates:
            # Sanitisation collision (distinct uids -> same cid):
            # ambiguous, skip rather than overwrite.
            continue
        if len(candidates) >= MAX_CRITERIA:
            break
        candidates[cid] = {"kind": "click", "x": x, "y": y, "button": "left"}
        criteria[cid] = (
            f"Left-click to select visible unit {uid} currently projected "
            f"at screen ({x}, {y}). Selection only; no move/attack implied."
        )
    return candidates, criteria


class DynamicJevProvider(Provider):
    """Provider-callable adapter: rebuilds candidates from EACH Observation.

    Wraps one retained JevProvider (key loaded once, in-process).
    Per call: build_candidates(obs) -> fresh JevProvider with those
    candidates/criteria (same api_key object, same transport/settings,
    no key reload) -> delegate __call__(obs). No framework, no mutable
    validation bypass: JevProvider construction + per-call viewport
    validation still gate every candidate.
    """
    name = "jev-dynamic"

    def __init__(self, api_key=None, *, transport=None, model=None,
                 url=None, timeout_s=None, include_image=False,
                 image_ref="FRAME_REF", max_request_bytes=None,
                 max_response_bytes=None, key_path=None, environ=None,
                 provider_kwargs: Optional[dict] = None):
        # Build a template provider once to validate/retain the key
        # in-process (loaded once, never reloaded per step) and to
        # inherit endpoint/model/transport defaults. Its own frozen
        # candidates are never used for selection.
        kwargs = dict(provider_kwargs or {})
        if transport is not None:
            kwargs["transport"] = transport
        if model is not None:
            kwargs["model"] = model
        if url is not None:
            kwargs["url"] = url
        if timeout_s is not None:
            kwargs["timeout_s"] = timeout_s
        if max_request_bytes is not None:
            kwargs["max_request_bytes"] = max_request_bytes
        if max_response_bytes is not None:
            kwargs["max_response_bytes"] = max_response_bytes
        kwargs["include_image"] = include_image
        kwargs["image_ref"] = image_ref
        # Retain api_key handle without exposing it: if explicit, pass
        # through; otherwise forward key_path/environ for the one-time
        # load inside JevProvider. Stored provider holds the loaded key.
        self._template_kwargs = kwargs
        self._api_key = api_key  # may be None -> resolved once below
        self._key_path = key_path
        self._environ = environ
        # One-time construction (noop-only candidates) loads/validates
        # key in-process and validates transport/model settings.
        self._probe = JevProvider(
            api_key=api_key, key_path=key_path, environ=environ, **kwargs)
        # Reuse the probe's resolved key object for per-call providers
        # so load_key_in_process is not re-run per step.
        self._resolved_key = self._probe._api_key
        self.calls = 0
        self.last_summary: dict = {}
        self.last_candidates: dict = {}
        self.last_criteria: dict = {}

    def _build(self, obs, candidates, criteria) -> JevProvider:
        kw = {k: v for k, v in self._template_kwargs.items() if v is not None}
        return JevProvider(api_key=self._resolved_key,
                           candidates=candidates, criteria=criteria, **kw)

    def __call__(self, obs: Observation) -> dict:
        self.calls += 1
        candidates, criteria = build_candidates(obs)
        self.last_candidates = candidates
        self.last_criteria = criteria
        provider = self._build(obs, candidates, criteria)
        action = provider(obs)
        self.last_summary = provider.last_summary
        return action
