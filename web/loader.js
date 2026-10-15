// Boots the wasm launcher, hands it the original glue-screen art the menu is
// drawn from, and hands it the archives the user picks.
//
// Plain static files, deliberately: no trunk, no wasm-pack, no bundler step.
// The only build that has to happen is
//
//     cargo build --target wasm32-unknown-unknown --release -p peacecraft3
//     wasm-bindgen --target web --out-dir web/pkg \
//         target/wasm32-unknown-unknown/release/peacecraft3.wasm
//
// which produces the `web/pkg/` this imports. See ../crates/peacecraft3/README.md.

import init, {
  add_archive,
  add_asset,
  drop_assets,
  selftest,
  stored_asset,
  stored_assets,
  begin_archive_import, cancel_archive_import, stage_archive,
  pending_archive_member, commit_archive_import, archive_member,
  asset_revision, menu_music_path, menu_font_files, menu_font_alias,
  inspect_map_archive, submit_map_configuration, poll_map_configuration_result,
  enable_local_demo, local_menu_editions, open_local_menu, active_local_menu_edition,
  menu_state,
} from './pkg/peacecraft3.js';
import { discoverIso, importIsoCandidates } from './iso/import.js';
import { clickDescriptor } from './iso/audio.js';
import { createLoadingProgress, monitorWasmDownload } from './loading.js';
import { createInstallerDecoder } from './iso/installer-import.js';
import { installMapConfiguration } from './map-configuration.js';
import './menu-settings.js';
let mapConfiguration;
const demoOnly = document.documentElement.dataset.localAssets === 'true';
const wasmLoading = createLoadingProgress({ progressId: 'wasm-progress', statusId: 'wasm-status' });
const isoLoading = createLoadingProgress({ progressId: 'iso-byte-progress', statusId: 'iso-progress' });
let localImportActive = demoOnly || new URLSearchParams(location.search).has('local');
function registerServedAsset(name, bytes) { return localImportActive ? 0 : add_asset(name, bytes); }

// The original glue-screen frames, extracted from the shipped archives into
// `assets/` next to this page. A browser cannot read an install directory, so
// serving the few files the menu needs is what lets it draw the real button art
// instead of a rectangle that resembles it.
//
// Each name here is the one `mainmenu::served_name` maps the game's own path to.
const GLUE = [
  ['menu/globalstrings.fdf', './assets/globalstrings.fdf'],
  ['menu/war3skins.txt', './assets/war3skins.txt'],
  ['menu/standardtemplates.fdf', './assets/standardtemplates.fdf'],
  ['menu/mainmenu.fdf', './assets/mainmenu.fdf'],
  ['menu/button.blp', './assets/button.blp'],
  ['menu/border.blp', './assets/border.blp'],
  ['menu/bg.blp', './assets/bg.blp'],
  // `Textures\WarcraftIII-logo-alpha.blp`, 512x256. Four-component JPEG whose
  // fourth component is alpha, so the artwork has a transparent field. It is
  // the *Reign of Chaos* logo and the expansion FDF comments its model out, so
  // a TFT menu should not draw it. See `mainmenu::LOGO_TEXTURE`.
  ['menu/logo.blp', './assets/logo.blp'],
];

// Hands the launcher the glue-screen art, then both main-menu scenes.
//
// The scenes are registered unconditionally rather than chosen by the page: the
// page does not know which build the user is targeting, and the app does. Both
// namespaces are cheap to register and an absent one is reported.
async function loadGlueArt() {
  for (const [name, url] of GLUE) {
    try {
      const r = await fetch(url);
      if (!r.ok) {
        say(`glue art ${url}: HTTP ${r.status}`, true);
        continue;
      }
      const bytes = new Uint8Array(await r.arrayBuffer());
      const n = registerServedAsset(name, bytes);
      say(`glue art ${name}: ${n} bytes -> ${stored_asset(name) ? 'readable' : 'NOT readable'}`);
    } catch (err) {
      fail(`glue art ${url}`, err);
    }
  }

  try {
    for (const path of menu_font_files()) {
      const alias = menu_font_alias(path);
      if (!alias) throw Error(`invalid derived font path ${path}`);
      const response = await fetch(`./assets/fonts/${encodeURIComponent(alias.split('/').at(-1))}`);
      if (!response.ok) throw Error(`${path}: HTTP ${response.status}`);
      registerServedAsset(alias, new Uint8Array(await response.arrayBuffer()));
      say(`menu font ${path}: derived from FDF and skin table`);
    }
  } catch (error) { fail('menu font binding', error); }

  // The logo model and its texture, for the expansion menu's entrance.
  let panelOk = 0;
  const panelMissing = [];
  for (const [name, url] of MENU_PANEL) {
    try {
      const r = await fetch(url);
      if (!r.ok) { panelMissing.push(name); continue; }
      registerServedAsset(name, new Uint8Array(await r.arrayBuffer()));
      panelOk += 1;
    } catch (err) {
      panelMissing.push(name);
    }
  }
  say(
    `menu-exp logo: ${panelOk}/${MENU_PANEL.length}` +
      (panelMissing.length ? `, absent: ${panelMissing.join(', ')}` : ''),
    panelMissing.length > 0,
  );

  for (const scene of SCENES) {
    await loadScene(scene);
  }
}

// The expansion menu's own entrance sprite: the Frozen Throne logo. A separate
// namespace from the scene because it is separate art with its own model and
// texture — see `docs/re-notes/menu-panel.md`. Served names match
// `menu_panel::LOGO_MODEL_SERVED` / `LOGO_TEXTURE_SERVED`.
const MENU_PANEL = [
  ['menu-exp/warcraftiiilogo_exp.mdx', './assets/menu-exp/warcraftiiilogo_exp.mdx'],
  ['menu-exp/warcraftiiiftlogo.blp', './assets/menu-exp/warcraftiiiftlogo.blp'],
];

// The two shipped main-menu scenes. They are **different scenes in different
// archives** — see `docs/re-notes/main-menu-rendering.md` — so both are served
// and the app chooses by its target: the expansion ("The Frozen Throne") scene
// lives in `War3x.mpq` and is an iced scene, while the base ("Reign of Chaos")
// one is an Orc camp.
//
// The served name is the contract with the Rust side: the model is
// `<namespace>/<model file>` and each texture is `<namespace>/<lowercased
// basename>`.
const SCENES = [
  {
    label: 'menu3d (Reign of Chaos)',
    dir: 'menu3d',
    model: 'menu3d/mainmenu3d.mdx',
    url: './assets/menu3d/mainmenu3d.mdx',
    textures: [
    'anyhandleredwrapped.blp',
    'arrow_64.blp',
    'axebladebluesteel.blp',
    'banner_orc2.blp',
    'catails.blp',
    'clouds8x8.blp',
    'cloudstile2.blp',
    'critter_bird.blp',
    'dandilion.blp',
    'dirt1.blp',
    'dirt2c.blp',
    'dust3_thunder.blp',
    'dust3x.blp',
    'elwynnflower03a.blp',
    'elwynnflower03b.blp',
    'elwynnpoppy2skin.blp',
    'elwynnrock1skin.blp',
    'elwynnrock2skin.blp',
    'fishblue.blp',
    'fishgreen.blp',
    'grad1a.blp',
    'grasstuft.blp',
    'horn1.blp',
    'ladybug.blp',
    'metal2.blp',
    'pond_reflect2.blp',
    'shieldwood.blp',
    'shockwavewater1.blp',
    'swordbladegoldtrim.blp',
    'treebark.blp',
    'wetrock1.blp',
    'white32.blp',
    'wood.blp',
    ],
  },
  {
    label: 'menu3d-exp (The Frozen Throne)',
    dir: 'menu3d-exp',
    model: 'menu3d-exp/mainmenu3d_exp.mdx',
    url: './assets/menu3d-exp/mainmenu3d_exp.mdx',
    textures: [
    'albatross.blp',
    'blue_glow2.blp',
    'chains_silver.blp',
    'clouds8x8.blp',
    'clouds8x8fade.blp',
    'dust5a.blp',
    'frostwyrm.blp',
    'grad1.blp',
    'ice_ice.blp',
    'ice_icedull.blp',
    'ice_natural01.blp',
    'iceberg_new.blp',
    'lensflare1ax.blp',
    'ocean_h.01.blp',
    'ocean_h.02.blp',
    'ocean_h.03.blp',
    'ocean_h.04.blp',
    'ocean_h.05.blp',
    'ocean_h.06.blp',
    'ocean_h.07.blp',
    'ocean_h.08.blp',
    'ocean_h.09.blp',
    'ocean_h.10.blp',
    'ocean_h.11.blp',
    'ocean_h.12.blp',
    'ocean_h.13.blp',
    'ocean_h.14.blp',
    'ocean_h.15.blp',
    'ocean_h.16.blp',
    'ocean_h.17.blp',
    'ocean_h.18.blp',
    'ocean_h.19.blp',
    'ocean_h.20.blp',
    'ocean_h.21.blp',
    'ocean_h.22.blp',
    'ocean_h.23.blp',
    'ocean_h.24.blp',
    'ocean_h.25.blp',
    'ocean_h.26.blp',
    'ocean_h.27.blp',
    'ocean_h.28.blp',
    'ocean_h.29.blp',
    'ocean_h.30.blp',
    'shockwavewater1.blp',
    'sky2.blp',
    'snowflake.blp',
    'snowyrocksdull2.blp',
    'star32.blp',
    'white_64_foam1.blp',
    ],
  },
];

// Registers one scene's model and textures, and says plainly whether each was
// there. A surface that is absent is reported as absent rather than silently
// skipped: the app decides what to draw from what the store holds, so a missing
// texture has to be visible here or it becomes a flat-shaded geoset with no
// explanation.
async function loadScene(scene) {
  let modelOk = false;
  try {
    const r = await fetch(scene.url);
    if (r.ok) {
      const bytes = new Uint8Array(await r.arrayBuffer());
      const n = registerServedAsset(scene.model, bytes);
      modelOk = stored_asset(scene.model) !== undefined;
      say(`${scene.model}: ${n} bytes -> ${modelOk ? 'readable' : 'NOT readable'}`);
    } else {
      say(`${scene.model}: absent (HTTP ${r.status})`, true);
    }
  } catch (err) {
    fail(scene.model, err);
  }

  let got = 0;
  const missing = [];
  for (const file of scene.textures) {
    try {
      const r = await fetch(`./assets/${scene.dir}/${file}`);
      if (!r.ok) { missing.push(file); continue; }
      registerServedAsset(`${scene.dir}/${file}`, new Uint8Array(await r.arrayBuffer()));
      got += 1;
    } catch (err) {
      missing.push(file);
    }
  }
  say(
    `${scene.label}: model ${modelOk ? 'ok' : 'MISSING'}, ` +
    `textures ${got}/${scene.textures.length}` +
    (missing.length ? `, absent: ${missing.slice(0, 5).join(', ')}${missing.length > 5 ? ' …' : ''}` : ''),
    !modelOk || missing.length > 0,
  );
}

const log = document.getElementById('log');

function say(line, bad = false) {
  const li = document.createElement('li');
  li.textContent = line;
  if (bad) {
    li.className = 'bad';
  }
  log.prepend(li);
  // Keep the visible log bounded; the wasm module keeps its own.
  while (log.children.length > 50) {
    log.lastElementChild.remove();
  }
}

function fail(where, err) {
  say(`${where}: ${err instanceof Error ? err.message : String(err)}`, true);
}

try {
  if (demoOnly) {
    wasmLoading.update({ phase: 'Connecting to demo download' });
    const response = await fetch(new URL('./pkg/peacecraft3_bg.wasm', import.meta.url));
    await init({ module_or_path: monitorWasmDownload(response, wasmLoading) });
    wasmLoading.complete('Demo ready. Choose your local files.');
  } else await init();
  if (demoOnly) enable_local_demo();
} catch (error) {
  wasmLoading.error(`Demo loading failed: ${error.message}`);
  fail('wasm module', error);
  for (const id of ['archives', 'self', 'drop', 'iso', 'iso-import', 'iso-cancel']) document.getElementById(id).disabled = true;
  throw error;
}
say('wasm module loaded');

// Every archive the user picks is read into memory and opened there. Nothing is
// written anywhere: `add_archive` reports what the archive's own tables say.
for (const el of [document.getElementById('self'), document.getElementById('drop')]) {
  el.addEventListener('click', () => {
    if (isoBusy) return;
    try {
      if (el.id === 'self') {
        say(selftest());
        say(`in memory: ${stored_assets().join(', ') || '(nothing)'}`);
      } else {
        say(`dropped ${drop_assets()} entries`);
        globalThis.peacecraft3Audio?.resetLocalAssets();
        refreshDemoControls();
        mapConfiguration?.refreshArchives();
        isoLoading.reset('Loaded assets cleared. Choose local files.');
      }
    } catch (err) {
      fail(el.id, err);
    }
  });
}

document.getElementById('archives').addEventListener('change', async (ev) => {
  const files = [...ev.target.files];
  if (demoOnly) {
    if (isoBusy || !files.length) return;
    isoController = new AbortController();
    isoState(true);
    isoLoading.update({ phase: 'Reading local archives', loadedBytes: 0 });
    try {
      begin_archive_import();
      const totalBytes = files.reduce((sum, file) => sum + file.size, 0); let loadedBytes = 0;
      for (const file of files) {
        const bytes = new Uint8Array(await file.arrayBuffer());
        isoController.signal.throwIfAborted();
        stage_archive(file.name, bytes);
        loadedBytes += bytes.length;
        isoLoading.update({ phase: 'Reading local archives', loadedBytes, totalBytes, totalReliable: true });
        await new Promise(resolve => setTimeout(resolve, 0));
      }
      isoController.signal.throwIfAborted();
      isoLoading.update({ phase: 'Validating authored dependencies locally' });
      await new Promise(resolve => setTimeout(resolve, 0));
      const diagnostics = commit_archive_import();
      for (const line of diagnostics) say(line, true);
      await applyLocalAudio({ sourceName: files.map(f => f.name).join(', '), edition: null, build: null });
      refreshDemoControls();
      say('Local archives committed; available menus come from validated content');
      isoLoading.complete('Local archives imported. Available menus follow supplied content.');
    } catch (error) { cancel_archive_import(); isoLoading.error(error.name === 'AbortError' ? 'Import cancelled; current assets retained' : String(error)); fail('local archives', error); }
    finally { isoState(false); }
    ev.target.value = '';
    return;
  }
  for (const file of files) {
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      const summary = add_archive(file.name, bytes);
      say(summary, summary.startsWith('✕'));
    } catch (err) {
      fail(file.name, err);
    }
  }
  mapConfiguration?.refreshArchives();
  ev.target.value = '';
});

// Exported for the console, so the module's API can be poked at by hand: the
// same calls a future asset loader would make.
window.peacecraft3 = {
  add_archive,
  add_asset,
  drop_assets,
  selftest,
  stored_asset,
  stored_assets, begin_archive_import, cancel_archive_import, stage_archive,
  pending_archive_member, commit_archive_import, archive_member,
  asset_revision, menu_music_path, menu_font_files, menu_font_alias, discoverIso, importIsoCandidates,
  local_menu_editions, open_local_menu, active_local_menu_edition,
  menu_state,
};

function refreshDemoControls() {
  if (!demoOnly) return;
  const select = document.getElementById('demo-edition');
  const start = document.getElementById('demo-start');
  const editions = local_menu_editions();
  const previous = select.value;
  select.replaceChildren();
  for (const edition of editions) {
    const option = document.createElement('option');
    option.value = edition;
    option.textContent = edition === 'roc' ? 'Reign of Chaos' : 'The Frozen Throne';
    select.append(option);
  }
  if (!editions.length) {
    select.append(new Option('Import local assets first', ''));
  } else {
    select.value = editions.includes(previous) ? previous : editions.at(-1);
  }
  select.disabled = start.disabled = isoBusy || editions.length === 0;
  if (!editions.length) {
    document.getElementById('launcher-canvas').hidden = true;
    document.getElementById('demo-placeholder').hidden = false;
  }
}
if (demoOnly) {
  document.getElementById('demo-start').addEventListener('click', () => {
    try {
      open_local_menu(document.getElementById('demo-edition').value);
      document.getElementById('launcher-canvas').hidden = false;
      document.getElementById('demo-placeholder').hidden = true;
      document.getElementById('launcher-canvas').focus({ preventScroll: true });
    } catch (error) { fail('local menu', error); }
  });
}

const isoInput = document.getElementById('iso');
const isoImport = document.getElementById('iso-import');
const isoCancel = document.getElementById('iso-cancel');
const isoProgress = document.getElementById('iso-progress');
const isoCandidates = document.getElementById('iso-candidates');
const isoProvenance = document.getElementById('iso-provenance');
let isoFile, isoDiscovery, isoController, isoBusy = false, installerDecoder, installerNeedsRestart = false;
if (demoOnly) refreshDemoControls();
function isoState(busy) {
  isoBusy = busy; isoInput.disabled = busy;
  mapConfiguration?.setImportBusy(busy);
  document.getElementById('archives').disabled = busy;
  document.getElementById('drop').disabled = busy;
  isoCancel.disabled = !busy;
  isoImport.disabled = busy || !isoDiscovery?.candidates.length;
  for (const input of isoCandidates.querySelectorAll('input')) input.disabled = busy;
  if (demoOnly) refreshDemoControls();
}
isoCancel.addEventListener('click', () => isoController?.abort());
function installerProgress(work) {
  isoLoading.update(work.phase === 'decode'
    ? { phase: 'Unpacking installer files locally', loadedBytes: work.decodedBytes, totalBytes: work.decodedTotal, totalReliable: true }
    : { phase: work.phase });
}
isoInput.addEventListener('change', async () => {
  const file = isoInput.files[0];
  if (!file || isoBusy) return;
  isoFile = file; isoDiscovery = null; isoCandidates.replaceChildren();
  installerDecoder?.close(); installerDecoder = undefined; installerNeedsRestart = false;
  isoController = new AbortController(); isoState(true);
  isoLoading.update({ phase: 'Inspecting ISO locally' });
  let lastProgress = 0;
  try {
    isoDiscovery = await discoverIso(file, { signal: isoController.signal, onProgress(work) {
      if (performance.now() - lastProgress > 100) {
        isoLoading.update({ phase: `Inspecting ISO locally (${work.reads} bounded reads)`, loadedBytes: work.bytesRead });
        lastProgress = performance.now();
      }
    } });
    if (!isoDiscovery.candidates.length && isoDiscovery.diagnostics.some(d => d.code === 'UNSUPPORTED_FREEARC')) {
      installerDecoder = createInstallerDecoder({ onProgress: installerProgress });
      const inspected = await installerDecoder.inspect(file, isoDiscovery, isoController.signal);
      isoDiscovery = { ...isoDiscovery, candidates: inspected.candidates, installer: inspected.diagnostic };
    }
    isoProvenance.textContent = JSON.stringify({ provenance: isoDiscovery.provenance, diagnostics: isoDiscovery.diagnostics, installer: isoDiscovery.installer }, null, 2);
    isoDiscovery.candidates.forEach((candidate, index) => {
      const label = document.createElement('label'), input = document.createElement('input');
      input.type = 'checkbox'; input.value = String(index); input.checked = isoDiscovery.candidates.length === 1;
      label.append(input, document.createTextNode(` ${candidate.path} — ${candidate.archiveSize.toLocaleString()} bytes${candidate.validation === 'directory-only' ? ' (installer file; header checked during extraction)' : ` at ${candidate.offset}`}`));
      isoCandidates.append(label);
    });
    isoLoading.complete(isoDiscovery.installer
      ? `${isoDiscovery.candidates.length} MPQ-named installer files found. Imports use stored base files; language patches are not applied. Select an archive to preview its supplied menu.`
      : `${isoDiscovery.candidates.length} readable MPQ archives found; edition/build not established`);
    if (!isoDiscovery.candidates.length) {
      const codes = [...new Set(isoDiscovery.diagnostics.filter(d => d.code.startsWith('UNSUPPORTED_')).map(d => d.code))];
      isoLoading.error(`No readable MPQ archives found. ${codes.join(', ') || 'No supported payload signature'}. Import details show the local decoder blocker.`);
    }
    if (!isoDiscovery.candidates.length) say('No directly supported MPQ payload. See source diagnostics for the local decoder blocker.', true);
  } catch (error) {
    installerDecoder?.close(); installerDecoder = undefined;
    isoLoading.error(error.name === 'AbortError' ? 'Inspection cancelled; current assets retained' : String(error));
    fail('local ISO discovery', error);
  } finally { isoState(false); }
});
async function applyLocalAudio(provenance) {
  const sources = {};
  try {
    const metadataPath = 'UI\\SoundInfo\\UISounds.slk';
    const descriptor = clickDescriptor(archive_member(metadataPath), metadataPath);
    sources.click = { ...descriptor, blob: new Blob([archive_member(descriptor.path)]), provenance: { ...descriptor.provenance, source: provenance } };
  } catch (error) { fail('local click audio', error); }
  try {
    const path = menu_music_path();
    sources.music = { path, blob: new Blob([archive_member(path)]), provenance: { source: provenance, binding: 'existing native menu music contract', volume: 'original user setting unverified' } };
  } catch (error) { fail('local music', error); }
  if (!globalThis.peacecraft3Audio?.setLocalAssets) throw new Error('Local audio setter unavailable');
  globalThis.peacecraft3Audio.resetLocalAssets();
  try { await globalThis.peacecraft3Audio.setLocalAssets(sources); }
  catch (error) { fail('local audio format/codec', error); }
  say('Local music retains browser loop/volume policy; original user music-volume setting is unverified');
}
isoImport.addEventListener('click', async () => {
  if (isoBusy || !isoDiscovery) return;
  const selected = [...isoCandidates.querySelectorAll('input:checked')].map(input => isoDiscovery.candidates[Number(input.value)]);
  isoController = new AbortController(); isoState(true);
  isoLoading.update({ phase: 'Reading selected MPQs locally', loadedBytes: 0 });
  const priorLocalMode = localImportActive; localImportActive = true;
  try {
    let result;
    if (isoDiscovery.installer) {
      if (!selected.length) throw new Error('Select at least one installer archive file');
      if (selected.some(c => c.archiveSize > 512 * 1024 * 1024) || selected.reduce((sum, c) => sum + c.archiveSize, 0) > 1024 * 1024 * 1024)
        throw new Error('Selected installer archives exceed the local memory budget');
      if (installerNeedsRestart) {
        installerDecoder?.close(); installerDecoder = createInstallerDecoder({ onProgress: installerProgress });
        await installerDecoder.inspect(isoFile, isoDiscovery, isoController.signal); installerNeedsRestart = false;
      }
      const extracted = await installerDecoder.extract(selected.map(c => c.id), isoController.signal);
      begin_archive_import();
      for (const entry of extracted.files) {
        isoController.signal.throwIfAborted();
        isoLoading.update({ phase: `Validating extracted archive ${entry.path}` });
        stage_archive(`${isoDiscovery.provenance.sourceName}:${entry.path} (stored, unpatched)`, new Uint8Array(await entry.file.arrayBuffer()));
        await new Promise(resolve => setTimeout(resolve, 0));
      }
      isoController.signal.throwIfAborted();
      isoLoading.update({ phase: 'Validating authored model dependencies locally' });
      await new Promise(resolve => setTimeout(resolve, 0)); isoController.signal.throwIfAborted();
      const diagnostics = commit_archive_import();
      result = { provenance: { ...isoDiscovery.provenance, variant: extracted.variant, languagePatchesApplied: false },
        canonicalPaths: stored_assets(), revision: asset_revision(), diagnostics };
    } else result = await importIsoCandidates(isoFile, isoDiscovery, selected, window.peacecraft3, { signal: isoController.signal, onProgress(work) {
      isoLoading.update(work.phase === 'hydrate' ? { phase: 'Validating authored model dependencies locally' } : { phase: 'Reading MPQs locally', loadedBytes: work.imported, totalBytes: work.total, totalReliable: true });
    } });
    localImportActive = true;
    isoProvenance.textContent = JSON.stringify(result, null, 2);
    for (const line of result.diagnostics) say(line, true);
    await applyLocalAudio(result.provenance);
    refreshDemoControls();
    isoLoading.complete(`Committed local assets at revision ${result.revision}; ${result.canonicalPaths.length} runtime paths. Edition/build unverified.`);
    window.dispatchEvent(new CustomEvent('peacecraft3-local-import', { detail: result }));
  } catch (error) {
    cancel_archive_import();
    if (isoDiscovery.installer) installerNeedsRestart = true;
    isoLoading.error(error.name === 'AbortError' ? 'Import cancelled; current assets retained' : String(error));
    localImportActive = priorLocalMode;
    fail('local ISO import', error);
  } finally {
    if (isoDiscovery.installer) await installerDecoder?.cleanup().catch(error => fail('temporary local files', error));
    isoState(false);
  }
});

// Expose initialized APIs and attach controls before the larger scene assets
// arrive. Each renderer waits for its own required bytes in the memory store.
if (demoOnly) { isoState(false); document.getElementById('self').disabled = false; }
if (!localImportActive) await loadGlueArt();
else say('Local-only asset mode: waiting for an ISO or archive selection');

mapConfiguration = installMapConfiguration({ api: {
  asset_revision, inspect_map_archive, submit_map_configuration, poll_map_configuration_result,
} });
mapConfiguration?.setImportBusy(isoBusy);
window.addEventListener('pagehide', event => { if (!event.persisted) mapConfiguration?.dispose(); });
