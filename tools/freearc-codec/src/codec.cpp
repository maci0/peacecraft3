// FreeArc 0.67 decoders for browser import: LZMA control blocks, AES-256-CTR, and
// the `rep:...+4x4:tor:...` data-block chain. Tornado, LzmaDec and the 7-Zip AES
// code are compiled unmodified from upstream/; the 4x4 framing loop and the REP
// decoder are re-implemented from upstream C_4x4.cpp and rep.cpp with bounds
// checks, because rep_decompress trusts its tables and both are callback-driven.
//
// Copyright (C) Bulat Ziganshin (FreeArc, Tornado, REP, 4x4 formats).
// Licensed under the GNU General Public License version 2; see ../upstream/LICENSE.

#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include "Compression/Tornado/Tornado.cpp"

extern "C" {
#include "Compression/LZMA2/C/LzmaDec.h"
#include "Compression/_Encryption/7zAES/Aes.h"
}

// Status codes beyond upstream FREEARC_ERRCODE_* (Common.h).
#define FA_ERR_LIMIT     (-100)  // stream declares a size beyond a caller limit
#define FA_ERR_TRUNCATED (-101)  // input ended inside a structure
#define FA_ERR_TRAILING  (-102)  // bytes follow the end of a stream
#define FA_ERR_AES_COUNTER (-103)  // AES-CTR range where the upstream counters disagree

#ifdef __EMSCRIPTEN__
#define FA_EXPORT extern "C" __attribute__((used, visibility("default")))
#else
#define FA_EXPORT extern "C"
#endif

// Input is pushed in chunks of at most this size, a multiple of the AES block so a
// host decrypting each chunk keeps CTR alignment.
static const uint32_t FA_INPUT_CHUNK = 1u << 20;
static const uint32_t FA_AES_BLOCK = 16;
static const uint32_t FA_ALLOC_ALIGN = 16;

// 4x4 (C_4x4.cpp): stream version, and the OutSize marking a stored block.
static const uint32_t FA_4X4_VERSION = 0;
static const uint32_t FA_4X4_STORED = 0xFFFFFFFFu;
// Tornado stream header (Tornado.cpp tor_decompress): method, minlen, u32 buffer.
static const uint32_t FA_TOR_HEADER = 6;
// REP (rep.cpp rep_decompress): initial block buffer is min(dict, MAX_BLOCK)+1024.
static const uint32_t FA_REP_MAX_BLOCK = 8u << 20;
static const uint32_t FA_REP_BUF_SLACK = 1024;

/* ---------------- allocation budget (upstream MyAlloc/MyFree hooks) ---------------- */

static uint64_t g_budget = UINT64_MAX, g_used = 0, g_peak = 0;

void *MyAlloc(size_t size) throw()
{
  if (size == 0 || size > SIZE_MAX - FA_ALLOC_ALIGN || size > g_budget - g_used) return NULL;
  uint8_t *p = (uint8_t *) malloc(size + FA_ALLOC_ALIGN);
  if (!p) return NULL;
  memcpy(p, &size, sizeof size);
  g_used += size;
  if (g_used > g_peak) g_peak = g_used;
  return p + FA_ALLOC_ALIGN;
}

void MyFree(void *address) throw()
{
  if (!address) return;
  uint8_t *p = (uint8_t *) address - FA_ALLOC_ALIGN;
  size_t size;
  memcpy(&size, p, sizeof size);
  g_used -= size;
  free(p);
}

// 4x4 decompression runs Tornado with compress_all_at_once set (C_4x4.cpp
// Set_compress_all_at_once_Until_end_of_block). CHECK() never longjmps here.
int compress_all_at_once = 1;
int jmpready = FALSE;
jmp_buf jumper;

/* ---------------- plain heap for JS-owned buffers ---------------- */

FA_EXPORT void *fa_malloc(uint32_t size) { return malloc(size ? size : 1); }
FA_EXPORT void fa_free(void *p) { free(p); }

/* ---------------- LZMA ---------------- */

// LZMA probabilities use the plain heap so control blocks can be decoded while a
// data stream holds the MyAlloc budget.
static void *lzma_alloc(void *, size_t size) { return malloc(size); }
static void lzma_free(void *, void *p) { free(p); }
static ISzAlloc g_lzma_alloc = { lzma_alloc, lzma_free };

// Decodes a raw LZMA1 stream (props byte + dictionary size given separately) into
// out[0..out_cap), which also serves as the dictionary. With has_expected the
// output must be exactly out_cap bytes; otherwise decoding stops at the end mark,
// or at end of input when the range coder reports a clean finish.
FA_EXPORT int32_t fa_lzma_decode(const uint8_t *in, uint32_t in_len, uint32_t props, uint32_t dict_size,
                                 uint8_t *out, uint32_t out_cap, int32_t has_expected, uint32_t *out_len)
{
  *out_len = 0;
  if (props >= 9 * 5 * 5) return FREEARC_ERRCODE_INVALID_COMPRESSOR;
  uint8_t header[LZMA_PROPS_SIZE] = { (uint8_t) props, (uint8_t) dict_size, (uint8_t) (dict_size >> 8),
                                      (uint8_t) (dict_size >> 16), (uint8_t) (dict_size >> 24) };
  CLzmaDec dec;
  LzmaDec_Construct(&dec);
  if (LzmaDec_AllocateProbs(&dec, header, LZMA_PROPS_SIZE, &g_lzma_alloc) != SZ_OK)
    return FREEARC_ERRCODE_NOT_ENOUGH_MEMORY;
  dec.dic = out;
  dec.dicBufSize = out_cap;
  LzmaDec_Init(&dec);

  SizeT consumed = in_len;
  ELzmaStatus status;
  SRes res = LzmaDec_DecodeToDic(&dec, out_cap, in, &consumed,
                                 has_expected ? LZMA_FINISH_ANY : LZMA_FINISH_END, &status);
  SizeT produced = dec.dicPos;
  LzmaDec_FreeProbs(&dec, &g_lzma_alloc);
  *out_len = (uint32_t) produced;

  if (has_expected) {
    if (res != SZ_OK) return FREEARC_ERRCODE_BAD_COMPRESSED_DATA;
    return produced == out_cap ? FREEARC_OK : FA_ERR_TRUNCATED;
  }
  if (res == SZ_OK && status == LZMA_STATUS_FINISHED_WITH_MARK) return FREEARC_OK;
  if (res == SZ_OK && status == LZMA_STATUS_MAYBE_FINISHED_WITHOUT_MARK && consumed == in_len) return FREEARC_OK;
  if (produced == out_cap) return FREEARC_ERRCODE_OUTBLOCK_TOO_SMALL;
  if (res == SZ_OK && status == LZMA_STATUS_NEEDS_MORE_INPUT) return FA_ERR_TRUNCATED;
  return FREEARC_ERRCODE_BAD_COMPRESSED_DATA;
}

/* ---------------- AES-CTR (C_Encryption.cpp EncryptionMode, 7-Zip AesCtr_Code) ---------------- */

// FreeArc's r0 AES-CTR uses one of two upstream counters, chosen by the encoding
// machine's CPU: 7zAES/Aes.c AesCtr_Code carries across all 128 bits, while
// AesOpt.asm AesCtr_Code_Intel (paddq) increments only the low 64. They agree only
// while the low 64 bits of IV + block do not wrap and the IV's low 64 bits are
// nonzero (upstream's IV-1 would borrow into the high half). Other ranges are
// refused, since which one encrypted the archive is unknown.

struct AesCtr
{
  UInt32 ivAes[AES_NUM_IVMRK_WORDS];  // counter (IV-1+start, pre-incremented) + key schedule
  uint64_t blocks_left;               // blocks before the low 64 counter bits wrap
  int32_t finished;                   // a partial block was processed; no further input allowed
};

FA_EXPORT int32_t fa_aes_ctr_new(const uint8_t *key, uint32_t key_len, const uint8_t *iv, uint64_t start_block,
                                 AesCtr **out)
{
  static bool tables = false;
  if (!tables) AesGenTables(), tables = true;
  *out = NULL;
  if (key_len != 16 && key_len != 24 && key_len != 32) return FREEARC_ERRCODE_INVALID_COMPRESSOR;
  uint64_t low = 0;
  for (uint32_t i = 0; i < 8; i++) low |= (uint64_t) iv[i] << (8 * i);
  uint64_t first = low + start_block;
  if (low == 0 || first < low) return FA_ERR_AES_COUNTER;
  AesCtr *c = (AesCtr *) aligned_alloc(FA_AES_BLOCK, sizeof(AesCtr));
  if (!c) return FREEARC_ERRCODE_NOT_ENOUGH_MEMORY;
  c->finished = 0;
  c->blocks_left = 0 - first;  // 2^64 - first, nonzero since first >= 1
  // Upstream stores IV-1 because AesCtr_Code increments before encrypting; the
  // first block uses the IV. Within the accepted range the add stays in the low half.
  uint64_t ctr_low = first - 1;
  for (uint32_t w = 0; w < 2; w++) c->ivAes[w] = (UInt32) (ctr_low >> (32 * w));
  for (uint32_t w = 2; w < 4; w++)
    c->ivAes[w] = (UInt32) iv[w * 4] | (UInt32) iv[w * 4 + 1] << 8 | (UInt32) iv[w * 4 + 2] << 16 | (UInt32) iv[w * 4 + 3] << 24;
  Aes_SetKey_Enc(c->ivAes + 4, key, key_len);
  *out = c;
  return FREEARC_OK;
}

// XORs the keystream into buf. A length that is not a multiple of 16 ends the stream.
FA_EXPORT int32_t fa_aes_ctr_xor(AesCtr *c, uint8_t *buf, uint32_t len)
{
  if (c->finished) return FREEARC_ERRCODE_GENERAL;
  uint32_t whole = len / FA_AES_BLOCK, tail = len % FA_AES_BLOCK;
  if (whole + (tail ? 1 : 0) > c->blocks_left) return FA_ERR_AES_COUNTER;
  c->blocks_left -= whole + (tail ? 1 : 0);
  g_AesCtr_Code(c->ivAes, buf, whole);
  if (tail) {
    // docrypt rounds the last read up to a whole block; only the prefix is output.
    uint8_t block[FA_AES_BLOCK] = { 0 };
    memcpy(block, buf + len - tail, tail);
    g_AesCtr_Code(c->ivAes, block, 1);
    memcpy(buf + len - tail, block, tail);
    c->finished = 1;
  }
  return FREEARC_OK;
}

// Clears the key schedule before release.
FA_EXPORT void fa_aes_ctr_free(AesCtr *c)
{
  if (c) explicit_bzero(c, sizeof *c);
  free(c);
}

/* ---------------- rep + 4x4:tor data block ---------------- */

// Tornado presets (Tornado.cpp std_Tornado_method), for method-string limits.
FA_EXPORT int32_t fa_tor_preset_count(void) { return elements(std_Tornado_method); }
FA_EXPORT uint32_t fa_tor_preset_buffer(int32_t n) { return std_Tornado_method[n].buffer; }
FA_EXPORT int32_t fa_tor_default_preset(void) { return default_Tornado_method; }

// fa_stream_step results; negative values are errors.
#define FA_STEP_NEED_INPUT 1  // push more input (or end of input) with fa_stream_input
#define FA_STEP_OUTPUT     2  // fa_stream_output_ptr/len hold the next output chunk
#define FA_STEP_DONE       3  // the stream ended; no input or output remains

enum X4Phase { X4_VERSION, X4_HEADER, X4_PAYLOAD };
enum RepPhase { REP_DICT, REP_SIZE, REP_BODY, REP_DECODE, REP_TRAILER, REP_DONE };
enum { FILL_READY, FILL_NEED_INPUT, FILL_EOF };

struct Stream
{
  uint64_t max_output, max_alloc;
  uint32_t max_dict, max_block, max_tor_buffer;
  int32_t error;  // sticky

  // Pushed input: inbuf[in_pos, in_len).
  uint8_t *inbuf;
  uint32_t in_pos, in_len;
  bool in_eof;

  // 4x4 framing (C_4x4.cpp ReaderThread/Process): output is x_out[x_pos, x_len).
  X4Phase x_phase;
  uint8_t x_hdr[8];
  uint32_t x_hdr_have, x_in_size, x_out_size, x_in_have;
  uint8_t *x_in, *x_out;
  uint32_t x_pos, x_len;
  bool x_eof;

  // REP (rep.cpp rep_decompress): block buffer, then decode state.
  RepPhase r_phase;
  uint8_t r_hdr[4];
  uint32_t r_hdr_have, r_compr, r_have, r_bufsize;
  uint8_t *r_buf;
  const uint8_t *lens, *offsets, *datalens, *lit;
  uint32_t num, item, lit_left, remaining, from, offset;
  bool in_match;

  // Window ring; [flushed, pos) is decoded but not yet output.
  uint8_t *ring;
  uint32_t size, pos, flushed;
  uint64_t total;
  bool pending;
};

static uint32_t le32(const uint8_t *p) { return p[0] | p[1] << 8 | p[2] << 16 | (uint32_t) p[3] << 24; }
static uint32_t min32(uint32_t a, uint32_t b) { return a < b ? a : b; }

// Copies pushed input into dst until have == n. True when complete.
static bool take_input(Stream &s, uint8_t *dst, uint32_t n, uint32_t &have)
{
  uint32_t k = min32(n - have, s.in_len - s.in_pos);
  memcpy(dst + have, s.inbuf + s.in_pos, k);
  s.in_pos += k, have += k;
  return have == n;
}

// Memory-to-memory Tornado I/O, as upstream ReadWriteMem.
struct TorMem
{
  const uint8_t *in;
  uint32_t in_left;
  uint8_t *out;
  uint32_t out_left;
};

static int tor_mem_callback(const char *what, void *buf, int size, void *aux)
{
  TorMem *m = (TorMem *) aux;
  if (strequ(what, "read")) {
    uint32_t k = min32(m->in_left, (uint32_t) size);
    memcpy(buf, m->in, k);
    m->in += k, m->in_left -= k;
    return (int) k;
  }
  if (strequ(what, "write")) {
    if ((uint32_t) size > m->out_left) return FREEARC_ERRCODE_OUTBLOCK_TOO_SMALL;
    memcpy(m->out, buf, size);
    m->out += size, m->out_left -= size;
    return size;
  }
  return strequ(what, "progress") ? FREEARC_OK : FREEARC_ERRCODE_NOT_IMPLEMENTED;
}

static int x4_decode_block(Stream &s)
{
  s.x_pos = 0;
  if (s.x_out_size == FA_4X4_STORED) {
    memcpy(s.x_out, s.x_in, s.x_in_size);
    s.x_len = s.x_in_size;
    return FREEARC_OK;
  }
  if (s.x_in_size < FA_TOR_HEADER) return FREEARC_ERRCODE_BAD_COMPRESSED_DATA;
  // tor_decompress allocates from the buffer size in its header; bound it first.
  if (le32(s.x_in + 2) > s.max_tor_buffer) return FA_ERR_LIMIT;
  TorMem m = { s.x_in, s.x_in_size, s.x_out, s.x_out_size };
  int res = tor_decompress(tor_mem_callback, &m, NULL, -1);
  if (res < 0) return res;
  // Upstream leaves "check that OutputSize==job.OutSize" unimplemented and writes
  // OutSize bytes regardless; a short or overlong block here is corrupt data.
  if (m.out_left != 0 || m.in_left != 0) return FREEARC_ERRCODE_BAD_COMPRESSED_DATA;
  s.x_len = s.x_out_size;
  return FREEARC_OK;
}

// Makes 4x4 output available: FILL_READY, FILL_NEED_INPUT, FILL_EOF, or an error.
static int x4_fill(Stream &s)
{
  while (s.x_pos == s.x_len) {
    if (s.x_eof) return FILL_EOF;
    bool at_end = s.in_pos == s.in_len && s.in_eof;
    switch (s.x_phase) {
    case X4_VERSION:
    case X4_HEADER: {
      uint32_t need = s.x_phase == X4_VERSION ? 4 : 8;
      // An empty stream or a block boundary may end the input (READ4_OR_EOF).
      if (s.x_hdr_have == 0 && at_end) { s.x_eof = true; return FILL_EOF; }
      if (!take_input(s, s.x_hdr, need, s.x_hdr_have)) return s.in_eof ? FA_ERR_TRUNCATED : FILL_NEED_INPUT;
      s.x_hdr_have = 0;
      if (s.x_phase == X4_VERSION) {
        if (le32(s.x_hdr) != FA_4X4_VERSION) return FREEARC_ERRCODE_BAD_COMPRESSED_DATA;
        s.x_phase = X4_HEADER;
        break;
      }
      s.x_out_size = le32(s.x_hdr), s.x_in_size = le32(s.x_hdr + 4);
      // Compressed blocks are only kept when smaller than their input (C_4x4.cpp
      // Process stores the rest), so both sizes are bounded by the 4x4 block size.
      if (s.x_in_size > s.max_block || (s.x_out_size != FA_4X4_STORED && s.x_out_size > s.max_block))
        return FA_ERR_LIMIT;
      if (!s.x_in && (!(s.x_in = (uint8_t *) MyAlloc(s.max_block)) || !(s.x_out = (uint8_t *) MyAlloc(s.max_block))))
        return FREEARC_ERRCODE_NOT_ENOUGH_MEMORY;
      s.x_in_have = 0;
      s.x_phase = X4_PAYLOAD;
      break;
    }
    case X4_PAYLOAD: {
      if (!take_input(s, s.x_in, s.x_in_size, s.x_in_have)) return s.in_eof ? FA_ERR_TRUNCATED : FILL_NEED_INPUT;
      int res = x4_decode_block(s);
      if (res < 0) return res;
      s.x_phase = X4_HEADER;
      break;
    }
    }
  }
  return FILL_READY;
}

// Copies 4x4 output into dst until have == n: FILL_READY, FILL_NEED_INPUT, FILL_EOF or an error.
static int rep_take(Stream &s, uint8_t *dst, uint32_t n, uint32_t &have)
{
  while (have < n) {
    int r = x4_fill(s);
    if (r != FILL_READY) return r;
    uint32_t k = min32(n - have, s.x_len - s.x_pos);
    memcpy(dst + have, s.x_out + s.x_pos, k);
    s.x_pos += k, have += k;
  }
  return FILL_READY;
}

// Emits [flushed, pos) when non-empty.
static int rep_emit(Stream &s)
{
  if (s.pos == s.flushed) return 0;
  s.pending = true;
  return FA_STEP_OUTPUT;
}

// Decodes the current REP block until the ring fills or the block ends.
static int rep_run(Stream &s)
{
  for (;;) {
    if (s.remaining > s.max_output - s.total) return FREEARC_ERRCODE_OUTBLOCK_TOO_SMALL;
    while (s.remaining) {
      uint32_t k = min32(s.remaining, s.size - s.pos);
      if (s.in_match) {
        k = min32(k, s.size - s.from);
        if (s.offset >= k) {
          memmove(s.ring + s.pos, s.ring + s.from, k);
        } else {
          // memcpy_lz_match: forward byte copy, replicating an overlapping period.
          for (uint32_t i = 0; i < k; i++) s.ring[s.pos + i] = s.ring[s.from + i];
        }
        s.from += k;
        if (s.from == s.size) s.from = 0;
      } else {
        memcpy(s.ring + s.pos, s.lit, k);
        s.lit += k;
      }
      s.pos += k, s.remaining -= k, s.total += k;
      if (s.pos == s.size) return rep_emit(s);
    }
    if (!s.in_match) {
      if (s.item == s.num) {
        if (s.lit_left != 0) return FREEARC_ERRCODE_BAD_COMPRESSED_DATA;
        s.r_phase = REP_SIZE;
        return rep_emit(s);
      }
      s.offset = le32(s.offsets + s.item * 4);
      s.remaining = le32(s.lens + s.item * 4);
      // The encoder emits offset = i - match >= 1 within the window; anything else
      // would copy bytes that were never decoded.
      if (s.remaining > INT32_MAX || s.offset == 0 || s.offset > s.size || s.offset > s.total)
        return FREEARC_ERRCODE_BAD_COMPRESSED_DATA;
      s.from = s.pos >= s.offset ? s.pos - s.offset : s.pos + s.size - s.offset;
      s.in_match = true;
    } else {
      s.item++;
      s.remaining = le32(s.datalens + s.item * 4);
      if (s.remaining > s.lit_left) return FREEARC_ERRCODE_BAD_COMPRESSED_DATA;
      s.lit_left -= s.remaining;
      s.in_match = false;
    }
  }
}

static int fill_status(int r) { return r == FILL_NEED_INPUT ? FA_STEP_NEED_INPUT : r == FILL_EOF ? FA_ERR_TRUNCATED : r; }

static int stream_step(Stream &s)
{
  if (s.pending) {
    s.pending = false;
    s.flushed = s.pos;
    if (s.pos == s.size) s.pos = s.flushed = 0;
  }
  for (;;) {
    int r;
    switch (s.r_phase) {
    case REP_DICT:
      if ((r = rep_take(s, s.r_hdr, 4, s.r_hdr_have)) != FILL_READY) return fill_status(r);
      s.r_hdr_have = 0;
      s.size = le32(s.r_hdr);
      if (s.size == 0) return FREEARC_ERRCODE_BAD_COMPRESSED_DATA;
      if (s.size > s.max_dict) return FA_ERR_LIMIT;
      if (!(s.ring = (uint8_t *) MyAlloc(s.size))) return FREEARC_ERRCODE_NOT_ENOUGH_MEMORY;
      s.r_phase = REP_SIZE;
      break;
    case REP_SIZE:
      if ((r = rep_take(s, s.r_hdr, 4, s.r_hdr_have)) != FILL_READY) return fill_status(r);
      s.r_hdr_have = 0;
      s.r_compr = le32(s.r_hdr);
      if (s.r_compr == 0) { s.r_phase = REP_TRAILER; break; }
      // num, lens[num], offsets[num], datalens[num+1], literal bytes.
      if (s.r_compr < 8) return FREEARC_ERRCODE_BAD_COMPRESSED_DATA;
      if (s.r_compr > s.r_bufsize) {
        uint32_t want = min32(s.size, FA_REP_MAX_BLOCK) + FA_REP_BUF_SLACK;
        if (want < s.r_compr) want = s.r_compr;
        MyFree(s.r_buf);
        s.r_bufsize = 0;
        if (!(s.r_buf = (uint8_t *) MyAlloc(want))) return FREEARC_ERRCODE_NOT_ENOUGH_MEMORY;
        s.r_bufsize = want;
      }
      s.r_have = 0;
      s.r_phase = REP_BODY;
      break;
    case REP_BODY: {
      if ((r = rep_take(s, s.r_buf, s.r_compr, s.r_have)) != FILL_READY) return fill_status(r);
      s.num = le32(s.r_buf);
      uint64_t tables = 4 + (uint64_t) s.num * 12 + 4;
      if (tables > s.r_compr) return FREEARC_ERRCODE_BAD_COMPRESSED_DATA;
      s.lens = s.r_buf + 4, s.offsets = s.lens + s.num * 4, s.datalens = s.offsets + s.num * 4;
      s.lit = s.r_buf + tables;
      s.lit_left = s.r_compr - (uint32_t) tables;
      s.item = 0;
      s.in_match = false;
      s.remaining = le32(s.datalens);
      if (s.remaining > s.lit_left) return FREEARC_ERRCODE_BAD_COMPRESSED_DATA;
      s.lit_left -= s.remaining;
      s.r_phase = REP_DECODE;
      break;
    }
    case REP_DECODE:
      if ((r = rep_run(s)) != 0) return r;
      break;
    case REP_TRAILER: {
      // rep_compress writes nothing after its EOF flag, and 4x4 ends with the input.
      uint8_t extra;
      uint32_t have = 0;
      r = rep_take(s, &extra, 1, have);
      if (r == FILL_READY) return FA_ERR_TRAILING;
      if (r != FILL_EOF) return fill_status(r);
      s.r_phase = REP_DONE;
      break;
    }
    case REP_DONE:
      return FA_STEP_DONE;
    }
  }
}

static void stream_release(Stream *s)
{
  MyFree(s->ring), MyFree(s->r_buf), MyFree(s->x_in), MyFree(s->x_out), MyFree(s->inbuf);
  s->ring = s->r_buf = s->x_in = s->x_out = s->inbuf = NULL;
}

// Sum of the stream's allocations for these limits: input chunk, REP window, REP
// block buffer, 4x4 input and output blocks, and Tornado's InputByteStream
// (MAXELEM + 1.25 buffer) and output buffer for compress_all_at_once. A REP block
// may exceed rep.cpp's min(dict, MAX_BLOCK)+1024 initial buffer by its tables;
// FA_REP_BUF_FACTOR is an allowance for that, not an upstream bound.
static const uint64_t FA_REP_BUF_FACTOR = 2;
// DataTables entry list (MidAlloc, 10000 entries doubling) and other small upstream buffers.
static const uint64_t FA_UPSTREAM_SLACK = 1u << 20;

FA_EXPORT uint64_t fa_stream_default_budget(uint32_t max_dict, uint32_t max_block, uint32_t max_tor_buffer)
{
  uint64_t tb = max_tor_buffer;
  return FA_INPUT_CHUNK + (uint64_t) max_dict
         + FA_REP_BUF_FACTOR * min32(max_dict, FA_REP_MAX_BLOCK) + FA_REP_BUF_SLACK
         + 2 * (uint64_t) max_block
         + MAXELEM + tb + tb / 4
         + tornado_decompressor_outbuf_size(max_tor_buffer) + PAD_FOR_TABLES * 2
         + FA_UPSTREAM_SLACK;
}

// One stream at a time: its limits bound every codec allocation (MyAlloc) until
// fa_stream_free. max_dict and max_block come from the authored method string;
// max_tor_buffer bounds the Tornado header buffer.
FA_EXPORT Stream *fa_stream_new(uint64_t max_output, uint32_t max_dict, uint32_t max_block,
                                uint32_t max_tor_buffer, uint64_t max_alloc)
{
  Stream *s = (Stream *) calloc(1, sizeof(Stream));
  if (!s) return NULL;
  s->max_output = max_output, s->max_dict = max_dict, s->max_block = max_block;
  s->max_tor_buffer = max_tor_buffer, s->max_alloc = max_alloc;
  g_budget = max_alloc, g_used = 0, g_peak = 0;
  if (!(s->inbuf = (uint8_t *) MyAlloc(FA_INPUT_CHUNK))) { g_budget = UINT64_MAX; free(s); return NULL; }
  return s;
}

FA_EXPORT uint8_t *fa_stream_input_ptr(Stream *s) { return s->inbuf; }
FA_EXPORT uint32_t fa_stream_input_cap(void) { return FA_INPUT_CHUNK; }

// After FA_STEP_NEED_INPUT: len bytes were written to fa_stream_input_ptr; 0 ends the input.
FA_EXPORT int32_t fa_stream_input(Stream *s, uint32_t len)
{
  if (s->error) return s->error;
  if (len > FA_INPUT_CHUNK || s->in_pos != s->in_len || s->in_eof) return s->error = FREEARC_ERRCODE_GENERAL;
  s->in_pos = 0, s->in_len = len, s->in_eof = len == 0;
  return FREEARC_OK;
}

FA_EXPORT int32_t fa_stream_step(Stream *s)
{
  if (s->error) return s->error;
  int r = stream_step(*s);
  if (r < 0) s->error = r;
  return r;
}

FA_EXPORT const uint8_t *fa_stream_output_ptr(Stream *s) { return s->ring + s->flushed; }
FA_EXPORT uint32_t fa_stream_output_len(Stream *s) { return s->pos - s->flushed; }
FA_EXPORT uint64_t fa_stream_total(Stream *s) { return s->total; }
FA_EXPORT uint64_t fa_stream_peak_alloc(Stream *) { return g_peak; }

FA_EXPORT void fa_stream_free(Stream *s)
{
  stream_release(s);
  free(s);
  g_budget = UINT64_MAX;
}
