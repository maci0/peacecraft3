//! Synthetic sky model and texture bytes for the production-path sky tests. Layout follows
//! the decoders in `pcx-mdl` (classic v800 `GEOS`, `MATS` trailer) and `pcx-blp`
//! (direct BLP1). Nothing here is shipped original data. Pure bytes, no crate imports, so a
//! private non-Bevy test can include this file and check it against the decoders.

fn block(out: &mut Vec<u8>, tag: &[u8; 4], count: u32, data: &[u8]) {
    out.extend(tag);
    out.extend(count.to_le_bytes());
    out.extend(data);
}

fn chunk(out: &mut Vec<u8>, tag: &[u8; 4], payload: &[u8]) {
    out.extend(tag);
    out.extend((payload.len() as u32).to_le_bytes());
    out.extend(payload);
}

fn f32_bytes(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// One triangle, one material layer with the sky's authored shading flags, one bone and one
/// `Stand` sequence. Vertex 0 sits at the bone's origin, so the bone pose is the rest pose.
pub fn sky_model() -> Vec<u8> {
    let mut out = b"MDLX".to_vec();
    chunk(&mut out, b"VERS", &800u32.to_le_bytes());

    let mut modl = vec![0u8; 372];
    modl[..9].copy_from_slice(b"SkyTest\0\0");
    chunk(&mut out, b"MODL", &modl);

    let mut seqs = vec![0u8; 132];
    seqs[..5].copy_from_slice(b"Stand");
    seqs[80..84].copy_from_slice(&0u32.to_le_bytes());
    seqs[84..88].copy_from_slice(&1000u32.to_le_bytes());
    chunk(&mut out, b"SEQS", &seqs);

    // Material record: size (inclusive), priority plane, flags, `LAYS`, one layer.
    // Layer: size, filter mode, shading flags (0xf0 as in the sky), texture id, texture
    // animation id (none), coordinate set, static alpha.
    let mut mtls = 48u32.to_le_bytes().to_vec();
    mtls.extend([0; 8]);
    mtls.extend(b"LAYS");
    mtls.extend(1u32.to_le_bytes());
    for word in [28u32, 0, 0xf0, 0, u32::MAX, 0] {
        mtls.extend(word.to_le_bytes());
    }
    mtls.extend(1.0f32.to_le_bytes());
    chunk(&mut out, b"MTLS", &mtls);

    // One texture: replaceable id, then the 256-byte path at offset 4.
    let mut texs = vec![0u8; 268];
    let texture = b"Environment\\Sky\\Test\\Test.blp";
    texs[4..4 + texture.len()].copy_from_slice(texture);
    chunk(&mut out, b"TEXS", &texs);

    // Geoset data: three vertices, normals, one triangle, one group on node 0, the
    // geoset trailer (material 0, bounds), and one UV set.
    let mut geoset = Vec::new();
    block(
        &mut geoset,
        b"VRTX",
        3,
        &f32_bytes(&[0., 0., 0., 1., 0., 0., 0., 1., 0.]),
    );
    block(
        &mut geoset,
        b"NRMS",
        3,
        &f32_bytes(&[0., 0., 1., 0., 0., 1., 0., 0., 1.]),
    );
    block(&mut geoset, b"PTYP", 1, &4u32.to_le_bytes());
    block(&mut geoset, b"PCNT", 1, &3u32.to_le_bytes());
    let indices: Vec<u8> = [0u16, 1, 2].iter().flat_map(|i| i.to_le_bytes()).collect();
    block(&mut geoset, b"PVTX", 3, &indices);
    block(&mut geoset, b"GNDX", 3, &[0, 0, 0]);
    block(&mut geoset, b"MTGC", 1, &1u32.to_le_bytes());
    block(&mut geoset, b"MATS", 1, &0u32.to_le_bytes());
    geoset.extend(0u32.to_le_bytes()); // material id
    geoset.extend(0u32.to_le_bytes()); // selection group
    geoset.extend(0u32.to_le_bytes()); // unselectable
    geoset.extend(f32_bytes(&[1.0])); // bounds radius
    geoset.extend(f32_bytes(&[0., 0., 0.])); // minimum extent
    geoset.extend(f32_bytes(&[1., 1., 0.])); // maximum extent
    geoset.extend(0u32.to_le_bytes()); // sequence bounds
    block(&mut geoset, b"UVAS", 1, &[]);
    let uvs: Vec<u8> = f32_bytes(&[0., 0., 1., 0., 0., 1.]);
    block(&mut geoset, b"UVBS", 3, &uvs);
    let mut geos = ((geoset.len() + 4) as u32).to_le_bytes().to_vec();
    geos.extend(geoset);
    chunk(&mut out, b"GEOS", &geos);

    // One bone, no tracks. The trailing geoset pair (geoset 0, no geoset animation) is
    // inside the payload, after the 96-byte record.
    let mut bone = 96u32.to_le_bytes().to_vec();
    let mut name = [0u8; 80];
    name[..13].copy_from_slice(b"SkyInnerClods");
    bone.extend(name);
    bone.extend(0u32.to_le_bytes()); // object id
    bone.extend((-1i32).to_le_bytes()); // parent
    bone.extend(0x100u32.to_le_bytes()); // bone flag
    bone.extend(0u32.to_le_bytes()); // geoset id
    bone.extend((-1i32).to_le_bytes()); // geoset animation id
    chunk(&mut out, b"BONE", &bone);

    chunk(&mut out, b"PIVT", &[0; 12]);
    out
}

/// A 4×4 direct BLP1: 256-entry BGRA palette, one mip of palette indices.
pub fn sky_texture() -> Vec<u8> {
    const PALETTE_BASE: usize = 0x9C;
    const PALETTE_BYTES: usize = 256 * 4;
    const WIDTH: u32 = 4;
    const HEIGHT: u32 = 4;
    let mip_offset = (PALETTE_BASE + PALETTE_BYTES) as u32;
    let mip_size = WIDTH * HEIGHT;
    let mut out = b"BLP1".to_vec();
    out.extend(1u32.to_le_bytes()); // content: direct
    out.extend(0u32.to_le_bytes()); // alpha bit depth
    out.extend(WIDTH.to_le_bytes());
    out.extend(HEIGHT.to_le_bytes());
    out.extend(5u32.to_le_bytes()); // picture type: palettised
    out.extend(0u32.to_le_bytes()); // has mipmaps
    let mut mip_offsets = [0u32; 16];
    mip_offsets[0] = mip_offset;
    let mut mip_sizes = [0u32; 16];
    mip_sizes[0] = mip_size;
    for offset in mip_offsets {
        out.extend(offset.to_le_bytes());
    }
    for size in mip_sizes {
        out.extend(size.to_le_bytes());
    }
    for entry in 0..256u32 {
        out.extend([entry as u8, entry as u8, entry as u8, 0xff]);
    }
    out.extend((0..mip_size).map(|i| (i % 2) as u8));
    out
}
