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

Run from the repository root with `PYTHONPATH=tools/jev-harness` (no new CLI;
no install step). All commands below are fully headless and muted.

```sh
PYTHONPATH=tools/jev-harness python3 -m jev_harness.example_fake   # runnable library example
PYTHONPATH=tools/jev-harness python3 -m pytest tools/jev-harness/tests -q
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

## Review note: `OriginalReferenceBackend` full-root capture risk (lane09-owned `backend.py`)

**Do NOT treat current `OriginalReferenceBackend` captures as live-game evidence.**
This section is a precise review for the lane that owns `tools/jev-harness/jev_harness/backend.py`
and the actual PeaceCraft driver, so this lane does not make conflicting edits there.

### What the current code does

`OriginalReferenceBackend.capture()` (in `backend.py`) shells out to:

```
tools/original-game-reference/reference.py capture --run-dir <run_dir> --label jev-N
```

and `tools/original-game-reference/reference.py:capture()` runs, inside the
owned Docker/Wine/Xvfb container:

```
import -window root /run/reference/captures/<label>.png
```

i.e. a **full-root (whole X framebuffer) capture**, not a game-viewport-clipped
capture. It then records `xwininfo -root -tree`, `xdpyinfo`, and a Wine video
registry dump into `<label>.json` beside the PNG.

### Why this is a risk / not yet verified

1. **Full-root != game viewport.** `import -window root` captures every pixel of
   the Xvfb root window: game client plus any other root-window content
   (Wine desktop, explorer chrome, black bars, other windows). There is **no
   step in the JEV path** that verifies the captured PNG is the Warcraft client
   viewport, that it occupies the full framebuffer, or that it matches the
   backend's declared `width`/`height` (default 1024×768).
2. **No viewport verification exists in `OriginalReferenceBackend`.** It only
   checks the subprocess exit code, then `read_bytes()`s the PNG and returns it
   with a sha256. It never:
   - parses the PNG header to confirm dimensions == `self.width × self.height`;
   - checks the capture receipt/label existed and is fresh;
   - verifies via the receipt's `xwininfo`/`xdpyinfo` dump (already written by
     `reference.py:capture()` but discarded by the backend — only `receipt`,
     `label`, `sha256` are returned) that exactly one original game window
     occupies the full framebuffer at origin at the expected extent/DPI.
3. **Correct pattern already exists, but only in `replay.py` (native targets).**
   `tools/original-game-reference/replay.py:native_window()` (used by `compare`,
   not by the JEV backend) verifies, before any input:
   - `xdpyinfo`: exactly one screen (#0, default), dimensions and DPI equal the
     scenario extent/DPI;
   - `xrandr --current`: exactly one connected output at that extent at `+0+0`;
   - `xdotool search --onlyvisible --name <title>`: exactly one visible window;
   - `xdotool getwindowgeometry --shell`: that window is `WIDTH×HEIGHT` at
     `X=0,Y=0,SCREEN=0`;
   - `xdotool windowfocus --sync` + `getwindowfocus` round-trip, and
     `check_native_pointer()` confirms the pointer after every move.
   The JEV `OriginalReferenceBackend` applies **none** of these checks, and
   `reference.py:capture()` itself only asserts the label pattern and that the
   label is unused before capturing.
4. **Therefore a live claim is unavailable today.** A full-root PNG could be a
   black framebuffer, a Wine desktop, a launcher/menu, or a window at the wrong
   extent, and the loop would still feed it to the provider and apply clicks at
   coordinates validated only against the backend's *declared* `width`/`height`,
   not the *actual* captured extent. No test in `tools/jev-harness/tests/`
   exercises this backend (by design: fake backend only, no live native client).

### What lane09 / the PeaceCraft driver must do before any live use

- In `OriginalReferenceBackend.capture()` (and `apply()` for clicks), verify
  **before returning/applying** that the capture is the game viewport:
  e.g. reuse/extend the `replay.py:native_window()` checks (single screen +
  RandR output + exactly one visible window at full extent at origin + focus),
  and confirm the PNG header extent equals the declared `width × height`.
  Refuse (raise `BackendError`, redacted) on any mismatch — never rescale or
  crop to fit.
- Until that verification lands and is proven against a real prepared+launched
  owned `run_dir`, keep the backend labelled and gated as **NOT PeaceCraft
  gameplay / unavailable for live runs**, exactly as the table above states.
- The PeaceCraft browser/WASM driver (integrator wiring, below) must satisfy
  the same rule via its own path: capture clipped to `#launcher-canvas` only,
  with actual canvas geometry checked before/after (as `browser-worker.mjs` /
  `browser-adapter.mjs` already do for `compare`).

No change was made to `backend.py`, `reference.py`, or `replay.py` in this lane.

## Integrator wiring (PeaceCraft browser, one function)

```python
# Adapter satisfying GameBackend: capture() -> CDP Page.captureScreenshot
# clipped to #launcher-canvas; apply() -> existing validated input path
# (browser-adapter.mjs input/capture). No new infrastructure.
```
