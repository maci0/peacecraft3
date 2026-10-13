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
  // The texture `WarCraftIIILogo.mdx` names in its own TEXS chunk — 512x256,
  // opaque, drawn on black. See `mainmenu::LOGO_TEXTURE`.
  ['menu/logo.blp', './assets/logo.blp'],
];


// The MainMenu3d background's own textures, one per entry in the model's
// TEXS chunk. Served under `menu3d/<lowercased name>`; `mainmenu_scene
// ::texture_served_name` is the same mapping on the Rust side.
const SCENE_GLUE = [
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
];

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

  // The MainMenu3d background itself, then its textures. The scene fades in on
  // the first frame that can decode them.
  try {
    const r = await fetch('./assets/menu3d/mainmenu3d.mdx');
    if (r.ok) {
      const bytes = new Uint8Array(await r.arrayBuffer());
      const n = add_asset('menu3d/mainmenu3d.mdx', bytes);
      say(`menu3d model: ${n} bytes -> ${stored_asset('menu3d/mainmenu3d.mdx') ? 'readable' : 'NOT readable'}`);
    } else {
      say(`menu3d model: HTTP ${r.status}`, true);
    }
  } catch (err) {
    fail('menu3d model', err);
  }

  let got = 0;
  for (const file of SCENE_GLUE) {
    try {
      const r = await fetch(`./assets/menu3d/${file}`);
      if (!r.ok) continue;
      add_asset(`menu3d/${file}`, new Uint8Array(await r.arrayBuffer()));
      got += 1;
    } catch (err) {
      fail(`menu3d ${file}`, err);
    }
  }
  say(`menu3d background: ${got}/${SCENE_GLUE.length} textures registered`);
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

await init().catch((err) => fail('wasm module', err));
say('wasm module loaded');

// Hand the original button frames to the running app. The menu re-checks for
// them every frame, so registering them here is enough — no ordering dance.
await loadGlueArt();

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
