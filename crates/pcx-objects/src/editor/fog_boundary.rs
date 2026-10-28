//! The selected profile's terrain boundary color; source order is ARGB.
use super::defaults::{parse_config_profile, profile_elements};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoundaryColor(pub [u8; 4]);
impl BoundaryColor {
    pub fn read(bytes: &[u8], max_bytes: usize) -> Result<Self, String> {
        read_color(bytes, max_bytes, "boundaryterrain").map(Self)
    }
}

/// Source-authored ARGB color for ground shadow images.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShadowColor(pub [u8; 4]);
impl ShadowColor {
    pub fn read(bytes: &[u8], max_bytes: usize) -> Result<Self, String> {
        read_color(bytes, max_bytes, "shadowimagecolor").map(Self)
    }
}

fn read_color(bytes: &[u8], max_bytes: usize, field: &str) -> Result<[u8; 4], String> {
    let member = "UI\\MiscData.txt";
    if bytes.len() > max_bytes {
        return Err(format!("{member}: boundary profile exceeds byte budget"));
    }
    let p = parse_config_profile(bytes, member).map_err(|e| e.to_string())?;
    if let Some(e) = p.unresolved().first() {
        return Err(e.to_string());
    }
    let fields = p.raw_fields("fogofwar", field);
    let [(_, raw)] = fields.as_slice() else {
        return Err(format!("{member}: expected one FogOfWar/{field}"));
    };
    let values = profile_elements(raw).map_err(|e| e.to_string())?;
    let [a, r, g, b] = values.as_slice() else {
        return Err(format!("{member}: expected ARGB boundary color"));
    };
    let byte = |v: &str| {
        v.parse::<u8>()
            .map_err(|_| format!("{member}: invalid boundary color byte"))
    };
    Ok([byte(r)?, byte(g)?, byte(b)?, byte(a)?])
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shadow_color_is_source_argb_and_has_no_fallback() {
        assert_eq!(
            ShadowColor::read(b"[FogOfWar]\nShadowImageColor=91,17,43,79\n", 128)
                .unwrap()
                .0,
            [17, 43, 79, 91]
        );
        assert!(ShadowColor::read(b"[FogOfWar]\nBoundaryTerrain=91,17,43,79\n", 128).is_err());
        assert!(ShadowColor::read(b"[FogOfWar]\nShadowImageColor=256,0,0,0\n", 128).is_err());
    }
    #[test]
    fn boundary_color_comes_from_the_selected_profile_without_defaults() {
        let b = b"[FogOfWar]\nBoundaryTerrain=127,11,29,43 // synthetic\n";
        assert_eq!(
            BoundaryColor::read(b, 128).unwrap(),
            BoundaryColor([11, 29, 43, 127])
        );
        assert!(BoundaryColor::read(b, 1).is_err());
        for b in [
            b"[FogOfWar]\nBoundaryTerrain=1,2,3".as_slice(),
            b"[FogOfWar]\nBoundaryTerrain=256,0,0,0",
            b"[FogOfWar]\nBoundaryTerrain=1,2,3,4\nBoundaryTerrain=1,2,3,4",
            b"[Other]\nBoundaryTerrain=1,2,3,4",
        ] {
            assert!(BoundaryColor::read(b, 256).is_err());
        }
    }
}
