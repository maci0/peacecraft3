"""Provider-neutral gameplay harness for PeaceCraft III (harness mechanics only).

FakeProvider is the default (no network, no key). JevProvider implements the
official TypeSafe AI System One contract verified in
.scratch/jev-research/report.md (POST /v1/systemone, pinned jev-1.14.0,
structured state, typed choice -> validated candidate action).
"""
from .actions import Action, validate_action, ValidationError, ALLOWED_KEYS
from .loop import run_loop, LoopConfig, TraceEntry, StopReason
from .provider import Provider, FakeProvider, Observation, ProviderError
from .jev_provider import (
    JevProvider, parse_box_answer, box_to_pixels, default_candidates,
    API_URL, MODEL,
)
from .backend import GameBackend, SyntheticBackend, OriginalReferenceBackend, BackendError
from .secrets import load_key_in_process, SecretsError, KEY_PATH, ENV_NAME
from .redact import redact, redact_text

__all__ = [
    "Action", "validate_action", "ValidationError", "ALLOWED_KEYS",
    "run_loop", "LoopConfig", "TraceEntry", "StopReason",
    "Provider", "FakeProvider", "Observation", "ProviderError",
    "JevProvider", "parse_box_answer", "box_to_pixels", "default_candidates",
    "API_URL", "MODEL",
    "GameBackend", "SyntheticBackend", "OriginalReferenceBackend", "BackendError",
    "load_key_in_process", "SecretsError", "KEY_PATH", "ENV_NAME",
    "redact", "redact_text",
]
__version__ = "0.1.0"
