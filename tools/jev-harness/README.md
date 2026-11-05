# JEV gameplay harness (mechanics prototype)

Provider-neutral, bounded capture→provider→validate→apply→capture loop.
FakeProvider is the offline default. `JevProvider` implements the official
TypeSafe AI System One contract verified in `.scratch/jev-research/report.md`
(official OpenAPI at `https://api.typesafe.ai/openapi.json`, docs at
`https://docs.typesafe.ai`). No OpenAI/chat schema is used anywhere.

## JevProvider (live adapter)

- **Endpoint:** `POST https://api.typesafe.ai/v1/systemone`
- **Auth:** `Authorization: Bearer $TYPESAFE_API_KEY`, key loaded in-process
  only via `load_key_in_process()` with explicit precedence:
  explicit `path` argument > `TYPESAFE_API_KEY` env (in-process
  `os.environ` read) > legacy default file
  `/home/maci/Desktop/peacecraft3/.secrets` (parent checkout). Never
  logged, never sent as model text, never sourced as shell; transport
  refuses redirects so the header is never forwarded across hosts. No
  filesystem search for other credential files; no alternate access
  after a read refusal.
- **Pinned model:** `jev-1.14.0` (verified current 2026-10-14; aliases
  `jev-latest`/`jev-preview` resolve to it — code pins the versioned ID and
  fails closed on any other response model).
- **Request:** `{model, state, questions:{action:{type:"choice",
  instructions, criteria:map<id,text>}}}`. No `messages`/`tools`/
  chat-completions fields exist in this API.
- **Response:** `{model, answers:{action:{type:"choice", choice,
  confidence, probabilities}}, usage:{input_tokens, output_tokens}}`.
  `choice` must be in the candidate set, probabilities must be finite
  0..1 covering exactly the candidate IDs and sum to ~1 (tol 0.02),
  confidence finite 0..1. Any such violation → fail closed
  (`ProviderError`), no action, no retry/replay. **Argmax policy
  (code-owned, not a contract claim):** OpenAPI documents `choice` as
  the max-probability option but does not define a non-argmax response
  with a valid offered-set choice and valid probabilities as a protocol
  violation; a second surveyed Doom demo samples from probabilities.
  This adapter explicitly selects argmax deterministically rather than
  rejecting such a response as a contract violation.
- **Action mapping:** Jev never returns actions directly. Code owns a small
  finite `candidates` map (id → strict action dict, default includes
  `stop_noop` safe no-op, `select_unit`, `move_selected`, `order_attack`);
  Jev selects a candidate ID; the strict `actions.validate_action` re-gates
  it against the viewport before `run_loop` applies it.
- **State mode (default):** structured JSON state only — no images, no raw
  assets sent. Mirrors both surveyed Doom demos (zero pixels by default).
- **Image mode (opt-in, experimental):** `include_image=True` sends the
  viewport PNG as an opaque asset ref that appears in `state["frame_ref"]`,
  with `format:"png"`, bare `data_base64` (no `data:` prefix), and header
  `TypeSafe-Danger-Zone-Images: true`. `parse_box_answer`/`box_to_pixels`
  validate Jev box answers (normalized 0..1 fractions) and convert to
  clamped integer pixels. Image mode never blocks structured smoke.
- **Limits:** request ≤2 MB, response ≤1 MB, per-call timeout 30 s default,
  exactly one transport attempt per call (no retry/replay of billable
  requests). HTTP errors surface as `jev HTTP error status=<code>` only —
  raw error bodies (which may echo state) are never logged.

### Live status (honest)

- **Real-API status 2026-11-05:** no live call was made. The authorized key
  file `/home/maci/Desktop/peacecraft3/.secrets` is **not present** on this
  machine (`load_key_in_process` → `SecretsError: key file not found`);
  parent checkout listing contains no `.secrets` entry. Blocker: user must
  place `TYPESAFE_API_KEY` at that path (key from
  `https://console.typesafe.ai/keys`). No provisioning/payment/account
  action was taken.
- **Synthetic ≠ PeaceCraft play:** all runs here drive `SyntheticBackend`
  (in-memory fixture) or fully mocked transports. Nothing observed here is
  PeaceCraft gameplay or the original WC3 client.

### Run (stdlib only, headless, no network in tests)

```sh
python3 -m jev_harness.example_fake   # runnable library example (fake)
python3 -m pytest tools/jev-harness/tests -q
```

Live synthetic-state call (only when key present; max 30 s, no auto-retry):

```python
from jev_harness import JevProvider, Observation
p = JevProvider()  # key from authorized .secrets, in-process only
action = p(Observation(step=0, png_bytes=b"", width=640, height=480,
                       state={"synthetic": True, "selected": False}))
print(action, p.last_summary)  # sanitized: model/choice/usage only
```

## Backends

| Backend | What it is | Live? |
|---|---|---|
| `SyntheticBackend` | In-memory state machine | Smoke test only — **not** PeaceCraft gameplay |
| `OriginalReferenceBackend` | Original WC3 via Docker/Wine/Xvfb (`tools/original-game-reference/reference.py`) | Explicitly **NOT** PeaceCraft gameplay; needs a prepared+launched owned run_dir |
| PeaceCraft browser | Existing `browser-adapter.mjs` + Bevy WASM build | **Not wired** — needs integrator; see below |

## Bounds

Default `LoopConfig`: ≤3 model calls, ≤8 game actions, per-call 30s, total 120s. No retry/replay of state-changing or billable requests. Stop/cancel via `threading.Event` or `backend.cancel()`.

## Action schema (explicit, validated)

`click{x,y,button}` (in-bounds only), `key{key}` (allowlist only, no chords), `wait{wait_ms≤2000}`. No shell/JS/URLs/files/credential typing/navigation.

## Secrets

Credential selection is explicit (no search, no shell sourcing):
explicit `path` > `TYPESAFE_API_KEY` env > legacy default file
`/home/maci/Desktop/peacecraft3/.secrets` (parent checkout). `.secrets`
is git-ignored (root `.gitignore` entries `.secrets` and `*.secrets` —
verify before staging; private `.git/info/exclude` checked 2026-11-05
contains no secrets rule but root `.gitignore` covers it). Key is loaded
in-process only via `load_key_in_process()` at the auth boundary; never
printed, never sent as model text, never sourced as shell. Trace logs
redact `Authorization`/`api-key`/`Bearer`/`sk-…` patterns. Until the
user supplies the exact path/env location, no live call is made; tests
use fake fixtures only (injected `environ` mapping / tmp `path`).

## Live-contract blockers (exact)

1. **Key location pending** (see Live status above) — no live Jev call
   until the user confirms the exact `TYPESAFE_API_KEY` path or env
   location. Adapter, schema validation, and mock-transport tests are
   complete without it; credential selection is now configurable by
   explicit path or documented env (fake fixtures only in tests).
2. **PeaceCraft browser backend unwired** — needs integrator to drive the
   existing `browser-adapter.mjs` (`createBrowserAdapter`) against a locally
   built WASM launcher; this harness defines the `GameBackend` protocol it
   must satisfy (isolated `#launcher-canvas` viewport capture only).
3. **Original-client backend** needs a prepared run_dir (game install +
   `docker build`); container `12a9ce5eacfdab02` is stopped — do not reopen
   unowned clients.

## Integrator wiring (PeaceCraft browser, one function)

```python
# Adapter satisfying GameBackend: capture() -> CDP Page.captureScreenshot
# clipped to #launcher-canvas; apply() -> existing validated input path
# (browser-adapter.mjs input/capture). No new infrastructure.
```
