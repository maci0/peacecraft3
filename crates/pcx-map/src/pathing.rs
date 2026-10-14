//! Authored WPM raster, native writer 0x50f9c0; see format-map-pathing.md.
use crate::{Error, Map};

pub const PATHING_NAME: &str = "war3map.wpm";
const MAGIC: &[u8; 4] = b"MP3W";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PathingError {
    #[error("pathing file needs its 16-byte header")]
    Truncated,
    #[error("bad pathing magic {0:?}, expected MP3W")]
    BadMagic([u8; 4]),
    #[error("unsupported pathing version {0}")]
    UnsupportedVersion(u32),
    #[error("pathing raster {width}x{height} does not fit its {bytes} cell bytes")]
    Size {
        width: u32,
        height: u32,
        bytes: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pathing {
    pub version: u32,
    pub width: u32,
    pub height: u32,
    /// Authored pathing flags, in native row order. No terrain-derived fallback.
    pub cells: Vec<u8>,
}

impl Pathing {
    pub fn parse(data: &[u8]) -> Result<Self, PathingError> {
        let header = data.get(..16).ok_or(PathingError::Truncated)?;
        let magic = header[..4].try_into().unwrap();
        if &magic != MAGIC {
            return Err(PathingError::BadMagic(magic));
        }
        let word = |at| u32::from_le_bytes(header[at..at + 4].try_into().unwrap());
        let version = word(4);
        if version != 0 {
            return Err(PathingError::UnsupportedVersion(version));
        }
        let width = word(8);
        let height = word(12);
        let bytes = data.len() - 16;
        let cells = (width as usize).checked_mul(height as usize);
        if cells != Some(bytes) {
            return Err(PathingError::Size {
                width,
                height,
                bytes,
            });
        }
        Ok(Self {
            version,
            width,
            height,
            cells: data[16..].to_vec(),
        })
    }

    /// Returns `None` for an out-of-grid, missing or unrepresentable cell index.
    pub fn cell(&self, x: u32, y: u32) -> Option<u8> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let index = (y as usize)
            .checked_mul(self.width as usize)?
            .checked_add(x as usize)?;
        self.cells.get(index).copied()
    }
}

impl Map<'_> {
    pub fn pathing(&self) -> Result<Pathing, Error> {
        let data = self.extract(PATHING_NAME)?;
        Ok(Pathing::parse(&data)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(width: u32, height: u32, cells: &[u8]) -> Vec<u8> {
        let mut data = MAGIC.to_vec();
        for word in [0, width, height] {
            data.extend(word.to_le_bytes());
        }
        data.extend(cells);
        data
    }
    #[test]
    fn dimensions_and_native_row_order_are_authored() {
        let pathing = Pathing::parse(&fixture(3, 2, &[2, 4, 8, 32, 64, 128])).unwrap();
        assert_eq!(pathing.cell(2, 0), Some(8));
        assert_eq!(pathing.cell(0, 1), Some(32));
        assert_eq!(pathing.cell(3, 1), None);
    }
    #[test]
    fn malformed_dimensions_and_versions_are_refused() {
        assert!(matches!(
            Pathing::parse(&fixture(u32::MAX, u32::MAX, &[])),
            Err(PathingError::Size { .. })
        ));
        assert!(matches!(
            Pathing::parse(&fixture(2, 2, &[2, 4, 8])),
            Err(PathingError::Size { .. })
        ));
        let mut data = fixture(1, 1, &[2]);
        data[4..8].copy_from_slice(&7u32.to_le_bytes());
        assert_eq!(
            Pathing::parse(&data),
            Err(PathingError::UnsupportedVersion(7))
        );
    }
}
