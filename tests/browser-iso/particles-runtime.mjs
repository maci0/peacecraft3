// Synthetic archives through the production WASM import API; no DOM or original files.
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { menuMembers, mpqFixture } from './fixtures.mjs';

const [modulePath, wasmPath] = process.argv.slice(2);
assert(modulePath && wasmPath, 'Pass generated web JS and WASM paths');
const api = await import(pathToFileURL(resolve(modulePath)).href);
api.initSync({ module: await readFile(wasmPath) });
const text = value => new TextEncoder().encode(value);
function concat(...arrays) {
  const out = new Uint8Array(arrays.reduce((sum, a) => sum + a.length, 0));
  let at = 0;
  for (const array of arrays) { out.set(array, at); at += array.length; }
  return out;
}
function chunk(tag, bytes) {
  const out = new Uint8Array(8 + bytes.length);
  out.set(text(tag)); new DataView(out.buffer).setUint32(4, bytes.length, true);
  out.set(bytes, 8); return out;
}
function image() {
  const out = new Uint8Array(21), view = new DataView(out.buffer);
  out[2] = 2; view.setUint16(12, 1, true); view.setUint16(14, 1, true);
  out[16] = 24; out.set([37, 83, 149], 18); return out;
}
const base = menuMembers(api.menu_music_path());
const skin = new TextDecoder().decode(base['UI\\war3skins.txt']);
const selected = edition => skin.split('\n').find(line => line.startsWith(`GlueSpriteLayerBackground_V${edition}=`)).split('=').slice(1).join('=');
const modelPath = selected(0), expansionPath = selected(1);
const texturePath = 'Synthetic\\Texture.blp', latePath = 'Synthetic\\Late.tga';
function candidate({ late = false, texture = 0, life = 2, sampler = 0, flags = 0x49000, truncated = false } = {}) {
  const model = base[modelPath];
  const texs = new Uint8Array(late ? 536 : 268);
  texs.set(model.subarray(24));
  new DataView(texs.buffer).setUint32(264, sampler, true);
  if (late) texs.set(text(latePath), 268 + 4);
  const seq = new Uint8Array(132);
  seq.set(text('Stand')); new DataView(seq.buffer).setUint32(84, 1000, true);
  const record = new Uint8Array(4 + 96 + 171), v = new DataView(record.buffer);
  v.setUint32(0, record.length + (truncated ? 4 : 0), true);
  v.setUint32(4, 96, true); v.setInt32(92, -1, true); v.setUint32(96, flags, true);
  const floats = { 16: life, 20: 5, 24: 13, 28: 7, 52: .5, 56: .2, 60: .4, 64: .7, 68: .8, 72: .3, 76: .1, 80: .5, 84: .6, 88: .9, 95: 1, 99: 3, 103: 1 };
  for (const [offset, value] of Object.entries(floats)) v.setFloat32(100 + Number(offset), value, true);
  for (const offset of [36, 40]) v.setUint32(100 + offset, 1, true);
  record.set([31, 213, 67], 100 + 92);
  for (const offset of [107, 119, 131, 143]) {
    v.setUint32(100 + offset + 4, 1, true); v.setUint32(100 + offset + 8, 1, true);
  }
  v.setUint32(100 + 155, texture, true);
  return { ...base, [texturePath]: image(), [modelPath]: concat(model.subarray(0, 16), chunk('TEXS', texs), chunk('SEQS', seq), chunk('PIVT', new Uint8Array(12)), chunk('PRE2', record)) };
}
function commit(label, members) {
  api.begin_archive_import(); api.stage_archive(label, mpqFixture(members));
  try { return api.commit_archive_import(); }
  catch (error) { api.cancel_archive_import(); throw error; }
}
function snapshot() {
  return { revision: api.asset_revision(), editions: api.local_menu_editions(), assets: api.stored_assets().map(path => [path, [...api.stored_asset(path)]]) };
}
commit('valid-particle-base', candidate());
assert.deepEqual(api.local_menu_editions(), ['roc']);
assert.throws(() => api.open_local_menu('tft'), /UNAVAILABLE_MENU_EDITION/);
const before = snapshot();
const malformedImage = candidate(); malformedImage[texturePath] = new Uint8Array([255]);
const oversizedImage = candidate(), oversized = new DataView(oversizedImage[texturePath].buffer);
oversized.setUint16(12, 65535, true); oversized.setUint16(14, 65535, true);
for (const [name, members, pattern] of [
  ['missing-late-image', candidate({ late: true, texture: 1 }), /MISSING_MEMBER/],
  ['malformed-image', malformedImage, /TEXS/],
  ['oversized-image', oversizedImage, /budget/],
  ['malformed-properties', candidate({ life: NaN }), /particle scalars/],
  ['missing-texture-record', candidate({ texture: 7 }), /TEXS reference/],
  ['truncated-record', candidate({ truncated: true }), /PRE2/],
]) {
  assert.throws(() => commit(name, members), pattern);
  assert.deepEqual(snapshot(), before, `${name} changed committed paths, aliases, bytes, edition or revision`);
}
const late = candidate({ late: true, texture: 1 }); late[latePath] = image();
commit('valid-late-image', late);
assert.deepEqual([...api.stored_asset(latePath)], [...late[latePath]]);
const unsupported = commit('reported-unsupported-sampler', candidate({ sampler: 0x80000000 }));
assert(unsupported.some(note => note.includes('PRE2 unsupported') && note.includes('sampler')));
const paired = candidate(); paired[expansionPath] = paired[modelPath];
commit('paired-particle-editions', paired);
assert.deepEqual(api.local_menu_editions(), ['roc', 'tft']); api.open_local_menu('tft');
api.drop_assets();
assert.deepEqual(api.stored_assets(), []); assert.deepEqual(api.local_menu_editions(), []);
console.log(JSON.stringify({ ok: true, rollbackCases: 6, validLateImage: true, unsupportedReported: true, pairedEditions: true, cleared: true, scope: 'Production WASM import/rollback with synthetic records; no draw or native fidelity claim' }));
