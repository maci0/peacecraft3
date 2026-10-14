import test from 'node:test';
import assert from 'node:assert/strict';
import { discoverIso, sliceIsoFile } from '../../web/iso.js';
const S = 2048;
function number(bytes, at, n, width = 4) {
  const v = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  if (width === 4) { v.setUint32(at, n, true); v.setUint32(at + 4, n); }
  else { v.setUint16(at, n, true); v.setUint16(at + 2, n); }
}
function rec(name, extent, size, flags = 0) {
  const encoded = typeof name === 'string' ? Uint8Array.from(name, c => c.charCodeAt(0)) : name;
  const r = new Uint8Array(33 + encoded.length + (encoded.length % 2 === 0 ? 1 : 0));
  r[0] = r.length; number(r, 2, extent); number(r, 10, size); r[25] = flags;
  number(r, 28, 1, 2); r[32] = encoded.length;
  r.set(encoded, 33); return r;
}
function wide(name) {
  return Uint8Array.from([...name].flatMap(c => [c.charCodeAt(0) >> 8, c.charCodeAt(0) & 255]));
}
function jolietFixture(name = wide('遊戲 archive.mpq'), level = '%/E') {
  const bytes = fixture(), svd = bytes.subarray(17 * S, 18 * S);
  svd.set(bytes.subarray(16 * S, 17 * S)); svd[0] = 2;
  svd.fill(0, 40, 72); svd.set(wide('Unicode volume'), 40);
  svd.set(new TextEncoder().encode(level), 88); svd.set(rec('\0', 21, S, 2), 156);
  const end = bytes.subarray(18 * S, 19 * S); end.set(bytes.subarray(17 * S, 17 * S + 7)); end[0] = 255;
  // Primary and supplementary directories intentionally have distinct names.
  bytes.set(rec('\0', 21, S, 2), 21 * S);
  bytes.set(rec('\x01', 21, S, 2), 21 * S + 34);
  bytes.set(rec(name, 24, 64), 21 * S + 68);
  return bytes;
}
function fixture(files = [{ name: 'ASSET.BIN;1', data: mpq() }]) {
  const bytes = new Uint8Array((24 + files.length) * S);
  const pvd = bytes.subarray(16 * S, 17 * S);
  pvd[0] = 1; pvd.set(new TextEncoder().encode('CD001'), 1); pvd[6] = 1;
  pvd.fill(32, 40, 72); pvd.set(new TextEncoder().encode('SYNTHETIC'), 40);
  number(pvd, 80, bytes.length / S); number(pvd, 120, 1, 2); number(pvd, 124, 1, 2); number(pvd, 128, S, 2);
  pvd.set(rec('\0', 20, S, 2), 156);
  const end = bytes.subarray(17 * S, 18 * S); end[0] = 255; end.set(new TextEncoder().encode('CD001'), 1); end[6] = 1;
  let at = 20 * S;
  for (const r of [rec('\0', 20, S, 2), rec('\x01', 20, S, 2), ...files.map((f, i) => rec(f.name, 24 + i, f.data.length, f.flags))]) { bytes.set(r, at); at += r.length; }
  files.forEach((f, i) => bytes.set(f.data, (24 + i) * S));
  return bytes;
}
function mpq() {
  const bytes = new Uint8Array(64), v = new DataView(bytes.buffer);
  bytes.set([77, 80, 81, 26]); v.setUint32(4, 32, true); v.setUint32(8, 64, true);
  v.setUint32(16, 32, true); v.setUint32(20, 48, true); v.setUint32(24, 1, true); v.setUint32(28, 1, true);
  return bytes;
}
const discover = bytes => discoverIso(new Blob([bytes]));
test('local content recognition, provenance, bounded reads and zero network calls', async () => {
  let calls = 0;
  const names = ['fetch', 'XMLHttpRequest', 'WebSocket', 'EventSource'];
  const saved = names.map(name => Object.getOwnPropertyDescriptor(globalThis, name));
  names.forEach(name => Object.defineProperty(globalThis, name, { configurable: true, value: () => { calls++; throw Error('network forbidden'); } }));
  try {
    const bytes = fixture([{ name: 'ASSET.BIN;1', data: mpq() }, { name: 'WAR3.MPQ;1', data: new Uint8Array([1, 2, 3]) }]);
    const blob = new Blob([bytes]); let largest = 0;
    const input = { size: blob.size, name: 'synthetic.iso', slice(a, b) { largest = Math.max(largest, b - a); return blob.slice(a, b); } };
    const result = await discoverIso(input);
    assert.equal(calls, 0); assert.equal(largest, S);
    assert.equal(result.candidates.length, 1); assert.equal(result.candidates[0].path, '/ASSET.BIN;1');
    assert.equal(result.candidates[0].offset, 24 * S); assert.equal(result.candidates[0].archiveSize, 64);
    assert.equal(result.provenance.volumeIdentifier, 'SYNTHETIC'); assert.equal(result.provenance.edition, null);
    assert.equal(result.provenance.sourceName, 'synthetic.iso');
    assert.equal(result.diagnostics[0].code, 'UNRECOGNIZED_CONTENT');
    assert.deepEqual(new Uint8Array(await sliceIsoFile(blob, result.candidates[0]).arrayBuffer()), mpq());
    assert.ok(result.work.bytesRead < bytes.length);
  } finally { names.forEach((name, i) => saved[i] ? Object.defineProperty(globalThis, name, saved[i]) : delete globalThis[name]); }
});
test('malformed extents, endian copies, padding, MPQ tables and truncated image fail closed', async () => {
  const mutations = [
    [b => number(b, 16 * S + 80, 1000), 'BOUNDS'],
    [b => { b[16 * S + 84] ^= 1; }, 'ENDIAN_MISMATCH'],
    [b => number(b, 20 * S + 68 + 2, 1000), 'BOUNDS'],
    [b => { b[20 * S + 200] = 1; }, 'DIRECTORY_PADDING'],
    [b => new DataView(b.buffer).setUint32(24 * S + 24, 1000, true), 'BOUNDS'],
    [b => { b[20 * S + 68] = 33; }, 'DIRECTORY_RECORD'],
  ];
  for (const [mutate, code] of mutations) { const b = fixture(); mutate(b); await assert.rejects(discover(b), e => e.code === code); }
  await assert.rejects(discover(fixture().slice(0, 16 * S + 4)), e => e.code === 'BOUNDS');
  assert.throws(() => sliceIsoFile(new Blob([new Uint8Array(2)]), { offset: 1, size: 2 }), e => e.code === 'BOUNDS');
});
test('unsupported namespaces and multi-extent files have specific blockers', async () => {
  const joliet = fixture(); joliet[17 * S] = 2;
  await assert.rejects(discover(joliet), e => e.code === 'UNSUPPORTED_SUPPLEMENTARY');
  const udf = fixture(); udf.set(new TextEncoder().encode('NSR03'), 16 * S + 1);
  await assert.rejects(discover(udf), e => e.code === 'UNSUPPORTED_UDF');
  await assert.rejects(discover(fixture([{ name: 'A;1', data: mpq(), flags: 128 }])), e => e.code === 'UNSUPPORTED_MULTI_EXTENT');
});
test('each recognized Joliet level decodes its own UCS-2 directory, without primary aliases', async () => {
  for (const [index, escape] of ['%/@', '%/C', '%/E'].entries()) {
    const result = await discover(jolietFixture(wide('遊戲 archive.mpq'), escape));
    assert.equal(result.provenance.filesystem, 'ISO9660-Joliet');
    assert.equal(result.provenance.jolietLevel, index + 1);
    assert.equal(result.provenance.volumeIdentifier, 'Unicode volume');
    assert.equal(result.provenance.descriptorOffset, 17 * S);
    assert.deepEqual(result.files.map(f => f.path), ['/遊戲 archive.mpq']);
    assert.equal(result.candidates.length, 1);
    assert.equal(result.candidates[0].offset, 24 * S);
  }
});
test('malformed or ambiguous Joliet cannot silently fall back to valid primary MPQ', async () => {
  for (const name of [new Uint8Array([65, 0, 66]), new Uint8Array([0xd8, 0]), wide('bad/name'), wide('bad\0name')]) {
    await assert.rejects(discover(jolietFixture(name)), e => ['JOLIET_IDENTIFIER', 'DIRECTORY_RECORD'].includes(e.code));
  }
  const wrongExtent = jolietFixture(); number(wrongExtent, 17 * S + 158, 9999);
  await assert.rejects(discover(wrongExtent), e => e.code === 'BOUNDS');
  const mismatch = jolietFixture(); number(mismatch, 17 * S + 80, 24);
  await assert.rejects(discover(mismatch), e => e.code === 'VOLUME_MISMATCH');
  const duplicate = jolietFixture(); duplicate.copyWithin(18 * S, 17 * S, 18 * S);
  await assert.rejects(discover(duplicate), e => e.code === 'AMBIGUOUS_SUPPLEMENTARY');
  const unknown = jolietFixture(wide('x'), '%/X');
  await assert.rejects(discover(unknown), e => e.code === 'UNSUPPORTED_SUPPLEMENTARY');
});
test('FreeArc signature reports the installer decoder gap without guessing names or edition', async () => {
  const result = await discover(fixture([{ name: 'PAYLOAD;1', data: new Uint8Array([65, 114, 67, 1]) }]));
  assert.equal(result.candidates.length, 0);
  assert.equal(result.diagnostics[0].code, 'UNSUPPORTED_FREEARC');
  assert.equal(result.provenance.edition, null);
});
test('PE is content evidence, not installer success or archive candidate', async () => {
  const pe = new Uint8Array(128); pe.set([77, 90]); new DataView(pe.buffer).setUint32(60, 64, true); pe.set([80, 69, 0, 0], 64);
  const result = await discover(fixture([{ name: 'PAYLOAD.BIN;1', data: pe }]));
  assert.equal(result.candidates.length, 0); assert.equal(result.diagnostics[0].code, 'UNSUPPORTED_PE_PAYLOAD');
  new DataView(pe.buffer).setUint32(60, 1000, true);
  await assert.rejects(discover(fixture([{ name: 'X;1', data: pe }])), e => e.code === 'BOUNDS');
});
test('directory recursion, duplicate extents and bounded entry budget', async () => {
  const b = fixture(); b.fill(0, 20 * S, 21 * S);
  b.set(rec('CHILD', 21, S, 2), 20 * S); b.set(rec('A;1', 24, 64), 21 * S);
  assert.equal((await discover(b)).candidates[0].path, '/CHILD/A;1');
  b.set(rec('LOOP', 20, S, 2), 21 * S);
  await assert.rejects(discover(b), e => e.code === 'DIRECTORY_CYCLE');
  await assert.rejects(discoverIso(new Blob([fixture()]), { maxEntries: 1 }), e => e.code === 'ENTRY_LIMIT');
});
test('cancellation before and during reads', async () => {
  const abort = new AbortController(); abort.abort();
  await assert.rejects(discoverIso(new Blob([fixture()]), { signal: abort.signal }), e => e.name === 'AbortError');
  const during = new AbortController();
  await assert.rejects(discoverIso(new Blob([fixture()]), { signal: during.signal, onProgress() { during.abort(); } }), e => e.name === 'AbortError');
});
