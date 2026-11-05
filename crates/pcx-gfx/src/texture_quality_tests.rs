// Texture Quality policy and consumer tests (headless).
use crate::texture_quality::{PolicyError, derive_texture_quality};
use crate::{TextureQuality, one_mip};
use pcx_blp::{MIP_BASE, PALETTE_BASE};

fn fixture() -> Vec<u8> {
    let mut data = vec![0; PALETTE_BASE + 256 * 4];
    data[..4].copy_from_slice(b"BLP1");
    for (offset, value) in [(4, 1u32), (8, 8), (12, 4), (16, 2), (20, 4), (24, 1)] {
        data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    for (level, (count, rgba)) in [
        (8, [11, 37, 83, 149]),
        (2, [17, 61, 127, 173]),
        (1, [23, 67, 131, 197]),
    ]
    .into_iter()
    .enumerate()
    {
        let palette = PALETTE_BASE + level * 4;
        data[palette..palette + 4].copy_from_slice(&[rgba[2], rgba[1], rgba[0], 0]);
        let offset = MIP_BASE + level * 4;
        let start = data.len() as u32;
        data[offset..offset + 4].copy_from_slice(&start.to_le_bytes());
        data[offset + 64..offset + 68].copy_from_slice(&(count as u32 * 2).to_le_bytes());
        data.extend(std::iter::repeat_n(level as u8, count));
        data.extend(std::iter::repeat_n(rgba[3], count));
    }
    data
}

// Synthetic branch windows with NONSTOCK operands (not the reviewed image's
// 1/16/32): mip special value 7 at q=0; depth default 32, special 16 at
// cumulative sub 3 (q=3). Derivation must follow operands, not a table.
fn nonstock_miplevel() -> Vec<u8> {
    vec![
        0x33, 0xc9, 0x8b, 0xc7, 0x2b, 0xc1, 0x74, 0x0e, 0x83, 0xe8, 0x01, 0x74, 0x05, 0x83,
        0xe8, 0x01, 0x75, 0x09, 0x33, 0xc9, 0xeb, 0x05, 0xb9, 7, 0, 0, 0, 0x51, 0x6a, 0x0a,
        0x8b, 0xcb, 0xe8, 0, 0, 0, 0,
    ]
}
fn nonstock_texcolor() -> Vec<u8> {
    vec![
        0xb8, 32, 0, 0, 0, 0x83, 0xef, 0x00, 0x74, 0x0f, 0x83, 0xef, 0x01, 0x74, 0x0a, 0x83,
        0xef, 0x02, 0x75, 0x05, 0xb8, 16, 0, 0, 0, 0x50, 0x6a, 0x0b, 0x8b, 0xcb, 0xe8, 0, 0,
        0, 0,
    ]
}

#[test]
fn nonstock_branch_operands_drive_derivation() {
    let m = nonstock_miplevel();
    let t = nonstock_texcolor();
    // q=0: mip special 7, depth default 32 (4 bytes)
    let q0 = derive_texture_quality(&m, &t, 0, 4).unwrap();
    assert_eq!(q0.mip_level, 7);
    assert_eq!(q0.texcolordepth_word, 32);
    assert_eq!(q0.color_depth_bytes, 4);
    // q=1,2: mip default 0, depth default
    for q in [1, 2] {
        let v = derive_texture_quality(&m, &t, q, 4).unwrap();
        assert_eq!(v.mip_level, 0, "q={q}");
        assert_eq!(v.texcolordepth_word, 32, "q={q}");
    }
    // q=3 (cumulative 0+1+2): depth special 16 (2 bytes)
    let q3 = derive_texture_quality(&m, &t, 3, 4).unwrap();
    assert_eq!(q3.mip_level, 0);
    assert_eq!(q3.texcolordepth_word, 16);
    assert_eq!(q3.color_depth_bytes, 2);
}

#[test]
fn unknown_depth_word_refuses() {
    let m = nonstock_miplevel();
    // depth special 24 (3 bytes) is not a validated bit depth pair
    let t = vec![
        0xb8, 16, 0, 0, 0, 0x83, 0xef, 0x00, 0x74, 0x0f, 0xb8, 24, 0, 0, 0, 0x50, 0x6a, 0x0b,
        0x8b, 0xcb, 0xe8, 0, 0, 0, 0,
    ];
    assert_eq!(
        derive_texture_quality(&m, &t, 0, 4),
        Err(PolicyError::UnsupportedDepth { word: 24 })
    );
}

#[test]
fn malformed_branch_refuses() {
    assert_eq!(
        derive_texture_quality(&[0xff; 8], &[0xff; 8], 0, 4),
        Err(PolicyError::MalformedBranch)
    );
    assert_eq!(
        derive_texture_quality(&nonstock_miplevel(), &nonstock_texcolor(), 4, 4),
        Err(PolicyError::OrdinalOutOfRange { ordinal: 4, domain: 4 })
    );
}

#[test]
fn caller_supplied_non_four_domain_bounds_ordinal() {
    let m = nonstock_miplevel();
    let t = nonstock_texcolor();
    // A synthetic 3-choice domain admits 0..=2 and refuses 3, proving the
    // bound comes from the caller, not a copied constant.
    assert!(derive_texture_quality(&m, &t, 2, 3).is_ok());
    assert_eq!(
        derive_texture_quality(&m, &t, 3, 3),
        Err(PolicyError::OrdinalOutOfRange { ordinal: 3, domain: 3 })
    );
    // A 6-choice domain admits the former out-of-range ordinal 4.
    assert!(derive_texture_quality(&m, &t, 4, 6).is_ok());
    // A zero domain admits nothing.
    assert_eq!(
        derive_texture_quality(&m, &t, 0, 0),
        Err(PolicyError::OrdinalOutOfRange { ordinal: 0, domain: 0 })
    );
}

#[test]
fn quality_selects_chosen_mip_and_preserves_depth() {
    let data = fixture();
    let m = nonstock_miplevel();
    let t = nonstock_texcolor();
    // q=1 (mip default 0, depth 32->4B) picks top mip
    let top = one_mip(&data, "q.blp", &derive_texture_quality(&m, &t, 1, 4).unwrap(), 64).unwrap();
    assert_eq!(top.width, 4);
    assert_eq!(top.height, 2);
    assert_eq!(top.rgba.len(), 4 * 2 * 4);
    assert_eq!(top.rgba, [11, 37, 83, 149].repeat(8));
    assert!(top.mipmaps.is_empty());
}

#[test]
fn depth_16_two_bytes_refuses_without_inventing_rgba16() {
    let data = fixture();
    let m = nonstock_miplevel();
    let t = nonstock_texcolor();
    // q=3 derived depth is 16 bits = 2 bytes; RGBA8 loader has no 2-byte
    // store, so consumer refuses fail-closed (no RGBA16 invented).
    let q3 = derive_texture_quality(&m, &t, 3, 4).unwrap();
    assert_eq!(q3.color_depth_bytes, 2);
    assert!(one_mip(&data, "q.blp", &q3, 64).is_err());
}

#[test]
fn absent_or_invalid_mip_refuses_without_fallback() {
    let data = fixture();
    let bad = TextureQuality {
        mip_level: 5,
        color_depth_bytes: 4,
        texcolordepth_word: 32,
    };
    let err = one_mip(&data, "absent.blp", &bad, 64).unwrap_err();
    assert!(err.to_string().contains("absent mip level 5"), "{err}");
    let mut gap = data.clone();
    gap[MIP_BASE + 8..MIP_BASE + 12].fill(0);
    let zero = TextureQuality {
        mip_level: 2,
        color_depth_bytes: 4,
        texcolordepth_word: 32,
    };
    assert!(one_mip(&gap, "gap.blp", &zero, 64).is_err());
    assert!(one_mip(&data, "budget.blp", &derive_texture_quality(&nonstock_miplevel(), &nonstock_texcolor(), 1, 4).unwrap(), 10).is_err());
}

// Explicit original-image gate: reads the reviewed Legacy executable named
// by PCX_WAR3_EXE and the selected install's Options FDF via
// PEACECRAFT3_INSTALL, extracts the two branch windows by VA via PE sections,
// measures the actual TextureQualityMenu popup count from the FDF
// MenuItem records, and checks the derived policy against the native bytes.
#[test]
#[ignore = "requires PCX_WAR3_EXE naming the reviewed Legacy Warcraft III.exe and PEACECRAFT3_INSTALL with the selected Options FDFs"]
fn selected_original_branch_windows_derive_native_policy() {
    let image = std::fs::read(std::env::var("PCX_WAR3_EXE").unwrap()).unwrap();
    // Measure the authored popup domain from the actual FDF, not a
    // copied count: TextureQualityMenu -> PopupMenuFrame -> MenuItem records.
    let install = std::path::PathBuf::from(
        std::env::var_os("PEACECRAFT3_INSTALL").expect("explicit original install required"),
    );
    let domain = {
        // The caller (menu integrator) supplies the measured count in
        // production; this gate proves the real source count by
        // counting MenuItem lines in the Options FDF payload.
        let bytes = find_options_fdf(&install);
        count_texture_quality_items(&bytes)
    };
    assert!(domain > 0, "selected Options FDF must supply TextureQualityMenu items");
    // PE32 section map (same shape as pcx-play pe_image, std-only here).
    let pe = u32::from_le_bytes(image[0x3c..0x40].try_into().unwrap()) as usize;
    assert_eq!(&image[pe..pe + 4], b"PE\0\0");
    let coff = pe + 4;
    let count = u16::from_le_bytes(image[coff + 2..coff + 4].try_into().unwrap()) as usize;
    let opt_len = u16::from_le_bytes(image[coff + 16..coff + 18].try_into().unwrap()) as usize;
    let opt = coff + 20;
    let base = u32::from_le_bytes(image[opt + 28..opt + 32].try_into().unwrap());
    let table = opt + opt_len;
    let va_slice = |va: u32, len: usize| -> Vec<u8> {
        let rva = va - base;
        for i in 0..count {
            let at = table + 40 * i;
            let raw_size = u32::from_le_bytes(image[at + 16..at + 20].try_into().unwrap()) as usize;
            let raw_at = u32::from_le_bytes(image[at + 20..at + 24].try_into().unwrap()) as usize;
            let sec_rva = u32::from_le_bytes(image[at + 12..at + 16].try_into().unwrap());
            if sec_rva <= rva && rva < sec_rva + raw_size as u32 {
                let off = raw_at + (rva - sec_rva) as usize;
                return image[off..off + len].to_vec();
            }
        }
        panic!("VA {va:#x} not in any section");
    };
    let miplevel = va_slice(0x53865c, 0x20);
    let texcolor = va_slice(0x538681, 0x1e);
    // Native: q0 mip1/16, q1-2 mip0/16, q3 mip0/16, q2 depth 32.
    // The domain is the measured FDF count; every native ordinal below
    // must fall inside it.
    let q0 = derive_texture_quality(&miplevel, &texcolor, 0, domain).unwrap();
    assert_eq!(q0.mip_level, 1);
    assert_eq!(q0.texcolordepth_word, 16);
    assert_eq!(q0.color_depth_bytes, 2);
    for q in [1, 3] {
        let v = derive_texture_quality(&miplevel, &texcolor, q, domain).unwrap();
        assert_eq!(v.mip_level, 0, "q={q}");
        assert_eq!(v.texcolordepth_word, 16, "q={q}");
    }
    let q2 = derive_texture_quality(&miplevel, &texcolor, 2, domain).unwrap();
    assert_eq!(q2.mip_level, 0);
    assert_eq!(q2.texcolordepth_word, 32);
    assert_eq!(q2.color_depth_bytes, 4);
}

// Count `MenuItem` records inside the `TextureQualityMenu` popup frame
// of an Options FDF payload: find `TextureQualityMenu`, follow its
// `PopupMenuFrame "<name>"`, then count `MenuItem` lines in that frame's
// block. Pure text scan; no FDF parser dependency in pcx-gfx.
fn count_texture_quality_items(fdf: &[u8]) -> usize {
    let text = String::from_utf8_lossy(fdf);
    // Locate the TextureQualityMenu frame block.
    let menu_at = text
        .find("TextureQualityMenu")
        .expect("TextureQualityMenu in Options FDF");
    let after = &text[menu_at..];
    let popup_at = after
        .find("PopupMenuFrame")
        .expect("TextureQualityMenu PopupMenuFrame");
    let popup_after = &after[popup_at..];
    // First quoted string after PopupMenuFrame is the popup frame name.
    let q1 = popup_after.find('"').expect("popup frame name open quote");
    let rest = &popup_after[q1 + 1..];
    let q2 = rest.find('"').expect("popup frame name close quote");
    let popup_name = &rest[..q2];
    // Find that frame's block and count MenuItem records until its close.
    let frame_at = text
        .find(&format!("\"{popup_name}\""))
        .expect("popup frame definition");
    let block = &text[frame_at..];
    let mut count = 0usize;
    for line in block.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('}') && count > 0 {
            break;
        }
        if trimmed.starts_with("MenuItem") {
            count += 1;
        }
        // Stop at the next Frame definition after the popup frame.
        if count > 0 && trimmed.starts_with("Frame ") {
            break;
        }
    }
    count
}

// Locate the Options FDF under an install root. The menu layer owns
// full screen discovery; this gate only needs the Video page payload
// that carries TextureQualityMenu, found by scanning the conventional
// ui directory for a file mentioning it.
fn find_options_fdf(install: &std::path::Path) -> Vec<u8> {
    let ui = install.join("ui");
    let mut candidates = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&ui) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "fdf") {
                candidates.push(path);
            }
        }
    }
    candidates.push(ui.join("Options.fdf"));
    candidates.push(install.join("Options.fdf"));
    for path in &candidates {
        if let Ok(bytes) = std::fs::read(path)
            && bytes.windows(b"TextureQualityMenu".len()).any(|w| w == b"TextureQualityMenu")
        {
            return bytes;
        }
    }
    panic!("no Options FDF with TextureQualityMenu under {}", ui.display());
}
