// Synthetic production-stage/reference equivalence, native and wasm32.
// GPL-2.0; see ../upstream/LICENSE and ../upstream-exe.json.
#include "../src/codec.cpp"
#include "Compression/LZMA2/exe_reference.inc"

static_assert(sizeof(UInt32) == 4, "BCJ requires wrapping UInt32 addresses");
static const uint32_t CAP = LARGE_BUFFER_SIZE * 2 + 37;
static uint8_t original[CAP], encoded[CAP], decoded[CAP], framed[CAP * 9 + 4];
struct Reference { const uint8_t *input; uint32_t length, pos, written; };
static int reference_io(const char *what, void *buf, int size, void *aux)
{
  Reference &r = *(Reference *) aux;
  if (!strcmp(what, "read")) {
    uint32_t n = min32((uint32_t) size, r.length - r.pos);
    memcpy(buf, r.input + r.pos, n); r.pos += n; return (int) n;
  }
  if (!strcmp(what, "write")) {
    if ((uint32_t) size > CAP - r.written) return FREEARC_ERRCODE_WRITE;
    memcpy(encoded + r.written, buf, size); r.written += size; return size;
  }
  return FREEARC_ERRCODE_NOT_IMPLEMENTED;
}
static void put32(uint32_t &pos, uint32_t v)
{
  for (uint32_t k = 0; k < 4; k++) framed[pos++] = (uint8_t) (v >> (k * 8));
}
static int stage_case(uint32_t length, uint32_t chunk, uint32_t block)
{
  Reference ref = { original, length, 0, 0 };
  if (bcj_x86_de_compress(1, reference_io, &ref) || ref.written != length) return 1;
  uint32_t framed_len = 0;
  put32(framed_len, FA_4X4_VERSION);
  for (uint32_t pos = 0; pos < length;) {
    uint32_t n = min32(length - pos, block);
    put32(framed_len, FA_4X4_STORED); put32(framed_len, n);
    memcpy(framed + framed_len, encoded + pos, n); framed_len += n; pos += n;
  }
  Stream *s = fa_stream_new_exe(0, 1, block, 32 * kb,
    fa_stream_exe_default_budget(1, block, 32 * kb), length, framed_len);
  if (!s) return 2;
  uint32_t pos = 0, out = 0; int status;
  while ((status = exe_fill(*s)) == FILL_READY || status == FILL_NEED_INPUT) {
    if (status == FILL_READY) {
      uint32_t n = s->e_len - s->e_pos;
      if (n > CAP - out) { status = -1; break; }
      memcpy(decoded + out, s->e_buf + s->e_pos, n); out += n; s->e_pos += n;
    } else {
      uint32_t n = min32(min32(chunk, FA_INPUT_CHUNK), framed_len - pos);
      memcpy(fa_stream_input_ptr(s), framed + pos, n); pos += n;
      if (fa_stream_input(s, n)) { status = -2; break; }
    }
  }
  bool correct = status == FILL_EOF && out == length && !memcmp(original, decoded, length)
                 && s->intermediate == length && s->input_total == framed_len;
  fa_stream_free(s);
  return correct && g_used == 0 ? 0 : 3;
}

FA_EXPORT int32_t fa_exe_stage_tests(void)
{
  uint32_t rng = 0x719ad25u;
  for (uint32_t i = 0; i < CAP; ++i) { rng = rng * 1664525u + 1013904223u; original[i] = (uint8_t) (rng >> 24); }
  for (uint32_t i = 0; i + 5 < CAP; i += 13) { original[i] = i & 1 ? 0xe8 : 0xe9; original[i+4] = i & 2 ? 0xff : 0; }
  const uint32_t lengths[] = {0,1,4,5,6,9,17,LARGE_BUFFER_SIZE-4,LARGE_BUFFER_SIZE,LARGE_BUFFER_SIZE+5,CAP};
  const uint32_t chunks[] = {1,4,5,6,97,LARGE_BUFFER_SIZE-3,CAP};
  for (uint32_t n = 0; n < elements(lengths); n++) for (uint32_t k = 0; k < elements(chunks); k++) {
    int r = stage_case(lengths[n], chunks[k], chunks[(k+2)%elements(chunks)]);
    if (r) return (int32_t) (100 + (n * elements(chunks) + k) * 10 + r);
  }
  // The source wrapper copies a complete five-byte branch unchanged at EOF.
  memset(original, 0, 6); original[0] = 0xe8;
  if (stage_case(5, 1, 1) || memcmp(original, encoded, 5)) return 900;
  if (stage_case(6, 1, 1) || !memcmp(original, encoded, 5)) return 901;
  // Position wrap must not affect the independent wide work counters.
  Stream *s = fa_stream_new_exe(0, 1, 1, 32 * kb,
    fa_stream_exe_default_budget(1, 1, 32 * kb), 6, 0);
  if (!s) return 902;
  s->e_ip = UINT32_MAX - 2; s->e_have = 6; s->x_eof = true;
  memcpy(s->e_buf, original, 6);
  bool wrap = exe_fill(*s) == FILL_READY;
  s->e_pos = s->e_len;
  wrap = wrap && exe_fill(*s) == FILL_READY;
  s->e_pos = s->e_len;
  wrap = wrap && exe_fill(*s) == FILL_EOF && s->e_ip == 3;
  fa_stream_free(s);
  if (!wrap || g_used) return 903;
  // Announced intermediate limit is checked before allocating either x4 block.
  s = fa_stream_new_exe(0, 1, 16, 32 * kb, FA_INPUT_CHUNK + LARGE_BUFFER_SIZE, 0, 12);
  if (!s) return 904;
  uint32_t pos = 0; put32(pos, 0); put32(pos, 1); put32(pos, 0);
  memcpy(fa_stream_input_ptr(s), framed, pos);
  bool capped = fa_stream_input(s, pos) == 0 && fa_stream_step(s) == FA_ERR_LIMIT
    && fa_stream_step(s) == FA_ERR_LIMIT && fa_stream_input(s, 0) == FA_ERR_LIMIT
    && fa_stream_peak_alloc(s) == FA_INPUT_CHUNK + LARGE_BUFFER_SIZE;
  fa_stream_free(s);
  return capped && g_used == 0 ? 0 : 905;
}

FA_EXPORT int32_t fa_test_pointer_bits(void) { return sizeof(void *) * 8; }
#ifndef __EMSCRIPTEN__
int main()
{
  int r = fa_exe_stage_tests();
  printf("EXE production-stage partitions=77 result=%d\n", r);
  return r ? 1 : 0;
}
#endif
