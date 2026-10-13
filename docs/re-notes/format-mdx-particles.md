# MDX PRE2 authored state

## Native proof

Local Legacy World Editor SHA-256
`d198d2ca47fc29d4901f3de27ff17619824afef5af7e42b9ff17cbe03723d7b7`.
Fresh REA binary reader `0x69ceb0`, evidence
`ev_7cf83419ce22c509b37d4b1de2e9edd85329b483f0b2aac97c45e26cb2be75e1`,
reads the fixed PRE2 state positionally after its generic node. Its destination
fields are matched against the same executable's keyword handlers:

- static scalar handler `0x69c060`,
  `ev_5a5e7812cd24cfa08d04f3c8fb75c1b04cf6768cc6271217438c689913529ee0`;
- authored-property handler `0x69bc60`,
  `ev_503c59bf1dcee98d4db9a52e6d453accfe4c9e6f92c3de7f0d2b4cdb79bd62a4`;
- filter modes `0x69d700`,
  `ev_2135f8cd9917065f2ca74b1c87571e768f398d802a27af50704b21e360b8328c`;
- Head/Tail/Both `0x69d830`,
  `ev_65c061386c3f4173a0babde676eaeacc19ba925a5c913bb775865992da52a641`.

Keyword identities are derived from the original executable's token/pointer
records. Joining binary destinations to keyword destinations proves names
without importing modern in-memory PDB layouts.

| Fixed-body offset | Authored field |
|---|---|
| 0, 4, 8, 12 | speed, variation, latitude, gravity: f32 |
| 16, 20, 24, 28 | life span, emission rate, length, width: f32 |
| 32, 36, 40, 44 | filter mode, rows, columns, head/tail: u32 |
| 48, 52 | tail length, time: f32 |
| 56..92 | three segment colours, three f32 components each |
| 92..95 | three alpha bytes |
| 95..107 | three particle-scaling floats, unaligned |
| 107, 119, 131, 143 | life-span, decay, tail, tail-decay UV triplets: u32[3] |
| 155, 159, 163, 167 | texture id, squirt, signed priority plane, replaceable id |

The native reader also accepts `KP2R` (variation) and `KP2G` (gravity). Neither
appeared in the earlier shipped corpus, which did not prove they were unsupported.
Their framing is the same scalar key reader as the existing property tracks.

## Implementation

`Particle2::properties()` parses the retained 171 bytes into
`Particle2Properties`. Every value comes from that emitter's bytes. Colours stay
in file component order, without assuming a renderer's colour convention;
filter modes and identifiers stay authored rather than receiving fallback values.
`to_bytes()` reconstructs the complete fixed state for byte-exact local checks.
The raw state remains available. The parser now retains variation/gravity tracks.

Synthetic fixtures exercise unaligned fields, signed priority, full fixed-state
round trips and the two newly native-proven track tags. The gated original model
fixture checks every original PRE2 state's round trip. This establishes parsing;
particle simulation, emission scheduling, UV evaluation and rendering still need
caller implementation and native/runtime behavioral checks.
