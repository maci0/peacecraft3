import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import vm from 'node:vm';

const repository = new URL('../../', import.meta.url);
const { clickDescriptor } = await import('../../web/iso/audio.js');
const playbackSource = await readFile(new URL('web/menu-audio.js', repository), 'utf8');

function metadata(directory = '', volume = 37, flags = 'NODUPLICATES', pitch = 1) {
  const fields = { SoundName: 'GlueScreenClick', DirectoryBase: directory, FileNames: 'synthetic-click.wav', Volume: volume, Pitch: pitch, Flags: flags };
  const cell = value => typeof value === 'string' ? `"${value.replaceAll('"', '""')}"` : String(value);
  const rows = ['ID;PTEST'];
  Object.entries(fields).forEach(([key, value], index) => {
    rows.push(`C;X${index + 1};Y1;K${cell(key)}`, `C;X${index + 1};Y2;K${cell(value)}`);
  });
  return new TextEncoder().encode(rows.join('\n') + '\nE\n');
}
function source(directory = '', volume = 37, flags = 'NODUPLICATES') {
  const descriptor = clickDescriptor(metadata(directory, volume, flags), 'synthetic/UISounds.slk');
  // No original audio. Only the existing content classifier is exercised.
  const members = new Map([['synthetic-click.wav', new Blob(['RIFF0000WAVE'])]]);
  const blob = members.get(descriptor.path);
  assert.ok(blob, `missing synthetic archive member: ${descriptor.path}`);
  return { ...descriptor, blob, provenance: { ...descriptor.provenance, source: { edition: null, archive: 'synthetic-only' } } };
}
function playback() {
  const players = [], liveUrls = new Map(), revoked = [], warnings = [], handlers = new Map();
  let nextUrl = 0;
  let rejectNextPlay = false;
  class Audio {
    constructor(src = '') { Object.assign(this, { src, paused: true, ended: false, muted: false, currentTime: 0, volume: 1, plays: 0 }); players.push(this); }
    canPlayType() { return 'probably'; }
    pause() { this.paused = true; }
    play() {
      this.paused = false; this.plays++;
      const reject = rejectNextPlay; rejectNextPlay = false;
      return reject ? Promise.reject(new Error('synthetic play failure')) : Promise.resolve();
    }
    load() { this.paused = true; this.currentTime = 0; }
    removeAttribute(name) { if (name === 'src') this.src = ''; }
    addEventListener(name, callback) { (this.listeners ??= new Map()).set(name, callback); }
  }
  class AudioContext {
    constructor() { this.currentState = 'suspended'; this.resumes = 0; this.stateReads = 0; }
    get state() { this.stateReads++; return this.currentState; }
    resume() {
      this.resumes++;
      if (this.rejectResume) return Promise.reject(new Error('synthetic resume failure'));
      this.currentState = 'running'; return Promise.resolve();
    }
    close() { this.currentState = 'closed'; this.changed?.(); }
    addEventListener(name, callback) { assert.equal(name, 'statechange'); this.changed = callback; }
  }
  const toggle = { textContent: '', setAttribute() {}, addEventListener(name, callback) { handlers.set(`toggle:${name}`, callback); } };
  const document = { documentElement: { dataset: { localAssets: 'true' } }, querySelector() { return toggle; }, addEventListener(name, callback) { handlers.set(`document:${name}`, callback); }, dispatchEvent() {} };
  const context = vm.createContext({ Audio, AudioContext, Blob, Uint8Array, URLSearchParams, document, location: { search: '' },
    window: { addEventListener(name, callback) { handlers.set(`window:${name}`, callback); } },
    URL: { createObjectURL(blob) { const url = `blob:synthetic-${++nextUrl}`; liveUrls.set(url, blob); return url; }, revokeObjectURL(url) { revoked.push(url); liveUrls.delete(url); } },
    console: { warn(...args) { warnings.push(args); } }, CustomEvent: class { constructor(type, options) { this.type = type; this.detail = options.detail; } },
  });
  vm.runInContext(playbackSource, context, { filename: 'web/menu-audio.js' });
  return { api: context.peacecraft3Audio, music: players[0], click: players[1], players, liveUrls, revoked, warnings,
    webAudio() { return new context.AudioContext(); },
    gesture(name) { handlers.get(`document:${name}`)(); },
    rejectNextPlay() { rejectNextPlay = true; },
    toggle() { handlers.get('toggle:click')(); }, pagehide() { handlers.get('window:pagehide')(); } };
}

test('gestures resume script contexts independently of menu media and release closed contexts', async () => {
  const state = playback(), stream = state.webAudio();
  state.api.pauseForIntro();
  state.gesture('pointerdown');
  assert.equal(stream.resumes, 1);
  assert.equal(stream.state, 'running');
  assert.equal(state.music.plays, 0);
  state.gesture('keydown');
  assert.equal(stream.resumes, 1);
  stream.currentState = 'suspended'; stream.rejectResume = true;
  state.gesture('keydown');
  await Promise.resolve();
  assert.equal(state.warnings.length, 1);
  assert.match(state.warnings[0][1], /synthetic resume failure/);
  stream.rejectResume = false;
  state.gesture('pointerdown');
  assert.equal(stream.resumes, 3);
  stream.close();
  const reads = stream.stateReads;
  state.gesture('pointerdown');
  assert.equal(stream.stateReads, reads, 'closed contexts leave the live-context collection');
});

test('empty authored directory binds the exact root-level member and retains raw settings', () => {
  const descriptor = source();
  assert.equal(descriptor.path, 'synthetic-click.wav');
  assert.equal(descriptor.settings.volume, 37 / 127);
  assert.equal(descriptor.settings.noDuplicates, true);
  assert.equal(descriptor.provenance.authored.DirectoryBase, '');
  assert.equal(descriptor.provenance.authored.Volume, 37);
  assert.equal(descriptor.provenance.source.edition, null);
});

test('nonempty authored directories retain the existing single separator join', () => {
  for (const directory of ['Synthetic\\Audio', 'Synthetic\\Audio\\', 'Synthetic\\Audio/']) {
    const descriptor = clickDescriptor(metadata(directory, 23, 0), 'synthetic/table.slk');
    assert.equal(descriptor.path, 'Synthetic\\Audio\\synthetic-click.wav');
    assert.equal(descriptor.settings.volume, 23 / 127);
    assert.equal(descriptor.settings.noDuplicates, false);
    assert.equal(descriptor.provenance.authored.DirectoryBase, directory);
  }
  for (const directory of ['\\', '/']) {
    const descriptor = clickDescriptor(metadata(directory), 'synthetic/table.slk');
    assert.equal(descriptor.path, '\\synthetic-click.wav');
    assert.equal(descriptor.provenance.authored.DirectoryBase, directory);
  }
});

test('malformed source settings stay explicit errors without fallback', () => {
  assert.throws(() => clickDescriptor(metadata('', 999), 'synthetic/error.slk'), /synthetic\/error.slk.*Volume/);
  assert.throws(() => clickDescriptor(metadata('', 7, 'UNKNOWN'), 'synthetic/error.slk'), /synthetic\/error.slk.*Flags/);
  assert.throws(() => clickDescriptor(metadata('', 7, 0, 2), 'synthetic/error.slk'), /synthetic\/error.slk.*Pitch/);
  const missing = new TextDecoder().decode(metadata()).replace('C;X2;Y2;K""\n', '');
  assert.throws(() => clickDescriptor(new TextEncoder().encode(missing), 'synthetic/error.slk'), /file mapping/);
});

test('selected member and authored settings reach the actual audio binding API', async () => {
  const state = playback(), click = source();
  await state.api.setLocalAssets({ click });
  assert.equal(state.api.status().localSources.click.path, click.path);
  assert.equal(state.api.status().localSources.click.provenance.metadataPath, 'synthetic/UISounds.slk');
  assert.equal(state.api.status().localSources.click.provenance.source.edition, null);
  assert.equal(state.click.volume, click.settings.volume);
  assert.equal(state.click.muted, true);
  const staged = state.liveUrls.get(state.click.src);
  assert.equal(staged.type, 'audio/wav');
  assert.equal(await staged.text(), await click.blob.text());
  state.toggle(); state.api.click(); state.api.click();
  assert.equal(state.click.plays, 1);
  assert.equal(state.api.status().clicks, 1);
  assert.equal(state.warnings.length, 0);
});

test('authored duplicate policy updates on replacement and ended playback can restart', async () => {
  const state = playback();
  await state.api.setLocalAssets({ click: source('', 21, 0) });
  state.toggle(); state.api.click(); state.api.click();
  assert.equal(state.click.plays, 2);
  await state.api.setLocalAssets({ click: source('', 51, 'NODUPLICATES') });
  state.api.click(); state.api.click();
  assert.equal(state.click.plays, 3);
  state.click.ended = true; state.api.click();
  assert.equal(state.click.plays, 4);
  assert.equal(state.click.volume, 51 / 127);
});

test('replacement and reset release URLs and preserve mute and intro ownership', async () => {
  const state = playback(), music = { path: 'synthetic-music.mp3', blob: new Blob(['ID3synthetic']), provenance: { edition: null } };
  await state.api.setLocalAssets({ click: source(), music });
  const priorUrls = [...state.liveUrls.keys()];
  state.toggle(); assert.equal(state.music.plays, 1);
  state.api.pauseForIntro();
  await state.api.setLocalAssets({ click: source('', 63, 0), music });
  assert.equal(state.api.status().introActive, true);
  assert.equal(state.api.status().enabled, true);
  assert.equal(state.music.plays, 1);
  state.api.click(); assert.equal(state.click.plays, 0);
  assert.ok(priorUrls.every(url => state.revoked.includes(url)));
  assert.equal(state.liveUrls.size, 2);
  state.api.resumeAfterIntro(); assert.equal(state.music.plays, 2);
  state.toggle(); assert.equal(state.api.status().muted, true);
  state.pagehide();
  assert.equal(state.liveUrls.size, 0);
  assert.equal(state.api.status().localUrlCount, 0);
  assert.equal(state.api.status().enabled, false);
  assert.equal(state.api.status().localSources.click, null);
  assert.equal(state.click.src, '');
  state.api.click(); assert.equal(state.click.plays, 0);
});

test('invalid replacement preserves the prior binding without URL revocation', async () => {
  const state = playback(), click = source();
  await state.api.setLocalAssets({ click });
  const priorUrl = state.click.src, revoked = state.revoked.length;
  await assert.rejects(state.api.setLocalAssets({ click: { ...click, settings: undefined } }), /authored click settings/);
  await assert.rejects(state.api.setLocalAssets({ click: { ...click, blob: new Blob(['bad codec']) } }), /UNSUPPORTED_LOCAL_AUDIO/);
  assert.equal(state.click.src, priorUrl);
  assert.equal(state.liveUrls.size, 1);
  assert.equal(state.revoked.length, revoked);
  assert.equal(state.api.status().localSources.click.path, click.path);
});

test('superseded asynchronous binding cannot replace the newer source', async () => {
  const state = playback(), click = source();
  let release;
  const blocked = new Promise(resolve => { release = resolve; });
  class SlowBlob extends Blob {
    slice(...args) { const slice = super.slice(...args); return { async arrayBuffer() { await blocked; return slice.arrayBuffer(); } }; }
  }
  const prior = state.api.setLocalAssets({ click: { ...click, blob: new SlowBlob(['RIFF0000WAVE']) } });
  const rejected = assert.rejects(prior, /superseded/);
  await state.api.setLocalAssets({ click: { ...click, provenance: { selected: 'newer-synthetic' } } });
  const currentUrl = state.click.src;
  release(); await rejected;
  assert.equal(state.click.src, currentUrl);
  assert.equal(state.liveUrls.size, 1);
  assert.equal(state.api.status().localSources.click.provenance.selected, 'newer-synthetic');
});

test('scene event cancellation pauses voices and revokes each URL once despite late completion', async () => {
  const state = playback(); state.toggle();
  state.api.setUserGains(1, 0.5, true, true);
  state.api.event(new Uint8Array([1]), 0.25);
  state.rejectNextPlay(); state.api.event(new Uint8Array([2]), 0.75);
  const voices = state.players.slice(2);
  assert.equal(state.api.status().eventSoundCount, 2);
  assert.deepEqual(voices.map(v => v.volume), [0.125, 0.375]);
  state.api.stopEvents(); state.api.stopEvents();
  for (const voice of voices) {
    assert.equal(voice.paused, true); assert.equal(voice.src, '');
    voice.listeners.get('ended')();
  }
  await Promise.resolve();
  assert.equal(state.api.status().eventSoundCount, 0);
  assert.equal(state.liveUrls.size, 0); assert.equal(state.revoked.length, 2);
  assert.equal(state.warnings.length, 0);
});

test('only successful source replacement, intro and reset cancel current scene events', async () => {
  const state = playback(); state.toggle();
  state.api.event(new Uint8Array([1]), 0.5);
  await assert.rejects(state.api.setLocalAssets({ click: { ...source(), blob: new Blob(['invalid']) } }), /UNSUPPORTED_LOCAL_AUDIO/);
  assert.equal(state.api.status().eventSoundCount, 1);
  await state.api.setLocalAssets({ click: source() });
  assert.equal(state.api.status().eventSoundCount, 0);
  for (const cleanup of [() => state.api.pauseForIntro(), () => state.api.resetLocalAssets(), () => state.pagehide()]) {
    state.api.resumeAfterIntro(); state.api.event(new Uint8Array([3]), 0.5);
    assert.equal(state.api.status().eventSoundCount, 1);
    cleanup(); assert.equal(state.api.status().eventSoundCount, 0);
  }
  assert.equal(state.liveUrls.size, 0);
});
