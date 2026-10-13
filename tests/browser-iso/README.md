# Local browser import verification

The frozen integration source/API is in `web/loader.js`, `web/index.html`,
`web/menu-audio.js`, `web/iso.js`, `web/iso/import.js`, `web/iso/audio.js`, and
`crates/peacecraft3/src/browser.rs`. Root owns WASM generation and renderer cache
invalidation. No manifest, lockfile, original assets or generated original
content are part of these fixtures.

Run the synthetic parser/import/metadata tests:

```sh
node --test tests/browser-iso/*.test.mjs
```

Run actual generated WASM and browser audio/network verification after root
builds WASM and regenerates `web/pkg`:

```sh
node tests/browser-iso/browser-runtime.mjs
```

The runner uses the existing `.scratch/browser-check` Playwright installation
and `/usr/bin/chromium`. Set `PCX_PLAYWRIGHT_MODULE` to an alternate installed
Playwright module entry point if necessary. No package install is performed.
It starts a GET-only loopback server with an explicit static file allowlist;
static audio boot URLs receive synthetic WAV bytes, never original content.
ISO and MPQ bytes are generated inside the browser and read from a local File.
Browser-context requests and server requests are captured. An unexpected URL,
request body, non-GET request, or WebSocket fails verification. Blob playback
URLs stay local. This is a real browser capture, separate from Node API traps.

The real WASM runs in a no-DOM Worker. The test verifies staged archives are
invisible, commit increments revision once, named extraction works without a
listfile, model-authored texture paths and FDF/font aliases are hydrated,
unequal duplicate members fail closed with previous assets/revision intact,
and equal duplicate members can commit. The main page exercises the actual
menu audio module: authored synthetic Volume is derived from SYLK cells;
mute and intro state survive replacement; click playback remains available;
unsupported content rejects; URL creation/revocation balances on reset.

Recorded verification: Node suite **10 passed, 0 failed, 0 skipped**; real
Chromium/WASM **15 checked assertions**, audio lifecycle assertions passed,
**0 uploads, 0 unexpected requests, 0 sockets**. The actual store self-check
reports its assets are served by the default memory source, with no missing
entries. Counts are observed synthetic results, not original-install gates.

This proves the executable import/storage/extraction/audio pipeline. It does
not prove the file-input UI or rendered menu fidelity: the synthetic font and
texture bytes are placeholders, and the model is a minimal authored fixture.
Root's actual renderer/F8/resolution checks are separate. Original ISO
filesystem/installer limitations remain explicit; no executable runs and no
remote extraction fallback exists. Edition/build/priority are never inferred
from ISO/archive filenames. Original campaigns remain blocked on real game
Host/native execution, including exact GetPlayerNeutralPassive behavior.

## Public integration APIs

- `discoverIso(file, {signal,onProgress,maxEntries})` returns checked local
  candidates, image provenance and unsupported-format diagnoses.
- `importIsoCandidates(file, discovery, selected, wasmApi, options)` stages one
  candidate at a time, honors cancellation and memory budgets, then returns
  provenance, archive offsets/sizes, normalized runtime paths (original members
  and renderer aliases), revision and hydration diagnostics.
- Rust: `begin_archive_import`, `stage_archive`, `pending_archive_member`,
  `cancel_archive_import`, `commit_archive_import`, `archive_member`,
  `asset_revision`, `menu_music_path`. Extraction errors include archive/member
  context and unsupported codec diagnoses. Overlapping unequal member bytes
  fail closed; there is no guessed priority order.
- `peacecraft3Audio.setLocalAssets({music,click})` accepts local Blob sources with
  canonical member path/provenance and authored click settings. Replacement and
  `resetLocalAssets()` revoke URLs, preserve mute/intro policy and reset playback.
  WAV/MP3 are checked by content and browser support. Existing music loop/volume
  policy is preserved; the original engine's user music-volume setting remains
  explicitly unverified.

A transaction prepares all renderer assets before changing the visible store.
Missing required frame definitions/font/models, conflicting bytes, missing
model textures and unsupported required codecs reject. Optional logo/audio
failures are diagnosed; successful graphics import does not claim sound success.
Clear/import revision changes let root invalidate caches without restarting the
app. The default Bevy reader resolves missing original named paths from committed
archives locally and caches extracted bytes; it has no HTTP fallback.
