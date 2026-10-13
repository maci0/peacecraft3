//! Exact keyed original fixture; borrowed game bytes stay in memory only.
use pcx_casc::{blte::Limits, storage::Storage};
use pcx_mdl::{Error, Mdlx};

#[test]
#[ignore = "requires PEACECRAFT3_REFORGED_DIR and PEACECRAFT3_REFORGED_HD_ASSET for the configured HD Footman"]
fn original_casc_hd_footman_fafx_preserves_raw_fields_and_offsets() {
    let install =
        std::env::var_os("PEACECRAFT3_REFORGED_DIR").expect("explicit local original CASC install");
    let path = std::env::var("PEACECRAFT3_REFORGED_HD_ASSET")
        .expect("explicit configured HD Footman path");
    assert_eq!(path, "war3.w3mod:_hd.w3mod:units/human/footman/footman.mdx");
    let storage = Storage::open(
        std::path::Path::new(&install),
        Limits {
            encoded_bytes: 64 * 1024 * 1024,
            decoded_bytes: 256 * 1024 * 1024,
        },
    )
    .unwrap();
    let encoding = storage.load_encoding().unwrap();
    let tvfs = storage.load_tvfs(&encoding).unwrap();
    let asset = storage.read_path(&tvfs, &path).unwrap();
    assert_eq!(asset.file.decoded.content_key, asset.resolved.ckey);
    assert_eq!(
        pcx_casc::hex(&asset.resolved.ckey),
        "6fcd268161babbd73f4601060be39df5"
    );
    assert_eq!(
        pcx_casc::hex(&asset.resolved.ekey),
        "55ddc4ab832e5eadc526d91d40cdd9dd"
    );
    let bytes = &asset.file.decoded.bytes;
    assert_eq!(bytes.len() as u64, asset.resolved.decoded_size);
    let model = Mdlx::parse_stored_v1800(bytes).unwrap();
    let stream = model.fafx_stream_v1800(bytes).unwrap().unwrap();
    assert_eq!(stream.version, 1800);
    assert_eq!(stream.offset, 0x4367c2);
    assert_eq!(stream.payload_offset, 0x4367ca);
    assert_eq!(stream.record_count(), 1);
    assert_eq!(stream.data().len(), 340);
    assert_eq!(stream.data(), &bytes[0x4367ca..0x43691e]);
    assert_eq!(stream.data().as_ptr(), bytes[0x4367ca..].as_ptr());
    let record = stream.get(0).unwrap();
    assert_eq!(record.offset, stream.payload_offset);
    assert_eq!(record.name.as_slice(), &bytes[0x4367ca..0x43681a]);
    assert_eq!(record.path.as_slice(), &bytes[0x43681a..0x43691e]);
    assert_eq!(record.name.as_ptr(), bytes[0x4367ca..].as_ptr());
    assert_eq!(record.path.as_ptr(), bytes[0x43681a..].as_ptr());
    assert!(stream.get(1).is_none());
    assert!(matches!(Mdlx::parse(bytes), Err(Error::UnknownTag(tag)) if &tag == b"FAFX"));
    for geoset in model.geosets(bytes).unwrap() {
        assert!(matches!(
            model.mesh(bytes, &geoset),
            Err(Error::UnsupportedGeosetFeature { version: 1800, tag, .. })
                if &tag == b"TANG" || &tag == b"SKIN"
        ));
    }
    println!(
        "source={path} build={} ckey={} ekey={} version={} FAFX_records={} payload=[{:#x},{:#x}) bytes={} name_bytes={} path_bytes={} framing=exact stored-only render/native=unverified",
        storage.build.info.source.version,
        pcx_casc::hex(&asset.resolved.ckey),
        pcx_casc::hex(&asset.resolved.ekey),
        stream.version,
        stream.record_count(),
        stream.payload_offset,
        stream.payload_offset + stream.data().len(),
        stream.data().len(),
        record.name.len(),
        record.path.len()
    );
}
