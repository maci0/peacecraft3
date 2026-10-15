# MDX PRE2 authored state

## Retained native layout evidence

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

The retained evidence derives keyword identities from the executable's
token/pointer records and joins binary destinations to keyword destinations.
The earlier width/length naming from that join conflicts with two primary
readers and their consumers. That naming remains disputed; these receipts were
not re-probed for this correction. The table identifies the explicit convention
used for those two fields rather than presenting it as renewed native proof.

| Fixed-body offset | Authored field |
|---|---|
| 0, 4, 8, 12 | speed, variation, latitude, gravity: f32 |
| 16, 20 | life span, emission rate: f32 |
| 24, 28 | width, length: f32; pinned primary convention, see qualification below |
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

## Signed key times and qualified extent names

`KeyTrack::frame_signed` reads the stored frame word as signed milliseconds with
checked key-count/record-span/byte bounds. It preserves negative preroll and
returns `None` for malformed spans. `frame` retains its unsigned view,
`times_are_ordered` retains unsigned sorting, and `has_negative_frame` retains
its existing first-word check;
particle callers must validate ordering through the signed view, select the
source clip/global clock explicitly, and retain original raw bytes. The accessor
does not define interpolation, event firing, native clocks or rendered behavior.

The retained RoC compat and Legacy dandelion PRE2 tracks include a negative
preroll key. Their source identities remain distinct; see the private explicit
fixture gate rather than checking original data into tests.

The earlier table named fixed-body +24 length and +28 width and described its
native destination/keyword join as proof. The pinned primary implementations
below instead both read +24 width and +28 length. The public properties and
serializer now follow that explicit reader/consumer convention; this correction
must not be described as a re-proven native mapping. The native byte-layout
receipts above are preserved, while that destination-name join remains disputed.

- [flowtsohg reader, revision2ff0bc00, lines62–69](https://github.com/flowtsohg/mdx-m3-viewer/blob/2ff0bc00c6363f425016e23d88c0fb2929d3b3cc/src/parsers/mdlx/particleemitter2.ts#L62-L69)
  reads width before length; its [particle consumer, lines36–63](https://github.com/flowtsohg/mdx-m3-viewer/blob/2ff0bc00c6363f425016e23d88c0fb2929d3b3cc/src/viewer/handlers/mdx/particle2.ts#L36-L63)
  uses width on local X and length on local Y.
- [war3-model reader, revision52d7f421, lines745–752](https://github.com/4eb0da/war3-model/blob/52d7f421a5505a0434d764294057f2009ce3987b/mdx/parse.ts#L745-L752)
  uses the same static field order. Its [particle consumer, lines811–825](https://github.com/4eb0da/war3-model/blob/52d7f421a5505a0434d764294057f2009ce3987b/renderer/particles.ts#L811-L825)
  also assigns width to X and length to Y, but uses different extent ranges.
- Both primary key readers use signed32 frame words; [flowtsohg animation reader,
  lines27–45](https://github.com/flowtsohg/mdx-m3-viewer/blob/2ff0bc00c6363f425016e23d88c0fb2929d3b3cc/src/parsers/mdlx/animations.ts#L27-L45)
  supplies an explicit example. This is format-implementation evidence, not a
  recovered native event-clock rule.

KP2N/KP2W naming is deliberately unchanged: those two primary implementations
disagree about the tag-to-axis name association, and no retained menu emitter has
either track. Static-field naming does not settle that separate association.
Byte-exact round trips prove preservation, not semantic field names or rendering.
