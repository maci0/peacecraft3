# JEV gameplay harness (mechanics prototype)

Provider-neutral, bounded capture→provider→validate→apply→capture loop.
FakeProvider is the offline default. `JevProvider` implements the official
TypeSafe AI System One contract verified in `.scratch/jev-research/report.md`
(official OpenAPI at `https://api.typesafe.ai/openapi.json`, docs at
`https://docs.typesafe.ai`). No chat-completions schema is used anywhere.

## JevProvider (live adapter)

- **Endpoint:** `POST https://api.typesafe.ai/v1/systemone`
- **Auth:** `Authorization: Bearer $TYPESAFE_API_KEY`, key loaded in-process only via `load_key_in_process()` with explicit precedence: explicit `path` argument > `TYPESAFE_API_KEY` env (in-process `os.environ` read) > default file `.secrets` at the repository root. Never logged, never sent as model text, never sourced as shell; transport refuses redirects so the header is never forwarded across hosts. No filesystem search for other credential files; no alternate access after a read refusal.
- **Pinned model:** `jev-1.14.0` (verified current 2026-10-14; aliases
  `jev-latest`/`jev-preview` resolve to it — code pins the versioned ID and
  fails closed on any other response model).
- **Request:** `{model, state, questions:{action:{type:"choice",
  instructions, criteria:map<id,text>}}}`. No `messages`/`tools`/
  chat-completions fields exist in this API.
- **Response:** `{model, answers:{action:{type:"choice", choice, confidence, probabilities}}, usage:{input_tokens, output_tokens}}`. `choice` must be in the candidate set, probabilities must be finite 0..1 covering exactly the candidate IDs and sum to ~1 (tol 0.02), confidence finite 0..1. Any violation → fail closed (`ProviderError`), no action, no retry/replay. **Argmax policy (code-owned, not a contract claim):** OpenAPI documents `choice` as the max-probability option but does not define a non-argmax response with a valid offered-set choice and valid probabilities as a protocol violation; another surveyed Doom demo samples from probabilities. The adapter explicitly selects argmax deterministically rather than rejecting such a response as a contract violation.
- **Action mapping:** Jev never returns actions directly. Code owns a small finite `candidates` map (id → strict action dict). Default `default_candidates()` is fail-closed `stop_noop` wait only; fixed synthetic clicks/key from earlier defaults were removed because they were tuned to `SyntheticBackend` and must not be treated as live-game targets. Real candidates must be integrator-supplied per-step. Jev selects a candidate ID; all candidates are schema-checked at construction (kind/button/int/non-negative/key-allowlist/wait bounds, no fake viewport), and the chosen action is re-gated against the actual viewport before `run_loop` applies it.
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

### November 5, 2026 verification

- Provider tests used injected credentials and did not call the live API.
- Early provider-loop checks used `SyntheticBackend` or mocked transports.
  Their candidate coordinates belong to the fixtures.
- **Structured observation (2ecd48b, integrated in 1ac7c02):** the bridge in
  `gameplay_observation.rs` uses `ObjectHost` state under `test-fixtures`. It
  reports owned or visible units, hexadecimal handles, positions, vitals,
  current orders and viewport coordinates. The `screen` field is null when
  projection is unavailable. This check covers observable state and orders.
  The upstream `observe()` filters for owned or visible units. The provider
  rejects explicit per-unit `visible=False` or `hidden=True` values.

## Canonical fixture client and decision adapter

The canonical gameplay-fixture client (`jev_harness.canonical_fixture.FixtureClient`,
protocol 1) speaks the file protocol in `crates/peacecraft3/src/gameplay_fixture.rs`:
typed Move/Step/Cancel/Stop orders, request ≤8192 bytes, response ≤1 MiB, atomic
writes, id-matched receipts. It is exported as `FixtureClient` from
`jev_harness` alongside `DecisionAdapter`. `DecisionAdapter` is explicit typed
canonical mode (not mouse/vision/fake clicks): the model returns only a typed
choice id; code derives legal canonical targets from current observed owned
units; choice ids map 1:1 to protocol orders via `FixtureClient`. Provider
reuse is from verified `campaign_agent` (`jev_request`/`post_json`/`answers_of`/
env key), adapted to the committed harness transport: no-retry, size caps,
no-redirect, redaction, strict action validation preserved. Key comes only
from supported worker env (`TYPESAFE_API_KEY`/`JEV_API_KEY`) or injected
mock; secret-file reads refused. Tests live under `tools/jev-harness/tests/`
(standard pytest discovery, no duplicate collection); the pc3-menu-10
security/backend/dynamic suite (78 tests) is preserved unchanged.

### Run (stdlib only, headless, no network in tests)

Env isolation for any live run: `DISPLAY`/`WAYLAND_DISPLAY` unset, Pulse
inaccessible, ALSA `ALSA_CARD`/`/dev/null`, browser `headless:true` +
`--mute-audio`, native backend private Xvfb/null sink. Clean up own processes.

```sh
PYTHONPATH=tools/jev-harness python3 -m jev_harness.example_fake   # runnable library example (fake)
PYTHONPATH=tools/jev-harness python3 -m pytest tools/jev-harness/tests -q
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
| `PeaceCraftBrowserBackend` | Local browser adapter and WASM canvas | Wired with geometry checks. Live gameplay acceptance remains open. |

## Bounds

Default `LoopConfig`: ≤3 model calls, ≤8 game actions, per-call 30s, total 120s. No retry/replay of state-changing or billable requests. Stop/cancel via `threading.Event` or `backend.cancel()`.

## Action schema (explicit, validated)

`click{x,y,button}` (in-bounds only), `key{key}` (allowlist only, no chords), `wait{wait_ms≤2000}`. No shell/JS/URLs/files/credential typing/navigation.

## Secrets

Credential selection is explicit (no search, no shell sourcing): explicit `path` > `TYPESAFE_API_KEY` env > default file `.secrets` at the repository root. `.secrets` is git-ignored (root `.gitignore` entries `.secrets` and `*.secrets` — verify before staging; private `.git/info/exclude` checked 2026-11-05 contains no secrets rule but root `.gitignore` covers it). Key is loaded in-process only via `load_key_in_process()` at the auth boundary; never printed, never sent as model text, never sourced as shell. Trace logs redact `Authorization`/`api-key`/`Bearer`/`sk-…` patterns. Until the user supplies the exact path/env location, no live call is made; tests use fake fixtures only (injected `environ` mapping / tmp `path`).

## Live-contract blockers (exact)

1. **Key location pending** (see Live status above) — no live Jev call until the user confirms the exact `TYPESAFE_API_KEY` path or env location. Adapter, schema validation, and mock-transport tests are complete without it; credential selection is now configurable by explicit path or documented env (fake fixtures only in tests).
2. **PeaceCraft browser live acceptance remains open.** The backend checks
   the clipped canvas and PNG geometry before input with the local WASM app.
3. **Original-client backend** needs a prepared run_dir (game install +
   `docker build`); container `12a9ce5eacfdab02` is stopped — do not reopen
   unowned clients.

## Capture scope

The original reference backend uses a root capture from its owned Xvfb
display. It verifies PNG dimensions and the sidecar’s reported screen and
window geometry. Input requires a successful capture. These checks can reject
a mismatched viewport, but they do not prove the frame still represents the
same state when input arrives.

The browser backend checks its canvas bounds and clipped capture before
mapping input. Mock tests exercise both backends, including geometry failures
and invalidated capture state. These tests provide no live gameplay receipt.

## Observation bridge integration (2ecd48b, pc3-menu-10)

`crates/peacecraft3/src/gameplay_observation.rs` (`#[cfg(feature = "test-fixtures")]`,
public `observe(ObjectHost, camera, terrain_z, limit) -> Observation`) integrated
cleanly onto 3603772: +257 lines, `campaign_entry_gpu_tests.rs` harness only.
Contract available here: read-only structured-state sensors over ObjectHost
executed VM state — own/visible units only (vision_cell Visible), opaque hex
HandleId, live/creation-request position, vitals, `GetUnitCurrentOrder`, camera
`world_to_viewport` screen + in_viewport, selection, game_time, camera bounds.
Hidden enemies excluded; no raw assets/paths/secrets. Ignored GPU test
`structured_observation_bridge_before_after_one_real_order` proves before/after
one real `IssuePointOrder(standard_target)` with fake policy (16 units,
moved_sq=40000 on Prologue02 per prior run; not re-run, no changed behavior).

Verification (headless/muted): `cargo test -p peacecraft3 --features
test-fixtures` → 80 + 950 + 2 + 209 + 217 + 7 passed, 0 failed, 127 ignored
(lane run). `pytest tools/jev-harness/tests` → 91 passed: the 78
pc3-menu-10 security/backend/dynamic tests plus 13 canonical tests, with no
duplicate collection (re-run 2026-11-10 on the merged tree). Mock-real
canonical fixture proof: transcript sha256
`38eeaf95ea2db7e46fd27e3a1fa4e315290696a95d1fc0345541d82dc310e3c3`,
moved_sq=332800.0, journal [1,2], child exit 0. Live blocker: no confirmed
env key location, so no live call; secret-file refusal is asserted.
Full-repo pytest has 8 pre-existing `jsonschema` ModuleNotFound failures in
other tools.
