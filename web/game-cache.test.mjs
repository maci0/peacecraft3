import assert from 'node:assert/strict';
const saved = new Map();
let failRead = false, failWrite = false;
globalThis.localStorage = {
  getItem(key) { if (failRead) throw new Error('read denied'); return saved.get(key) ?? null; },
  setItem(key, text) { if (failWrite) throw new Error('quota exceeded'); saved.set(key, text); },
};
await import('./game-cache.js');
const storage = globalThis.peacecraft3GameCaches;
assert.equal(storage.load(1024), undefined);
const bytes = Uint8Array.from({length: 256}, (_, i) => i);
storage.save(bytes);
assert.deepEqual(storage.load(256), bytes);
assert.throws(() => storage.load(255), RangeError);
for (const limit of [-1, NaN, Infinity, 1.5]) assert.throws(() => storage.load(limit), RangeError);
assert.throws(() => storage.save([1, 2]), RangeError);
assert.throws(() => storage.save(new Uint8Array(8 * 1024 * 1024 + 1)), RangeError);
failWrite = true;
assert.throws(() => storage.save(new Uint8Array([9])), /quota exceeded/);
assert.deepEqual(storage.load(256), bytes);
failWrite = false;
failRead = true;
assert.throws(() => storage.load(256), /read denied/);
failRead = false;
storage.save(new Uint8Array());
assert.deepEqual(storage.load(0), new Uint8Array());
saved.set('peacecraft3.campaign-cache.v1', '!!invalid-base64');
assert.throws(() => storage.load(256));
console.log('campaign cache browser storage: roundtrip, absence, limits and failure preservation pass');
