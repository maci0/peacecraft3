//! Explicit local original-input gate; no game bytes are included.
use pcx_blp::{Header, decode_jpeg};

#[test]
#[ignore = "requires PCX_NATIVE_MINIMAP_BLP and PCX_NATIVE_MINIMAP_BGRA from the same original-client map"]
fn supplied_minimap_base_matches_every_native_pixel() {
    let source = std::env::var_os("PCX_NATIVE_MINIMAP_BLP").expect("set PCX_NATIVE_MINIMAP_BLP");
    let reference =
        std::env::var_os("PCX_NATIVE_MINIMAP_BGRA").expect("set PCX_NATIVE_MINIMAP_BGRA");
    let bytes = std::fs::read(&source).expect("read supplied minimap BLP");
    let header = Header::parse(&bytes).expect("parse supplied minimap BLP");
    let rgba = decode_jpeg(&bytes, &header, 0).expect("decode supplied minimap JPEG");
    let mut native = std::fs::read(&reference).expect("read native minimap BGRA");
    assert_eq!(rgba.len(), native.len(), "different map dimensions");
    for pixel in native.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    assert_eq!(rgba.len() % 4, 0);
    let different = rgba
        .as_chunks::<4>()
        .0
        .iter()
        .zip(native.as_chunks::<4>().0)
        .filter(|(actual, expected)| actual != expected)
        .count();
    assert_eq!(
        different, 0,
        "different RGBA pixels: source={source:?}, reference={reference:?}"
    );
}
