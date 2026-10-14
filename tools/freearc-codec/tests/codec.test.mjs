// Tests for the built freearc-codec.wasm through js/freearc-codec.mjs (bun test).
// Synthetic inputs only. Requires FREEARC_CODEC_BUILD (build.sh output with
// freearc-codec.wasm and native_test fixtures/) and xz for LZMA streams.
import { describe, expect, test } from 'bun:test';
import { createCipheriv } from 'node:crypto';
import { loadFreeArcCodec, FreeArcCodecError } from '../js/freearc-codec.mjs';

const build = process.env.FREEARC_CODEC_BUILD;
if (!build) throw new Error('FREEARC_CODEC_BUILD must point at the build.sh output directory');
const wasm = await Bun.file(`${build}/freearc-codec.wasm`).arrayBuffer();
const module = await WebAssembly.compile(wasm);
const fresh = () => loadFreeArcCodec(module);
const codec = await fresh();

const hex = s => Uint8Array.from(s.match(/../g), b => parseInt(b, 16));
const rejects = async (promise, code) => {
  const err = await promise.then(() => null, e => e);
  expect(err).toBeInstanceOf(FreeArcCodecError);
  expect(err.code).toBe(code);
  return err;
};

describe('lzma', () => {
  // xz is the only LZMA encoder available; no in-process alternative exists here.
  const encode = (data, opts) => {
    const r = Bun.spawnSync(['xz', '--format=raw', `--lzma1=${opts}`, '-c'], { stdin: data });
    expect(r.exitCode).toBe(0);
    return new Uint8Array(r.stdout);
  };
  const data = new TextEncoder().encode('Lok\'tar ogar! '.repeat(4000) + 'end');
  const props = (lc, lp, pb) => (pb * 5 + lp) * 9 + lc;

  test('end marker, exact size, non-default lc/lp/pb', async () => {
    const raw = encode(data, 'lc=0,lp=2,pb=0,dict=64KiB');
    const opts = { properties: props(0, 2, 0), dictSize: 64 << 10 };
    expect(await codec.decodeLzma(raw, { ...opts, maxOutputBytes: 1 << 20 })).toEqual(data);
    expect(await codec.decodeLzma(raw, { ...opts, maxOutputBytes: data.length, expectedSize: data.length })).toEqual(data);
    const prefix = await codec.decodeLzma(raw, { ...opts, maxOutputBytes: 100, expectedSize: 100 });
    expect(prefix).toEqual(data.subarray(0, 100));
  });

  test('ceilings, truncation and corruption', async () => {
    const raw = encode(data, 'lc=3,lp=0,pb=2,dict=1MiB');
    const opts = { properties: props(3, 0, 2), dictSize: 1 << 20 };
    await rejects(codec.decodeLzma(raw, { ...opts, maxOutputBytes: data.length - 1 }), 'OUTPUT_LIMIT');
    await rejects(codec.decodeLzma(raw, { ...opts, maxOutputBytes: 1 << 20, expectedSize: data.length + 1 }), 'TRUNCATED');
    await rejects(codec.decodeLzma(raw.subarray(0, raw.length - 8), { ...opts, maxOutputBytes: 1 << 20 }), 'TRUNCATED');
    const bad = raw.slice();
    bad[0] = 1;  // the range coder's first byte must be zero
    await rejects(codec.decodeLzma(bad, { ...opts, maxOutputBytes: 1 << 20 }), 'BAD_COMPRESSED_DATA');
    await rejects(codec.decodeLzma(raw, { ...opts, properties: 225, maxOutputBytes: 1 }), 'INVALID_ARGUMENT');
    await rejects(codec.decodeLzma(raw, { ...opts, maxOutputBytes: 10, expectedSize: 11 }), 'OUTPUT_LIMIT');
    const ac = new AbortController();
    ac.abort();
    await expect(codec.decodeLzma(raw, { ...opts, maxOutputBytes: 1 << 20, signal: ac.signal })).rejects.toThrow();
  });
});

describe('aes-ctr', () => {
  const key = hex('000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f');
  // Reference keystream: AES-ECB of IV + k, k as a 128-bit little-endian add.
  const reference = (iv, blocks, start = 0n) => {
    let ctr = 0n;
    for (let i = 15; i >= 0; i--) ctr = (ctr << 8n) | BigInt(iv[i]);
    const counters = new Uint8Array(blocks * 16);
    for (let k = 0; k < blocks; k++) {
      let v = (ctr + start + BigInt(k)) & ((1n << 128n) - 1n);
      for (let i = 0; i < 16; i++, v >>= 8n) counters[k * 16 + i] = Number(v & 0xFFn);
    }
    const ecb = createCipheriv(`aes-${key.length * 8}-ecb`, key, null).setAutoPadding(false);
    return new Uint8Array(Buffer.concat([ecb.update(counters), ecb.final()]));
  };

  test('FIPS-197 C.3 block as the first keystream block', () => {
    const ctr = codec.aesCtr(key, hex('00112233445566778899aabbccddeeff'));
    expect(ctr.transform(new Uint8Array(16))).toEqual(hex('8ea2b7ca516745bfeafc49904b496089'));
    ctr.close();
  });

  test('keystream, chunking, start offset, partial last block', () => {
    const iv = hex('0100000000000000ffffffffffffffff');  // high half all ones: no carry reaches it
    const data = Uint8Array.from({ length: 1000 }, (_, i) => i * 7);
    const ks = reference(iv, 63);
    const want = data.map((b, i) => b ^ ks[i]);
    const whole = codec.aesCtr(key, iv);
    expect(whole.transform(data)).toEqual(want);
    whole.close();
    const split = codec.aesCtr(key, iv);
    const a = split.transform(data.subarray(0, 320)), b = split.transform(data.subarray(320));
    expect(Uint8Array.from([...a, ...b])).toEqual(want);
    expect(() => split.transform(new Uint8Array(16))).toThrow('only the last');
    split.close();
    const resumed = codec.aesCtr(key, iv, 20n);
    expect(resumed.transform(data.subarray(320))).toEqual(want.subarray(320));
    resumed.close();
  });

  test('ranges where the C and AES-NI counters disagree are refused', () => {
    const near = hex('fdffffffffffffff0000000000000000');  // low64 = 2^64 - 3
    const ctr = codec.aesCtr(key, near);
    expect(ctr.transform(new Uint8Array(48))).toEqual(reference(near, 3));  // last block before the wrap
    expect(() => ctr.transform(new Uint8Array(16))).toThrow(FreeArcCodecError);
    ctr.close();
    const over = codec.aesCtr(key, near);
    expect(() => over.transform(new Uint8Array(49))).toThrow('AES_COUNTER_AMBIGUOUS');
    over.close();
    expect(() => codec.aesCtr(key, near, 3n)).toThrow('AES_COUNTER_AMBIGUOUS');
    expect(() => codec.aesCtr(key, hex('0000000000000000aa00000000000000'))).toThrow('AES_COUNTER_AMBIGUOUS');
    expect(() => codec.aesCtr(key.subarray(0, 20), near)).toThrow(FreeArcCodecError);
  });
});

describe('method limits', () => {
  test('authored chains', () => {
    expect(codec.methodLimits('rep:96mb:256+4x4:tor:3:8mb')).toEqual({ repDictionary: 96 << 20, blockSize: 8 << 20, torBuffer: 8 << 20 });
    expect(codec.methodLimits('rep:6kb:256+4x4:tor:6kb:h32kb')).toEqual({ repDictionary: 6 << 10, blockSize: 6 << 10, torBuffer: 32 << 10 });
    // Defaults from REP_METHOD() and the upstream Tornado preset table (5: 16mb, 3: 4mb).
    expect(codec.methodLimits('rep+4x4:tor')).toEqual({ repDictionary: 64 << 20, blockSize: 16 << 20, torBuffer: 16 << 20 });
    expect(codec.methodLimits('rep:b1g+4x4:b2mb:tor:8mb:3')).toEqual({ repDictionary: 1 << 30, blockSize: 2 << 20, torBuffer: 4 << 20 });
    for (const m of ['rep:32mb:256+exe+4x4:tor:16mb', 'lzma+4x4:tor', 'rep+4x4:lzma', 'rep:zz+4x4:tor'])
      expect(() => codec.methodLimits(m)).toThrow(FreeArcCodecError);
  });
});

// 4x4 stream of stored blocks around raw REP bytes, and REP block builders.
const u32 = v => [v & 255, (v >>> 8) & 255, (v >>> 16) & 255, v >>> 24];
const stored = (rep, block = 8192) => {
  const out = [...u32(0)];
  for (let p = 0; p < rep.length; p += block) {
    const part = rep.slice(p, p + block);
    out.push(...u32(0xFFFFFFFF), ...u32(part.length), ...part);
  }
  return Uint8Array.from(out);
};
const repBlock = (items, last) => {
  const body = [...u32(items.length)];
  for (const it of items) body.push(...u32(it.len));
  for (const it of items) body.push(...u32(it.offset));
  for (const it of [...items, { lit: last }]) body.push(...u32(it.lit.length));
  for (const it of [...items, { lit: last }]) body.push(...it.lit);
  return [...u32(body.length), ...body];
};
const repStream = (dict, blocks, tail = []) => [...u32(dict), ...blocks.flat(), ...u32(0), ...tail];

const METHOD = 'rep:4kb+4x4:tor:3:8kb';
async function run(c, input, opts = {}) {
  const chunks = [];
  let pos = 0;
  const res = await c.decodeStream({
    method: METHOD,
    read: async max => {
      const n = Math.min(max, 1 + ((pos * 7919) % 5000));
      const part = input.subarray(pos, pos + n);
      pos += part.length;
      return part;
    },
    onWrite: async chunk => { chunks.push(chunk.slice()); },
    maxOutputBytes: 1 << 20,
    ...opts,
  });
  return { ...res, out: Uint8Array.from(chunks.flatMap(c => [...c])) };
}

describe('rep + 4x4:tor stream', () => {
  test('tornado fixture from the native test (upstream compressor)', async () => {
    const input = new Uint8Array(await Bun.file(`${build}/fixtures/tor3-wrap.fa`).arrayBuffer());
    const want = new Uint8Array(await Bun.file(`${build}/fixtures/tor3-wrap.out`).arrayBuffer());
    const res = await run(codec, input, { expectedSize: want.length });
    expect(res.outputBytes).toBe(want.length);
    expect(res.out).toEqual(want);
    expect(res.peakAllocBytes).toBeGreaterThan(0);
  });

  const lit = n => Array.from({ length: n }, (_, i) => (i * 31 + 7) & 255);
  test('stored blocks, overlapping matches, empty block list', async () => {
    const rep = repStream(4096, [repBlock([{ lit: [1, 2, 3], offset: 3, len: 10 }], [9]), repBlock([], [])]);
    const res = await run(codec, stored(rep), { expectedSize: 14 });
    expect([...res.out]).toEqual([1, 2, 3, 1, 2, 3, 1, 2, 3, 1, 2, 3, 1, 9]);
  });

  test('malformed streams return status errors and leave the instance usable', async () => {
    const ok = repStream(4096, [repBlock([], lit(10))]);
    const cases = [
      [repStream(4096, [repBlock([{ lit: lit(5), offset: 6, len: 1 }], [])]), 'BAD_COMPRESSED_DATA'],  // offset > decoded
      [repStream(4096, [repBlock([{ lit: lit(5), offset: 0, len: 1 }], [])]), 'BAD_COMPRESSED_DATA'],
      [[...u32(4096), ...u32(12), ...u32(0), ...u32(9), 1, 2, 3, 4, ...u32(0)], 'BAD_COMPRESSED_DATA'],  // literal overrun
      [repStream(8192, [repBlock([], lit(10))]), 'STREAM_LIMIT'],  // dictionary above rep:4kb
      [repStream(0, []), 'BAD_COMPRESSED_DATA'],
      [repStream(4096, [repBlock([], lit(10))], [1]), 'TRAILING_DATA'],
      [ok.slice(0, ok.length - 2), 'TRUNCATED'],
    ];
    for (const [rep, code] of cases) await rejects(run(codec, stored(Uint8Array.from(rep))), code);
    const badVersion = stored(Uint8Array.from(ok));
    badVersion[0] = 1;
    await rejects(run(codec, badVersion), 'BAD_COMPRESSED_DATA');
    const bigBlock = Uint8Array.from([...u32(0), ...u32(0xFFFFFFFF), ...u32(8193)]);
    await rejects(run(codec, bigBlock), 'STREAM_LIMIT');
    await rejects(run(codec, stored(Uint8Array.from(ok)), { expectedSize: 11 }), 'SIZE_MISMATCH');
    await rejects(run(codec, stored(Uint8Array.from(ok)), { maxOutputBytes: 9 }), 'OUTPUT_LIMIT');
    await rejects(run(codec, stored(Uint8Array.from(ok)), { maxOutputBytes: 10, maxDictionaryBytes: 4095 }), 'STREAM_LIMIT');
    await rejects(run(codec, stored(Uint8Array.from(ok)), { maxOutputBytes: 10, maxMemoryBytes: 1 << 16 }), 'MEMORY_LIMIT');
    expect((await run(codec, stored(Uint8Array.from(ok)), { expectedSize: 10 })).out).toEqual(Uint8Array.from(lit(10)));
  });

  test('callback failures, abort and concurrency', async () => {
    const input = stored(Uint8Array.from(repStream(4096, [repBlock([], lit(10))])));
    const base = { method: METHOD, maxOutputBytes: 10 };
    await expect(codec.decodeStream({ ...base, read: async () => { throw new Error('disk'); }, onWrite: async () => {} })).rejects.toThrow('disk');
    await expect(codec.decodeStream({ ...base, read: async () => input, onWrite: async () => { throw new Error('quota'); } })).rejects.toThrow('quota');
    const ac = new AbortController();
    ac.abort();
    await expect(codec.decodeStream({ ...base, signal: ac.signal, read: async () => input, onWrite: async () => {} })).rejects.toThrow();
    let release;
    const gate = new Promise(r => (release = r));
    const first = codec.decodeStream({ ...base, read: async () => (await gate, input), onWrite: async () => {} });
    await rejects(codec.decodeStream({ ...base, read: async () => input, onWrite: async () => {} }), 'BUSY');
    release();
    await first.catch(() => {});
    expect((await run(codec, input, { expectedSize: 10 })).outputBytes).toBe(10);
  });

  test('corrupt tornado data fails; an aborted instance is poisoned, a fresh one works', async () => {
    const input = new Uint8Array(await Bun.file(`${build}/fixtures/tor3-wrap.fa`).arrayBuffer());
    const want = await Bun.file(`${build}/fixtures/tor3-wrap.out`).arrayBuffer();
    let poisoned = 0, statuses = 0;
    for (let t = 0; t < 40; t++) {
      const c = await fresh();
      const bad = input.slice();
      for (let k = 0; k < 8; k++) bad[100 + ((t * 977 + k * 131) % (bad.length - 100))] ^= 0x5A;
      const err = await run(c, bad, { maxOutputBytes: want.byteLength }).then(r => r, e => e);
      if (!(err instanceof Error)) continue;  // a flip inside literals may still decode; root checks CRCs
      expect(err).toBeInstanceOf(FreeArcCodecError);
      if (err.code === 'CODEC_ABORT') {
        poisoned++;
        await rejects(c.decodeLzma(new Uint8Array(5), { properties: 93, dictSize: 1 << 16, maxOutputBytes: 1 }), 'POISONED');
      } else statuses++;
    }
    expect(poisoned + statuses).toBeGreaterThan(0);
    expect((await run(await fresh(), input, { expectedSize: want.byteLength })).out).toEqual(new Uint8Array(want));
  });
});
