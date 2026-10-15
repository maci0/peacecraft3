// Explicit opt-in: synthetic inputs against an already-built production codec.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { loadFreeArcCodec } from '../../tools/freearc-codec/js/freearc-codec.mjs';
import { extractArcMembers } from '../../web/iso/arc-extract.js';
import { crc32 } from '../../web/iso/freearc.js';

const wasmPath = process.env.PCX_FREEARC_CODEC_WASM;
test('real codec requires EXE policy, refuses unknown methods and preserves router cleanup/reuse', {
  skip: wasmPath ? false : 'set PCX_FREEARC_CODEC_WASM to an explicit built codec artifact',
}, async () => {
  const codec = await loadFreeArcCodec(await readFile(wasmPath));
  const u32 = value => [value & 255, value >>> 8 & 255, value >>> 16 & 255, value >>> 24];
  const payload = new Uint8Array([17, 29, 41]);
  const body = [...u32(0), ...u32(payload.length), ...payload];
  const rep = [...u32(64), ...u32(body.length), ...body, ...u32(0)];
  const packed = Uint8Array.from([...u32(0), ...u32(0xffffffff), ...u32(rep.length), ...rep]);
  const member = { path: 'synthetic.bin', size: payload.length, crc: crc32(payload), directory: false, block: 0 };
  const block = { method: 'rep:64b+exe+4x4:tor:32kb', offset: 0, storedSize: packed.length, originalSize: payload.length };
  const archive = { files: [member], blocks: [block] };
  const adapter = { decodeStream: options => codec.decodeStream(options) };
  async function run(maxIntermediateBytes) {
    let aborted = false; const chunks = [];
    try {
      const result = await extractArcMembers(new Blob([packed]), archive, [member], new Uint8Array([1]), adapter, {
        maxIntermediateBytes, readBytes: 16,
        createSink: async () => ({ write: async bytes => chunks.push(bytes.slice()), close: async () => new Blob(chunks), abort: async () => { aborted = true; } }),
      });
      assert.equal(aborted, false);
      assert.deepEqual(new Uint8Array(await result[0].file.arrayBuffer()), payload);
    } catch (error) { assert.equal(aborted, true); throw error; }
  }
  await assert.rejects(run(undefined), error => error.code === 'INVALID_ARGUMENT');
  await assert.rejects(run(payload.length), error => error.code === 'STREAM_LIMIT');
  block.method = 'rep:64b+exe:1+4x4:tor:32kb';
  await assert.rejects(run(rep.length), error => error.code === 'UNSUPPORTED_METHOD');
  block.method = 'rep:64b+exe+4x4:tor:32kb';
  await run(rep.length);
});
