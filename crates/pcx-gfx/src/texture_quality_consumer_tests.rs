// Texture Quality consumer tests (headless): wiring derive + one_mip +
// retain-on-refusal, including the unresolved 16-bit refusal state.
use crate::texture_quality_consumer::{Refusal, Request, policy, replacement, retain_on_refusal};
use crate::{LoadedTexture, TextureQuality};
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

// Synthetic NONSTOCK windows: mip special 7 at q=0; depth default 32,
// special 16 at cumulative sub 3 (q=3). Same shapes as the sibling
// texture_quality_tests; operands differ from the reviewed image.
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
fn consumer_loads_selected_mip_as_image() {
    let data = fixture();
    let m = nonstock_miplevel();
    let t = nonstock_texcolor();
    // q=1: mip 0, depth 32 -> top level as RGBA8. Domain 4 is
    // caller-supplied here only as synthetic context; production takes the
    // measured FDF popup length.
    let r = replacement(&data, "q.blp", &m, &t, 1, 4, 64).unwrap();
    assert_eq!((r.texture.width, r.texture.height), (4, 2));
    assert_eq!(r.texture.rgba, [11, 37, 83, 149].repeat(8));
    assert!(r.texture.mipmaps.is_empty());
    assert_eq!(r.image.data.as_deref(), Some(r.texture.rgba.as_slice()));
}

#[test]
fn depth16_refuses_with_user_visible_state_and_retains_previous() {
    let data = fixture();
    let m = nonstock_miplevel();
    let t = nonstock_texcolor();
    // q=3 derives depth 16 (2 bytes); the consumer refuses fail-closed.
    let err = replacement(&data, "q.blp", &m, &t, 3, 4, 64).unwrap_err();
    assert_eq!(err, Refusal::Depth16Unresolved { word: 16 });
    assert!(err.to_string().contains("no established channel format"), "{err}");
    // Retain-on-refusal returns the current texture unchanged.
    let current = LoadedTexture {
        width: 1,
        height: 1,
        rgba: vec![1, 2, 3, 4],
        mipmaps: Vec::new(),
        source: "previous.blp".into(),
    };
    let (kept, refusal) = retain_on_refusal(
        current.clone(),
        Request {
            bytes: &data,
            source: "q.blp",
            miplevel_branch: &m,
            texcolordepth_branch: &t,
            ordinal: 3,
            domain: 4,
            max_rgba_bytes: 64,
        },
    )
    .unwrap_err();
    assert_eq!(refusal, Refusal::Depth16Unresolved { word: 16 });
    assert_eq!(kept.source, current.source);
    assert_eq!(kept.rgba, current.rgba);
}

#[test]
fn ordinal_outside_authored_domain_refuses() {
    let data = fixture();
    let m = nonstock_miplevel();
    let t = nonstock_texcolor();
    // Domain 3 is a synthetic non-four popup length: ordinal 3 refuses
    // even though it is a well-formed branch index.
    assert_eq!(
        replacement(&data, "q.blp", &m, &t, 3, 3, 64).unwrap_err(),
        Refusal::OrdinalOutOfRange { ordinal: 3, domain: 3 }
    );
}

#[test]
fn malformed_windows_and_absent_mip_refuse() {
    let data = fixture();
    assert_eq!(
        replacement(&data, "q.blp", &[0xff; 8], &[0xff; 8], 0, 4, 64).unwrap_err(),
        Refusal::MalformedBranch
    );
    // Absent mip level (bad policy mip 5 via direct policy path is
    // covered in sibling tests; here a zero budget refuses the decode).
    let m = nonstock_miplevel();
    let t = nonstock_texcolor();
    assert_eq!(
        replacement(&data, "q.blp", &m, &t, 1, 4, 10).unwrap_err(),
        Refusal::MipUnavailable
    );
}

#[test]
fn policy_alone_resolves_words_without_pixels() {
    let m = nonstock_miplevel();
    let t = nonstock_texcolor();
    let p = policy(&m, &t, 3, 4).unwrap();
    assert_eq!(p, TextureQuality {
        mip_level: 0,
        color_depth_bytes: 2,
        texcolordepth_word: 16,
    });
}
