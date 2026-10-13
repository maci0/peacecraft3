//! Explicit configured-CASC HD fixture; original bytes stay in memory.
use pcx_casc::{blte::Limits, storage::Storage};
use pcx_mdl::{Error, Mdlx, tag};

#[test]
#[ignore = "requires PEACECRAFT3_REFORGED_DIR and PEACECRAFT3_REFORGED_HD_ASSET from configured TVFS entries"]
fn original_casc_hd_geosets_preserve_wide_words_and_refuse_classic_import() {
    let install =
        std::env::var_os("PEACECRAFT3_REFORGED_DIR").expect("explicit original CASC install");
    let path = std::env::var("PEACECRAFT3_REFORGED_HD_ASSET")
        .expect("explicit authored qualified HD path, not an inferred suffix");
    let storage = Storage::open(
        std::path::Path::new(&install),
        Limits {
            encoded_bytes: 64 * 1024 * 1024,
            decoded_bytes: 256 * 1024 * 1024,
        },
    )
    .unwrap();
    let encoding = storage.load_encoding().unwrap();
    let resolver = storage.load_tvfs(&encoding).unwrap();
    let asset = storage
        .read_path(&resolver, &path)
        .expect("checked namespace/encoding/content identity");
    let bytes = &asset.file.decoded.bytes;
    assert_eq!(asset.file.decoded.content_key, asset.resolved.ckey);
    assert_eq!(bytes.len() as u64, asset.resolved.decoded_size);
    for link in &asset.resolved.links {
        println!(
            "namespace_link={} manifest={} node={:#x} configured={:?}",
            String::from_utf8_lossy(&link.path),
            pcx_casc::hex(&link.ekey),
            link.node_offset,
            link.config_fields
        );
    }
    let model = Mdlx::parse_stored_v1800(bytes).expect("checked stored v1800 container");
    let geosets = model.geosets(bytes).unwrap();
    assert!(!geosets.is_empty());
    assert_eq!(
        geosets.iter().map(|g| g.size as usize).sum::<usize>(),
        model.payload(bytes, tag::GEOS).unwrap().len()
    );
    let base = model.chunk(tag::GEOS).unwrap().payload_offset;
    let mut vertices = 0;
    let mut tangent_records = 0;
    let mut skin_records = 0;
    let mut skin_words = 0;
    let mut uv_sets = 0;
    let mut weighted_geosets = 0;
    for (index, geoset) in geosets.iter().enumerate() {
        let parsed = model
            .geoset_streams_v1800(geoset)
            .unwrap_or_else(|error| panic!("{path} geoset {index}: {error}"));
        let tangents = parsed
            .tangents
            .as_ref()
            .expect("fixture must exercise TANG");
        let skin = parsed
            .skin
            .as_ref()
            .expect("fixture must exercise weighted SKIN");
        assert_eq!(parsed.vertex_count, tangents.count);
        assert_eq!(parsed.vertex_count as usize, skin.record_count());
        assert_eq!(skin.word_count as usize, skin.record_count() * 8);
        assert_eq!(
            tangents.data,
            &geoset.data[tangents.offset + 8..tangents.offset + 8 + tangents.data.len()]
        );
        assert_eq!(
            skin.data,
            &geoset.data[skin.offset + 8..skin.offset + 8 + skin.data.len()]
        );
        let mut next = 0;
        for block in &parsed.blocks {
            assert_eq!(block.offset, next);
            assert_eq!(block.tag, geoset.data[next..next + 4]);
            assert_eq!(
                block.count,
                u32::from_le_bytes(geoset.data[next + 4..next + 8].try_into().unwrap())
            );
            next += 8 + block.body.len() + block.trailer.len();
            uv_sets += usize::from(&block.tag == b"UVBS");
        }
        assert_eq!(next, geoset.data.len());
        for vertex in 0..skin.record_count() {
            let record = skin.get(vertex).unwrap();
            let words = record.joint_words.into_iter().chain(record.weight_words);
            for (i, word) in words.enumerate() {
                let at = vertex * 16 + i * 2;
                assert_eq!(word.to_le_bytes(), skin.data[at..at + 2]);
            }
            for (i, bits) in tangents.bits(vertex).unwrap().into_iter().enumerate() {
                let at = vertex * 16 + i * 4;
                assert_eq!(bits.to_le_bytes(), tangents.data[at..at + 4]);
            }
        }
        assert!(
            matches!(model.mesh(bytes,geoset),Err(Error::UnsupportedGeosetFeature {version:1800,tag,..}) if &tag==b"TANG" || &tag==b"SKIN")
        );
        println!(
            "geoset={index} vertices={} TANG_count={} TANG_file={:#x} TANG_bytes={} SKIN_words={} SKIN_records={} SKIN_file={:#x} SKIN_bytes={} end_file={:#x}",
            parsed.vertex_count,
            tangents.count,
            base + geoset.offset + 4 + tangents.offset,
            tangents.data.len(),
            skin.word_count,
            skin.record_count(),
            base + geoset.offset + 4 + skin.offset,
            skin.data.len(),
            base + geoset.offset + geoset.size as usize
        );
        vertices += parsed.vertex_count as usize;
        tangent_records += tangents.count as usize;
        skin_records += skin.record_count();
        skin_words += skin.word_count as usize;
        weighted_geosets += 1;
    }
    assert!(weighted_geosets > 0);
    println!(
        "source={path} build={} mdx_version={} bytes={} ckey={} ekey={} geosets={} vertices={vertices} tangent_records={tangent_records} skin_records={skin_records} skin_words={skin_words} uv_sets={uv_sets} strict_mesh=typed-refusal render/native=unverified",
        storage.build.info.source.version,
        model.version,
        bytes.len(),
        pcx_casc::hex(&asset.resolved.ckey),
        pcx_casc::hex(&asset.resolved.ekey),
        geosets.len()
    );
}
