//! Explicit original-asset and actual-device gate; no asset bytes are saved.
#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "requires PEACECRAFT3_REFORGED_DIR and PEACECRAFT3_DDS_DEVICE_TEST=1; headless Vulkan device"]
fn named_original_ati2_on_actual_enabled_device() {
    use bevy::{
        asset::RenderAssetUsages,
        image::ImageSampler,
        render::{
            renderer::initialize_renderer,
            settings::{Backends, WgpuFeatures, WgpuSettings, WgpuSettingsPriority},
        },
    };
    use pcx_gfx::{dds, dds_gpu};
    assert_eq!(
        std::env::var("PEACECRAFT3_DDS_DEVICE_TEST").as_deref(),
        Ok("1")
    );
    let install =
        std::env::var_os("PEACECRAFT3_REFORGED_DIR").expect("explicit original install required");
    let storage = pcx_casc::storage::Storage::open(
        std::path::Path::new(&install),
        pcx_casc::blte::Limits {
            encoded_bytes: 64 * 1024 * 1024,
            decoded_bytes: 256 * 1024 * 1024,
        },
    )
    .unwrap();
    let encoding = storage.load_encoding().unwrap();
    let resolver = storage.load_tvfs(&encoding).unwrap();
    let path = "war3.w3mod:_hd.w3mod:units/human/footman/human_footman_main_normal.dds";
    let asset = storage.read_path(&resolver, path).unwrap();
    let ekey = pcx_casc::hex(&asset.resolved.ekey);
    let ckey = pcx_casc::hex(&asset.resolved.ckey);
    assert_eq!(ekey, "a94fde4333e1fd247c30b65c3db6b4aa");
    assert_eq!(ckey, "dba0c21180bef10bda12e16e709aa266");
    assert!(!asset.resolved.links.is_empty());
    let bytes = &asset.file.decoded.bytes;
    assert_eq!(bytes.len(), 2796368);
    let file_hash = pcx_casc::Evidence::of(asset.file.decoded.evidence.source.clone(), bytes);
    assert_eq!(pcx_casc::hex(&file_hash.md5), ckey);
    let limits = dds_gpu::Limits {
        max_encoded_bytes: bytes.len() as u64,
        max_payload_bytes: (bytes.len() - 128) as u64,
        max_working_bytes: 8 * 1024 * 1024,
    };
    let inspected = dds_gpu::inspect(bytes, path, limits).unwrap();
    assert_eq!(
        (inspected.width, inspected.height, inspected.mips.len()),
        (1024, 2048, 12)
    );
    assert_eq!(&inspected.raw_header[84..88], b"ATI2");
    assert_eq!(&inspected.raw_header[88..92], b"A2XY");
    assert_eq!(inspected.mips.last().unwrap().bytes.end, bytes.len());
    let cpu = dds::inspect(
        bytes,
        path,
        dds::Limits {
            max_encoded_bytes: bytes.len() as u64,
            max_rgba_bytes: 0,
            max_working_bytes: 0,
        },
    )
    .unwrap_err();
    assert!(matches!(cpu.kind, dds::Failure::Unsupported(_)));
    assert_eq!(cpu.offset, 84);
    // Explicit TEST POLICY: headless Vulkan, request BC on a real device.
    // Neither this choice nor device creation establishes native rendering.
    let settings = WgpuSettings {
        backends: Some(Backends::VULKAN),
        priority: WgpuSettingsPriority::WebGPU,
        features: WgpuFeatures::TEXTURE_COMPRESSION_BC,
        ..Default::default()
    };
    let resources = bevy::tasks::block_on(initialize_renderer(Backends::VULKAN, None, &settings));
    println!(
        "ATI2_DEVICE test_policy=headless_vulkan_request_bc adapter={:?} adapter_supported={:?} device_enabled={:?} max_texture_dimension_2d={}",
        *resources.2,
        resources.3.features(),
        resources.0.features(),
        resources.0.limits().max_texture_dimension_2d
    );
    assert!(
        resources
            .0
            .features()
            .contains(WgpuFeatures::TEXTURE_COMPRESSION_BC)
    );
    let imported = dds_gpu::import(
        bytes,
        path,
        &resources.0,
        limits,
        ImageSampler::nearest(),
        RenderAssetUsages::MAIN_WORLD,
    )
    .unwrap();
    assert_eq!(imported.profile, inspected);
    assert_eq!(imported.profile.raw_header.as_slice(), &bytes[..128]);
    let payload = imported.image.data.as_deref().unwrap();
    assert_eq!(payload, &bytes[128..]);
    let payload_hash = pcx_casc::Evidence::of(asset.file.decoded.evidence.source.clone(), payload);
    assert_eq!(
        pcx_casc::hex(&payload_hash.md5),
        "f6b3843fada5c1569ec0bd7cb118057f"
    );
    println!(
        "ATI2_OK path={path} build={} ekey={ekey} ckey={ckey} bytes={} format={:?} dimensions={}x{} mips={} payload_bytes={} payload_md5={} raw_header_equal=true cpu_ati2_refusal=true",
        storage.build.info.source.version,
        bytes.len(),
        imported.image.texture_descriptor.format,
        imported.profile.width,
        imported.profile.height,
        imported.profile.mips.len(),
        payload.len(),
        pcx_casc::hex(&payload_hash.md5)
    );
    // Same physical adapter, second actual device with BC deliberately disabled.
    let disabled = WgpuSettings {
        features: WgpuFeatures::empty(),
        ..settings
    };
    let disabled_resources =
        bevy::tasks::block_on(initialize_renderer(Backends::VULKAN, None, &disabled));
    println!(
        "ATI2_DEVICE test_policy=headless_vulkan_no_bc adapter={:?} adapter_supported={:?} device_enabled={:?}",
        *disabled_resources.2,
        disabled_resources.3.features(),
        disabled_resources.0.features()
    );
    assert!(
        disabled_resources
            .3
            .features()
            .contains(WgpuFeatures::TEXTURE_COMPRESSION_BC)
    );
    assert!(
        !disabled_resources
            .0
            .features()
            .contains(WgpuFeatures::TEXTURE_COMPRESSION_BC)
    );
    let error = dds_gpu::import(
        bytes,
        path,
        &disabled_resources.0,
        limits,
        ImageSampler::nearest(),
        RenderAssetUsages::MAIN_WORLD,
    )
    .unwrap_err();
    assert!(matches!(error, dds_gpu::Error::Capability { .. }));
    println!("ATI2_CAPABILITY_REFUSAL {error}");
}
