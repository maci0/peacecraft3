# MDX bindings and interpolation corrections

## Implemented

`GeosetAnim.geoset_id` preserves the final word of the 24-byte GEOA body,
previously discarded at body+20 (inclusive record+24). Its value comes from each
record; record order is not a binding. The original local Legacy corpus check
resolves every non-sentinel reference against that model's own GEOS table.

`Node.tracks` preserves HELP's keyed transforms within its inclusive node size,
using the existing strict track parser. Key times, interpolation kinds, tangent
bytes and global-sequence references remain authored values. Other node families
use their typed readers; this field is only populated by the HELP path.

The GEOA track parser now rejects unknown interpolation, unknown track tags and
incomplete track framing instead of stopping and returning a partially decoded
record with unexplained trailing bytes.

The scalar and vector samplers distinguish cubic Bezier control points from
Hermite derivatives. The previous combined branch evaluated both as Hermite.
A synthetic symmetric pair has zero endpoints and equal positive controls:
Bezier rises at its midpoint; Hermite with equal derivatives crosses zero there.
This verifies the mathematical distinction. It does not establish native
quaternion interpolation, sequence clipping or global-sequence clock behavior.

## Evidence and verification

The existing source-asset framing notes are `004-mdx.md`; its GEOA field table
omitted the body+20 word. New explicit fixture checks require
`PEACECRAFT3_FORMAT_INSTALL` and fail if the local original fixture is absent.
They keep all extracted bytes in memory and introduce no copies of original models.

Against the local Legacy install, the check traversed 3,249 models, validated
5,405 GEOA bindings and retained 15,810 helper tracks. The complete model crate
suite, including its original framing sweeps, passed 53 unit tests and the new
explicit local binding check. Counts are observations, not hard-coded assertions.
Synthetic regressions distinguish non-positional GEOA IDs, retain helper key and
global-sequence data, reject malformed helper/GEOA tracks, and discriminate
Bezier from Hermite in both scalar and vector paths.

Fresh REA string evidence for the local Legacy World Editor, SHA-256
`d198d2ca47fc29d4901f3de27ff17619824afef5af7e42b9ff17cbe03723d7b7`,
`ev_365e09851ade2604c972a39c08d3d798395465741c120820ff3ce2b051b8e965`,
locates `GeosetId` at `0x905104`. Native binary reader `0x691d80`,
`ev_ebef5cc295a5c6d2f443c1fafb890f97e2acfdcf7fa23c89322d7ce37f20bbf3`,
reads the body+20 word into in-memory +0x5c. Native keyword reader `0x694640`,
`ev_8bfcbd9654be5526c07b060ea15a822ffe02ed28be49574164dfc12dfc59136e`,
writes `GeosetId` (token 0x160, derived from the executable's token table) to
that same destination. Together these establish the binding's meaning.
Native HELP reader `0x698510`, `ev_87ab0c2a8848921ae8aa6882388f6751cdcd5a0bcba15cf369304a652eefe616`,
calls the generic node reader `0x6894a0`,
`ev_cca099f253a41bd94f6a467998a5e4bcd42ec78e03767a2c0316d80eb1c2605e`,
which consumes KGTR/KGRT/KGSC inside the authored node size.
These observations establish parsing, not rendered behavior.

## Caller handoff

Consumers must bind GEOA by `geoset_id`, respecting `NO_ID`, and sample helper
tracks along parent chains. `Node` now implements `PartialEq` rather than `Eq`
because its keyed float data does not admit `Eq`. No caller or rendering files
were changed in this workstream. Native material-alpha offset, TXAN rendering,
PRE2 semantics and native interpolation behavior remain under investigation.
