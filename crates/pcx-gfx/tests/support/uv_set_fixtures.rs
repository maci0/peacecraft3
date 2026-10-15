use pcx_mdl::{Error, Geoset, Mdlx};
pub fn block(out: &mut Vec<u8>, tag: &[u8; 4], count: u32, data: &[u8]) {
    out.extend(tag);
    out.extend(count.to_le_bytes());
    out.extend(data);
}
pub fn base() -> Vec<u8> {
    let mut data = vec![];
    block(&mut data, b"VRTX", 2, &[0; 24]);
    block(&mut data, b"MATS", 0, &[0; 44]);
    data
}
pub fn uv(data: &mut Vec<u8>, value: f32) {
    let mut raw = Vec::new();
    for coordinate in [value, value + 0.25, value + 0.5, value + 0.75] {
        raw.extend(coordinate.to_le_bytes());
    }
    block(data, b"UVBS", 2, &raw);
}
pub fn geoset(data: Vec<u8>) -> Geoset {
    Geoset {
        offset: 0,
        size: (data.len() + 4) as u32,
        data,
    }
}
pub fn mesh(data: Vec<u8>) -> Result<pcx_mdl::Mesh, Error> {
    Mdlx {
        version: 800,
        chunks: vec![],
    }
    .mesh(&[], &geoset(data))
}
pub fn data_with_sets(count: u32) -> Vec<u8> {
    let mut data = base();
    block(&mut data, b"UVAS", count, &[]);
    for i in 0..count {
        uv(&mut data, i as f32 + 2.0);
    }
    data
}
pub fn model(geosets: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = b"MDLX".to_vec();
    block(&mut bytes, b"VERS", 4, &800u32.to_le_bytes());
    let mut material = vec![];
    material.extend(48u32.to_le_bytes());
    material.extend([0; 8]);
    material.extend(b"LAYS");
    material.extend(1u32.to_le_bytes());
    for word in [28u32, 0, 1, 0, pcx_mdl::NO_ID, 0, 1.0f32.to_bits()] {
        material.extend(word.to_le_bytes());
    }
    block(&mut bytes, b"MTLS", material.len() as u32, &material);
    let mut texture = [0u8; 268];
    texture[4..17].copy_from_slice(b"synthetic.blp");
    block(&mut bytes, b"TEXS", texture.len() as u32, &texture);
    let mut geometry = vec![];
    for data in geosets {
        geometry.extend((data.len() as u32 + 4).to_le_bytes());
        geometry.extend(data);
    }
    block(&mut bytes, b"GEOS", geometry.len() as u32, &geometry);
    bytes
}
