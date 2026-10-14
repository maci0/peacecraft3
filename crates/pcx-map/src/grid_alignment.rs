use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GridDimensions {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerrainGridDimensions {
    pub tiles: GridDimensions,
    pub corners: GridDimensions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathingGridDimensions {
    pub raster: GridDimensions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridAlignment {
    pub terrain: TerrainGridDimensions,
    pub pathing: PathingGridDimensions,
    pub width_scale: u32,
    pub height_scale: u32,
    pub uniform_scale: u32,
    pub terrain_tile_count: u64,
    pub terrain_corner_count: u64,
    pub pathing_cell_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GridAlignmentError {
    #[error("terrain tile width must be greater than zero")]
    ZeroTerrainWidth,
    #[error("terrain tile height must be greater than zero")]
    ZeroTerrainHeight,
    #[error("pathing raster width must be greater than zero")]
    ZeroPathingWidth,
    #[error("pathing raster height must be greater than zero")]
    ZeroPathingHeight,
    #[error("pathing width {pathing} is not a multiple of terrain width {terrain}")]
    NonDivisibleWidth { terrain: u32, pathing: u32 },
    #[error("pathing height {pathing} is not a multiple of terrain height {terrain}")]
    NonDivisibleHeight { terrain: u32, pathing: u32 },
    #[error("width scale {width_scale} does not match height scale {height_scale}")]
    ScaleMismatch { width_scale: u32, height_scale: u32 },
    #[error("terrain corner dimension overflows u32")]
    Overflow,
}

impl GridAlignment {
    pub fn from_dimensions(
        terrain_width: u32,
        terrain_height: u32,
        pathing_width: u32,
        pathing_height: u32,
    ) -> Result<Self, GridAlignmentError> {
        if terrain_width == 0 {
            return Err(GridAlignmentError::ZeroTerrainWidth);
        }
        if terrain_height == 0 {
            return Err(GridAlignmentError::ZeroTerrainHeight);
        }
        if pathing_width == 0 {
            return Err(GridAlignmentError::ZeroPathingWidth);
        }
        if pathing_height == 0 {
            return Err(GridAlignmentError::ZeroPathingHeight);
        }
        if !pathing_width.is_multiple_of(terrain_width) {
            return Err(GridAlignmentError::NonDivisibleWidth {
                terrain: terrain_width,
                pathing: pathing_width,
            });
        }
        if !pathing_height.is_multiple_of(terrain_height) {
            return Err(GridAlignmentError::NonDivisibleHeight {
                terrain: terrain_height,
                pathing: pathing_height,
            });
        }
        let width_scale = pathing_width / terrain_width;
        let height_scale = pathing_height / terrain_height;
        if width_scale != height_scale {
            return Err(GridAlignmentError::ScaleMismatch {
                width_scale,
                height_scale,
            });
        }
        let uniform_scale = width_scale;

        let terrain_tiles_product = u64::from(terrain_width) * u64::from(terrain_height);
        let corners_width = terrain_width
            .checked_add(1)
            .ok_or(GridAlignmentError::Overflow)?;
        let corners_height = terrain_height
            .checked_add(1)
            .ok_or(GridAlignmentError::Overflow)?;
        let terrain_corners_product = u64::from(corners_width) * u64::from(corners_height);
        let pathing_product = u64::from(pathing_width) * u64::from(pathing_height);

        Ok(Self {
            terrain: TerrainGridDimensions {
                tiles: GridDimensions {
                    width: terrain_width,
                    height: terrain_height,
                },
                corners: GridDimensions {
                    width: corners_width,
                    height: corners_height,
                },
            },
            pathing: PathingGridDimensions {
                raster: GridDimensions {
                    width: pathing_width,
                    height: pathing_height,
                },
            },
            width_scale,
            height_scale,
            uniform_scale,
            terrain_tile_count: terrain_tiles_product,
            terrain_corner_count: terrain_corners_product,
            pathing_cell_count: pathing_product,
        })
    }
}

pub fn terrain_pathing_alignment(
    terrain_width: u32,
    terrain_height: u32,
    pathing_width: u32,
    pathing_height: u32,
) -> Result<GridAlignment, GridAlignmentError> {
    GridAlignment::from_dimensions(terrain_width, terrain_height, pathing_width, pathing_height)
}
