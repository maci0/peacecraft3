// Actual wasm32 production EXE-stage/reference proof; synthetic bytes only.
import { expect, test } from 'bun:test';
const build = process.env.FREEARC_CODEC_BUILD;
if (!build) throw new Error('FREEARC_CODEC_BUILD is required');
test('77 production-stage partitions, EOF rule and UInt32 wrap on wasm32', async () => {
  const module = await WebAssembly.compile(await Bun.file(`${build}/exe_stage_test.wasm`).arrayBuffer());
  const unexpected = () => { throw new Error('unexpected runtime I/O in synthetic EXE test'); };
  const { exports } = await WebAssembly.instantiate(module, {
    env: { emscripten_notify_memory_growth() {}, _emscripten_throw_longjmp: unexpected },
    wasi_snapshot_preview1: { proc_exit: unexpected, fd_write: unexpected, fd_close: unexpected, fd_seek: unexpected },
  });
  exports._initialize();
  expect(exports.fa_test_pointer_bits()).toBe(32);
  expect(exports.fa_exe_stage_tests()).toBe(0);
}, 60000);
