#!/usr/bin/env python3
"""Skin the expansion glue panel model and report posed extents.

Read-only. Replicates the released `pcx-mdl` layout so the numbers can be checked
against `Mdlx::mesh` without building:

  * a geoset record's leading `u32` **includes its own 4 bytes**, so the advance
    is `size`, not `size + 4` — the off-by-four that made an earlier ad-hoc walk
    desync after one geoset;
  * the geoset sub-block walk is `VRTX NRMS PTYP PCNT PVTX GNDX MTGC MATS UVAS
    UVBS`, and after the `MATS` indices come a 44-byte trailer (whose first `u32`
    is the geoset's **material id**) and one 28-byte record per sequence;
  * the `GEOA` record's explicit geoset id is at record `+24` (`body + 20`),
    which `pcx-mdl` currently skips — this script reads it so the association can
    be *correlated*, never paired by position.

Skinning uses the measured convention from `pcx-mdl::compose`:

    local = T(pivot) . T(translation) . R . S . T(-pivot)
    world = parent_world . local

Usage: panel_skin.py [model.mdx]
"""

import struct
import sys

DEFAULT = "/tmp/opencode/panel/topright-exp.mdx"
SEQ_STRIDE = 132
GEOA_GEOSET_ID_OFFSET = 20  # inside the record body, i.e. record + 24
TRACK_HEADER = 16
TANGENT_INTERP = (2, 3)  # Hermite, Bezier


def payloads(d):
    """{tag: concatenated payload} for every top-level chunk."""
    i, out = 4, {}
    while i + 8 <= len(d):
        tag = d[i : i + 4]
        size = struct.unpack("<I", d[i + 4 : i + 8])[0]
        out.setdefault(tag, b"")
        out[tag] += d[i + 8 : i + 8 + size]
        i += 8 + size
    return out


def read_f32(b, o):
    return struct.unpack("<f", b[o : o + 4])[0]


def read_u32(b, o):
    return struct.unpack("<I", b[o : o + 4])[0]


def name80(b, o=0):
    return b[o : o + 80].split(b"\0")[0].decode("latin1")


def sequences(p):
    raw = p.get(b"SEQS", b"")
    return [
        (
            name80(raw[k * SEQ_STRIDE : (k + 1) * SEQ_STRIDE]),
            struct.unpack("<2I", raw[k * SEQ_STRIDE + 80 : k * SEQ_STRIDE + 88]),
        )
        for k in range(len(raw) // SEQ_STRIDE)
    ]


def pivots(p):
    raw = p.get(b"PIVT", b"")
    return [struct.unpack("<3f", raw[k : k + 12]) for k in range(0, len(raw) - 11, 12)]


def key_width(magic):
    if magic in (b"KGTR", b"KGSC"):
        return 3
    if magic == b"KGRT":
        return 4
    if magic in (b"KGAO", b"KGAA"):
        return 1
    if magic == b"KGAC":
        return 3
    return 1


def parse_tracks(b, o, end):
    """Tracks at `b[o:end]`: magic(4) count(4) interp(4) global(4) then keys."""
    out = []
    while o + TRACK_HEADER <= end:
        magic = b[o : o + 4]
        if magic not in (b"KGTR", b"KGRT", b"KGSC", b"KGAO", b"KGAC"):
            break
        count = read_u32(b, o + 4)
        interp = read_u32(b, o + 8)
        w = key_width(magic)
        stride = 4 + w * 4 * (3 if interp in TANGENT_INTERP else 1)
        keys = []
        base = o + TRACK_HEADER
        for k in range(count):
            at = base + k * stride
            frame = read_u32(b, at)
            vals = struct.unpack("<%df" % w, b[at + 4 : at + 4 + w * 4])
            keys.append((frame, vals))
        out.append({"magic": magic, "interp": interp, "keys": keys})
        o = base + count * stride
    return out


NODE_TRAILER = 8  # pcx-mdl's BONE_TRAILER: the record spans size + 8


def nodes(p, tag=b"BONE"):
    """BONE nodes: size(4) name(80) objectId(4) parentId(4) flags(4), tracks at 96.

    The leading `u32` does **not** include the trailing 8 bytes, so the record
    advance is `size + 8` — the framing `pcx-mdl::Mdlx::bones` calls
    `BONE_TRAILER`. Walking `size` alone lands on a node's `0xFFFFFFFF` parent
    sentinel and desyncs, which is what this probe originally did.
    """
    raw = p.get(tag, b"")
    i, out = 0, []
    while i + 4 <= len(raw):
        size = read_u32(raw, i)
        if size < 92 or i + size + NODE_TRAILER > len(raw):
            break
        body = raw[i + 4 : i + size]
        out.append(
            {
                "name": name80(body),
                "object_id": read_u32(body, 80),
                "parent_id": struct.unpack("<i", body[84:88])[0],
                "flags": read_u32(body, 88),
                "tracks": parse_tracks(body, 92, len(body)),
            }
        )
        i += size + NODE_TRAILER
    return out


def geoset_anims(p):
    """GEOA records with their explicit geoset id at body+20."""
    raw = p.get(b"GEOA", b"")
    i, out = 0, []
    while i + 4 <= len(raw):
        size = read_u32(raw, i)
        if size < 4 or i + size > len(raw):
            break
        body = raw[i + 4 : i + size]
        out.append(
            {
                "geoset_id": read_u32(body, GEOA_GEOSET_ID_OFFSET),
                "static_alpha": read_f32(body, 0),
                "flags": read_u32(body, 4),
                "tracks": parse_tracks(body, 24, len(body)),
            }
        )
        i += size
    return out


SUB_WIDTH = {
    b"VRTX": lambda n: n * 12,
    b"NRMS": lambda n: n * 12,
    b"UVBS": lambda n: n * 8,
    b"PVTX": lambda n: n * 2,
    b"GNDX": lambda n: n,
    b"PTYP": lambda n: 4,
    b"PCNT": lambda n: 4,
    b"MTGC": lambda n: n * 4,
    b"UVAS": lambda n: 0,
}


def geosets(p, seq_count):
    raw = p.get(b"GEOS", b"")
    i, out = 0, []
    while i + 4 <= len(raw):
        size = read_u32(raw, i)
        if size < 4 or i + size > len(raw):
            break
        body = raw[i + 4 : i + size]
        g = {"verts": [], "gndx": [], "mtgc": [], "mats": [], "material_id": None,
             "min": None, "max": None}
        at = 0
        while at + 8 <= len(body):
            tag = body[at : at + 4]
            count = read_u32(body, at + 4)
            if tag == b"MATS":
                g["mats"] = [
                    read_u32(body, at + 8 + k * 4) for k in range(count)
                ]
                body_end = at + 8 + count * 4
                g["material_id"] = read_u32(body, body_end)
                g["min"] = struct.unpack("<3f", body[body_end + 16 : body_end + 28])
                g["max"] = struct.unpack("<3f", body[body_end + 28 : body_end + 40])
                break
            width = SUB_WIDTH.get(tag)
            if width is None:
                break
            if tag == b"VRTX":
                g["verts"] = [
                    struct.unpack("<3f", body[at + 8 + k * 12 : at + 20 + k * 12])
                    for k in range(count)
                ]
            elif tag == b"GNDX":
                g["gndx"] = list(body[at + 8 : at + 8 + count])
            elif tag == b"MTGC":
                g["mtgc"] = [read_u32(body, at + 8 + k * 4) for k in range(count)]
            at += 8 + width(count)
        out.append(g)
        i += size
    return out


# --- math ------------------------------------------------------------------

def mat_identity():
    return [1.0 if r == c else 0.0 for r in range(4) for c in range(4)]


def mat_mul(a, b):
    return [
        sum(a[r * 4 + k] * b[k * 4 + c] for k in range(4))
        for r in range(4)
        for c in range(4)
    ]


def mat_trans(t):
    m = mat_identity()
    m[3], m[7], m[11] = t
    return m


def mat_scale(s):
    m = mat_identity()
    m[0], m[5], m[10] = s
    return m


def mat_rot_xyzw(q):
    x, y, z, w = q
    n = (x * x + y * y + z * z + w * w) ** 0.5
    if n == 0:
        return mat_identity()
    x, y, z, w = x / n, y / n, z / n, w / n
    return [
        1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w), 0,
        2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w), 0,
        2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y), 0,
        0, 0, 0, 1,
    ]


def apply(m, v):
    x, y, z = v
    return (
        m[0] * x + m[1] * y + m[2] * z + m[3],
        m[4] * x + m[5] * y + m[6] * z + m[7],
        m[8] * x + m[9] * y + m[10] * z + m[11],
    )


def bezier(p0, p1, p2, p3, t):
    u = 1 - t
    return u * u * u * p0 + 3 * u * u * t * p1 + 3 * u * t * t * p2 + t * t * t * p3


def sample(keys, interp, frame, default):
    if not keys:
        return default
    if frame <= keys[0][0]:
        return list(keys[0][1])
    if frame >= keys[-1][0]:
        return list(keys[-1][1])
    hi = next(i for i, k in enumerate(keys) if k[0] >= frame)
    lo = hi - 1
    f0, v0 = keys[lo]
    f1, v1 = keys[hi]
    if f1 == f0:
        return list(v0)
    t = (frame - f0) / (f1 - f0)
    out = []
    for c in range(len(v0)):
        a, b = v0[c], v1[c]
        if interp == 0:
            out.append(a)
        elif interp == 1:
            out.append(a + (b - a) * t)
        else:
            # No tangent bytes retained here; the panel's own interpolation is
            # linear in the sampled windows, and tangents are noted separately.
            out.append(a + (b - a) * t)
    return out


def world_matrices(ns, pivs, frame):
    by_id = {n["object_id"]: n for n in ns}
    cache = {}

    def build(oid):
        if oid in cache:
            return cache[oid]
        node = by_id[oid]
        pivot = pivs[oid] if oid < len(pivs) else (0.0, 0.0, 0.0)
        trans, rot, scale = (0.0, 0.0, 0.0), (0.0, 0.0, 0.0, 1.0), (1.0, 1.0, 1.0)
        for t in node["tracks"]:
            v = sample(t["keys"], t["interp"], frame, None)
            if v is None:
                continue
            if t["magic"] == b"KGTR":
                trans = tuple(v[:3])
            elif t["magic"] == b"KGRT":
                rot = tuple(v[:4])
            elif t["magic"] == b"KGSC":
                scale = tuple(v[:3])
        local = mat_trans(pivot)
        local = mat_mul(local, mat_trans(trans))
        local = mat_mul(local, mat_rot_xyzw(rot))
        local = mat_mul(local, mat_scale(scale))
        local = mat_mul(local, mat_trans((-pivot[0], -pivot[1], -pivot[2])))
        parent = node["parent_id"]
        world = mat_mul(build(parent), local) if parent >= 0 and parent in by_id else local
        cache[oid] = world
        return world

    for n in ns:
        build(n["object_id"])
    return cache


def main():
    path = sys.argv[1] if len(sys.argv) > 1 else DEFAULT
    d = open(path, "rb").read()
    p = payloads(d)
    ns = nodes(p)
    pivs = pivots(p)
    geos = geosets(p, len(sequences(p)))
    anims = geoset_anims(p)
    seqs = {n: (s, e) for n, (s, e) in sequences(p)}
    print(f"  sequences {len(seqs)}: {[n for n in seqs if 'MainMenu' in n]}")

    print(f"model {path}")
    print(f"  nodes {len(ns)}  pivots {len(pivs)}  geosets {len(geos)}  geoa {len(anims)}")
    ids = {g["geoset_id"] for g in anims}
    legs = {g["material_id"] for g in geos}
    print(f"  GEOA explicit geoset ids: {sorted(ids)}")
    print(f"  GEOS material ids:        {sorted(legs)}")

    # Association: report the GEOA ids against the GEOS index space.
    print("  association (retained indices, not positional pairing):")
    for a in anims:
        gid = a["geoset_id"]
        note = "in range" if 0 <= gid < len(geos) else "OUT OF RANGE"
        print(f"    GEOA geoset_id {gid:>3}  static alpha {a['static_alpha']:.3f}  {note}")

    for name in ("MainMenu Birth", "MainMenu Stand"):
        if name not in seqs:
            print(f"  {name}: absent")
            continue
        start, end = seqs[name]
        print(f"  {name} {start}..{end}")
        for frame in (start, (start + end) // 2, end):
            world = world_matrices(ns, pivs, frame)
            xs, ys, zs = [], [], []
            vx, vy = [], []
            for idx, g in enumerate(geos):
                alpha = None
                for a in anims:
                    if a["geoset_id"] == idx:
                        alpha = a["static_alpha"]
                pts = []
                for vi, v in enumerate(g["verts"]):
                    group = g["gndx"][vi] if vi < len(g["gndx"]) else 0
                    off = sum(g["mtgc"][:group]) if group < len(g["mtgc"]) else 0
                    cnt = g["mtgc"][group] if group < len(g["mtgc"]) else 0
                    bones = g["mats"][off : off + cnt] or [0]
                    acc = [0.0, 0.0, 0.0]
                    for b in bones:
                        w = world.get(b)
                        if w is None:
                            continue
                        q = apply(w, v)
                        acc = [acc[k] + q[k] for k in range(3)]
                    pts.append(tuple(c / len(bones) for c in acc))
                if pts:
                    gx = [q[0] for q in pts]
                    gy = [q[1] for q in pts]
                    gz = [q[2] for q in pts]
                    xs += [min(gx), max(gx)]
                    ys += [min(gy), max(gy)]
                    zs += [min(gz), max(gz)]
                    if alpha is None or alpha > 0.01:
                        vx += [min(gx), max(gx)]
                        vy += [min(gy), max(gy)]
            if xs:
                print(
                    f"    frame {frame:>5}: posed x[{min(xs):.4f},{max(xs):.4f}] "
                    f"y[{min(ys):.4f},{max(ys):.4f}] z[{min(zs):.4f},{max(zs):.4f}]"
                )
                if vx:
                    print(
                        f"      alpha-visible x[{min(vx):.4f},{max(vx):.4f}] "
                        f"y[{min(vy):.4f},{max(vy):.4f}]"
                    )


if __name__ == "__main__":
    main()
