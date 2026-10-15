# freearc-codec

Browser WebAssembly decoders for the FreeArc 0.67 archives inside the supplied
installer ISO: LZMA control blocks, AES-CTR, and `rep:...+[exe+]4x4:tor:...` data
blocks. It decodes a 1.2 GB block in a dedicated worker without
SharedArrayBuffer, threads or cross-origin isolation, and never holds the
decoded block in memory.

This directory is a separate GPL-2.0 component (see [License](#license)). The
application loads the built `freearc-codec.wasm` and `js/freearc-codec.mjs` at
runtime; no Rust crate links them.

## Build and test

```sh
tools/freearc-codec/build.sh wasm   # build/freearc-codec.wasm + build/freearc-codec.mjs
tools/freearc-codec/build.sh test   # wasm, native ASan/UBSan tests, then bun tests
```

`build.sh [wasm|native|all|test] [out-dir]` needs only this directory. It
writes to `build/` here by default and refuses to build if any file under
`upstream/` differs from `upstream.sha256`. Checked with emcc 6.0.12-git
(5488e087), clang 23.1.1 and bun 1.4.2. `test` also needs `xz`, used to
encode the LZMA test streams.
`test` also runs the same production EXE-stage/reference proof natively and on
wasm32 (77 synthetic length/chunk partitions), plus full-chain WASM lifetime,
limits, cancellation and reuse cases. Test outputs belong in a private output
directory. When the global SDK cache is read-only, set `EM_CACHE` to a private
copy/cache; the installed SDK's `embuilder.py build libstandalonewasm-nocatch`
can build that runtime offline from installed sources. Do not change the global
cache's permissions or fetch a toolchain to run these checks.

## API

```js
import { loadFreeArcCodec } from './freearc-codec.mjs';
const codec = await loadFreeArcCodec(wasmModuleOrBytesOrResponse);

const bytes = await codec.decodeLzma(raw, { properties, dictSize, maxOutputBytes, expectedSize, signal });

const ctr = codec.aesCtr(key, iv, startBlock /* bigint, default 0n */);
const plain = ctr.transform(chunk);   // every chunk but the last: multiple of 16 bytes
ctr.close();

const { outputBytes, peakAllocBytes } = await codec.decodeStream({
  method,            // chain without AES, e.g. 'rep:96mb:256+4x4:tor:3:8mb'
  read,              // async maxBytes => Uint8Array | null (end of input)
  onWrite,           // async chunk => void; chunk is a view, valid until settled
  expectedSize, maxOutputBytes, maxDictionaryBytes, maxMemoryBytes, signal,
  maxInputBytes, maxIntermediateBytes, // both REQUIRED only for an EXE chain
});
```

- `decodeLzma` allocates `min(expectedSize ?? maxOutputBytes, maxOutputBytes)`
  once and decodes into it. Without `expectedSize` it stops at the end mark,
  or at end of input when LzmaDec reports a clean finish (untested here: `xz`
  always writes an end mark). It throws on output past the ceiling, truncation
  or corrupt data. `properties` is the LZMA props byte.
- `aesCtr` takes raw key and IV bytes, so the caller applies `buggy_decode16`
  or `decode16` first. Pinned FreeArc encrypts r0 AES-CTR with one of two
  counters, chosen by the encoding machine's CPU: `7zAES/Aes.c AesCtr_Code`
  carries across all 128 bits, while `AesOpt.asm AesCtr_Code_Intel` (`paddq`)
  increments only the low 64. The codec accepts only IVs and ranges where both
  agree: the IV's low 64 bits are nonzero and do not wrap. Anything else throws
  `AES_COUNTER_AMBIGUOUS`.
- `decodeStream` reads limits from the authored method string: the REP
  dictionary, the 4x4 block (the Tornado buffer unless `4x4:b<size>`), and
  the Tornado header buffer. Defaults come from upstream: `REP_METHOD()`
  64 MB and the compiled `std_Tornado_method` table. `maxMemoryBytes`
  defaults to the sum of the stream's allocations for those limits
  (`fa_stream_default_budget`). Only one stream runs per instance.
- The only optional filter placement is bare `exe` between REP and 4x4, e.g.
  `rep:<...>+exe+4x4:tor:<...>`. Parameters, repetition, other positions and
  other filters refuse. Existing non-EXE calls/result shapes remain unchanged.
  EXE calls require explicit safe-integer `maxInputBytes` (decrypted 4x4 input)
  and `maxIntermediateBytes` (sum of announced decoded 4x4 bytes, the encoded REP
  stream). The latter is checked before block allocation/payload decoding;
  it cannot truthfully default to REP's final `expectedSize`. The input cap
  independently bounds framing input, including empty stored blocks, and is
  checked before copying into WASM. The reader may receive a smaller request,
  including one excess sentinel byte to distinguish exact-cap EOF; that sentinel
  is refused without copying. Null **or empty** read means EOF, never retry.
  The normal maximum read request remains the fixed codec input chunk.
  `maxMemoryBytes` defaults to the old counted-buffer budget plus the source EXE
  window (`fa_stream_exe_default_budget`). Both cumulative counters are wide and
  separate from BCJ's wrapping UInt32 instruction position. These bounds describe
  actual byte/framing/filter work; they are not a whole-heap or total decoder
  instruction bound. The C ABI preserves `fa_stream_new` and adds the explicit
  `fa_stream_new_exe(..., max_alloc, max_intermediate, max_input)` constructor.
- Errors are `FreeArcCodecError` with a `code`. A status error leaves the
  instance usable. A trap, or an upstream `exit()` (`CODEC_ABORT`, e.g.
  Tornado's range decoder on corrupt data), poisons it, and every later call
  throws `POISONED`. Load a fresh instance for each retry or session.

Decoded bytes are not authenticated; callers check member CRCs.

## Design

`src/codec.cpp` compiles upstream Tornado (decode only), LzmaDec, x86 BCJ and the 7-Zip
AES code unmodified. The 4x4 framing (`C_4x4.cpp`) and REP decoding
(`rep.cpp rep_decompress`) are reimplemented as one resumable state machine.
Upstream's versions are callback-driven and multithreaded, and
`rep_decompress` never checks its tables against the block. Each
`fa_stream_step` either asks for input, yields one output chunk (at a REP
window wrap or block end), or finishes, so JavaScript can await I/O between
steps without Asyncify. Codec working buffers go through upstream's
`MyAlloc`/`MyFree` hooks and count against the stream budget. This excludes the
small stream object, allocator overhead and host/JS memory. Tornado's
header buffer is checked against the method limit before Tornado allocates
it.

The optional EXE stage follows pinned `C_BCJ.cpp`: coalesce full
`LARGE_BUFFER_SIZE` reads from `Compression.h` or stop at EOF, preserve IP/mask
state and the unconsumed suffix, and leave the final buffer of at most five bytes
unchanged. It never filters arbitrary network/4x4 chunks independently or resets
at their boundaries. Converted prefix bytes stay owned until REP consumes them,
including across JavaScript output awaits. This is a length-preserving byte
filter with no header/checksum or executable validation of its own. Surrounding
framing, output-size and member CRC checks remain necessary.

## Corresponding source

The complete source of the built artifacts is this directory: `build.sh`,
`upstream.sha256`, `.gitignore`, `src/` (`codec.cpp`, `lzma_dec.c`, `aes.c`),
`js/freearc-codec.mjs`, all `tests/` sources, `upstream-exe.json`, and
`upstream/` with `LICENSE` and the pinned files listed in `upstream.sha256`.
`upstream/` holds unmodified files from
[FreeArc 0.67 at 537e4ff9](https://github.com/mrkaban/FreeArc-0.67/tree/537e4ff921e55da60b27bdbae0bc757010be7e53);
there are no local patches. `OptimalParsing.cpp` is used only by the native
test build, which keeps the Tornado compressor to make fixtures. No original
game assets or fixtures derived from them are part of this component.
`upstream-exe.json` records the exact upstream wrapper-source digest and the
unchanged callback-loop slice used by reference tests. The full production-stage
synthetic proof establishes no installer-block, PE/runtime recovery or browser
import compatibility; those require separate explicit original-input gates.

## License

FreeArc, Tornado, REP and 4x4 are by Bulat Ziganshin. The repository ships
`LICENSE` as the GNU GPL version 2 text, and the Tornado sources state "This
code is provided on the GPL license". This component, including `src/`, `js/`
and the built wasm, is therefore distributed under GPL-2.0. The bundled
LzmaDec, x86 branch converter and 7-Zip AES (Igor Pavlov) files are public domain, per
their headers.

The rest of the project is `MIT OR Apache-2.0`. The FSF lists the MIT (Expat)
license as GPLv2-compatible and Apache-2.0 as incompatible with GPLv2. Any work
combined with this codec can therefore use only the MIT option. Distributing
the wasm requires distributing the corresponding source above with it.
