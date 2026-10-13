# Authored map regions

Fresh REA analysis of the local Legacy World Editor, SHA-256
`d198d2ca47fc29d4901f3de27ff17619824afef5af7e42b9ff17cbe03723d7b7`:
reader `0x5523d0`, `ev_a810d28ebc12f27c8c129b758c4931d2a513dbe314e7e1d598ed51d029f9eabc`,
and writer `0x5526a0`, `ev_ac033870614bb5b513dce174c4ec7cc5577c189c00e99585d71530a34ad0a3ee`.
The reader establishes each version gate; the writer emits version five.

| Version | Record fields after version/count |
|---|---|
| 0 | four signed integer bounds, NUL-terminated name |
| 1 | additionally signed creation id |
| 2 | bounds become four floats |
| 3 | additionally effect/weather word |
| 4 | additionally NUL-terminated ambient-sound name |
| 5 | additionally stored colour word |

String writer `0x700590`, `ev_97a84eb5323ca7b1cdda9211d31eb2856ac74c8834dab25b8415becc419dac2f`,
writes strlen+1 bytes. Reader `0x6ff6e0`,
`ev_9a249bb859ade856d945b071d48cbac5e9761e18cca41f8c3fc1e3b146c11758`,
advances through the NUL terminator. No length-prefix encoding is invented.

`Regions::parse()` consumes every record exactly, preserving authored string
bytes and each version's coordinate numeric type. Older absent fields are None.
Effect identifiers and colour components remain in stored byte order. The parser
does not infer a localization encoding, channel permutation or missing-field
value. `Map::regions()` retains extraction and parsing errors.

The map suite passed 50 tests and its new explicit original fixture check over
44 archive maps. Synthetic tests exercise versions zero through five and every
truncated prefix of a nontrivial region record. Native parsing and original
framing checks do not establish runtime region-event or effect behavior.

The broader local census revealed loose TFT `.w3x` maps omitted by previous
archive-only sweeps. Those files also contain W3I version25, placements version8,
and actual W3A/W3B/W3D/W3H/W3Q object-edit files. The older claim that these
families never ship was a coverage error, not format evidence. Their support is
being implemented separately; W3I version18 and terrain meshes already exist.
