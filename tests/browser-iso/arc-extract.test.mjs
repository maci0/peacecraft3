import test from 'node:test';
import assert from 'node:assert/strict';
import { extractArcMembers } from '../../web/iso/arc-extract.js';
import { crc32 } from '../../web/iso/freearc.js';

function fixture() {
  const bytes = Uint8Array.from({ length: 37 }, (_, n) => n);
  const files = [
    { path: 'ignored.txt', size: 7, crc: crc32(bytes.subarray(0, 7)), directory: false, block: 0 },
    { path: 'selected.mpq', size: 21, crc: crc32(bytes.subarray(7, 28)), directory: false, block: 0 },
    { path: 'last.txt', size: 9, crc: crc32(bytes.subarray(28)), directory: false, block: 0 },
  ];
  const archive = { files, blocks: [{ method: 'storing', offset: 0, storedSize: bytes.length, originalSize: bytes.length }] };
  return { bytes, archive, selected: [files[1]] };
}
async function codecStream({ read, onWrite }) {
  // Small odd requests must never receive oversized chunks, even when source
  // reads are aligned. Output boundaries intentionally cross member boundaries.
  const all = [];
  for (;;) { const b = await read(3); assert(b.length <= 3); if (!b.length) break; all.push(...b); }
  for (let at = 0; at < all.length; at += 11) await onWrite(new Uint8Array(all.slice(at, at + 11)));
}
function sinks() {
  const created = [], aborted = [];
  return { created, aborted, async createSink(file) {
    created.push(file.path); const bytes = [];
    return { async write(chunk) { bytes.push(...chunk); }, async close() { return new Blob([new Uint8Array(bytes)]); },
      async abort() { aborted.push(file.path); } };
  } };
}
test('solid stream checks every member and retains only selected bytes through bounded sinks', async () => {
  const f = fixture(), storage = sinks(), progress = [];
  const results = await extractArcMembers(new Blob([f.bytes]), f.archive, f.selected, new Uint8Array([1]),
    { decodeStream: codecStream }, { createSink: storage.createSink, readBytes: 16, onProgress: p => progress.push(p) });
  assert.deepEqual(storage.created, ['selected.mpq']); assert.deepEqual(storage.aborted, []);
  assert.deepEqual(new Uint8Array(await results[0].file.arrayBuffer()), f.bytes.subarray(7, 28));
  assert.equal(progress.at(-1).sourceBytes, 37); assert.equal(progress.at(-1).decodedBytes, 37);
});
test('unselected corruption, incomplete streams and excessive outputs abort all selected sinks', async () => {
  for (const variant of ['crc', 'short', 'extra']) {
    const f = fixture(), storage = sinks();
    if (variant === 'crc') f.archive.files[2].crc = 0;
    const codec = variant === 'short' ? { async decodeStream({ read, onWrite }) { await onWrite(await read(3)); } }
      : variant === 'extra' ? { async decodeStream({ onWrite }) { await onWrite(new Uint8Array(100)); } }
      : { decodeStream: codecStream };
    await assert.rejects(extractArcMembers(new Blob([f.bytes]), f.archive, f.selected, new Uint8Array([1]), codec,
      { createSink: storage.createSink, readBytes: 16 }), e => ['ARC_CRC', 'ARC_SIZE'].includes(e.code));
    assert.deepEqual(storage.aborted, ['selected.mpq']);
  }
});
test('selection budgets fail before storage or codec allocation, cancellation is preserved', async () => {
  const f = fixture(), storage = sinks(); let calls = 0;
  const codec = { decodeStream() { calls++; } };
  await assert.rejects(extractArcMembers(new Blob([f.bytes]), f.archive, f.selected, new Uint8Array([1]), codec,
    { createSink: storage.createSink, maxSelectedBytes: 1 }), e => e.code === 'ARC_LIMIT');
  assert.deepEqual(storage.created, []); assert.equal(calls, 0);
  const abort = new AbortController(); abort.abort();
  await assert.rejects(extractArcMembers(new Blob([f.bytes]), f.archive, f.selected, new Uint8Array([1]), codec,
    { createSink: storage.createSink, signal: abort.signal }), e => e.name === 'AbortError');
  assert.deepEqual(storage.created, []);
});
