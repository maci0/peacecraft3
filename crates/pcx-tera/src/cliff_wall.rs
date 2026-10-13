//! Caller-selected wall geometry, not the original game's cliff rule.
//!
//! Coordinates and the step threshold must come from the caller. No world
//! scale, cliff level, material, or threshold is supplied here. For each edge
//! exceeding the threshold, the chosen construction is a vertical rectangle
//! between its endpoints' minimum and maximum z. It does not replace the ground
//! surface or certify native cliff/ramp behavior; [`crate::cliff_class`] and
//! [`crate::cliff::cliff_threshold`] still refuse those decisions.

/// A vertical wall in the caller's x/y plane, with z pointing up.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallQuad {
    /// Bottom start, bottom end, top end, top start.
    pub positions: [[f32; 3]; 4],
    /// Unit face normal matching [`Self::TRIANGLE_INDICES`].
    pub normal: [f32; 3],
}

impl WallQuad {
    /// Triangle-list topology local to this quad; no texture coordinates implied.
    pub const TRIANGLE_INDICES: [u32; 6] = [0, 1, 2, 0, 2, 3];
}

/// Invalid caller geometry is reported rather than emitted as a broken mesh.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum WallGeometryError {
    #[error("wall step threshold must be finite and nonnegative")]
    InvalidThreshold,
    #[error("wall coordinates or their differences are not finite")]
    NonFiniteGeometry,
    #[error("a stepped wall edge has coincident x/y endpoints")]
    CoincidentEndpoints,
}

/// Construct a wall when an edge's absolute z difference strictly exceeds the
/// caller's threshold (in the same units as its coordinates).
///
/// Flat edges never produce a quad, including with a zero threshold. Reversing
/// an edge reverses its winding and normal. This is a geometry helper, not an
/// asset-defined classification or the original game's wall placement.
pub fn cliff_wall(
    edge: [[f32; 3]; 2],
    step_threshold: f32,
) -> Result<Option<WallQuad>, WallGeometryError> {
    if !step_threshold.is_finite() || step_threshold < 0.0 {
        return Err(WallGeometryError::InvalidThreshold);
    }
    if edge.iter().flatten().any(|value| !value.is_finite()) {
        return Err(WallGeometryError::NonFiniteGeometry);
    }
    let [start, end] = edge;
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    let step = (end[2] - start[2]).abs();
    let length = dx.hypot(dy);
    if !step.is_finite() || !length.is_finite() {
        return Err(WallGeometryError::NonFiniteGeometry);
    }
    if step <= step_threshold {
        return Ok(None);
    }
    if length == 0.0 {
        return Err(WallGeometryError::CoincidentEndpoints);
    }
    let bottom = start[2].min(end[2]);
    let top = start[2].max(end[2]);
    Ok(Some(WallQuad {
        positions: [
            [start[0], start[1], bottom],
            [end[0], end[1], bottom],
            [end[0], end[1], top],
            [start[0], start[1], top],
        ],
        normal: [dy / length, -dx / length, 0.0],
    }))
}

/// Apply [`cliff_wall`] to a tile's four perimeter edges in counterclockwise
/// order: bottom, right, top, left. Corner order is bottom-left, bottom-right,
/// top-left, top-right, matching [`crate::TileProfile`].
///
/// Adjacent tiles share edges. A whole-grid caller should enumerate each edge
/// once with [`cliff_wall`] instead of concatenating duplicate tile perimeters.
pub fn cliff_walls(
    corners: [[f32; 3]; 4],
    step_threshold: f32,
) -> Result<Vec<WallQuad>, WallGeometryError> {
    let mut walls = Vec::new();
    for [start, end] in [[0, 1], [1, 3], [3, 2], [2, 0]] {
        if let Some(wall) = cliff_wall([corners[start], corners[end]], step_threshold)? {
            walls.push(wall);
        }
    }
    Ok(walls)
}
