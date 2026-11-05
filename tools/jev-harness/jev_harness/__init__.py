"""Provider-neutral gameplay harness for PeaceCraft III (harness mechanics only).

No live model calls. FakeProvider is the default. JEV contract not yet verified
(lane05 research pending) — no endpoint/model/schema invented here.
"""
from .actions import Action, validate_action, ValidationError, ALLOWED_KEYS
from .loop import run_loop, LoopConfig, TraceEntry, StopReason
from .provider import Provider, FakeProvider, Observation, ProviderError
from .backend import GameBackend, SyntheticBackend, OriginalReferenceBackend, BackendError, PeaceCraftBrowserBackend
from .secrets import load_key_in_process, SecretsError, KEY_PATH
from .redact import redact, redact_text

__all__ = [
    "Action", "validate_action", "ValidationError", "ALLOWED_KEYS",
    "run_loop", "LoopConfig", "TraceEntry", "StopReason",
    "Provider", "FakeProvider", "Observation", "ProviderError",
    "GameBackend", "SyntheticBackend", "OriginalReferenceBackend", "BackendError", "PeaceCraftBrowserBackend",
    "load_key_in_process", "SecretsError", "KEY_PATH",
    "redact", "redact_text",
]
__version__ = "0.1.0"
