// Loader for freearc-codec.wasm: FreeArc 0.67 LZMA, AES-256-CTR and
// `rep+[exe+]4x4:tor` decoding for local browser imports. Runs in a window or worker,
// needs no SharedArrayBuffer and no cross-origin isolation.
//
// Copyright (C) Bulat Ziganshin (FreeArc formats and decoders).
// Licensed under the GNU General Public License version 2; see ../upstream/LICENSE.

// Status codes: FREEARC_ERRCODE_* (upstream Common.h) and FA_ERR_* (src/codec.cpp).
const STATUS = new Map([
  [-1, 'GENERAL'], [-2, 'INVALID_COMPRESSOR'], [-4, 'OUTPUT_LIMIT'], [-5, 'MEMORY_LIMIT'],
  [-6, 'READ'], [-7, 'BAD_COMPRESSED_DATA'], [-8, 'NOT_IMPLEMENTED'], [-11, 'WRITE'],
  [-15, 'INTERNAL'], [-100, 'STREAM_LIMIT'], [-101, 'TRUNCATED'], [-102, 'TRAILING_DATA'],
  [-103, 'AES_COUNTER_AMBIGUOUS'],
]);
const STEP_NEED_INPUT = 1, STEP_OUTPUT = 2, STEP_DONE = 3;
const U32_MAX = 0xFFFFFFFF;
const AES_IV_BYTES = 16, AES_KEY_BYTES = [16, 24, 32];
const MEM_UNITS = { b: 1, k: 1 << 10, m: 1 << 20, g: 1 << 30 };
const LZMA_PROPS_LIMIT = 9 * 5 * 5;
const EXIT_MESSAGE_LIMIT = 256;
// FreeArc REP_METHOD() default BlockSize (Compression/REP/C_REP.cpp), when `rep` has none.
const REP_DEFAULT_DICTIONARY = 64 << 20;
// Tornado rounds its buffer as tor_compress0 does for DeCompressMem (bytes_to_compress -1).
const TOR_MIN_BUFFER = 32 << 10, TOR_BUFFER_ALIGN = 4 << 10;

export class FreeArcCodecError extends Error {
  constructor(code, message) {
    super(message);
    this.name = 'FreeArcCodecError';
    this.code = code;
  }
}

function fail(code, message) {
  throw new FreeArcCodecError(code, message);
}

function statusError(status, what) {
  const code = STATUS.get(status) ?? `STATUS_${status}`;
  return new FreeArcCodecError(code, `${what}: ${code} (${status})`);
}

function uint(value, name, max = Number.MAX_SAFE_INTEGER) {
  if (!Number.isSafeInteger(value) || value < 0 || value > max) fail('INVALID_ARGUMENT', `${name} must be an integer in 0..${max}`);
  return value;
}

// parseMem64 (Compression/Common.cpp): digits, then b/k/m/g/^ (first letter only; '^' if absent).
function parseMem(param) {
  const m = /^=?(\d+)(.?)/.exec(param);
  if (!m) return null;
  const n = Number(m[1]), unit = m[2] || '^';
  const value = unit === '^' ? 2 ** n : MEM_UNITS[unit] === undefined ? null : n * MEM_UNITS[unit];
  return Number.isSafeInteger(value) ? value : null;
}

const isInt = p => /^\d+$/.test(p);

// Limits for `rep:<...>+4x4:<...>:tor:<...>` from the authored method string,
// following parse_REP, parse_4x4 and parse_TORNADO. Parameters that set sizes
// must parse; the others (match lengths, hashes, threads) are skipped unchecked.
export function parseRep4x4TorMethod(method, presets) {
  const fields = String(method).split('+');
  const exe = fields.length === 3 && fields[1] === 'exe';
  if (fields.length !== 2 && !exe) fail('UNSUPPORTED_METHOD', 'expected rep+[exe+]4x4:tor');
  const rep = fields[0].split(':'), x4 = fields[exe ? 2 : 1].split(':');
  if (rep[0] !== 'rep' || x4[0] !== '4x4') fail('UNSUPPORTED_METHOD', 'expected rep+4x4:tor');

  let repDictionary = REP_DEFAULT_DICTIONARY;
  for (const p of rep.slice(1)) {
    if (p === 'max' || /^[lcdsha]/.test(p) || p.endsWith('%')) continue;
    const mem = p[0] === 'b' ? parseMem(p.slice(1)) : isInt(p) ? -1 : parseMem(p);
    if (mem === null) fail('UNSUPPORTED_METHOD', `rep parameter ${p}`);
    if (mem >= 0) repDictionary = mem;
  }

  let blockSize = 0, i = 1;
  for (; i < x4.length; i++) {
    const p = x4[i];
    if (!(/\d/.test(p[0] ?? '') || /\d/.test(p[1] ?? ''))) break;  // submethod starts
    if (p[0] === 'b') blockSize = parseMem(p.slice(1));
    else if (/^[ti]\d+$/.test(p) || /^r[\d.]+$/.test(p) || isInt(p)) continue;
    else blockSize = parseMem(p);
    if (blockSize === null) fail('UNSUPPORTED_METHOD', `4x4 parameter ${p}`);
  }
  const tor = x4.slice(i);
  if (tor[0] !== 'tor') fail('UNSUPPORTED_METHOD', '4x4 submethod must be tor');
  let torBuffer = presets.buffer(presets.defaultPreset);
  for (const p of tor.slice(1)) {
    if (p[0] === 'b') torBuffer = parseMem(p.slice(1));
    else if (/^(fb|ah|al)/.test(p) || /^[hlcpxsut]/.test(p)) continue;
    else if (isInt(p) && Number(p) < presets.count) torBuffer = presets.buffer(Number(p));
    else torBuffer = parseMem(p);
    if (torBuffer === null) fail('UNSUPPORTED_METHOD', `tor parameter ${p}`);
  }
  // _4x4MTCompressor: BlockSize || cmethod->GetDictionary() (the tornado buffer).
  const block = blockSize || torBuffer;
  const headerBuffer = Math.ceil(Math.max(torBuffer, TOR_MIN_BUFFER) / TOR_BUFFER_ALIGN) * TOR_BUFFER_ALIGN;
  for (const [name, v] of [['rep dictionary', repDictionary], ['4x4 block', block], ['tor buffer', headerBuffer]])
    if (!(v > 0 && v <= U32_MAX)) fail('UNSUPPORTED_METHOD', `${name} ${v}`);
  // Keep the old non-EXE methodLimits result shape for existing callers.
  return { repDictionary, blockSize: block, torBuffer: headerBuffer, ...(exe ? { exe: true } : {}) };
}

async function toModule(source) {
  if (source instanceof WebAssembly.Module) return source;
  if (typeof Response !== 'undefined' && source instanceof Response) source = await source.arrayBuffer();
  return WebAssembly.compile(source);
}

export async function loadFreeArcCodec(source) {
  let exports, exitMessage = '';
  const bytes = () => new Uint8Array(exports.memory.buffer);
  const imports = {
    env: {
      emscripten_notify_memory_growth() {},
      _emscripten_throw_longjmp() { fail('CODEC_ABORT', 'codec longjmp'); },
    },
    wasi_snapshot_preview1: {
      // Upstream CHECK() and the Tornado range decoder print, then exit().
      proc_exit(code) { fail('CODEC_ABORT', `codec exit(${code}): ${exitMessage.trim()}`); },
      fd_write(fd, iovs, count, written) {
        const view = new DataView(exports.memory.buffer);
        let total = 0;
        for (let k = 0; k < count; k++) {
          const ptr = view.getUint32(iovs + k * 8, true), len = view.getUint32(iovs + k * 8 + 4, true);
          if (exitMessage.length < EXIT_MESSAGE_LIMIT) exitMessage += new TextDecoder().decode(bytes().subarray(ptr, ptr + len));
          total += len;
        }
        view.setUint32(written, total, true);
        return 0;
      },
      fd_close() { return 0; },
      fd_seek() { return 70; },  // ESPIPE: no seekable descriptors
    },
  };
  const instance = await WebAssembly.instantiate(await toModule(source), imports);
  exports = instance.exports;
  exports._initialize();
  return new FreeArcCodec(exports, bytes);
}

class FreeArcCodec {
  #ex; #bytes; #poisoned = null; #streaming = false;
  #presets;

  constructor(ex, bytes) {
    this.#ex = ex;
    this.#bytes = bytes;
    this.#presets = {
      count: ex.fa_tor_preset_count(),
      defaultPreset: ex.fa_tor_default_preset(),
      buffer: n => ex.fa_tor_preset_buffer(n) >>> 0,
    };
  }

  // A trap or upstream exit() inside a codec call leaves its heap unknown; status
  // errors do not.
  #call(fn) {
    if (this.#poisoned) throw new FreeArcCodecError('POISONED', `codec instance unusable after: ${this.#poisoned.message}`);
    try {
      return fn();
    } catch (e) {
      if (!(e instanceof FreeArcCodecError) || e.code === 'CODEC_ABORT') this.#poisoned = e;
      throw e;
    }
  }

  #alloc(n) {
    const p = this.#ex.fa_malloc(n);
    if (!p) fail('MEMORY_LIMIT', `cannot allocate ${n} bytes`);
    return p;
  }

  methodLimits(method) {
    return parseRep4x4TorMethod(method, this.#presets);
  }

  async decodeLzma(raw, { properties, dictSize, maxOutputBytes, expectedSize, signal } = {}) {
    signal?.throwIfAborted();
    uint(properties, 'properties', LZMA_PROPS_LIMIT - 1);
    uint(dictSize, 'dictSize', U32_MAX);
    uint(maxOutputBytes, 'maxOutputBytes', U32_MAX);
    if (expectedSize !== undefined && uint(expectedSize, 'expectedSize', U32_MAX) > maxOutputBytes)
      fail('OUTPUT_LIMIT', `expectedSize ${expectedSize} exceeds maxOutputBytes ${maxOutputBytes}`);
    const cap = expectedSize ?? maxOutputBytes;
    const out = this.#call(() => {
      const ex = this.#ex;
      const inPtr = this.#alloc(raw.length), outPtr = this.#alloc(cap), lenPtr = this.#alloc(4);
      try {
        this.#bytes().set(raw, inPtr);
        const status = ex.fa_lzma_decode(inPtr, raw.length, properties, dictSize, outPtr, cap,
          expectedSize === undefined ? 0 : 1, lenPtr);
        if (status < 0) throw statusError(status, 'lzma');
        const len = new DataView(ex.memory.buffer).getUint32(lenPtr, true);
        return this.#bytes().slice(outPtr, outPtr + len);
      } finally {
        ex.fa_free(inPtr), ex.fa_free(outPtr), ex.fa_free(lenPtr);
      }
    });
    signal?.throwIfAborted();
    return out;
  }

  // FreeArc 0.67 AES-CTR (r0): block k of the stream uses IV + k, little-endian.
  // Ranges where the upstream C (128-bit carry) and AES-NI (low 64-bit) counters
  // disagree throw AES_COUNTER_AMBIGUOUS. Key and IV are raw bytes (after
  // buggy_decode16/decode16). Every chunk but the last is a multiple of 16 bytes.
  aesCtr(key, iv, startBlock = 0n) {
    if (!AES_KEY_BYTES.includes(key.length)) fail('INVALID_ARGUMENT', `AES key must be ${AES_KEY_BYTES.join('/')} bytes`);
    if (iv.length !== AES_IV_BYTES) fail('INVALID_ARGUMENT', `AES IV must be ${AES_IV_BYTES} bytes`);
    if (typeof startBlock !== 'bigint' || startBlock < 0n || startBlock >= 1n << 64n) fail('INVALID_ARGUMENT', 'startBlock must be a u64 bigint');
    const ex = this.#ex;
    const ctx = this.#call(() => {
      const tmp = this.#alloc(key.length + iv.length + 4);
      const outPtr = tmp + key.length + iv.length;
      try {
        this.#bytes().set(key, tmp);
        this.#bytes().set(iv, tmp + key.length);
        const status = ex.fa_aes_ctr_new(tmp, key.length, tmp + key.length, startBlock, outPtr);
        if (status < 0) throw statusError(status, 'aes-ctr');
        return new DataView(ex.memory.buffer).getUint32(outPtr, true);
      } finally {
        this.#bytes().fill(0, tmp, tmp + key.length);
        ex.fa_free(tmp);
      }
    });
    let scratch = 0, scratchLen = 0, closed = false;
    return {
      transform: chunk => {
        if (closed) fail('CLOSED', 'AES context closed');
        return this.#call(() => {
          if (chunk.length > scratchLen) {
            ex.fa_free(scratch);
            scratch = 0, scratchLen = 0;
            scratch = this.#alloc(chunk.length);
            scratchLen = chunk.length;
          }
          this.#bytes().set(chunk, scratch);
          const status = ex.fa_aes_ctr_xor(ctx, scratch, chunk.length);
          if (status === -1) fail('AES_PARTIAL_BLOCK', 'only the last AES-CTR chunk may end inside a 16-byte block');
          if (status < 0) throw statusError(status, 'aes-ctr');
          return this.#bytes().slice(scratch, scratch + chunk.length);
        });
      },
      close: () => {
        if (closed) return;
        closed = true;
        ex.fa_free(scratch);
        ex.fa_aes_ctr_free(ctx);
      },
    };
  }

  // Decodes one `rep+[exe+]4x4:tor` stream with AES already removed. `read(maxBytes)`
  // resolves to up to maxBytes or null/empty at end of input;
  // the codec never reads past what it needs. `onWrite(chunk)` receives a view
  // into codec memory that stays valid until its promise settles. Only one
  // stream runs per instance at a time.
  async decodeStream({ method, read, onWrite, expectedSize, maxOutputBytes = expectedSize, maxDictionaryBytes, maxMemoryBytes,
    maxInputBytes, maxIntermediateBytes, signal }) {
    const lim = this.methodLimits(method);
    uint(maxOutputBytes, 'maxOutputBytes');
    if (expectedSize !== undefined) uint(expectedSize, 'expectedSize');
    if (maxDictionaryBytes !== undefined && lim.repDictionary > uint(maxDictionaryBytes, 'maxDictionaryBytes'))
      fail('STREAM_LIMIT', `rep dictionary ${lim.repDictionary} exceeds maxDictionaryBytes ${maxDictionaryBytes}`);
    if (lim.exe) {
      uint(maxInputBytes, 'maxInputBytes');
      uint(maxIntermediateBytes, 'maxIntermediateBytes');
    }
    const ex = this.#ex, inputCap = ex.fa_stream_input_cap();
    maxMemoryBytes ??= Number((lim.exe ? ex.fa_stream_exe_default_budget : ex.fa_stream_default_budget)(lim.repDictionary, lim.blockSize, lim.torBuffer));
    uint(maxMemoryBytes, 'maxMemoryBytes');
    if (this.#streaming) fail('BUSY', 'a stream is already running on this codec instance');
    this.#streaming = true;
    let s = 0, inputBytes = 0;
    try {
      s = this.#call(() => lim.exe
        ? ex.fa_stream_new_exe(BigInt(maxOutputBytes), lim.repDictionary, lim.blockSize, lim.torBuffer, BigInt(maxMemoryBytes),
          BigInt(maxIntermediateBytes), BigInt(maxInputBytes))
        : ex.fa_stream_new(BigInt(maxOutputBytes), lim.repDictionary, lim.blockSize, lim.torBuffer, BigInt(maxMemoryBytes)));
      if (!s) fail('MEMORY_LIMIT', 'cannot allocate stream');
      for (;;) {
        signal?.throwIfAborted();
        const r = this.#call(() => ex.fa_stream_step(s));
        if (r === STEP_DONE) break;
        if (r === STEP_OUTPUT) {
          const ptr = ex.fa_stream_output_ptr(s), len = ex.fa_stream_output_len(s);
          await onWrite(this.#bytes().subarray(ptr, ptr + len));
        } else if (r === STEP_NEED_INPUT) {
          // One sentinel byte can distinguish exact-cap EOF from excess input;
          // it is refused before copying into WASM. Empty reads always mean EOF.
          const request = lim.exe ? Math.min(inputCap, maxInputBytes - inputBytes + 1) : inputCap;
          const chunk = (await read(request)) ?? new Uint8Array(0);
          if (chunk.length > request) fail('READ', `read returned ${chunk.length} > ${request} bytes`);
          if (lim.exe && chunk.length > maxInputBytes - inputBytes) fail('STREAM_LIMIT', 'EXE input exceeds maxInputBytes');
          if (lim.exe) inputBytes += chunk.length;
          this.#call(() => {
            this.#bytes().set(chunk, ex.fa_stream_input_ptr(s));
            const st = ex.fa_stream_input(s, chunk.length);
            if (st < 0) throw statusError(st, 'stream input');
          });
        } else {
          throw statusError(r, `stream at output ${ex.fa_stream_total(s)}`);
        }
      }
      const outputBytes = Number(ex.fa_stream_total(s));
      if (expectedSize !== undefined && outputBytes !== expectedSize)
        fail('SIZE_MISMATCH', `decoded ${outputBytes} bytes, expected ${expectedSize}`);
      return { outputBytes, peakAllocBytes: Number(ex.fa_stream_peak_alloc(s)) };
    } finally {
      if (s && !this.#poisoned) ex.fa_stream_free(s);
      this.#streaming = false;
    }
  }
}
