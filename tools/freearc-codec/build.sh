#!/usr/bin/env bash
# Builds the FreeArc codec: wasm (browser) and a native test runner.
# usage: build.sh [wasm|native|all|test] [out-dir (default: build/ beside this script)]
# Needs only this directory: emcc for wasm, clang with ASan/UBSan for native.
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
what=${1:-all}
out=${2:-$here/build}
mkdir -p "$out"

# Refuse to build anything but the pinned upstream files (no local patches).
(cd "$here/upstream" && sha256sum --quiet --strict -c ../upstream.sha256)

defs=(-DFREEARC_UNIX -DFREEARC_INTEL_BYTE_ORDER)
cxx_flags=(-std=gnu++11 -O2 -Wall -Wextra -Werror -fno-exceptions -fno-rtti -isystem "$here/upstream" "${defs[@]}")
c_flags=(-std=gnu99 -O2 -Wall -Werror -isystem "$here/upstream")

if [[ $what == wasm || $what == all ]]; then
  emcc "${cxx_flags[@]}" -DFREEARC_DECOMPRESS_ONLY -c "$here/src/codec.cpp" -o "$out/codec.wasm.o"
  emcc "${c_flags[@]}" -c "$here/src/lzma_dec.c" -o "$out/lzma_dec.wasm.o"
  emcc "${c_flags[@]}" -c "$here/src/aes.c" -o "$out/aes.wasm.o"
  emcc -O2 "$out/codec.wasm.o" "$out/lzma_dec.wasm.o" "$out/aes.wasm.o" -o "$out/freearc-codec.wasm" \
    --no-entry -sSTANDALONE_WASM -sALLOW_MEMORY_GROWTH -sMAXIMUM_MEMORY=1gb -sSTACK_SIZE=1mb \
    -sABORTING_MALLOC=0 -sFILESYSTEM=0
  cp "$here/js/freearc-codec.mjs" "$out/"
fi

if [[ $what == native || $what == all ]]; then
  # Test build keeps the upstream Tornado compressor for synthetic fixtures.
  clang++ "${cxx_flags[@]}" -g -fsanitize=address,undefined -fno-sanitize=alignment -c "$here/tests/native_test.cpp" -I"$here/src" -o "$out/native_test.o"
  clang "${c_flags[@]}" -g -fsanitize=address,undefined -c "$here/src/lzma_dec.c" -o "$out/lzma_dec.o"
  clang "${c_flags[@]}" -g -fsanitize=address,undefined -c "$here/src/aes.c" -o "$out/aes.o"
  clang++ -fsanitize=address,undefined "$out/native_test.o" "$out/lzma_dec.o" "$out/aes.o" -lm -o "$out/native_test"
  clang++ "${cxx_flags[@]}" -DFREEARC_DECOMPRESS_ONLY -g -fsanitize=address,undefined -fno-sanitize=alignment \
    -c "$here/tests/exe_stage_test.cpp" -o "$out/exe_stage_test.o"
  clang++ -fsanitize=address,undefined "$out/exe_stage_test.o" "$out/lzma_dec.o" "$out/aes.o" -lm -o "$out/exe_stage_test"
fi

if [[ $what == test ]]; then
  "$0" all "$out"
  # Same production EXE stage/reference test on the actual 32-bit WASM target.
  emcc "${cxx_flags[@]}" -DFREEARC_DECOMPRESS_ONLY -c "$here/tests/exe_stage_test.cpp" -o "$out/exe_stage_test.wasm.o"
  emcc -O2 "$out/exe_stage_test.wasm.o" "$out/lzma_dec.wasm.o" "$out/aes.wasm.o" -o "$out/exe_stage_test.wasm" \
    --no-entry -sSTANDALONE_WASM -sALLOW_MEMORY_GROWTH -sMAXIMUM_MEMORY=1gb -sSTACK_SIZE=1mb \
    -sABORTING_MALLOC=0 -sFILESYSTEM=0
  mkdir -p "$out/fixtures"
  "$out/native_test" "$out/fixtures"
  "$out/exe_stage_test"
  FREEARC_CODEC_BUILD=$out bun test "$here/tests/codec.test.mjs" "$here/tests/exe-stage.test.mjs"
fi
