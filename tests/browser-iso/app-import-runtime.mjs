// Explicit local acceptance harness. Original bytes/containers/captures stay in .scratch.
import assert from 'node:assert/strict';
import { readFile, writeFile, mkdir, readdir } from 'node:fs/promises';
import { createReadStream } from 'node:fs';
import { createHash } from 'node:crypto';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { mpqFixture, isoFixture } from './fixtures.mjs';
import { clickDescriptor, soundRows } from '../../web/iso/audio.js';
const exec = promisify(execFile);
const root = fileURLToPath(new URL('../../', import.meta.url));
const output = process.env.PCX_APP_IMPORT_OUTPUT ?? `${root}.scratch/browser-iso-app-import`;
assert(output.startsWith(`${root}.scratch/`) || output.startsWith('/tmp/'), 'Original-data outputs must stay local in .scratch or /tmp');
const install = process.env.PCX_APP_IMPORT_INSTALL ?? '/home/maci/Faugus/battle-net/drive_c/Program Files (x86)/Warcraft III (Legacy)';
const expectedExe = '3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed';
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const decode = bytes => new TextDecoder('utf-8', { fatal: true }).decode(bytes);
async function fileHash(path) { const h = createHash('sha256'); for await (const b of createReadStream(path)) h.update(b); return h.digest('hex'); }
function strings(source) { return [...source.matchAll(/r"([^"]*)"|"((?:\\.|[^"\\])*)"/g)].map(m => m[1] ?? JSON.parse(`"${m[2]}"`)); }
function constant(source, name) {
  const match = source.match(new RegExp(`const\\s+${name}\\s*:\\s*&str\\s*=\\s*(r"[^"]*"|"(?:\\\\.|[^"\\\\])*")`));
  assert(match, `Missing Rust path contract ${name}`); return strings(match[1])[0];
}
function block(source, name) { const match = source.match(new RegExp(`const\\s+${name}[^=]*=\\s*\\[([\\s\\S]*?)\\];`)); assert(match, `Missing Rust contract ${name}`); return match[1]; }
function frames(text) {
  const map = new Map(), stack = []; let pending;
  for (let line of text.split(/\r?\n/)) {
    line = line.replace(/\/\/.*$/, '').trim(); if (!line) continue;
    const header = line.match(/^Frame\s+"[^"]+"\s+"([^"]+)"(?:\s+INHERITS(?:\s+WITHCHILDREN)?\s+"([^"]+)")?/);
    if (header) { pending = { name: header[1], inherits: header[2], props: {} }; map.set(pending.name, pending); }
    if (line.includes('{') && pending) { stack.push(pending); pending = null; continue; }
    if (line.startsWith('}')) { stack.pop(); continue; }
    if (stack.length && !header) { const m = line.match(/^(\w+)\s+(.+?)[,;]?$/); if (m && !(m[1] in stack.at(-1).props)) stack.at(-1).props[m[1]] = m[2]; }
  }
  function prop(name, key, seen = new Set()) {
    assert(!seen.has(name), `Frame inheritance cycle ${name}`); seen.add(name);
    const f = map.get(name); assert(f, `Missing authored frame ${name}`);
    if (key in f.props) return f.props[key];
    return f.inherits ? prop(f.inherits, key, seen) : undefined;
  }
  return { map, prop };
}
const quoted = raw => { const m = raw?.match(/"((?:\\.|[^"\\])*)"/); assert(m, `Expected quoted record: ${raw}`); return m[1].replaceAll('\\\\', '\\').replaceAll('\\"', '"'); };
export function uniqueSkinFont(skin, face) {
  let section; const matches = [];
  for (const raw of skin.split(/\r?\n/)) {
    const line = raw.split('//')[0].trim(); if (!line) continue;
    const header = line.match(/^\[([^\]]+)\]$/);
    if (header) { section = header[1].trim(); assert(section, 'Empty skin section'); continue; }
    const equal = line.indexOf('='); assert(section && equal > 0, `Malformed skin line ${line}`);
    const key = line.slice(0, equal).trim(), value = line.slice(equal + 1).trim();
    if (key === face) matches.push({ face, section, file: value });
  }
  assert.equal(matches.length, 1, `Skin face ${face} requires exactly one definition across sections: ${JSON.stringify(matches)}`);
  assert(matches[0].file, `Empty skin font file for ${face}`); return matches[0];
}
function mdlChunks(bytes) {
  assert.equal(String.fromCharCode(...bytes.subarray(0, 4)), 'MDLX');
  const v = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength), chunks = new Map();
  for (let at = 4; at < bytes.length;) { assert(at + 8 <= bytes.length); const size = v.getUint32(at + 4, true); assert(at + 8 + size <= bytes.length); chunks.set(String.fromCharCode(...bytes.subarray(at, at + 4)), bytes.subarray(at + 8, at + 8 + size)); at += 8 + size; }
  return chunks;
}
export function panelSoundEvents(bytes) {
  const chunks = mdlChunks(bytes), sequences = chunks.get('SEQS');
  assert(sequences && sequences.length % 132 === 0, 'Malformed panel SEQS');
  const intervals = [];
  for (let at = 0; at < sequences.length; at += 132) {
    const name = decode(sequences.subarray(at, at + 80)).split('\0')[0].toLowerCase().split(/\s+/);
    if (name.length !== 2 || !name.some(n => ['mainmenu', 'singleplayer', 'options'].includes(n)) || !name.some(n => ['birth', 'stand', 'death'].includes(n))) continue;
    const view = new DataView(sequences.buffer, sequences.byteOffset + at);
    const start = view.getUint32(80, true), end = view.getUint32(84, true);
    assert(start <= end); intervals.push([start, end]);
  }
  assert(intervals.length >= 2, 'Missing panel startup phases');
  const events = chunks.get('EVTS') ?? new Uint8Array(), codes = [];
  let at = 0, count = 0;
  while (at < events.length) {
    assert(at + 96 <= events.length && ++count <= 4096, 'Panel EVTS bound');
    const view = new DataView(events.buffer, events.byteOffset + at), size = view.getUint32(0, true), rest = at + size;
    assert(size >= 96 && rest + 12 <= events.length, 'Panel EVTS header');
    assert.equal(decode(events.subarray(rest, rest + 4)), 'KEVT', 'Unsupported panel event track');
    const track = new DataView(events.buffer, events.byteOffset + rest), frames = track.getUint32(4, true);
    assert(frames <= 4096 && rest + 12 + frames * 4 <= events.length, 'Panel event frame bound');
    const name = decode(events.subarray(at + 4, at + 84)).split('\0')[0];
    const selected = Array.from({ length: frames }, (_, i) => track.getUint32(12 + i * 4, true))
      .some(frame => intervals.some(([start, end]) => start <= frame && frame <= end));
    if (selected && name.slice(0, 4).toUpperCase() === 'SNDX') {
      assert.equal(name.length, 8); assert.equal(track.getUint32(8, true), 0xffffffff, 'Global panel sound unsupported');
      codes.push(name.slice(4));
    }
    at = rest + 12 + frames * 4;
  }
  return codes;
}
function mdl(bytes) {
  const chunks = mdlChunks(bytes);
  const tex = chunks.get('TEXS'); assert(tex && tex.length % 268 === 0, 'Malformed model TEXS');
  const paths = []; for (let at = 0; at < tex.length; at += 268) { const raw = tex.subarray(at + 4, at + 264); const nul = raw.indexOf(0); assert(nul >= 0); const path = decode(raw.subarray(0, nul)); if (path) paths.push(path); }
  const geos = chunks.get('GEOS'); assert(geos); let geosets = 0;
  for (let at = 0; at < geos.length;) { const len = new DataView(geos.buffer, geos.byteOffset + at).getUint32(0, true); assert(len >= 4 && at + len <= geos.length); geosets++; at += len; }
  const seq = chunks.get('SEQS'); assert(seq && seq.length % 132 === 0); const sequences = [];
  for (let at = 0; at < seq.length; at += 132) { const sv = new DataView(seq.buffer, seq.byteOffset + at, 132), name = decode(seq.subarray(at, at + 80)).split('\0')[0]; sequences.push({ name, looping: sv.getUint32(92, true) === 0, duration: sv.getUint32(84, true) - sv.getUint32(80, true) }); }
  const clip = sequences.find(s => s.name === 'Stand' && s.looping) ?? sequences.find(s => s.name === 'Stand') ?? sequences[0]; assert(clip);
  return { paths, geosets, duration: clip.duration };
}
async function prepare() {
  await mkdir(output, { recursive: true });
  assert.equal(await fileHash(`${install}/Warcraft III.exe`), expectedExe, 'Legacy executable identity changed; do not reuse install assumptions');
  const files = await readdir(install), aliases = {};
  for (const [alias, filename] of Object.entries({ base: 'War3.mpq', expansion: 'War3x.mpq', locale: 'War3Local.mpq', expansionLocale: 'War3xLocal.mpq' })) {
    const actual = files.find(f => f.toLowerCase() === filename.toLowerCase()); assert(actual, `Missing explicit Legacy archive alias ${alias}: ${filename}`);
    aliases[alias] = { path: `${install}/${actual}`, sha256: await fileHash(`${install}/${actual}`) };
  }
  const main = await readFile(`${root}crates/peacecraft3/src/mainmenu.rs`, 'utf8');
  const scene = await readFile(`${root}crates/peacecraft3/src/mainmenu_scene.rs`, 'utf8');
  const panel = await readFile(`${root}crates/peacecraft3/src/menu_panel.rs`, 'utf8');
  const audio = await readFile(`${root}crates/peacecraft3/src/menu_audio.rs`, 'utf8');
  const expansion = scene.match(/if expansion\s*\{([\s\S]*?)\}\s*else/); assert(expansion);
  const [modelPath, modelAlias] = strings(expansion[1]); assert(modelPath && modelAlias);
  const mainPath = constant(main, 'MAIN_MENU_FDF'), templatePath = constant(main, 'STANDARD_TEMPLATES_FDF'), stringsPath = constant(main, 'GLOBAL_STRINGS_FDF');
  const skinPath = constant(main, 'SKIN_TABLE');
  const fogPath = constant(main, 'MENU_FOG_CONFIG');
  const sourceRules = JSON.parse(process.env.PCX_APP_IMPORT_MEMBER_SOURCES ?? '{}');
  // Explicit TFT fixture choices supported by the existing rendering evidence;
  // this is not an MPQ priority policy or an archive release identity claim.
  sourceRules[mainPath] ??= { alias: 'expansion', evidence: 'docs/re-notes/main-menu-rendering.md: TFT MainMenu.fdf in War3x' };
  sourceRules[modelPath] ??= { alias: 'expansion', evidence: 'existing scene_source(true) contract and docs/re-notes/main-menu-rendering.md' };
  sourceRules[skinPath] ??= { alias: 'expansion', evidence: 'Explicit coordinator-approved local fixture variant consistent with static browser staging; not native archive priority proof' };
  const decodedRules = JSON.parse(process.env.PCX_APP_IMPORT_DECODED_MEMBERS ?? '{}');
  const members = {}, provenance = [], cache = new Map();
  async function extract(alias, member) {
    const key = `${alias}:${member}`; if (cache.has(key)) return cache.get(key);
    const file = `${output}/extract-${hash(Buffer.from(key))}.bin`; let result;
    try { await exec(`${root}target/debug/mpqxtract`, ['get', aliases[alias].path, member, file], { maxBuffer: 1024 * 1024 }); result = new Uint8Array(await readFile(file)); }
    catch (e) { const reason = `${e.stderr ?? ''}${e.stdout ?? ''}`.trim(); if (/not found|file not found/i.test(reason)) result = null; else throw new Error(`${alias}:${member}: extraction blocked: ${reason || e.message}`); }
    cache.set(key, result); return result;
  }
  async function add(member) {
    if (members[member]) return members[member];
    const choice = sourceRules[member], decoded = decodedRules[member]; let bytes, sources, variants = [];
    if (decoded) {
      assert(decoded.file && decoded.archiveAlias && decoded.archiveSha256 && decoded.decodedSha256 && decoded.codecSourcePath && decoded.codecSourceSha256 && decoded.evidence, `Incomplete decoded-member provenance ${member}`);
      assert(decoded.file.startsWith(`${root}.scratch/`) || decoded.file.startsWith('/tmp/'), 'Decoded original member must stay in local .scratch/tmp');
      assert.equal(aliases[decoded.archiveAlias]?.sha256, decoded.archiveSha256, `${member}: decoded source archive identity mismatch`);
      assert.equal(await fileHash(decoded.codecSourcePath), decoded.codecSourceSha256, `${member}: imported codec source changed`);
      bytes = new Uint8Array(await readFile(decoded.file)); assert.equal(hash(bytes), decoded.decodedSha256, `${member}: decoded content identity mismatch`);
      sources = [decoded.archiveAlias];
      members[member] = bytes; provenance.push({ member, sources, size: bytes.length, sha256: hash(bytes), decodedInput: decoded,
        limitation: 'Caller-imported compatibility codec weights; not native Legacy executable runtime codec proof' });
      return bytes;
    }
    if (choice) { assert(aliases[choice.alias] && choice.evidence, `Unproven explicit source rule ${member}`); bytes = await extract(choice.alias, member); assert(bytes, `${choice.alias}: missing ${member}`); sources = [choice.alias];
      if (choice.sha256) assert.equal(hash(bytes), choice.sha256, `${member}: selected fixture variant changed`);
      for (const alias of Object.keys(aliases)) { const data = await extract(alias, member); if (data) variants.push({ alias, sha256: hash(data), size: data.length }); } }
    else {
      const found = []; for (const alias of Object.keys(aliases)) { const data = await extract(alias, member); if (data) found.push({ alias, data, sha256: hash(data) }); }
      assert(found.length, `MISSING_MEMBER: ${member}`);
      if (new Set(found.map(f => f.sha256)).size !== 1) throw new Error(`AMBIGUOUS_SOURCE: ${member}: ${found.map(f => `${f.alias}=${f.sha256}`).join(', ')}; no archive priority guessed`);
      bytes = found[0].data; sources = found.map(f => f.alias); variants = found.map(({ alias, sha256, data }) => ({ alias, sha256, size: data.length }));
    }
    members[member] = bytes; provenance.push({ member, sources, size: bytes.length, sha256: hash(bytes), variants, explicitChoice: choice ?? null }); return bytes;
  }
  async function addAll(paths) {
    const errors = [];
    for (const member of new Set(paths)) { try { await add(member); } catch (error) { errors.push(String(error)); } }
    if (errors.length) throw new Error(`REQUIRED_MEMBERS_BLOCKED:\n${errors.join('\n')}`);
  }
  const templates = decode(await add(templatePath)), menu = decode(await add(mainPath)), global = decode(await add(stringsPath));
  const fdf = frames(`${templates}\n${menu}`), ids = strings(block(main, 'MENU')), frameNames = strings(block(main, 'MENU_FRAMES'));
  const labels = ids.map((id, i) => { const textFrame = quoted(fdf.prop(frameNames[i * 2], 'ButtonText')), key = quoted(fdf.prop(textFrame, 'Text'));
    const value = global.split(/\r?\n/).map(l => l.trim()).find(l => l.startsWith(`${key} `) || l.startsWith(`${key}\t`)); assert(value, `Missing authored GlobalStrings key ${key}`);
    return { id, button: frameNames[i * 2], plate: frameNames[i * 2 + 1], key, raw: quoted(value), visible: quoted(value).replace(/\|C[0-9a-f]{8}|\|R/gi, '').replaceAll('||', '|') }; });
  const skin = decode(await add(skinPath));
  await add(fogPath);
  const fonts = labels.map(label => {
    const textFrame = quoted(fdf.prop(label.button, 'ButtonText'));
    const face = quoted(fdf.prop(textFrame, 'FrameFont'));
    return { id: label.id, textFrame, ...uniqueSkinFont(skin, face) };
  });
  const fontFiles = [...new Set(fonts.map(font => font.file))].sort();
  await addAll(fontFiles);
  const requiredFrames = new Set();
  function visitFrame(name) {
    if (requiredFrames.has(name)) return; requiredFrames.add(name);
    const frame = fdf.map.get(name); assert(frame, `Missing consumed frame ${name}`);
    if (frame.inherits) visitFrame(frame.inherits);
    for (const key of ['ControlBackdrop', 'ButtonText']) if (frame.props[key]) visitFrame(quoted(frame.props[key]));
  }
  for (const name of frameNames) visitFrame(name);
  const frameAssets = names => [...names].flatMap(name => ['BackdropBackground', 'BackdropEdgeFile'].filter(key => fdf.map.get(name).props[key]).map(key => quoted(fdf.map.get(name).props[key])));
  await addAll(frameAssets(requiredFrames));
  const optionalFrameDiagnostics = [];
  for (const member of new Set(frameAssets(fdf.map.keys()))) {
    if (members[member]) continue;
    try { await add(member); } catch (error) { optionalFrameDiagnostics.push({ member, reason: String(error), scope: 'not consumed by selected menu button/style ancestry; full FDF retained unchanged' }); }
  }
  const model = await add(modelPath), expectedScene = mdl(model);
  await addAll(expectedScene.paths);
  const logo = await add(constant(panel, 'LOGO_MODEL')); for (const texture of mdlTextures(logo)) await add(texture);
  const sprite = await readFile(`${root}crates/peacecraft3/src/menu_sprites.rs`, 'utf8');
  const screens = await readFile(`${root}crates/peacecraft3/src/menu_screens.rs`, 'utf8');
  const singlePlayerPath = strings(block(screens, 'SCREEN_FILES'))[1]; assert(singlePlayerPath);
  sourceRules[singlePlayerPath] ??= { alias: 'expansion', evidence: 'Explicit local TFT submenu fixture input; not native archive priority' };
  const pendingFrames = [singlePlayerPath], includedFrames = new Set(); let frameBytes = 0;
  while (pendingFrames.length) {
    const path = pendingFrames.pop(); if (includedFrames.has(path.toLowerCase())) continue;
    includedFrames.add(path.toLowerCase()); assert(includedFrames.size <= 64, 'Submenu include count bound');
    const bytes = await add(path); frameBytes += bytes.length; assert(frameBytes <= 8 * 1024 * 1024, 'Submenu include byte bound');
    for (const match of decode(bytes).matchAll(/^\s*IncludeFile\s+"([^"]+)"/gm)) pendingFrames.push(match[1]);
  }
  const scriptPath = constant(sprite, 'BLIZZARD_SCRIPT');
  // Explicit fixture inputs, not a native archive-priority rule.
  sourceRules[scriptPath] ??= { alias: 'base', evidence: 'Explicit local base-script fixture selection; archive release and native override priority unestablished' };
  await add(scriptPath);
  const lookupRows = soundRows(await add(constant(sprite, 'ANIM_LOOKUPS')));
  const eventRows = soundRows(await add(constant(sprite, 'ANIM_SOUNDS')));
  for (const layer of strings(block(sprite, 'LAYERS'))) {
    const member = uniqueSkinFont(skin, `${layer}_V1`).file.replace(/\.mdl$/i, '.mdx');
    const bytes = await add(member); await addAll(mdlTextures(bytes));
    for (const code of panelSoundEvents(bytes)) {
      const lookup = lookupRows.filter(row => row.AnimSoundEvent === code); assert.equal(lookup.length, 1, `Ambiguous/missing panel event ${code}`);
      const rows = eventRows.filter(row => row.SoundName === lookup[0].SoundLabel); assert.equal(rows.length, 1, `Ambiguous/missing sound ${lookup[0].SoundLabel}`);
      const row = rows[0]; assert.equal(typeof row.DirectoryBase, 'string');
      assert(typeof row.FileNames === 'string' && row.FileNames && !row.FileNames.includes(','), 'Unsupported panel file list');
      await add(`${row.DirectoryBase}${row.FileNames}`);
    }
  }
  const browserSource = await readFile(`${root}crates/peacecraft3/src/browser.rs`, 'utf8');
  const metadataContract = browserSource.match(/for path in \[\s*crate::menu_audio::MENU_MUSIC_PATH,\s*(r"[^"]+")/);
  assert(metadataContract, 'Missing browser audio metadata path contract');
  const audioMetadataPath = strings(metadataContract[1])[0];
  const click = clickDescriptor(await add(audioMetadataPath), audioMetadataPath); await add(click.path);
  const musicPath = constant(audio, 'MENU_MUSIC_PATH'); await add(musicPath);
  const isoPath = `${output}/local-repacked-menu.iso`;
  await writeFile(isoPath, isoFixture([{ name: 'SELECTED.BIN;1', bytes: mpqFixture(members) }]));
  // A second, unequal authored test member verifies rejection without inventing priority.
  const conflict = { [fontFiles[0]]: new Uint8Array([1, 2, 3]) };
  const conflictPath = `${output}/unequal-members.iso`;
  await writeFile(conflictPath, isoFixture([{ name: 'A.BIN;1', bytes: mpqFixture(members) }, { name: 'B.BIN;1', bytes: mpqFixture(conflict) }]));
  const numeric = name => { const m = main.match(new RegExp(`const\\s+${name}\\s*:\\s*f32\\s*=\\s*([0-9._]+)`)); assert(m); return Number(m[1].replaceAll('_', '')); };
  const result = { install, executableSha256: expectedExe, archiveAliases: aliases, provenance, sourceRules, optionalFrameDiagnostics, modelPath, modelAlias, expectedScene, labels, fonts, fontFiles, skinPath, fogPath, click, musicPath,
    frameUnits: [numeric('FRAME_W'), numeric('FRAME_H')], frameText: `${templates}\n${menu}`, isoPath, conflictPath,
    limitation: 'Locally repacked ISO/MPQ fixture from supplied original menu members; not retail ISO/installer acceptance or proven original archive priority' };
  await writeFile(`${output}/prepared.json`, JSON.stringify(result, null, 2));
  return result;
}
function mdlTextures(bytes) {
  const v = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  for (let at = 4; at < bytes.length;) { assert(at + 8 <= bytes.length); const size = v.getUint32(at + 4, true); assert(at + 8 + size <= bytes.length);
    if (String.fromCharCode(...bytes.subarray(at, at + 4)) === 'TEXS') { assert(size % 268 === 0); const paths = []; for (let pos = at + 8; pos < at + 8 + size; pos += 268) { const raw = bytes.subarray(pos + 4, pos + 264), end = raw.indexOf(0); assert(end >= 0); const path = decode(raw.subarray(0, end)); if (path) paths.push(path); } return paths; } at += 8 + size; }
  throw Error('Missing authored model TEXS');
}
async function run() {
  assert.equal(process.env.PCX_APP_IMPORT_WASM_READY, '1', 'Generated WASM/glue readiness required before this running-app check');
  const fixture = JSON.parse(await readFile(`${output}/prepared.json`, 'utf8'));
  assert(fixture.fogPath && fixture.provenance.some(record => record.member.toLowerCase() === fixture.fogPath.toLowerCase()), 'Regenerate prepared.json and its local ISO: this fixture lacks required authored fog input');
  const { chromium } = await import(pathToFileURL(process.env.PCX_PLAYWRIGHT_MODULE ?? `${root}.scratch/browser-check/node_modules/playwright-core/index.mjs`));
  const origin = process.env.PCX_APP_IMPORT_ORIGIN ?? 'http://127.0.0.1:8931';
  const browser = await chromium.launch({ executablePath: '/usr/bin/chromium', args: ['--no-sandbox', '--enable-unsafe-swiftshader'] });
  const requests = [], errors = [], logs = [], sockets = [];
  try {
    const context = await browser.newContext({ viewport: { width: 1200, height: 1120 }, deviceScaleFactor: 1 });
    context.on('request', r => requests.push({ url: r.url(), method: r.method(), body: r.postDataBuffer() !== null }));
    const page = await context.newPage(); page.setDefaultTimeout(120000);
    page.on('pageerror', e => errors.push(e.message)); page.on('console', m => logs.push({ type: m.type(), text: m.text() })); page.on('websocket', s => sockets.push(s.url()));
    await page.addInitScript(() => { window.localImportEvents = []; window.addEventListener('peacecraft3-local-import', e => localImportEvents.push(e.detail)); });
    await page.goto(`${origin}/index.html`, { waitUntil: 'domcontentloaded' });
    await page.waitForFunction(() => globalThis.peacecraft3?.stored_asset('menu/war3skins.txt') && peacecraft3.stored_asset('menu/miscdata.txt') && globalThis.peacecraft3?.stored_asset('menu/globalstrings.fdf') && globalThis.peacecraft3?.stored_asset('menu/standardtemplates.fdf') && globalThis.peacecraft3?.stored_asset('menu/mainmenu.fdf'));
    const initialFonts = await page.evaluate(() => peacecraft3.menu_font_files().map(path => ({ path, alias: peacecraft3.menu_font_alias(path) })));
    assert(initialFonts.length > 0);
    for (const font of initialFonts) await page.waitForFunction(alias => peacecraft3.stored_asset(alias)?.length > 0, font.alias);
    // Find the rendered launcher control through screenshot OCR, not canvas DOM text.
    const settingsCapture = `${output}/settings.png`; await page.screenshot({ path: settingsCapture });
    const { stdout } = await exec('/usr/bin/tesseract', [settingsCapture, 'stdout', '--psm', '11', 'tsv'], { maxBuffer: 4 * 1024 * 1024 });
    const words = stdout.split('\n').slice(1).map(l => l.split('\t')).filter(v => v.length >= 12);
    const controlWord = value => value.replace(/[^a-z]/gi, '').toLowerCase();
    const mainWord = words.find((v, i) => controlWord(v[11]) === 'main' && controlWord(words[i + 1]?.[11] ?? '') === 'menu');
    assert(mainWord, 'Rendered Main menu control not found by local OCR; inspect settings.png');
    await page.mouse.move(Number(mainWord[6]) + Number(mainWord[8]) / 2, Number(mainWord[7]) + Number(mainWord[9]) / 2);
    await page.waitForTimeout(250);
    await page.mouse.down(); await page.waitForTimeout(100); await page.mouse.up();
    await page.waitForFunction(() => globalThis.peacecraft3Intro?.status().played);
    if (await page.evaluate(() => peacecraft3Intro.status().active)) { await page.keyboard.press('Escape'); await page.waitForFunction(() => !peacecraft3Intro.status().active); }
    const scenePattern = `Original menu background: ${fixture.expectedScene.geosets} geosets, ${fixture.expectedScene.paths.length} textures, ${fixture.expectedScene.duration} ms loop; missing []`;
    await poll(() => logs.some(l => l.text.includes(scenePattern)), 'Initial running scene must render all required assets');
    await page.screenshot({ path: `${output}/before-import.png` });
    const f8Proof = [];
    f8Proof.push(await captureFps(page, 'before-import-f8-hidden', false));
    await toggleFps(page); f8Proof.push(await captureFps(page, 'before-import-f8-visible', true));
    const initialRevision = await page.evaluate(() => peacecraft3.asset_revision());
    const logStart = logs.length, importRequestStart = requests.length;
    await page.locator('#iso').setInputFiles(fixture.isoPath);
    await page.waitForFunction(() => document.querySelectorAll('#iso-candidates input').length === 1 && !document.querySelector('#iso').disabled);
    await page.locator('#iso-candidates input').check(); await page.locator('#iso-import').click();
    await page.waitForFunction(() => localImportEvents.length === 1);
    const committed = await page.evaluate(() => ({ result: localImportEvents[0], revision: peacecraft3.asset_revision(), audio: peacecraft3Audio.status(), selftest: peacecraft3.selftest() }));
    const importedFonts = await page.evaluate(() => peacecraft3.menu_font_files().map(path => ({ path, alias: peacecraft3.menu_font_alias(path), size: peacecraft3.stored_asset(peacecraft3.menu_font_alias(path))?.length })));
    assert.deepEqual(importedFonts.map(font => font.path).sort(), fixture.fontFiles);
    assert(importedFonts.every(font => font.alias && font.size > 0));
    assert.equal(committed.revision, initialRevision + 1); assert.match(committed.selftest, /still served by the default asset source: true/);
    await poll(() => logs.slice(logStart).some(l => l.text.includes(`Asset store changed (revision ${committed.revision})`)), 'Running-app revision cache invalidation missing');
    await poll(() => logs.slice(logStart).some(l => l.text.includes(scenePattern)), 'Imported scene must rebuild in running app');
    for (const label of fixture.labels) {
      const font = fixture.fonts.find(font => font.id === label.id); assert(font);
      await poll(() => logs.slice(logStart).some(l => l.text.includes(`Menu label ${font.textFrame}: ${JSON.stringify(label.visible)} (hotkey `) && l.text.includes(`face ${JSON.stringify(font.face)}`)), `Imported authored label/font ${label.id} must rebuild`);
    }
    f8Proof.push(await captureFps(page, 'after-import-f8-visible', true));
    await toggleFps(page); f8Proof.push(await captureFps(page, 'after-import-f8-hidden', false));
    await page.screenshot({ path: `${output}/after-import-menu.png` });
    await page.locator('#launcher-canvas').screenshot({ path: `${output}/after-import-canvas.png` });
    const canvasGeometry = await page.locator('#launcher-canvas').evaluate(c => ({ width: c.width, height: c.height, bounds: c.getBoundingClientRect().toJSON() }));
    const importedOcr = await exec('/usr/bin/tesseract', [`${output}/after-import-canvas.png`, 'stdout', '--psm', '11'], { maxBuffer: 4 * 1024 * 1024 });
    const normalizeLabel = text => text.toLowerCase().replace(/[^a-z0-9]/g, '');
    const renderedLabels = fixture.labels.map(l => ({ id: l.id, visible: l.visible, recognized: normalizeLabel(importedOcr.stdout).includes(normalizeLabel(l.visible)) }));
    await writeFile(`${output}/rendered-label-review.json`, JSON.stringify({ renderedLabels, ocr: importedOcr.stdout }, null, 2));
    const automaticLabelProof = renderedLabels.every(l => l.recognized);
    if (!automaticLabelProof) console.warn('Screenshot OCR inconclusive; captures require independent visual label review. Continuing interaction/network checks.');
    assert.equal(committed.audio.localMode, true); assert.equal(committed.audio.localSources.click.path, fixture.click.path);
    assert.equal(committed.audio.clickVolume, fixture.click.settings.volume);
    await clickAuthoredEntry(page, fixture);
    await page.waitForFunction(expected => peacecraft3Audio.status().clicks === expected, committed.audio.clicks + 1, { timeout: 10000 }); const afterClick = await page.evaluate(() => peacecraft3Audio.status());
    assert.equal(afterClick.clicks, committed.audio.clicks + 1, 'Real rendered menu button must play imported local click');
    await page.setViewportSize({ width: 1500, height: 1000 }); await page.waitForTimeout(1000); await page.screenshot({ path: `${output}/resized-menu.png` });
    await page.locator('#launcher-canvas').screenshot({ path: `${output}/resized-canvas.png` });
    f8Proof.push(await captureFps(page, 'resized-f8-hidden', false));
    await toggleFps(page); f8Proof.push(await captureFps(page, 'resized-f8-visible', true));
    await toggleFps(page);
    const resizedGeometry = await page.locator('#launcher-canvas').evaluate(c => ({ width: c.width, height: c.height, bounds: c.getBoundingClientRect().toJSON() }));
    await writeFile(`${output}/canvas-geometry.json`, JSON.stringify({ imported: canvasGeometry, resized: resizedGeometry }, null, 2));
    await page.waitForFunction(() => peacecraft3Audio.status().clickPaused); await clickAuthoredEntry(page, fixture);
    await page.waitForFunction(expected => peacecraft3Audio.status().clicks === expected, afterClick.clicks + 1, { timeout: 10000 });
    assert.equal(await page.evaluate(() => peacecraft3Audio.status().clicks), afterClick.clicks + 1, 'Authored button remains clickable after resize');
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false);
    const stableRevision = await page.evaluate(() => peacecraft3.asset_revision());
    await page.locator('#iso').setInputFiles(fixture.conflictPath); await page.waitForFunction(() => document.querySelectorAll('#iso-candidates input').length === 2 && !document.querySelector('#iso').disabled);
    for (const input of await page.locator('#iso-candidates input').all()) await input.check(); await page.locator('#iso-import').click();
    await page.waitForFunction(() => document.querySelector('#iso-progress').textContent.includes('AMBIGUOUS_MEMBER'));
    assert.equal(await page.evaluate(() => peacecraft3.asset_revision()), stableRevision); await page.screenshot({ path: `${output}/unequal-refused-menu-retained.png` });
    const unexpected = requests.filter(r => !r.url.startsWith('blob:') && (!r.url.startsWith(`${origin}/`) || r.method !== 'GET' || r.body || /\.(iso|mpq)(?:[?#]|$)/i.test(r.url)));
    assert.deepEqual(unexpected, []); assert.deepEqual(sockets, []); assert.deepEqual(errors, []);
    const proof = { ok: true, automaticLabelProof, visualAcceptance: 'pending independent capture review', fixture: `${output}/prepared.json`, importedFonts, f8Proof, committed, logs, network: { requests, requestsDuringLocalImport: requests.slice(importRequestStart), uploads: 0, unexpected: 0, sockets: 0 },
      captures: ['settings.png', 'before-import.png', 'after-import-menu.png', 'after-import-canvas.png', 'resized-menu.png', 'resized-canvas.png', 'unequal-refused-menu-retained.png'].map(p => `${output}/${p}`).concat(f8Proof.flatMap(p => [p.capture, p.roi])),
      visualReview: 'Independent capture review required for visible authored labels/menu; console and DOM canvas alone are insufficient', limitation: fixture.limitation };
    await writeFile(`${output}/proof.json`, JSON.stringify(proof, null, 2)); console.log(JSON.stringify({ ok: true, proof: `${output}/proof.json`, visualReview: proof.visualReview, limitation: proof.limitation }));
  } finally { await writeFile(`${output}/run-diagnostics.json`, JSON.stringify({ logs, requests, sockets, errors }, null, 2)); await browser.close(); }
}
async function runFocusHandoff() {
  assert.equal(process.env.PCX_APP_IMPORT_WASM_READY, '1', 'Generated WASM readiness required');
  await mkdir(output, { recursive: true });
  const { chromium } = await import(pathToFileURL(process.env.PCX_PLAYWRIGHT_MODULE ?? `${root}.scratch/browser-check/node_modules/playwright-core/index.mjs`));
  const origin = process.env.PCX_APP_IMPORT_ORIGIN ?? 'http://127.0.0.1:8931';
  const browser = await chromium.launch({ executablePath: '/usr/bin/chromium', args: ['--no-sandbox', '--enable-unsafe-swiftshader'] });
  const outcomes = [], requests = [], errors = [], sockets = [], logs = [];
  try {
    for (const method of ['escape', 'skip-button']) {
      const context = await browser.newContext({ viewport: { width: 1200, height: 1120 }, deviceScaleFactor: 1 });
      try {
        context.on('request', r => requests.push({ method, url: r.url(), verb: r.method(), body: r.postDataBuffer() !== null }));
        const page = await context.newPage(); page.setDefaultTimeout(120000);
        page.on('pageerror', e => errors.push({ method, error: e.message }));
        page.on('console', m => logs.push({ method, type: m.type(), text: m.text() }));
        page.on('websocket', socket => sockets.push(socket.url()));
        await page.addInitScript(() => {
          window.focusCompletions = [];
          document.addEventListener('peacecraft3-intro-finished', event => focusCompletions.push({ reason: event.detail,
            activeElement: document.activeElement?.id, canvasHidden: document.querySelector('#launcher-canvas')?.hidden,
            scrollX, scrollY, intro: peacecraft3Intro.status() }));
        });
        await page.goto(`${origin}/index.html`, { waitUntil: 'domcontentloaded' });
        await page.waitForFunction(() => globalThis.peacecraft3?.stored_asset('menu/standardtemplates.fdf') && peacecraft3.stored_asset('menu/mainmenu.fdf') && peacecraft3.stored_asset('menu/war3skins.txt') && peacecraft3.stored_asset('menu/miscdata.txt'));
        const settings = `${output}/${method}-launcher.png`; await page.screenshot({ path: settings });
        const { stdout } = await exec('/usr/bin/tesseract', [settings, 'stdout', '--psm', '11', 'tsv']);
        const words = stdout.split('\n').slice(1).map(line => line.split('\t')).filter(v => v.length >= 12);
        const normalized = value => value.replace(/[^a-z]/gi, '').toLowerCase();
        const control = words.find((v, i) => normalized(v[11]) === 'main' && normalized(words[i + 1]?.[11] ?? '') === 'menu');
        assert(control, `${method}: visible Main menu control not recognized`);
        await page.mouse.move(Number(control[6]) + Number(control[8]) / 2, Number(control[7]) + Number(control[9]) / 2);
        await page.waitForTimeout(250); await page.mouse.down(); await page.waitForTimeout(100); await page.mouse.up();
        await page.waitForFunction(() => peacecraft3Intro.status().active);
        const before = await page.evaluate(() => ({ activeTag: document.activeElement?.tagName, scrollX, scrollY, intro: peacecraft3Intro.status() }));
        if (method === 'escape') await page.keyboard.press('Escape');
        else await page.locator('.intro-skip').click();
        await page.waitForFunction(() => focusCompletions.length === 1);
        const completion = await page.evaluate(() => focusCompletions[0]);
        assert.equal(completion.reason, 'skipped'); assert.equal(completion.activeElement, 'launcher-canvas');
        assert.equal(completion.canvasHidden, false); assert.equal(completion.intro.active, false);
        assert.equal(completion.scrollX, before.scrollX); assert.equal(completion.scrollY, before.scrollY);
        const immediateFocus = await page.evaluate(() => document.activeElement?.id); assert.equal(immediateFocus, 'launcher-canvas');
        // Deliberately no canvas.focus(), pointer interaction or screenshot scrolling before this key.
        await page.keyboard.down('F8'); await page.waitForTimeout(100); await page.keyboard.up('F8'); await page.waitForTimeout(300);
        const visible = await captureFps(page, `${method}-direct-f8-visible`, true);
        await page.keyboard.down('F8'); await page.waitForTimeout(100); await page.keyboard.up('F8'); await page.waitForTimeout(300);
        const hidden = await captureFps(page, `${method}-direct-f8-hidden`, false);
        outcomes.push({ method, before, completion, immediateFocus, visible, hidden, input: 'Real Escape key or real rendered Skip button; F8 directly without canvas refocus', limitation: 'Skip/focus/key path proof, not original movie decoding/end-of-movie runtime proof' });
      } finally { await context.close(); }
    }
    const unexpected = requests.filter(r => !r.url.startsWith('blob:') && (!r.url.startsWith(`${origin}/`) || r.verb !== 'GET' || r.body || /\.(iso|mpq)(?:[?#]|$)/i.test(r.url)));
    assert.deepEqual(unexpected, []); assert.deepEqual(errors, []); assert.deepEqual(sockets, []);
    const proof = { ok: true, outcomes, network: { requests, unexpected, uploads: 0, sockets: 0 }, errors, logs };
    await writeFile(`${output}/focus-handoff-proof.json`, JSON.stringify(proof, null, 2));
    console.log(JSON.stringify({ ok: true, cases: outcomes.length, proof: `${output}/focus-handoff-proof.json` }));
  } finally { await writeFile(`${output}/focus-handoff-diagnostics.json`, JSON.stringify({ outcomes, requests, errors, sockets, logs }, null, 2)); await browser.close(); }
}
async function toggleFps(page) {
  await page.locator('#launcher-canvas').focus();
  await page.keyboard.down('F8'); await page.waitForTimeout(100); await page.keyboard.up('F8'); await page.waitForTimeout(300);
}
async function captureFps(page, name, visible) {
  const canvas = page.locator('#launcher-canvas');
  const capture = `${output}/${name}.png`, roi = `${output}/${name}-roi.png`;
  await canvas.screenshot({ path: capture });
  const bounds = await canvas.boundingBox(); assert(bounds);
  // Source overlay is top-right; a relative quadrant is an inspection region, not an authored layout replacement.
  await page.screenshot({ path: roi, clip: { x: bounds.x + bounds.width / 2, y: bounds.y, width: bounds.width / 2, height: bounds.height / 4 } });
  const { stdout } = await exec('/usr/bin/tesseract', [roi, 'stdout', '--psm', '6'], { maxBuffer: 4 * 1024 * 1024 });
  const recognized = /\bFPS\b/.test(stdout); assert.equal(recognized, visible, `${name}: rendered F8 glyph visibility mismatch: ${stdout}`);
  return { name, visible, recognized, ocr: stdout, capture, roi, limitation: 'Readable FPS glyph and preserved toggle state; exact font-handle identity remains source/ECS proof' };
}
async function poll(predicate, message) { const deadline = Date.now() + 120000; while (!predicate()) { if (Date.now() > deadline) throw Error(message); await new Promise(r => setTimeout(r, 250)); } }
async function clickAuthoredEntry(page, fixture) {
  const f = frames(fixture.frameText), [vw, vh] = fixture.frameUnits;
  const points = { TOPLEFT: [0, 1], TOP: [.5, 1], TOPRIGHT: [1, 1], LEFT: [0, .5], CENTER: [.5, .5], RIGHT: [1, .5], BOTTOMLEFT: [0, 0], BOTTOM: [.5, 0], BOTTOMRIGHT: [1, 0] };
  function rect(name, seen = new Set()) { assert(!seen.has(name)); seen.add(name); const anchor = f.prop(name, 'SetPoint');
    const width = Number(f.prop(name, 'Width')?.replace(/,$/, '')), height = Number(f.prop(name, 'Height')?.replace(/,$/, ''));
    if (!anchor) return [0, 0, Number.isFinite(width) ? width : vw, Number.isFinite(height) ? height : vh];
    assert(Number.isFinite(width) && Number.isFinite(height)); const m = anchor.match(/^(\w+)\s*,\s*"([^"]+)"\s*,\s*(\w+)\s*,\s*([-\d.]+)\s*,\s*([-\d.]+)/); assert(m, `Unsupported authored SetPoint ${anchor}`);
    const parent = rect(m[2], seen), own = points[m[1]], rel = points[m[3]]; assert(own && rel);
    return [parent[0] + parent[2] * rel[0] + Number(m[4]) - width * own[0], parent[1] + parent[3] * rel[1] + Number(m[5]) - height * own[1], width, height];
  }
  const selected = fixture.labels.find(l => l.id === (process.env.PCX_APP_IMPORT_CLICK_ID ?? 'credits')); assert(selected); const r = rect(selected.plate), box = await page.locator('#launcher-canvas').boundingBox(); assert(box);
  // Uses the app's source frame-unit conversion. No authored coordinates are copied.
  const physical = await page.locator('#launcher-canvas').evaluate(c => ({ width: c.width, height: c.height }));
  const ratio = vw / vh;
  let width = physical.width, height = physical.height;
  if (!(width >= height && width / height <= ratio)) { height = Math.min(height, Math.floor(width / ratio)); width = Math.min(width, Math.floor(height * ratio)); }
  const left = (physical.width - width) / 2, top = (physical.height - height) / 2;
  const x = box.x + (left + (r[0] + r[2] / 2) / vw * width) / physical.width * box.width;
  const y = box.y + (top + (vh - r[1] - r[3] / 2) / vh * height) / physical.height * box.height;
  await page.mouse.move(x, y); await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))); await page.mouse.down(); await page.waitForTimeout(100); await page.mouse.up();
}
if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) try {
  if (process.argv.includes('--focus-handoff')) await runFocusHandoff();
  else if (process.argv.includes('--run')) await run();
  else { const f = await prepare(); console.log(JSON.stringify({ prepared: `${output}/prepared.json`, members: f.provenance.length, expectedScene: f.expectedScene, labels: f.labels.map(l => ({ id: l.id, visible: l.visible })), limitation: f.limitation })); }
} catch (error) { await mkdir(output, { recursive: true }); await writeFile(`${output}/blocker.json`, JSON.stringify({ error: String(error), stack: error.stack }, null, 2)); console.error(String(error)); process.exitCode = 1; }
