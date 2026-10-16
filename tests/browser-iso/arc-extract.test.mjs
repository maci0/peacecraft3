import test from 'node:test';
import assert from 'node:assert/strict';
import { createCipheriv, pbkdf2Sync } from 'node:crypto';
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

// Synthetic FreeArc-compatible little-endian AES-CTR reference. Source chunks
// are encrypted independently at a block offset; codec requests may be odd.
function ctr(bytes, key, iv, blockOffset = 0) {
  let base = 0n;
  for (let i = 15; i >= 0; i--) base = base * 256n + BigInt(iv[i]);
  const counters = new Uint8Array(Math.ceil(bytes.length / 16) * 16);
  for (let block = 0; block < counters.length / 16; block++) {
    let value = base + BigInt(blockOffset + block);
    for (let i = 0; i < 16; i++, value >>= 8n) counters[block * 16 + i] = Number(value & 255n);
  }
  const aes = createCipheriv('aes-256-ecb', key, null).setAutoPadding(false);
  const stream = Buffer.concat([aes.update(counters), aes.final()]);
  return bytes.map((byte, i) => byte ^ stream[i]);
}
function encryptedFixture() {
  const f = fixture(), password = new Uint8Array([11, 23, 37]), salt = new Uint8Array([3, 5, 7]);
  const iv = new Uint8Array(16); iv[0] = 9;
  const derived = pbkdf2Sync(password, salt, 1, 34, 'sha512');
  const hex = bytes => Buffer.from(bytes).toString('hex');
  f.archive.blocks[0].method += `+aes-256/ctr:n1:r0:s${hex(salt)}:c${hex(derived.subarray(32))}:i${hex(iv)}:f`;
  return { ...f, password, encrypted: ctr(f.bytes, derived.subarray(0, 32), iv) };
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

test('packed input and explicit intermediate work policy are independent of final output', async () => {
  const f = fixture(), storage = sinks();
  f.archive.blocks[0].storedSize = 12;
  await extractArcMembers(new Blob([new Uint8Array(12)]), f.archive, f.selected, new Uint8Array([1]), {
    async decodeStream(options) {
      assert.equal(options.maxInputBytes, 12);
      assert.equal(options.expectedSize, 37);
      assert.equal(options.maxIntermediateBytes, 53);
      while ((await options.read(3)).length) {}
      await options.onWrite(f.bytes);
    },
  }, { createSink: storage.createSink, readBytes: 16, maxIntermediateBytes: 53 });
  for (const maxIntermediateBytes of [0, -1, NaN, Infinity, 1.5, Number.MAX_SAFE_INTEGER + 1]) {
    let allocated = false;
    await assert.rejects(extractArcMembers(new Blob([f.bytes]), f.archive, f.selected, new Uint8Array([1]), {}, {
      maxIntermediateBytes, createSink() { allocated = true; },
    }), TypeError);
    assert.equal(allocated, false);
  }
  f.archive.blocks[0].storedSize = 38;
  await assert.rejects(extractArcMembers(new Blob([f.bytes]), f.archive, f.selected, new Uint8Array([1]), {}, {
    createSink() { assert.fail('source bounds must precede sink allocation'); },
  }), error => error.code === 'ARC_BOUNDS');
});

test('aligned encrypted reads serve odd codec requests and exact-cap sentinel; key views clear', async () => {
  const f = encryptedFixture(), storage = sinks(), transforms = [], views = [];
  const results = await extractArcMembers(new Blob([f.encrypted]), f.archive, f.selected, f.password, {
    async aesCtrTransform(bytes, cipher, { blockOffset }) {
      transforms.push({ blockOffset, length: bytes.length }); views.push(cipher);
      return ctr(bytes, cipher.key, cipher.iv, blockOffset);
    },
    async decodeStream({ read, onWrite, maxInputBytes }) {
      assert.equal(maxInputBytes, f.encrypted.length);
      const chunks = []; let count = 0, at = 0;
      const requests = [1, 3, 17];
      while (count < maxInputBytes) {
        const requested = requests[at++ % requests.length], part = await read(requested);
        assert(part.length > 0 && part.length <= requested);
        count += part.length; chunks.push(...part);
      }
      assert.equal((await read(1)).length, 0);
      for (let at = 0; at < chunks.length; at += 11) await onWrite(Uint8Array.from(chunks.slice(at, at + 11)));
    },
  }, { createSink: storage.createSink, readBytes: 16, maxIntermediateBytes: 73 });
  assert.deepEqual(transforms, [{ blockOffset: 0, length: 16 }, { blockOffset: 1, length: 16 }, { blockOffset: 2, length: 5 }]);
  assert.deepEqual(new Uint8Array(await results[0].file.arrayBuffer()), f.bytes.subarray(7, 28));
  assert(views.length > 0);
  for (const view of views) { assert(view.key.every(byte => byte === 0)); assert(view.iv.every(byte => byte === 0)); }
});

test('encrypted cancellation, sink failures and unselected CRC failures abort sinks and clear observed cipher views', async () => {
  for (const variant of ['cancel-read', 'cancel-write', 'write', 'close', 'crc']) {
    const f = encryptedFixture(), storage = sinks(), controller = new AbortController(), views = [];
    let closes = 0;
    if (variant === 'crc') f.archive.files[2].crc ^= 1;
    const errorCode = variant.startsWith('cancel') ? 'AbortError' : variant === 'crc' ? 'ARC_CRC' : 'synthetic sink failure';
    await assert.rejects(extractArcMembers(new Blob([f.encrypted]), f.archive, f.selected, f.password, {
      decodeStream: variant === 'cancel-write' ? async ({ read, onWrite }) => {
        const all = [];
        for (;;) { const part = await read(3); if (!part.length) break; all.push(...part); }
        // Cancellation in the last awaited write must not be swallowed when
        // there is no subsequent codec callback to check the signal.
        await onWrite(Uint8Array.from(all));
      } : codecStream,
      async aesCtrTransform(bytes, cipher, { blockOffset }) {
        views.push(cipher);
        const plain = ctr(bytes, cipher.key, cipher.iv, blockOffset);
        if (variant === 'cancel-read') controller.abort();
        return plain;
      },
    }, { signal: controller.signal, readBytes: 16, maxIntermediateBytes: 73,
      createSink: async file => {
        const sink = await storage.createSink(file);
        return { ...sink,
          async write(bytes) {
            if (variant === 'write') throw new Error('synthetic sink failure');
            await sink.write(bytes);
            if (variant === 'cancel-write') controller.abort();
          },
          async close() { closes++; if (variant === 'close') throw new Error('synthetic sink failure'); return sink.close(); },
        };
      },
    }), error => error.name === errorCode || error.code === errorCode || error.message === errorCode);
    assert.deepEqual(storage.aborted, ['selected.mpq']);
    if (variant === 'cancel-write') assert.equal(closes, 0);
    assert(views.length > 0);
    for (const view of views) { assert(view.key.every(byte => byte === 0)); assert(view.iv.every(byte => byte === 0)); }
  }
});

test('initial zero-sized member close failure aborts sinks before codec entry', async () => {
  const f = encryptedFixture(), storage = sinks();
  const member = { ...f.selected[0], size: 0, crc: 0 };
  f.archive.files = [member]; f.archive.blocks[0].originalSize = 0;
  await assert.rejects(extractArcMembers(new Blob([f.encrypted]), f.archive, [member], f.password, {
    decodeStream() { assert.fail('initial close failure must precede codec entry'); },
  }, { createSink: async file => ({ ...await storage.createSink(file), async close() { throw new Error('initial close failure'); } }) }),
  /initial close failure/);
  assert.deepEqual(storage.aborted, ['selected.mpq']);
});

test('aggregate selected solid-block member counts are refused before prefix reads or sinks', async () => {
  // Each block is inside the entry limit. Only the combined member count is not.
  const limits = { entries: 3 };
  const prefix = Uint8Array.of(1), selectedBytes = Uint8Array.of(2, 3, 4);
  const blockBytes = new Uint8Array(prefix.length + selectedBytes.length);
  blockBytes.set(prefix, 0); blockBytes.set(selectedBytes, prefix.length);
  const blobBytes = new Uint8Array(blockBytes.length * 2);
  blobBytes.set(blockBytes, 0); blobBytes.set(blockBytes, blockBytes.length);
  const files = [0, 1].flatMap(block => [
    { path: `prefix-${block}.txt`, size: prefix.length, crc: crc32(prefix), directory: false, block },
    { path: `selected-${block}.mpq`, size: selectedBytes.length, crc: crc32(selectedBytes), directory: false, block },
  ]);
  assert.equal(files.filter(file => file.block === 0).length, limits.entries - 1);
  assert.equal(files.filter(file => file.block === 1).length, limits.entries - 1);
  const archive = {
    files,
    blocks: [0, 1].map(index => ({ method: 'storing', offset: index * blockBytes.length, storedSize: blockBytes.length, originalSize: blockBytes.length })),
  };
  let slices = 0, sinks = 0, codecCalls = 0;
  const blob = { size: blobBytes.length, slice(...args) { slices += 1; return new Blob([blobBytes]).slice(...args); } };
  await assert.rejects(extractArcMembers(blob, archive, [files[1], files[3]], new Uint8Array([1]), {
    async decodeStream({ read, onWrite }) {
      codecCalls += 1;
      const all = [];
      for (;;) { const part = await read(3); if (!part.length) break; all.push(...part); }
      await onWrite(Uint8Array.from(all));
    },
  }, {
    limits, readBytes: 16,
    createSink() { sinks += 1; return { async write() {}, async close() {}, async abort() {} }; },
  }), error => error.code === 'ARC_LIMIT' && error.message.includes('aggregate file count exceeds limit'));
  assert.equal(slices, 0); assert.equal(sinks, 0); assert.equal(codecCalls, 0);
});
