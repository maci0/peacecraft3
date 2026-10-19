use std::collections::BTreeMap;
pub(crate) fn assets() -> BTreeMap<String, crate::binding::Reply> {
    let mut blp = vec![0u8; 0x49C + 16];
    blp[..4].copy_from_slice(b"BLP1");
    blp[4..8].copy_from_slice(&1u32.to_le_bytes());
    blp[12..16].copy_from_slice(&4u32.to_le_bytes());
    blp[16..20].copy_from_slice(&4u32.to_le_bytes());
    blp[28..32].copy_from_slice(&0x49Cu32.to_le_bytes());
    blp[92..96].copy_from_slice(&16u32.to_le_bytes());
    blp[0x9C..0xA0].copy_from_slice(&[0, 255, 0, 255]);
    BTreeMap::from([
        ("TerrainArt\\Terrain.slk".into(), crate::binding::Reply::Bytes(
            b"ID;PWXL;N;E\r\nC;Y1;X1;K\"tileID\"\r\nC;X2;K\"dir\"\r\nC;X3;K\"file\"\r\nC;Y2;X1;K\"Qgrd\"\r\nC;X2;K\"Synthetic\"\r\nC;X3;K\"Ground\"\r\nE\r\n".to_vec()
        )),
        ("Synthetic\\Ground.blp".into(), crate::binding::Reply::Bytes(blp)),
    ])
}
