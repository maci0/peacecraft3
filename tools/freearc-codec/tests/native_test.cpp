// Native tests for src/codec.cpp under ASan/UBSan. Synthetic streams only: REP
// blocks are written from an explicit literal/match script, 4x4 blocks are
// compressed with the pinned upstream Tornado compressor (as C_4x4.cpp's
// DeCompressMem path does), then decoded through the fa_stream_* API with
// irregular input chunks. Mutated stored-block streams exercise REP bounds checks.
//
// usage: native_test [fixture-dir]   (writes tornado fixtures for tests/codec.test.mjs)

// Standard headers first: upstream Common.h defines macros that break libstdc++.
#include <stdio.h>
#include <string>
#include <vector>

#include "codec.cpp"
#include "Compression/LZMA2/exe_reference.inc"
typedef std::vector<uint8_t> Bytes;

static int failures = 0;
#define EXPECT(cond, ...)                                                    \
  do {                                                                       \
    if (!(cond)) { failures++; fprintf(stderr, "FAIL %s:%d: ", __FILE__, __LINE__); \
                   fprintf(stderr, __VA_ARGS__); fputc('\n', stderr); }      \
  } while (0)

static uint32_t rng_state = 0x9E3779B9u;
static uint32_t rnd() { rng_state ^= rng_state << 13; rng_state ^= rng_state >> 17; rng_state ^= rng_state << 5; return rng_state; }

static void put32(Bytes &b, uint32_t v) { for (int i = 0; i < 4; i++) b.push_back((uint8_t) (v >> (8 * i))); }

// One REP item: literal bytes followed by a match (offset 0 = no match, last item).
struct Item { Bytes lit; uint32_t offset, len; };

// Encodes REP blocks (rep.cpp layout) and the reference output by plain appends.
static Bytes rep_encode(uint32_t dict, const std::vector<std::vector<Item> > &blocks, Bytes &expect)
{
  Bytes out;
  put32(out, dict);
  for (size_t b = 0; b < blocks.size(); b++) {
    const std::vector<Item> &items = blocks[b];
    uint32_t num = (uint32_t) items.size() - 1;
    Bytes body;
    put32(body, num);
    for (uint32_t i = 0; i < num; i++) put32(body, items[i].len);
    for (uint32_t i = 0; i < num; i++) put32(body, items[i].offset);
    for (uint32_t i = 0; i <= num; i++) put32(body, (uint32_t) items[i].lit.size());
    for (uint32_t i = 0; i <= num; i++) {
      body.insert(body.end(), items[i].lit.begin(), items[i].lit.end());
      expect.insert(expect.end(), items[i].lit.begin(), items[i].lit.end());
      for (uint32_t k = 0; i < num && k < items[i].len; k++) expect.push_back(expect[expect.size() - items[i].offset]);
    }
    put32(out, (uint32_t) body.size());
    out.insert(out.end(), body.begin(), body.end());
  }
  put32(out, 0);
  return out;
}

struct MemIO { const uint8_t *in; size_t left; Bytes *out; };

static int memio(const char *what, void *buf, int size, void *aux)
{
  MemIO *m = (MemIO *) aux;
  if (!strcmp(what, "read")) {
    int k = (size_t) size < m->left ? size : (int) m->left;
    memcpy(buf, m->in, k);
    m->in += k, m->left -= k;
    return k;
  }
  if (!strcmp(what, "write")) { m->out->insert(m->out->end(), (uint8_t *) buf, (uint8_t *) buf + size); return size; }
  return 0;
}

// 4x4 framing of data in `block`-sized chunks; preset < 0 stores every block.
static Bytes x4_encode(const Bytes &data, uint32_t block, int preset)
{
  Bytes out;
  put32(out, FA_4X4_VERSION);
  for (size_t pos = 0; pos < data.size(); pos += block) {
    uint32_t n = (uint32_t) (data.size() - pos < block ? data.size() - pos : block);
    Bytes packed;
    if (preset >= 0) {
      PackMethod m = std_Tornado_method[preset];
      m.buffer = block;
      MemIO io = { &data[pos], n, &packed };
      int r = tor_compress(m, memio, &io, NULL, -1);
      EXPECT(r >= 0, "tor_compress preset %d: %d", preset, r);
    }
    if (preset < 0 || packed.size() >= n) {
      put32(out, FA_4X4_STORED), put32(out, n);
      out.insert(out.end(), data.begin() + pos, data.begin() + pos + n);
    } else {
      put32(out, n), put32(out, (uint32_t) packed.size());
      out.insert(out.end(), packed.begin(), packed.end());
    }
  }
  return out;
}

// Drives fa_stream_* with input chunks of 1..max_chunk bytes.
static int decode(const Bytes &in, uint32_t dict, uint32_t block, uint64_t max_out, Bytes &out,
                  uint32_t max_chunk, uint64_t *peak = NULL, uint64_t budget = 0,
                  bool exe = false, uint64_t max_intermediate = UINT64_MAX, uint64_t max_input = UINT64_MAX,
                  uint32_t *pending_exe_yields = NULL)
{
  out.clear();
  uint32_t tor_buffer = (block < 32 * kb ? 32 * kb : (block + 4095) & ~4095u);
  if (!budget) budget = exe ? fa_stream_exe_default_budget(dict, block, tor_buffer) : fa_stream_default_budget(dict, block, tor_buffer);
  Stream *s = exe ? fa_stream_new_exe(max_out, dict, block, tor_buffer, budget, max_intermediate, max_input)
                  : fa_stream_new(max_out, dict, block, tor_buffer, budget);
  if (!s) {
    EXPECT(g_used == 0, "constructor allocations leaked");
    return FREEARC_ERRCODE_NOT_ENOUGH_MEMORY;
  }
  size_t pos = 0;
  int r;
  uint32_t pending_yields = 0;
  while ((r = fa_stream_step(s)) > 0 && r != FA_STEP_DONE) {
    if (r == FA_STEP_OUTPUT) {
      if (s->e_buf && s->e_pos < s->e_len) pending_yields++;
      const uint8_t *p = fa_stream_output_ptr(s);
      out.insert(out.end(), p, p + fa_stream_output_len(s));
    } else {
      uint32_t n = 1 + rnd() % max_chunk;
      if (n > in.size() - pos) n = (uint32_t) (in.size() - pos);
      memcpy(fa_stream_input_ptr(s), in.data() + pos, n);
      pos += n;
      r = fa_stream_input(s, n);
      if (r < 0) break;
    }
  }
  if (r == FA_STEP_DONE) r = FREEARC_OK;
  if (r == FREEARC_OK && fa_stream_total(s) != out.size()) r = FREEARC_ERRCODE_INTERNAL;
  if (peak) *peak = fa_stream_peak_alloc(s);
  if (pending_exe_yields) *pending_exe_yields = pending_yields;
  fa_stream_free(s);
  EXPECT(g_used == 0, "allocations leaked: %llu", (unsigned long long) g_used);
  return r;
}

static Bytes text(size_t n)
{
  static const char *words[] = { "peon ", "footman ", "grunt ", "acolyte ", "wisp ", "\n" };
  Bytes b;
  while (b.size() < n) for (const char *p = words[rnd() % 6]; *p && b.size() < n; p++) b.push_back((uint8_t) *p);
  return b;
}

static Bytes noise(size_t n)
{
  Bytes b(n);
  for (size_t i = 0; i < n; i++) b[i] = (uint8_t) rnd();
  return b;
}

static void test_exe(const char *fixture_dir)
{
  // REP literals cross multiple EXE windows; a small REP dictionary forces
  // output yields while the EXE converted prefix is still pending.
  const uint32_t dict = 4096, block = 8 * kb;
  Bytes lit = text(LARGE_BUFFER_SIZE * 2 + 37);
  for (size_t i = 0; i + 5 < lit.size(); i += 13) { lit[i] = i & 1 ? 0xe8 : 0xe9; lit[i+4] = i & 2 ? 0xff : 0; }
  std::vector<std::vector<Item> > blocks;
  for (size_t pos = 0; pos < lit.size();) {
    size_t n = lit.size() - pos < 32771 ? lit.size() - pos : 32771;
    Item item = { Bytes(lit.begin() + pos, lit.begin() + pos + n), 0, 0 };
    blocks.push_back(std::vector<Item>(1, item)); pos += n;
  }
  Bytes want, transformed, out;
  Bytes rep = rep_encode(dict, blocks, want);
  MemIO io = { rep.data(), rep.size(), &transformed };
  EXPECT(bcj_x86_de_compress(1, memio, &io) == FREEARC_OK, "upstream EXE encoding");
  EXPECT(transformed != rep, "EXE fixture must exercise operand conversion");
  Bytes packed = x4_encode(transformed, block, 3);
  uint32_t compressed_blocks = 0;
  for (size_t pos = 4; pos < packed.size();) {
    if (le32(packed.data() + pos) != FA_4X4_STORED) compressed_blocks++;
    pos += 8 + le32(packed.data() + pos + 4);
  }
  EXPECT(compressed_blocks > 0, "EXE fixture must exercise Tornado decoding");
  uint64_t peak = 0;
  uint32_t pending_yields = 0;
  EXPECT(decode(packed, dict, block, want.size(), out, 97, &peak, 0, true, rep.size(), packed.size(), &pending_yields) == FREEARC_OK && out == want,
         "EXE/Tornado/REP roundtrip and pending-prefix yields");
  EXPECT(pending_yields > 0, "fixture must yield with an unconsumed converted EXE prefix");
  EXPECT(peak <= fa_stream_exe_default_budget(dict, block, 32 * kb), "EXE counted memory budget");
  EXPECT(rep.size() > want.size(), "intermediate policy must be independent of final output");
  EXPECT(decode(packed, dict, block, want.size(), out, 97, NULL, 0, true, rep.size()-1, packed.size()) == FA_ERR_LIMIT,
         "EXE intermediate cap");
  EXPECT(decode(packed, dict, block, want.size(), out, 97, NULL, 0, true, rep.size(), packed.size()-1) == FA_ERR_LIMIT,
         "EXE input cap");
  EXPECT(decode(packed, dict, block, want.size(), out, 97, NULL, FA_INPUT_CHUNK + LARGE_BUFFER_SIZE - 1, true, rep.size(), packed.size()) == FREEARC_ERRCODE_NOT_ENOUGH_MEMORY,
         "EXE buffer counted before allocation");
  EXPECT(decode(packed, dict, block, want.size()-1, out, 97, NULL, 0, true, rep.size(), packed.size()) == FREEARC_ERRCODE_OUTBLOCK_TOO_SMALL,
         "EXE final output cap");
  Bytes cut(packed.begin(), packed.end()-1);
  EXPECT(decode(cut, dict, block, want.size(), out, 97, NULL, 0, true, rep.size(), packed.size()) == FA_ERR_TRUNCATED,
         "EXE surrounding framing truncation");
  if (fixture_dir) {
    std::string base = std::string(fixture_dir) + "/exe-wrap";
    FILE *f = fopen((base + ".fa").c_str(), "wb"); fwrite(packed.data(), 1, packed.size(), f); fclose(f);
    f = fopen((base + ".out").c_str(), "wb"); fwrite(want.data(), 1, want.size(), f); fclose(f);
    f = fopen((base + ".intermediate-size").c_str(), "wb"); fprintf(f, "%zu\n", rep.size()); fclose(f);
  }
  printf("EXE full-chain limits/pending-prefix tests passed; Tornado blocks=%u pending-prefix yields=%u\n", compressed_blocks, pending_yields);
}

// A REP script whose output wraps a small window several times: matches cross the
// ring end, reach exactly one window back, and overlap themselves (offset 1 and 3).
static Bytes wrap_stream(uint32_t dict, Bytes &expect)
{
  std::vector<std::vector<Item> > blocks;
  for (int b = 0; b < 6; b++) {
    std::vector<Item> items;
    Item first = { b == 0 ? noise(dict) : noise(rnd() % 700), dict, dict / 2 + rnd() % dict };
    items.push_back(first);
    Item overlap1 = { text(5), 1, 300 }, overlap3 = { noise(3), 3, 1000 + rnd() % 3000 };
    Item back = { text(rnd() % 50), dict - rnd() % 100, 1 + rnd() % (2 * dict) };
    items.push_back(overlap1), items.push_back(overlap3), items.push_back(back);
    Item last = { text(rnd() % 2000), 0, 0 };
    items.push_back(last);
    blocks.push_back(items);
  }
  return rep_encode(dict, blocks, expect);
}

static void test_roundtrips(const char *fixture_dir)
{
  const int presets[] = { -1, 1, 2, 3, 5 };
  const uint32_t dict = 4096, block = 8 * kb;  // several 4x4 blocks per stream
  for (size_t p = 0; p < elements(presets); p++) {
    Bytes expect, out;
    Bytes rep = wrap_stream(dict, expect);
    Bytes x4 = x4_encode(rep, block, presets[p]);
    for (uint32_t chunk = 1; chunk <= FA_INPUT_CHUNK; chunk *= 64) {
      int r = decode(x4, dict, block, expect.size(), out, chunk);
      EXPECT(r == FREEARC_OK && out == expect, "preset %d chunk %u: status %d, %zu/%zu bytes", presets[p], chunk, r,
             out.size(), expect.size());
    }
    if (fixture_dir && presets[p] == 3) {
      std::string base = std::string(fixture_dir) + "/tor3-wrap";
      FILE *f = fopen((base + ".fa").c_str(), "wb");
      fwrite(x4.data(), 1, x4.size(), f), fclose(f);
      f = fopen((base + ".out").c_str(), "wb");
      fwrite(expect.data(), 1, expect.size(), f), fclose(f);
    }
  }

  // Text over many 4x4 blocks: REP blocks span 4x4 block boundaries.
  Bytes expect, out;
  std::vector<std::vector<Item> > blocks;
  for (int b = 0; b < 3; b++) {
    std::vector<Item> items;
    Item a = { text(300 * kb), 1000u + b, 20000 }, z = { text(1000), 0, 0 };
    items.push_back(a), items.push_back(z);
    blocks.push_back(items);
  }
  Bytes x4 = x4_encode(rep_encode(1 * mb, blocks, expect), 64 * kb, 3);
  uint64_t peak = 0;
  int r = decode(x4, 1 * mb, 64 * kb, expect.size(), out, 100000, &peak);
  EXPECT(r == FREEARC_OK && out == expect, "multi-block text: %d", r);
  EXPECT(peak <= fa_stream_default_budget(1 * mb, 64 * kb, 64 * kb), "peak %llu over default budget", (unsigned long long) peak);
  printf("multi-block text: %zu -> %zu bytes, peak alloc %llu\n", x4.size(), out.size(), (unsigned long long) peak);
}

static void test_limits()
{
  Bytes expect, out;
  Bytes x4 = x4_encode(wrap_stream(4096, expect), 8 * kb, 3);
  EXPECT(decode(x4, 4096, 8 * kb, expect.size() - 1, out, 4096) == FREEARC_ERRCODE_OUTBLOCK_TOO_SMALL, "output limit");
  EXPECT(decode(x4, 4095, 8 * kb, expect.size(), out, 4096) == FA_ERR_LIMIT, "dictionary limit");
  int r = decode(x4, 4096, 8 * kb - 1, expect.size(), out, 4096);
  EXPECT(r == FA_ERR_LIMIT, "4x4 block limit: %d", r);
  EXPECT(decode(x4, 4096, 8 * kb, expect.size(), out, 4096, NULL, FA_INPUT_CHUNK + 64 * kb) == FREEARC_ERRCODE_NOT_ENOUGH_MEMORY,
         "allocation budget");
  Bytes cut(x4.begin(), x4.end() - 1);
  EXPECT(decode(cut, 4096, 8 * kb, expect.size(), out, 4096) == FA_ERR_TRUNCATED, "truncated");
}

// Random byte changes to a stored-block stream: every result is a status, never a
// memory error (ASan), and successful decodes stay within the output limit.
static void test_mutations()
{
  Bytes expect, out;
  Bytes base = x4_encode(wrap_stream(4096, expect), 8 * kb, -1);
  int ok = 0;
  for (int t = 0; t < 3000; t++) {
    Bytes m = base;
    // Payload bytes only: a changed 4x4 header would feed raw bytes to Tornado,
    // whose range decoder exit()s on bad data (covered by the wasm tests).
    for (int k = 1 + rnd() % 4; k--;) {
      size_t p = 4 + rnd() % (m.size() - 4), in_block = (p - 4) % (8 + 8 * kb);
      if (in_block >= 8) m[p] ^= (uint8_t) (1 + rnd() % 255);
    }
    int r = decode(m, 4096, 8 * kb, expect.size() + 4096, out, 5000);
    EXPECT(r <= 0, "status %d", r);
    if (r == FREEARC_OK) {
      ok++;
      EXPECT(out.size() <= expect.size() + 4096, "output over limit");
    }
  }
  printf("mutations: %d of 3000 still decoded\n", ok);
}

int main(int argc, char **argv)
{
  test_roundtrips(argc > 1 ? argv[1] : NULL);
  test_limits();
  test_exe(argc > 1 ? argv[1] : NULL);
  test_mutations();
  if (failures) { fprintf(stderr, "%d failures\n", failures); return 1; }
  puts("native tests passed");
  return 0;
}
