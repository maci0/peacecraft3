use super::*;
fn image(module: &str, base: u32) -> (Vec<u8>, u32) {
    let mut b = vec![0; 0x1200];
    b[..2].copy_from_slice(b"MZ");
    put(&mut b, 0x3c, 0x80);
    b[0x80..0x84].copy_from_slice(b"PE\0\0");
    b[0x84..0x86].copy_from_slice(&0x14cu16.to_le_bytes());
    b[0x86..0x88].copy_from_slice(&1u16.to_le_bytes());
    b[0x94..0x96].copy_from_slice(&0xe0u16.to_le_bytes());
    b[0x98..0x9a].copy_from_slice(&0x10bu16.to_le_bytes());
    put(&mut b, 0xb4, base);
    put(&mut b, 0xd0, 0x3000);
    put(&mut b, 0xf4, 16);
    put(&mut b, 0x100, 0x1000);
    put(&mut b, 0x104, 60);
    b[0x178..0x180].copy_from_slice(b".rdata\0\0");
    for (at, v) in [
        (0x180, 0x1000),
        (0x184, 0x1000),
        (0x188, 0x1000),
        (0x18c, 0x200),
        (0x19c, 0x40000040),
    ] {
        put(&mut b, at, v);
    }
    // Two descriptors, each with its own terminated lookup list. The first IAT precedes the requested slot.
    for (at, v) in [
        (0x200, 0x1100),
        (0x20c, 0x1200),
        (0x210, 0x1300),
        (0x214, 0x1120),
        (0x220, 0x1220),
        (0x224, 0x1320),
        (0x300, 0x1400),
        (0x320, 0x1420),
        (0x500, 0x1400),
        (0x520, 0x1420),
    ] {
        put(&mut b, at, v);
    }
    b[0x400..0x406].copy_from_slice(b"other\0");
    b[0x420..0x420 + module.len()].copy_from_slice(module.as_bytes());
    b[0x602..0x608].copy_from_slice(b"alpha\0");
    b[0x622..0x628].copy_from_slice(b"omega\0");
    (b, base + 0x1320)
}
fn put(b: &mut [u8], at: usize, value: u32) {
    b[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn origin(source: &str) -> AssetIdentity {
    AssetIdentity {
        source: source.into(),
        edition: "synthetic".into(),
        version: "content".into(),
    }
}
#[test]
fn import_owner_uses_own_terminated_list_relocated_nonstock_names_and_budgets() {
    for base in [0x300000, 0x700000] {
        let (b, iat) = image("OddBackend.bin", base);
        let demand = ImportDemand::for_iat(&b, iat, ImportLimits::default()).unwrap();
        assert_eq!(demand.module_name, "OddBackend.bin");
        assert_eq!(demand.import_name, "omega");
        assert_eq!(demand.descriptor_rva, 0x1014);
        assert!(ImportDemand::for_iat(&b, iat + 4, ImportLimits::default()).is_err());
        for limits in [
            ImportLimits {
                work_bytes: 1,
                ..Default::default()
            },
            ImportLimits {
                descriptors: 1,
                ..Default::default()
            },
            ImportLimits {
                thunks: 1,
                ..Default::default()
            },
            ImportLimits {
                string_bytes: 2,
                ..Default::default()
            },
        ] {
            assert!(ImportDemand::for_iat(&b, iat, limits).is_err());
        }
    }
}
#[test]
fn import_ordinal_overflow_duplicate_and_unsafe_owner_refuse() {
    for name in ["../bad", "x/y", "x\\y", "c:bad", " . ", "bad."] {
        let (b, iat) = image(name, 0x300000);
        assert!(
            ImportDemand::for_iat(&b, iat, Default::default()).is_err(),
            "{name}"
        );
    }
    let (b, iat) = image("ok.bin", 0x300000);
    for (at, v) in [
        (0x320, 0x80000001),
        (0x214, u32::MAX - 1),
        (0x210, 0x1320),
        (0x104, u32::MAX),
    ] {
        let mut changed = b.clone();
        put(&mut changed, at, v);
        assert!(ImportDemand::for_iat(&changed, iat, Default::default()).is_err());
    }
    for n in [0, 0x30, 0x170, 0x627] {
        assert!(ImportDemand::for_iat(&b[..n], iat, Default::default()).is_err());
    }
}
#[test]
fn source_generation_tracks_content_and_roles_not_diagnostic_locations() {
    let (b, iat) = image("odd.bin", 0x300000);
    let b: Arc<[u8]> = b.into();
    let demand = ImportDemand::for_iat(&b, iat, Default::default()).unwrap();
    let make = |label: &str, data: Option<&[u8]>, archives: &[Sha256Digest]| {
        ClientSources::checked(
            b.clone(),
            origin(label),
            "game.exe",
            archives,
            demand.clone(),
            Ok(data.map(|v| ModuleInput {
                origin: origin(label),
                member: "ODD.bin".into(),
                bytes: Arc::from(v),
            })),
        )
        .unwrap()
    };
    let a = make("/first/install", Some(b"payload"), &[[7; 32]]);
    let rebound = make("different.iso:member", Some(b"payload"), &[[7; 32]]);
    assert_eq!(a.generation(), rebound.generation());
    assert_ne!(
        a.generation(),
        make("same", Some(b"other"), &[[7; 32]]).generation()
    );
    assert_ne!(
        a.generation(),
        make("same", Some(b"payload"), &[[8; 32]]).generation()
    );
    assert_ne!(a.generation(), make("same", None, &[[7; 32]]).generation());
    assert_eq!(
        make("native", Some(b"payload"), &[[7; 32], [8; 32]]).generation(),
        make("iso", Some(b"payload"), &[[8; 32], [7; 32]]).generation()
    );
    assert!(a.module(&demand, 2).is_err());
    assert_eq!(
        a.module(&demand, 99).unwrap().unwrap().bytes.as_ref(),
        b"payload"
    );
    let mut foreign = demand.clone();
    foreign.main_sha256[0] ^= 1;
    assert!(a.module(&foreign, 99).is_err());
    let input = ModuleInput {
        origin: origin("x"),
        member: "elsewhere/odd.bin".into(),
        bytes: Arc::from(&b"x"[..]),
    };
    assert!(
        ClientSources::checked(b, origin("x"), "game.exe", &[], demand, Ok(Some(input))).is_err()
    );
}
#[test]
#[cfg(not(target_arch = "wasm32"))]
#[ignore = "requires PCX_WAR3_EXE pointing to selected local client"]
fn selected_import_descriptor_and_native_sibling_pin_match_content() {
    use std::io::Read;
    let path = std::path::PathBuf::from(std::env::var_os("PCX_WAR3_EXE").unwrap());
    let mut b = Vec::new();
    std::fs::File::open(&path)
        .unwrap()
        .take(CLIENT_IMAGE_BYTES as u64 + 1)
        .read_to_end(&mut b)
        .unwrap();
    assert!(b.len() <= CLIENT_IMAGE_BYTES);
    assert_eq!(sha256(&b), crate::menu_lighting::REVIEWED_IMAGE);
    // Source instruction identity; its IAT operand and descriptor contents are read.
    let pe = Pe::parse(
        &b,
        RuleInputBudget {
            image_bytes: CLIENT_IMAGE_BYTES,
            profile_bytes: 4096,
            sections: 96,
        },
    )
    .unwrap();
    let code = pe.read_from(0x7c9b51, true).unwrap();
    assert_eq!(&code[..2], &[0xff, 0x15]);
    let demand = ImportDemand::for_iat(&b, word(code, 2).unwrap(), Default::default()).unwrap();
    assert_eq!(demand.import_name, "_AIL_set_stream_volume@8");
    let module = pin_sibling(path.parent().unwrap(), &path, &demand)
        .unwrap()
        .unwrap();
    assert!(module.member.eq_ignore_ascii_case(&demand.module_name));
    let original = module.bytes.clone();
    let bound = ClientSources::checked(
        b.into(),
        origin("selected image"),
        path.file_name().unwrap().to_str().unwrap(),
        &[],
        demand.clone(),
        Ok(Some(module)),
    )
    .unwrap();
    assert_eq!(
        bound
            .module(&demand, CLIENT_IMAGE_BYTES as u64)
            .unwrap()
            .unwrap()
            .receipt
            .module_sha256,
        sha256(&original)
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn native_sibling_pin_missing_duplicates_escape_and_refresh_are_explicit() {
    use std::{fs, path::PathBuf};
    let dir = std::env::temp_dir().join(format!("pcx-client-pin-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir(&dir).unwrap();
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(dir.clone());
    let (bytes, iat) = image("odd.bin", 0x300000);
    let main = dir.join("main.exe");
    fs::write(&main, &bytes).unwrap();
    let d = ImportDemand::for_iat(&bytes, iat, Default::default()).unwrap();
    assert!(pin_sibling(&dir, &main, &d).unwrap().is_none());
    let module = dir.join("ODD.bin");
    fs::write(&module, b"first").unwrap();
    let pinned = pin_sibling(&dir, &main, &d).unwrap().unwrap();
    fs::write(&module, b"second").unwrap();
    assert_eq!(&*pinned.bytes, b"first");
    assert_eq!(
        &*pin_sibling(&dir, &main, &d).unwrap().unwrap().bytes,
        b"second"
    );
    fs::write(dir.join("odd.bin"), b"second").unwrap();
    assert!(
        pin_sibling(&dir, &main, &d)
            .unwrap_err()
            .to_string()
            .contains("ambiguous")
    );
    fs::remove_file(dir.join("odd.bin")).unwrap();
    #[cfg(unix)]
    {
        fs::remove_file(&module).unwrap();
        std::os::unix::fs::symlink(&main, &module).unwrap();
        assert!(pin_sibling(&dir, &main, &d).is_err());
    }
    fs::write(&main, b"changed").unwrap();
    assert!(
        pin_sibling(&dir, &main, &d)
            .unwrap_err()
            .to_string()
            .contains("main file changed")
    );
    assert!(pin_sibling(&dir.join("missing"), &main, &d).is_err());
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
#[ignore = "requires PCX_WAR3_EXE and its complete local archive installation"]
fn selected_recipe_attaches_auxiliary_only_to_its_pinned_image_generation() {
    use crate::campaign_data_set::{CampaignRecipe, ImageRules};
    let path = std::path::PathBuf::from(std::env::var_os("PCX_WAR3_EXE").unwrap());
    let image = std::fs::read(&path).unwrap();
    let identity = origin(path.to_str().unwrap());
    let rules = ImageRules::read(&image, identity).unwrap();
    let iat = crate::playercolor_host::RegularMusicVolumeRules::backend_iat(&image).unwrap();
    let demand = ImportDemand::for_iat(&image, iat, Default::default()).unwrap();
    let module = pin_sibling(path.parent().unwrap(), &path, &demand)
        .unwrap()
        .unwrap();
    let recipe = CampaignRecipe::pin(
        path.parent().unwrap(),
        pcx_assets::GameMode::default(),
        rules,
        &image,
        None,
        "VERSION_FROZEN_THRONE",
    )
    .unwrap();
    assert!(recipe.client_sources().is_none());
    let available = recipe
        .clone()
        .with_client_modules(demand.clone(), Ok(Some(module.clone())))
        .unwrap();
    let pinned = available.client_sources().unwrap();
    assert_eq!(pinned.main().1, recipe.image());
    let backend = pinned
        .module(&demand, CLIENT_IMAGE_BYTES as u64)
        .unwrap()
        .unwrap();
    let policy = crate::playercolor_host::RegularMusicVolumeRules::read(
        pinned.main().1,
        &backend.bytes,
        backend.origin.clone(),
    )
    .unwrap();
    assert_eq!(policy.backend_sha256, backend.receipt.module_sha256);
    assert!(
        available
            .base_resolver()
            .unwrap()
            .resolve(&demand.module_name)
            .is_err()
    );
    let absent = recipe
        .clone()
        .with_client_modules(demand.clone(), Ok(None))
        .unwrap()
        .client_sources()
        .unwrap();
    let failed = recipe
        .clone()
        .with_client_modules(demand.clone(), Err("synthetic source I/O failure".into()))
        .unwrap()
        .client_sources()
        .unwrap();
    assert_ne!(pinned.generation(), absent.generation());
    assert_ne!(absent.generation(), failed.generation());
    assert!(
        failed
            .module(&demand, CLIENT_IMAGE_BYTES as u64)
            .unwrap_err()
            .contains("source I/O")
    );
    let mut foreign = demand;
    foreign.main_sha256[0] ^= 1;
    assert!(
        recipe
            .with_client_modules(foreign, Ok(Some(module)))
            .is_err()
    );
    assert!(Arc::ptr_eq(&pinned, &available.client_sources().unwrap()));
}
