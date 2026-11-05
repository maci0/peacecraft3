# JEV gameplay harness (mechanics prototype)

Provider-neutral, bounded capture→provider→validate→apply→capture loop.
FakeProvider only until lane05 verifies the official JEV endpoint/auth/model
schema — **no endpoint, model ID, or image/action schema is invented here.**

## Backends

| Backend | What it is | Live? |
|---|---|---|
| `SyntheticBackend` | In-memory state machine | Smoke test only — **not** PeaceCraft gameplay |
| `OriginalReferenceBackend` | Original WC3 via Docker/Wine/Xvfb (`tools/original-game-reference/reference.py`) | Explicitly **NOT** PeaceCraft gameplay; needs a prepared+launched owned run_dir |
| PeaceCraft browser | Existing `browser-adapter.mjs` + Bevy WASM build | **Not wired** — needs integrator; see below |

## Run (stdlib only, headless)

```sh
python3 -m jev_harness.example_fake   # runnable library example
python3 -m pytest tools/jev-harness/tests -q  # or: python3 -m unittest discover
```

Env isolation for any live run: `DISPLAY`/`WAYLAND_DISPLAY` unset, Pulse
inaccessible, ALSA `ALSA_CARD`/`/dev/null`, browser `headless:true` +
`--mute-audio`, native backend private Xvfb/null sink. Clean up own processes.

## Bounds

Default `LoopConfig`: ≤3 model calls, ≤8 game actions, per-call 30s, total
120s. No retry/replay of state-changing or billable requests. Stop/cancel via
`threading.Event` or `backend.cancel()`.

## Action schema (explicit, validated)

`click{x,y,button}` (in-bounds only), `key{key}` (allowlist only, no chords),
`wait{wait_ms≤2000}`. No shell/JS/URLs/files/credential typing/navigation.

## Secrets

Authorized key path: `/home/maci/Desktop/peacecraft3/.secrets` (parent
checkout). `.secrets` is git-ignored (root `.gitignore` + private
`.git/info/exclude` in the shared repo — verify before staging). Key is loaded
in-process only via `load_key_in_process()` at the auth boundary; never
printed, never sent as model text, never sourced as shell. Trace logs redact
`Authorization`/`api-key`/`Bearer`/`sk-…` patterns.

## Live-contract blockers (exact)

1. **Official JEV contract unverified** — lane05 research pending. No calls
   until endpoint, auth header, model ID, image encoding, and action schema
   are confirmed from official sources.
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
