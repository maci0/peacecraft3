//! The selected profile's terrain boundary color; source order is ARGB.
use super::defaults::{parse_config_profile, profile_elements};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoundaryColor(pub [u8; 4]);
impl BoundaryColor {
    pub fn read(bytes: &[u8], max_bytes: usize) -> Result<Self, String> {
        let member = "UI\\MiscData.txt";
        if bytes.len() > max_bytes {
            return Err(format!("{member}: boundary profile exceeds byte budget"));
        }
        let p = parse_config_profile(bytes, member).map_err(|e| e.to_string())?;
        if let Some(e) = p.unresolved().first() {
            return Err(e.to_string());
        }
        let fields = p.raw_fields("fogofwar", "boundaryterrain");
        let [(_, raw)] = fields.as_slice() else {
            return Err(format!("{member}: expected one FogOfWar/BoundaryTerrain"));
        };
        let values = profile_elements(raw).map_err(|e| e.to_string())?;
        let [a, r, g, b] = values.as_slice() else {
            return Err(format!("{member}: expected ARGB boundary color"));
        };
        let byte = |v: &str| {
            v.parse::<u8>()
                .map_err(|_| format!("{member}: invalid boundary color byte"))
        };
        Ok(Self([byte(r)?, byte(g)?, byte(b)?, byte(a)?]))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
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
