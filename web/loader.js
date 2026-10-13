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
} from './pkg/peacecraft3.js';

// The original glue-screen frames, extracted from the shipped archives into
// `assets/` next to this page. A browser cannot read an install directory, so
// serving the few files the menu needs is what lets it draw the real button art
// instead of a rectangle that resembles it.
//
// Each name here is the one `mainmenu::served_name` maps the game's own path to.
const GLUE = [
  ['menu/button.blp', './assets/button.blp'],
  ['menu/border.blp', './assets/border.blp'],
  ['menu/bg.blp', './assets/bg.blp'],
  ['menu.ttf', './assets/menu.ttf'],
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
      const n = add_asset(name, bytes);
      say(`glue art ${name}: ${n} bytes -> ${stored_asset(name) ? 'readable' : 'NOT readable'}`);
    } catch (err) {
      fail(`glue art ${url}`, err);
    }
  }

  // The logo model and its texture, for the expansion menu's entrance.
  let panelOk = 0;
  const panelMissing = [];
  for (const [name, url] of MENU_PANEL) {
    try {
      const r = await fetch(url);
      if (!r.ok) { panelMissing.push(name); continue; }
      add_asset(name, new Uint8Array(await r.arrayBuffer()));
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
      const n = add_asset(scene.model, bytes);
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
      add_asset(`${scene.dir}/${file}`, new Uint8Array(await r.arrayBuffer()));
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
  await init();
} catch (error) {
  fail('wasm module', error);
  for (const id of ['archives', 'self', 'drop']) document.getElementById(id).disabled = true;
  throw error;
}
say('wasm module loaded');

// Every archive the user picks is read into memory and opened there. Nothing is
// written anywhere: `add_archive` reports what the archive's own tables say.
for (const el of [document.getElementById('self'), document.getElementById('drop')]) {
  el.addEventListener('click', () => {
    try {
      if (el.id === 'self') {
        say(selftest());
        say(`in memory: ${stored_assets().join(', ') || '(nothing)'}`);
      } else {
        say(`dropped ${drop_assets()} entries`);
      }
    } catch (err) {
      fail(el.id, err);
    }
  });
}

document.getElementById('archives').addEventListener('change', async (ev) => {
  const files = [...ev.target.files];
  for (const file of files) {
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      const summary = add_archive(file.name, bytes);
      say(summary, summary.startsWith('✕'));
    } catch (err) {
      fail(file.name, err);
    }
  }
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
  stored_assets,
};

// Expose initialized APIs and attach controls before the larger scene assets
// arrive. Each renderer waits for its own required bytes in the memory store.
await loadGlueArt();
