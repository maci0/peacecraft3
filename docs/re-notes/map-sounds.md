# Map sounds — `war3map.w3s`

`war3map.w3s` is the *map-authored* sound table (World Editor →
Sound dialogues shipped inside a map). It shares the `.w3s` extension
with the native savegame format, which is **not** what this container
describes.

## Framing (measured)

Header:

| offset | size | field |
|---:|---:|---|
| 0 | 4 | version word; shipped maps use `1` |
| 4 | 4 | record count `N` |

Each record:

1. NUL-terminated raw bytes: map-visible sound name (`gg_snd_*`).
2. NUL-terminated raw bytes: authored asset path.
3. NUL-terminated raw bytes: authored EAX environment name
   (`DefaultEAXON` in shipped records).
4. Fixed tail of **17 little-endian `u32` words**.

File length equals `8 + Σ records`. Trailing bytes or an unterminated
string are refused, not tolerated. Unsupported versions are refused;
only version 1 is measured.

Record count is bounded before allocation: `N` must not exceed
`file_len / (3 + 17 * 4)`.

## Notes

- The 17 words are preserved verbatim in file order. Their individual
  semantics are not yet decoded; no flags/channel/volume/distance meaning
  is invented from them.
- Synthetic tests live in `crates/pcx-map/tests/sounds.rs`; an ignored
  sweep exercises every shipped map that contains `war3map.w3s`.
PY