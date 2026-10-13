#!/usr/bin/env python3
"""Enumerate a Warcraft III MDX model's geosets with their material, texture and
layer flags, sorted by how much screen they can cover.

Written to diagnose the `MainMenu3d` background's large red regions: a geoset
that covers the screen is one whose world-space bounding box is huge, so the
probe prints the boxes largest-first alongside the texture each geoset draws
with and the two flag words its material layer carries.

Reads the MDX directly; no game, no Rust, no network.

    python3 tools/diag/scene_geosets.py <model.mdx> [--mdl-install DIR]

With `--mdl-install`, MDL sources are also parsed for comparison — the shipped
`.mdl` text form names its flags, which is independent evidence for what the
numeric words mean.
"""
import struct
import sys
from pathlib import Path


def chunks(d: bytes) -> dict:
    """Top-level MDX chunks, concatenated by tag."""
    assert d[:4] == b"MDLX", d[:4]
    out: dict[bytes, bytearray] = {}
    i = 4
    while i + 8 <= len(d):
        tag = d[i:i + 4]
        size = struct.unpack("<I", d[i + 4:i + 8])[0]
        out.setdefault(tag, bytearray()).extend(d[i + 8:i + 8 + size])
        i += 8 + size
    return out


def textures(d: bytes) -> list[str]:
    t = chunks(d)[b"TEXS"]
    out = []
    for k in range(0, len(t) // 268):
        rec = t[k * 268:(k + 1) * 268]
        out.append(bytes(rec[4:]).split(b"\0")[0].decode("latin1"))
    return out


def materials(d: bytes) -> list[dict]:
    """One entry per MTLS record: layer filterMode / shadingFlags / textureId."""
    m = chunks(d)[b"MTLS"]
    out = []
    i = 0
    while i + 4 <= len(m):
        size = struct.unpack("<I", m[i:i + 4])[0]
        if size == 0 or i + size > len(m):
            break
        body = m[i:i + size]
        layers = []
        j = 0
        while True:
            j = body.find(b"LAYS", j)
            if j < 0:
                break
            idx, ln, filter_mode, shading, tex, txan, coord = struct.unpack(
                "<7I", body[j + 4:j + 32]
            )
            alpha = struct.unpack("<f", body[j + 32:j + 36])[0]
            layers.append(
                dict(index=idx, filter_mode=filter_mode, shading=shading, texture=tex,
                     texture_anim=txan, coord=coord, alpha=alpha)
            )
            j += 32
        out.append(dict(size=size, layers=layers))
        i += size
    return out


def geosets(d: bytes) -> list[dict]:
    """One entry per GEOS record: bounds, triangle count and material id."""
    g = chunks(d)[b"GEOS"]
    out = []
    i = 0
    while i + 4 <= len(g):
        size = struct.unpack("<I", g[i:i + 4])[0]
        if size == 0 or i + size > len(g):
            break
        body = g[i:i + size]
        verts = []
        p = body.find(b"VRTX")
        if p >= 0:
            n = struct.unpack("<I", body[p + 4:p + 8])[0]
            for k in range(n):
                o = p + 8 + k * 12
                verts.append(struct.unpack("<3f", body[o:o + 12]))
        tris = 0
        p = body.find(b"PVTX")
        if p >= 0:
            tris = struct.unpack("<I", body[p + 4:p + 8])[0] // 3
        # The trailer follows the MATS node indices; its first word is the
        # material id, which is what `docs/re-notes/004-mdx.md` measured.
        material_id = None
        p = body.find(b"MATS")
        if p >= 0:
            n = struct.unpack("<I", body[p + 4:p + 8])[0]
            t = p + 8 + n * 4
            if t + 4 <= len(body):
                material_id = struct.unpack("<I", body[t:t + 4])[0]
        if verts:
            lo = [min(v[c] for v in verts) for c in range(3)]
            hi = [max(v[c] for v in verts) for c in range(3)]
        else:
            lo = hi = [0.0, 0.0, 0.0]
        ext = [hi[c] - lo[c] for c in range(3)]
        out.append(dict(index=len(out), bytes=size, verts=len(verts), tris=tris,
                        material_id=material_id, lo=lo, hi=hi, ext=ext,
                        diag=(sum(e * e for e in ext)) ** 0.5))
        i += size
    return out


def main() -> int:
    args = sys.argv[1:]
    if not args:
        print(__doc__)
        return 2
    path = Path(args[0])
    d = path.read_bytes()
    tex = textures(d)
    mats = materials(d)
    geos = geosets(d)
    print(f"{path}: {len(geos)} geosets, {len(mats)} materials, {len(tex)} textures")
    print()
    print("geosets by bounding-box diagonal (a geoset that can fill the screen is big):")
    print(f"  {'idx':>3} {'verts':>5} {'tris':>5} {'mat':>4} {'diag':>10} "
          f"{'ext x':>9} {'ext y':>9} {'ext z':>9}  texture")
    for gs in sorted(geos, key=lambda x: -x["diag"])[:18]:
        mid = gs["material_id"]
        layer = mats[mid]["layers"][0] if mid is not None and mid < len(mats) and mats[mid]["layers"] else None
        name = "?"
        if layer and 0 <= layer["texture"] < len(tex):
            name = tex[layer["texture"]].split("\\")[-1]
        fm = layer["filter_mode"] if layer else -1
        sf = layer["shading"] if layer else -1
        a = layer["alpha"] if layer else -1
        print(f"  {gs['index']:>3} {gs['verts']:>5} {gs['tris']:>5} {str(mid):>4} "
              f"{gs['diag']:>10.0f} {gs['ext'][0]:>9.0f} {gs['ext'][1]:>9.0f} {gs['ext'][2]:>9.0f}"
              f"  {name}  filterMode={fm} shadingFlags=0x{sf:x} alpha={a:.2f}")
    print()
    print("flag words across every layer (the words that decide blend and culling):")
    from collections import Counter
    fm_c = Counter(l["filter_mode"] for m in mats for l in m["layers"])
    sf_c = Counter(hex(l["shading"]) for m in mats for l in m["layers"])
    print("  filterMode  :", dict(sorted(fm_c.items())))
    print("  shadingFlags:", dict(sorted(sf_c.items())))
    print()
    print("materials whose geometry has a very large bounding box:")
    big = [gs for gs in geos if gs["diag"] > 20000]
    for gs in sorted(big, key=lambda x: -x["diag"]):
        mid = gs["material_id"]
        layer = mats[mid]["layers"][0] if mid is not None and mid < len(mats) and mats[mid]["layers"] else None
        name = tex[layer["texture"]] if layer and 0 <= layer["texture"] < len(tex) else "(none)"
        print(f"  geoset {gs['index']:>3} mat {mid} diag {gs['diag']:>8.0f} "
              f"shading=0x{layer['shading']:x} filterMode={layer['filter_mode']} {name}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
