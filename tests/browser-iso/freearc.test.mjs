import test from 'node:test';
import assert from 'node:assert/strict';
import { webcrypto } from 'node:crypto';
import { ArcCursor, checkedLimits, crc32, footerDescriptor, parseFooter, parseDirectory, cipherParameters,
  deriveCipher, lzmaParameters, decodeControl, redactMethod } from '../../web/iso/freearc.js';
if (!globalThis.crypto) globalThis.crypto = webcrypto;
const join = (...parts) => Uint8Array.from(parts.flatMap(p => [...p]));
const u32 = n => { const b = new Uint8Array(4); new DataView(b.buffer).setUint32(0, n, true); return b; };
function integer(n) {
  const value = BigInt(n);
  for (let width = 1; width <= 8; width++) {
    if (value < 1n << BigInt(width * 7)) {
      let encoded = value << BigInt(width) | (1n << BigInt(width - 1)) - 1n;
      return Uint8Array.from({ length: width }, () => { const byte = Number(encoded & 255n); encoded >>= 8n; return byte; });
    }
  }
  let v = value; return join([255], Uint8Array.from({ length: 8 }, () => { const byte = Number(v & 255n); v >>= 8n; return byte; }));
}
const string = s => join(new TextEncoder().encode(s), [0]);
function descriptor(payload = new Uint8Array([1, 2, 3])) {
  const body = join([65, 114, 67, 1], integer(4), string('storing'), integer(payload.length), integer(payload.length), u32(crc32(payload)));
  return join(payload, body, u32(crc32(body)));
}
function directory({ name = 'data.mpq', size = 3, directoryIndex = 0, method = 'storing', storedSize = 3 } = {}) {
  return join(integer(1), integer(1), string(method), integer(10), integer(storedSize), integer(1), string(''), string(name),
    integer(directoryIndex), integer(size), u32(0), [0], u32(crc32(new Uint8Array([1, 2, 3]))), [0]);
}
test('checked integers preserve widths and reject truncation/unsafe values', () => {
  for (const n of [0, 1, 127, 128, 16383, 16384, 2 ** 28, 2 ** 49, Number.MAX_SAFE_INTEGER]) {
    const b = integer(n), cursor = new ArcCursor(b);
    assert.equal(cursor.integer(), n); assert.equal(cursor.at, b.length);
    if (b.length > 1) assert.throws(() => new ArcCursor(b.subarray(0, -1)).integer(), e => e.code === 'ARC_BOUNDS');
  }
  assert.throws(() => new ArcCursor(integer(2n ** 63n)).integer(), e => e.code === 'ARC_INTEGER');
  assert.throws(() => checkedLimits({ entries: 0 }), TypeError);
});
test('footer descriptor validates exact bounds, checksum and control budget before decode', async () => {
  const b = descriptor(); const result = await footerDescriptor(new Blob([b]));
  assert.equal(result.offset, 0); assert.equal(result.storedSize, 3);
  b[b.length - 1] ^= 1;
  await assert.rejects(footerDescriptor(new Blob([b])), e => e.code === 'ARC_CRC');
  await assert.rejects(footerDescriptor(new Blob([descriptor()]), { limits: { controlBytes: 2 } }), e => e.code === 'ARC_LIMIT');
  await assert.rejects(footerDescriptor(new Blob([new Uint8Array(1)])), e => e.code === 'ARC_FOOTER');
});
test('control decode checks size and CRC and retains cancellation', async () => {
  const b = descriptor(), blob = new Blob([b]), block = await footerDescriptor(blob);
  assert.deepEqual(await decodeControl(blob, block, new Uint8Array([1]), {}), new Uint8Array([1, 2, 3]));
  await assert.rejects(decodeControl(blob, { ...block, crc: 0 }, new Uint8Array([1]), {}), e => e.code === 'ARC_CRC');
  const controller = new AbortController(); controller.abort();
  await assert.rejects(decodeControl(blob, block, new Uint8Array([1]), {}, { signal: controller.signal }), e => e.name === 'AbortError');
  let calls = 0;
  await assert.rejects(decodeControl(blob, { ...block, method: 'lzma:1mb', originalSize: 100 }, new Uint8Array([1]), { decodeLzma() { calls++; } }, { limits: { controlBytes: 10 } }), e => e.code === 'ARC_LIMIT');
  assert.equal(calls, 0);
});
test('footer block pointers and file records retain source offsets and opaque tails', () => {
  const bytes = join(integer(1), integer(3), string('storing'), integer(10), integer(3), integer(3), u32(1), [0], integer(0), [0, 0]);
  const footer = parseFooter(bytes, 100, 200);
  assert.equal(footer.blocks[0].offset, 90); assert.deepEqual(footer.trailing, new Uint8Array([0, 0]));
  assert.throws(() => parseFooter(bytes, 1, 200), e => e.code === 'ARC_BOUNDS');
  const decoded = parseDirectory(directory(), 100, 200);
  assert.equal(decoded.blocks[0].offset, 90); assert.equal(decoded.blocks[0].originalSize, 3);
  assert.equal(decoded.files[0].path, 'data.mpq'); assert.equal(decoded.files[0].decodedOffset, 0);
  assert.deepEqual(decoded.trailing, new Uint8Array([0]));
  for (const opts of [{ name: '../data.mpq' }, { directoryIndex: 1 }, { size: 1000 }]) {
    assert.throws(() => parseDirectory(directory(opts), 100, 200, { limits: { decodedBlockBytes: 100 } }), e => ['ARC_PATH', 'ARC_DIRECTORY', 'ARC_LIMIT'].includes(e.code));
  }
  assert.throws(() => parseDirectory(directory(), 100, 200, { limits: { entries: 1, blocks: 1, stringBytes: 3 } }), e => e.code === 'ARC_LIMIT');
});
test('CRC accumulation is stable across arbitrary chunk boundaries', () => {
  const bytes = new TextEncoder().encode('123456789');
  assert.equal(crc32(bytes), 0xcbf43926);
  assert.equal(crc32(bytes.subarray(3), crc32(bytes.subarray(0, 3))), crc32(bytes));
});
test('cipher profile derives authored parameters and legacy hex policy without leaking values', async () => {
  const password = new TextEncoder().encode('synthetic password'), salt = new Uint8Array([1, 2, 3]), iv = new Uint8Array(16).fill(0xff);
  const imported = await crypto.subtle.importKey('raw', password, 'PBKDF2', false, ['deriveBits']);
  const derived = new Uint8Array(await crypto.subtle.deriveBits({ name: 'PBKDF2', hash: 'SHA-512', salt, iterations: 1 }, imported, 34 * 8));
  const hex = bytes => Array.from(bytes, b => b.toString(16).padStart(2, '0')).join('');
  const method = `aes-256/ctr:n1:r0:s${hex(salt)}:c${hex(derived.subarray(32))}:i${hex(iv)}`;
  const legacy = await deriveCipher(method, password);
  assert.deepEqual(legacy.iv, new Uint8Array(16).fill(0x55));
  const fixed = await deriveCipher(`${method}:f`, password);
  assert.deepEqual(fixed.iv, iv); assert.deepEqual(fixed.key, derived.subarray(0, 32));
  await assert.rejects(deriveCipher(method, new Uint8Array([1])), e => e.code === 'ARC_PASSWORD');
  assert.throws(() => cipherParameters(`${method}:n2`), e => e.code === 'ARC_CIPHER');
  assert.throws(() => cipherParameters(method.replace(':n1', ':n2'), checkedLimits({ iterations: 1 })), e => e.code === 'ARC_LIMIT');
  assert.doesNotMatch(redactMethod(method), new RegExp(hex(iv)));
});
test('LZMA profile dictionary and property limits precede allocation', () => {
  assert.deepEqual(lzmaParameters('lzma:1mb:normal:bt4:32'), { dictSize: 1024 ** 2, properties: 93 });
  assert.throws(() => lzmaParameters('lzma:1000gb'), e => e.code === 'ARC_LIMIT');
  assert.throws(() => lzmaParameters('lzma:lc9'), e => e.code === 'ARC_LIMIT');
  assert.throws(() => lzmaParameters('unknown'), e => e.code === 'ARC_CODEC');
});
