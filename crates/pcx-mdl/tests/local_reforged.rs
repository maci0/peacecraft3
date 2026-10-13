//! Explicit CASC fixture; no original model bytes are stored in the repository.
use pcx_casc::{blte::Limits, storage::Storage};
use pcx_mdl::{Error, Mdlx, tag};

#[test]
#[ignore = "requires PEACECRAFT3_REFORGED_DIR and PEACECRAFT3_REFORGED_ASSET from the authored TVFS namespace"]
fn original_casc_geosets_preserve_v1800_lod_and_classic_geometry() {
    let install = std::env::var_os("PEACECRAFT3_REFORGED_DIR")
        .expect("set PEACECRAFT3_REFORGED_DIR to the original CASC install");
    let path = std::env::var("PEACECRAFT3_REFORGED_ASSET")
        .expect("set PEACECRAFT3_REFORGED_ASSET to the authored qualified TVFS path");
    let storage = Storage::open(
        std::path::Path::new(&install),
        Limits {
            encoded_bytes: 64 * 1024 * 1024,
            decoded_bytes: 256 * 1024 * 1024,
        },
    )
    .expect("checked local storage");
    let encoding = storage.load_encoding().expect("verified encoding manifest");
    let resolver = storage
        .load_tvfs(&encoding)
        .expect("verified TVFS manifests");
    let asset = storage
        .read_path(&resolver, &path)
        .expect("verified named original content");
    let bytes = &asset.file.decoded.bytes;
    assert_eq!(asset.file.decoded.content_key, asset.resolved.ckey);
    assert_eq!(bytes.len() as u64, asset.resolved.decoded_size);
    let model = Mdlx::parse(bytes).expect("original MDLX container");
    assert_eq!(
        model.version, 1800,
        "this fixture must exercise the newer trailer"
    );
    let geosets = model.geosets(bytes).expect("inclusive GEOS framing");
    assert!(!geosets.is_empty());
    assert_eq!(
        geosets.iter().map(|g| g.size as usize).sum::<usize>(),
        model.payload(bytes, tag::GEOS).unwrap().len()
    );
    let mut vertices = 0;
    for (i, geoset) in geosets.iter().enumerate() {
        let mesh = model
            .mesh(bytes, geoset)
            .unwrap_or_else(|error| panic!("{path} geoset {i}: {error}"));
        let lod = mesh.lod.as_ref().expect("authored v1800 LoD field");
        assert_eq!(lod.version, model.version);
        let raw = &geoset.data[lod.offset..lod.offset + 84];
        assert_eq!(lod.level, i32::from_le_bytes(raw[..4].try_into().unwrap()));
        assert_eq!(lod.name.as_slice(), &raw[4..]);
        assert!(mesh.vertices.count > 0);
        assert_eq!(mesh.vertices.count, mesh.normals.count);
        assert_eq!(mesh.vertices.count, mesh.tex_coords.count);
        assert_eq!(mesh.vertices.count as usize, mesh.vertex_groups.len());
        assert_eq!(mesh.matrix_span_u32s, mesh.matrix_indices.len());
        assert_eq!(mesh.seq_bounds.len(), model.sequences(bytes).unwrap().len());
        vertices += mesh.vertices.count as usize;
        println!(
            "geoset={i} vertices={} lod_version={} lod_rel={:#x} lod_file={:#x} name_bytes={} seq_bounds={}",
            mesh.vertices.count,
            lod.version,
            lod.offset,
            model.chunk(tag::GEOS).unwrap().payload_offset + geoset.offset + 4 + lod.offset,
            lod.name.len(),
            mesh.seq_bounds.len()
        );
    }
    let textures = model
        .textures(bytes)
        .expect("TEXS records use the verified common layout");
    assert!(!textures.is_empty());
    assert_eq!(
        textures.len() * pcx_mdl::TEXTURE_SIZE,
        model.payload(bytes, tag::TEXS).unwrap().len()
    );
    assert!(matches!(
        model.materials(bytes),
        Err(Error::UnsupportedVersionedChunk {
            version: 1800,
            chunk: "MTLS"
        })
    ));
    println!(
        "source={path} build={} mdx_version={} bytes={} ckey={} ekey={} geosets={} vertices={vertices} textures={} materials=typed-refusal TANG/SKIN=absent",
        storage.build.info.source.version,
        model.version,
        bytes.len(),
        pcx_casc::hex(&asset.resolved.ckey),
        pcx_casc::hex(&asset.resolved.ekey),
        geosets.len(),
        textures.len()
    );
}

#[test]
#[ignore = "requires PEACECRAFT3_REFORGED_DIR and PEACECRAFT3_REFORGED_ASSET from the authored TVFS namespace"]
fn original_casc_v1800_materials_tile_and_preserve_authored_fields() {
    let install =
        std::env::var_os("PEACECRAFT3_REFORGED_DIR").expect("explicit original CASC install");
    let path = std::env::var("PEACECRAFT3_REFORGED_ASSET")
        .expect("explicit authored qualified TVFS asset");
    let storage = Storage::open(
        std::path::Path::new(&install),
        Limits {
            encoded_bytes: 64 * 1024 * 1024,
            decoded_bytes: 256 * 1024 * 1024,
        },
    )
    .expect("checked local storage");
    let encoding = storage.load_encoding().expect("verified encoding manifest");
    let resolver = storage
        .load_tvfs(&encoding)
        .expect("verified TVFS manifests");
    let asset = storage
        .read_path(&resolver, &path)
        .expect("verified named original content");
    let bytes = &asset.file.decoded.bytes;
    assert_eq!(asset.file.decoded.content_key, asset.resolved.ckey);
    assert_eq!(bytes.len() as u64, asset.resolved.decoded_size);
    let model = Mdlx::parse(bytes).unwrap();
    assert_eq!(model.version, 1800, "fixture must exercise v1800 materials");
    let materials = model
        .materials_v1800(bytes)
        .expect("established v1800 MTLS framing");
    assert!(!materials.is_empty());
    let chunk = model.chunk(tag::MTLS).unwrap();
    assert_eq!(
        materials.iter().map(|m| m.len as usize).sum::<usize>(),
        chunk.size
    );
    let textures = model.textures(bytes).unwrap();
    let mut layer_count = 0;
    let mut subtexture_count = 0;
    let mut track_count = 0;
    let mut key_count = 0;
    for (index, material) in materials.iter().enumerate() {
        let file = chunk.payload_offset + material.offset;
        assert_eq!(material.data, bytes[file + 4..file + material.len as usize]);
        let layers = material.checked_layers_v1800().unwrap();
        assert_eq!(
            16 + layers.iter().map(|l| l.size as usize).sum::<usize>(),
            material.data.len()
        );
        println!(
            "material={index} file={file:#x} size={} layers={}",
            material.len,
            layers.len()
        );
        for layer in &layers {
            let raw = &material.data[layer.offset..layer.offset + layer.size as usize];
            let word = |at| u32::from_le_bytes(raw[at..at + 4].try_into().unwrap());
            assert_eq!(layer.size, word(0));
            assert_eq!(layer.prefix.filter_mode, word(4));
            assert_eq!(layer.prefix.shading_flags, word(8));
            assert_eq!(layer.prefix.texture_id, word(12));
            assert_eq!(layer.prefix.texture_anim_id, word(16));
            assert_eq!(layer.prefix.coord_id, word(20));
            for (at, value) in [
                (24, layer.prefix.static_alpha),
                (28, layer.emissive_gain),
                (32, layer.fresnel_color[0]),
                (36, layer.fresnel_color[1]),
                (40, layer.fresnel_color[2]),
                (44, layer.fresnel_opacity),
                (48, layer.fresnel_team_color),
            ] {
                assert_eq!(value.to_bits(), word(at));
            }
            assert_eq!(layer.shader, word(52));
            assert_eq!(layer.subtextures.len(), word(56) as usize);
            for texture in &layer.subtextures {
                assert!((texture.texture_id as usize) < textures.len());
                let at = texture.offset - layer.offset;
                assert_eq!(texture.texture_id, word(at));
                assert_eq!(texture.stored_semantic, word(at + 4));
            }
            for track in layer.tracks.iter().chain(
                layer
                    .subtextures
                    .iter()
                    .filter_map(|t| t.texture_track.as_ref()),
            ) {
                let at = track.offset - layer.offset;
                assert_eq!(track.raw, raw[at + 16..at + 16 + track.raw.len()]);
                assert_eq!(track.raw.len(), track.count as usize * track.stride);
                assert_eq!(track.count, word(at + 4));
                assert_eq!(track.global_seq_id, word(at + 12));
                track_count += 1;
                key_count += track.count as usize;
            }
            println!(
                "  layer_file={:#x} size={} shader={} subtextures={} tracks={}",
                file + 4 + layer.offset,
                layer.size,
                layer.shader,
                layer.subtextures.len(),
                layer.tracks.len()
            );
            layer_count += 1;
            subtexture_count += layer.subtextures.len();
        }
    }
    assert!(layer_count > 0 && subtexture_count > 0 && track_count > 0);
    println!(
        "source={path} build={} mdx_version={} bytes={} ckey={} ekey={} MTLS_bytes={} materials={} layers={layer_count} subtextures={subtexture_count} tracks={track_count} keys={key_count} framing=exact render/native=unverified",
        storage.build.info.source.version,
        model.version,
        bytes.len(),
        pcx_casc::hex(&asset.resolved.ckey),
        pcx_casc::hex(&asset.resolved.ekey),
        chunk.size,
        materials.len()
    );
}
