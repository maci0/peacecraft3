//! Source corner boundary bits expanded on the map's terrain/pathing grid.
use pcx_map::{Terrain, grid_alignment::GridAlignment};

#[derive(Debug, PartialEq, Eq)]
pub struct BoundaryMask {
    pub width: u32,
    pub height: u32,
    pub bytes: Vec<u8>,
}
impl BoundaryMask {
    pub fn read(
        terrain: &Terrain,
        pathing_width: u32,
        pathing_height: u32,
        max_bytes: usize,
    ) -> Result<Self, String> {
        let alignment = GridAlignment::from_dimensions(
            terrain.width(),
            terrain.height(),
            pathing_width,
            pathing_height,
        )
        .map_err(|e| e.to_string())?;
        let count = usize::try_from(pathing_width)
            .ok()
            .and_then(|w| usize::try_from(pathing_height).ok()?.checked_mul(w))
            .ok_or("boundary mask dimensions overflow")?;
        if count > max_bytes {
            return Err("boundary mask exceeds byte budget".into());
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(count)
            .map_err(|_| "boundary mask allocation failed")?;
        bytes.resize(count, 0);
        let scale = alignment.uniform_scale;
        for y in 0..terrain.height() {
            for x in 0..terrain.width() {
                let corner = terrain.corner(x, y).ok_or("missing boundary corner")?;
                // W3E v11: high texture flag and position-word map-edge flag.
                if corner.corner_flags & 0b1000 == 0 && !corner.position_flag() {
                    continue;
                }
                let x0 = (x * scale).saturating_sub(1);
                let y0 = (y * scale).saturating_sub(1);
                let x1 = ((x + 1) * scale).saturating_add(1).min(pathing_width);
                let y1 = ((y + 1) * scale).saturating_add(1).min(pathing_height);
                for row in y0..y1 {
                    let start = row as usize * pathing_width as usize;
                    bytes[start + x0 as usize..start + x1 as usize].fill(u8::MAX);
                }
            }
        }
        Ok(Self {
            width: pathing_width,
            height: pathing_height,
            bytes,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn terrain(flags: [u8; 9], edges: [u16; 9]) -> Terrain {
        let mut b = pcx_map::TERRAIN_MAGIC.to_vec();
        b.extend(pcx_map::TERRAIN_VERSION.to_le_bytes());
        b.push(0);
        b.extend(0u32.to_le_bytes());
        b.extend(0u32.to_le_bytes());
        b.extend(0u32.to_le_bytes());
        b.extend(3u32.to_le_bytes());
        b.extend(3u32.to_le_bytes());
        b.extend(0f32.to_le_bytes());
        b.extend(0f32.to_le_bytes());
        for i in 0..9 {
            b.extend(8192u16.to_le_bytes());
            b.extend(edges[i].to_le_bytes());
            b.extend([flags[i] << 4, 0, 0]);
        }
        Terrain::from_bytes(b).unwrap()
    }
    #[test]
    fn boundary_bits_dilate_one_pathing_cell_and_clip_at_map_edges() {
        let mut flags = [0; 9];
        flags[0] = 8;
        let t = terrain(flags, [0; 9]);
        let mask = BoundaryMask::read(&t, 8, 8, 64).unwrap();
        assert_eq!(mask.bytes.iter().filter(|b| **b == 255).count(), 25);
        for y in 0..8 {
            for x in 0..8 {
                assert_eq!(mask.bytes[y * 8 + x], if x < 5 && y < 5 { 255 } else { 0 });
            }
        }
        let mut edges = [0; 9];
        edges[4] = 0x4000;
        let mask = BoundaryMask::read(&terrain([0; 9], edges), 8, 8, 64).unwrap();
        assert_eq!(mask.bytes.iter().filter(|b| **b == 255).count(), 25);
        assert_eq!(mask.bytes[3 * 8 + 3], 255);
        assert_eq!(mask.bytes[2 * 8 + 3], 0);
        flags[0] = 4;
        assert!(
            BoundaryMask::read(&terrain(flags, [0; 9]), 8, 8, 64)
                .unwrap()
                .bytes
                .iter()
                .all(|b| *b == 0)
        );
        assert!(BoundaryMask::read(&t, 8, 8, 63).is_err());
        assert!(BoundaryMask::read(&t, 8, 6, 64).is_err());
    }
}
