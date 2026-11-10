"""Provider-neutral gameplay harness for PeaceCraft III (harness mechanics only).

FakeProvider is the default (no network, no key). JevProvider implements the
official TypeSafe AI System One contract verified in
.scratch/jev-research/report.md (POST /v1/systemone, pinned jev-1.14.0,
structured state, typed choice -> validated candidate action).
CanonicalFixture (protocol 1) is the PeaceCraft gameplay-fixture client;
DecisionAdapter maps typed Jev choices to canonical Move/Step/Cancel/Stop.
"""
from .actions import Action, validate_action, ValidationError, ALLOWED_KEYS
from .loop import run_loop, LoopConfig, TraceEntry, StopReason
from .provider import Provider, FakeProvider, Observation, ProviderError
from .jev_provider import (
    JevProvider, parse_box_answer, box_to_pixels, default_candidates,
    API_URL, MODEL,
)
from .canonical_fixture import (
    FixtureClient, FixtureError, FixtureTimeout, RequestTooLarge,
    FixtureResponse, build_request, PROTOCOL_VERSION,
)
from .decision_adapter import (
    DecisionAdapter, DecisionError, CanonicalChoices, build_choices,
    choice_to_action, choice_questions, canonical_request_body,
    observed_owned_units, env_key, mock_transport, typed_payload,
    CHOICE_MOVE_EAST, CHOICE_MOVE_NORTH, CHOICE_STEP, CHOICE_CANCEL,
    CHOICE_STOP, CHOICE_IDS, JEV_URL, JEV_MODEL,
)
from .backend import GameBackend, SyntheticBackend, OriginalReferenceBackend, BackendError, PeaceCraftBrowserBackend
from .dynamic_candidates import DynamicJevProvider, build_candidates
from .secrets import load_key_in_process, SecretsError, KEY_PATH, ENV_NAME
from .redact import redact, redact_text

__all__ = [
    "Action", "validate_action", "ValidationError", "ALLOWED_KEYS",
    "run_loop", "LoopConfig", "TraceEntry", "StopReason",
    "Provider", "FakeProvider", "Observation", "ProviderError",
    "JevProvider", "DynamicJevProvider", "build_candidates", "parse_box_answer", "box_to_pixels", "default_candidates",
    "API_URL", "MODEL",
    "FixtureClient", "FixtureError", "FixtureTimeout", "RequestTooLarge",
    "FixtureResponse", "build_request", "PROTOCOL_VERSION",
    "DecisionAdapter", "DecisionError", "CanonicalChoices", "build_choices",
    "choice_to_action", "choice_questions", "canonical_request_body",
    "observed_owned_units", "env_key", "mock_transport", "typed_payload",
    "CHOICE_MOVE_EAST", "CHOICE_MOVE_NORTH", "CHOICE_STEP", "CHOICE_CANCEL",
    "CHOICE_STOP", "CHOICE_IDS", "JEV_URL", "JEV_MODEL",
    "GameBackend", "SyntheticBackend", "OriginalReferenceBackend", "BackendError", "PeaceCraftBrowserBackend",
    "load_key_in_process", "SecretsError", "KEY_PATH", "ENV_NAME",
    "redact", "redact_text",
]
__version__ = "0.1.0"
