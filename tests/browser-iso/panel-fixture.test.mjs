import test from 'node:test';
import assert from 'node:assert/strict';
import { panelSoundEvents } from './app-import-runtime.mjs';
const text = s => new TextEncoder().encode(s);
function join(...parts) {
  const b = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let at = 0; for (const p of parts) { b.set(p, at); at += p.length; } return b;
}
function word(n) { const b = new Uint8Array(4); new DataView(b.buffer).setUint32(0, n, true); return b; }
const chunk = (tag, data) => join(text(tag), word(data.length), data);
function model({ global = 0xffffffff, reverse = false, truncated = false, track = 'KEVT' } = {}) {
  const sequences = ['MainMenu Birth', 'Stand MainMenu', 'SinglePlayer Birth', 'Options Birth', 'Options Stand Alternate', 'Options Morph', 'Options Morph Alternate', 'Campaign Morph'].map((name, i) => {
    const b = new Uint8Array(132), v = new DataView(b.buffer); b.set(text(name));
    v.setUint32(80, 10 + i * 20, true); v.setUint32(84, reverse ? 0 : 19 + i * 20, true); return b;
  });
  const event = (name, frame) => {
    const b = new Uint8Array(96); b.set(word(96)); b.set(text(name), 4);
    return join(b, text(track), word(1), word(global), word(frame));
  };
  const bytes = join(text('MDLX'), chunk('VERS', word(800)), chunk('SEQS', join(...sequences)),
    chunk('EVTS', join(event('SNDXAAA1', 10), event('sndxBBB2', 50), event('SNDXOPTS', 70), event('SNDXALTR', 90), event('SNDXMRF1', 110), event('SNDXMRF2', 130), event('SNDXCAMP', 150), event('SNDXNONE', 500))));
  return truncated ? bytes.subarray(0, bytes.length - 1) : bytes;
}
test('original fixture dependency discovery follows selected source phase intervals', () => {
  assert.deepEqual(panelSoundEvents(model()), ['AAA1', 'BBB2', 'OPTS', 'ALTR', 'MRF1', 'MRF2']);
});
test('original fixture dependency discovery refuses malformed or unsupported event framing', () => {
  for (const options of [{ global: 0 }, { reverse: true }, { truncated: true }, { track: 'XXXX' }])
    assert.throws(() => panelSoundEvents(model(options)));
});
