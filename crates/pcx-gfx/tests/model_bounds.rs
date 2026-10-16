use pcx_gfx::{Error, load_model};
use pcx_mdl::{MODL_SIZE, Mdlx};

fn model(minimum: [f32; 3], maximum: [f32; 3]) -> Vec<u8> {
    let mut bytes = b"MDLXVERS".to_vec();
    bytes.extend(4u32.to_le_bytes());
    bytes.extend(800u32.to_le_bytes());
    let mut payload = vec![0; MODL_SIZE];
    for (slot, value) in minimum.into_iter().chain(maximum).enumerate() {
        let offset = 0x158 + slot * 4;
        payload[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes.extend(b"MODL");
    bytes.extend((payload.len() as u32).to_le_bytes());
    bytes.extend(payload);
    bytes
}

#[test]
fn nonfinite_bounds_refuse_before_graphics_import() {
    for axis in 0..3 {
        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            for lower in [true, false] {
                let mut minimum = [-1.0; 3];
                let mut maximum = [1.0; 3];
                if lower {
                    minimum[axis] = invalid;
                } else {
                    maximum[axis] = invalid;
                }
                let bytes = model(minimum, maximum);
                let raw = Mdlx::parse(&bytes).unwrap().model(&bytes).unwrap();
                let retained = if lower {
                    raw.minimum_extent
                } else {
                    raw.maximum_extent
                };
                assert_eq!(retained[axis].to_bits(), invalid.to_bits());
                let Error::Decode(message) = load_model(&bytes, "synthetic/bad.mdx").unwrap_err()
                else {
                    panic!("expected a contextual graphics decode refusal");
                };
                assert!(message.contains("synthetic/bad.mdx"));
                assert!(message.contains(&format!("MODL extent axis {axis}")));
            }
        }
    }
}

#[test]
fn flat_and_empty_models_preserve_finite_authored_bounds() {
    for (minimum, maximum) in [
        ([0.0; 3], [0.0; 3]),
        ([2.0, -1.0, 0.0], [2.0, 3.0, 0.0]),
        ([-1.0, 2.0, -3.0], [1.0, 2.0, 4.0]),
        // Shipped particle-only models can carry inverted header bounds.
        ([1.0; 3], [-1.0; 3]),
    ] {
        let loaded = load_model(&model(minimum, maximum), "synthetic/flat.mdx").unwrap();
        assert!(loaded.meshes.is_empty());
        assert_eq!(loaded.minimum_extent, minimum);
        assert_eq!(loaded.maximum_extent, maximum);
    }
}
