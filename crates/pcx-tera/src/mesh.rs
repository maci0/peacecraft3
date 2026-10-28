//! The triangle mesh itself: plain vectors, deterministic order, no engine types.

use std::collections::BTreeMap;

use pcx_map::Terrain;

use crate::world::{TILE_XY, Z_PER_INDEX, Z_REFERENCE_INDEX};

/// A renderable terrain mesh.
///
/// Vertices are the `(width + 1) * (height + 1)` corners in the terrain file's
/// own row-major order, so **vertex `i` is corner `i`** and
/// `index == cy * (width + 1) + cx`. Every other array is in that same order.
///
/// Each `width * height` tile is two triangles, `(a, b, d)` and `(a, d, c)`
/// where `a = (cx, cy)`, `b = (cx + 1, cy)`, `c = (cx, cy + 1)`,
/// `d = (cx + 1, cy + 1)`. Both wind so their face normal points along `+z`
/// (`+y` in Bevy's frame, after `pcx-gfx` lifts the mesh): with the row *not*
/// flipped, `+x` is east, `+y` is north and `+z` is up.
///
/// Nothing here is engine state, and nothing here iterates a hash map: the
/// output order is the file order, so two runs of [`TerrainMesh::build`] on the
/// same terrain are byte-identical.
#[derive(Debug, Clone, PartialEq)]
pub struct TerrainMesh {
    /// One `[x, y, z]` per corner, `cy * (width + 1) + cx`.
    pub positions: Vec<[f32; 3]>,
    /// One unit normal per corner, finite-differenced from the grid.
    pub normals: Vec<[f32; 3]>,
    /// Grid-space uv per corner: `u = cx`, `v = cy`.
    ///
    /// That is one full texture repeat per terrain tile, which is the tileset's
    /// own pitch — the ground textures are authored one-per-tile, so a shader
    /// samples them with repeat addressing. This is a renderer convention
    /// adopted deliberately, **not** a value read out of any file.
    pub uvs: Vec<[f32; 2]>,
    /// The corner's raw slot nibble: its ground texture slot index. See
    /// [`crate::ground`].
    pub slot_per_vertex: Vec<u8>,
    /// The corner's raw high-nibble flags, bit for bit, uninterpreted.
    pub flags_per_vertex: Vec<u8>,
    /// Triangle-list indices into the arrays above, six per tile.
    pub indices: Vec<u32>,
    /// Terrain width in tiles.
    pub width: u32,
    /// Terrain height in tiles.
    pub height: u32,
}

impl TerrainMesh {
    /// Build with the measured constants ([`TILE_XY`], [`Z_PER_INDEX`],
    /// [`Z_REFERENCE_INDEX`]).
    #[must_use]
    pub fn build(terrain: &Terrain) -> Self {
        Self::build_scaled(terrain, TILE_XY, Z_PER_INDEX, Z_REFERENCE_INDEX)
    }

    /// Build with a caller-supplied world scale.
    ///
    /// Only for a viewer that wants a different convention; the defaults are
    /// the measured ones.
    #[must_use]
    pub fn build_scaled(
        terrain: &Terrain,
        xy_per_tile: f32,
        z_per_index: f32,
        z_reference_index: f32,
    ) -> Self {
        let width = terrain.width();
        let height = terrain.height();
        let (origin_x, origin_y) = terrain.origin();
        let stride = width as usize + 1;

        let mut positions = Vec::with_capacity(terrain.corner_count());
        let mut uvs = Vec::with_capacity(terrain.corner_count());
        let mut slot_per_vertex = Vec::with_capacity(terrain.corner_count());
        let mut flags_per_vertex = Vec::with_capacity(terrain.corner_count());
        // One pass over the corners, in file order, so vertex i is corner i.
        for (i, corner) in terrain.corners().iter().enumerate() {
            let cx = (i % stride) as u32;
            let cy = (i / stride) as u32;
            positions.push([
                origin_x + cx as f32 * xy_per_tile,
                origin_y + cy as f32 * xy_per_tile,
                (corner.height_index as f32 - z_reference_index) * z_per_index
                    + crate::world::layer_offset(corner, xy_per_tile),
            ]);
            uvs.push([cx as f32, cy as f32]);
            slot_per_vertex.push(corner.slot_nibble);
            flags_per_vertex.push(corner.corner_flags);
        }

        let normals = compute_normals(&positions, width as usize, height as usize);

        let mut indices = Vec::with_capacity(width as usize * height as usize * 6);
        for ty in 0..height {
            for tx in 0..width {
                let base = ty as usize * stride + tx as usize;
                let a = base as u32;
                let b = (base + 1) as u32;
                let c = (base + stride) as u32;
                let d = (base + stride + 1) as u32;
                indices.extend_from_slice(&[a, b, d, a, d, c]);
            }
        }

        Self {
            positions,
            normals,
            uvs,
            slot_per_vertex,
            flags_per_vertex,
            indices,
            width,
            height,
        }
    }

    /// Number of vertices: `(width + 1) * (height + 1)`.
    #[must_use]
    pub fn vertex_count(&self) -> usize {
        self.positions.len()
    }

    /// Number of triangles: `2 * width * height`.
    #[must_use]
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// The topology check.
    ///
    /// A terrain sheet is an orientable **2-manifold with boundary**: the
    /// rectangular perimeter is the only boundary, every interior edge is shared
    /// by exactly two triangles facing opposite ways, and no edge is shared by
    /// more than two.
    #[must_use]
    pub fn manifold(&self) -> ManifoldReport {
        ManifoldReport::of(self)
    }

    /// A digest over the whole mesh, for a build-to-build identity check.
    ///
    /// FNV-1a 64 over every float, in order. It is not a format identity (the
    /// terrain's own canonical bytes are that) — it is a "this mesh is the same
    /// mesh as that one" check for a renderer or a sweep.
    #[must_use]
    pub fn digest(&self) -> u64 {
        fn mix(h: u64, bytes: &[u8]) -> u64 {
            let mut h = h;
            for b in bytes {
                h ^= *b as u64;
                h = h.wrapping_mul(0x0100_0000_01b3);
            }
            h
        }
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        h = mix(h, &self.width.to_le_bytes());
        h = mix(h, &self.height.to_le_bytes());
        for p in &self.positions {
            for c in p {
                h = mix(h, &c.to_bits().to_le_bytes());
            }
        }
        for s in &self.slot_per_vertex {
            h = mix(h, &[*s]);
        }
        for i in &self.indices {
            h = mix(h, &i.to_le_bytes());
        }
        h
    }
}

/// The result of [`TerrainMesh::manifold`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManifoldReport {
    /// Largest number of triangles sharing one undirected edge. `2` on a clean
    /// sheet; anything more is a T-junction or a folded surface.
    pub max_shared: usize,
    /// Undirected edges used by exactly two triangles.
    pub interior_edges: usize,
    /// Undirected edges used by exactly one triangle — the map perimeter.
    pub boundary_edges: usize,
    /// `true` when every interior edge is used once in each direction, i.e. the
    /// surface is consistently oriented with no flipped neighbour.
    pub oriented: bool,
}

impl ManifoldReport {
    /// `true` for a clean orientable 2-manifold sheet: no edge used more than
    /// twice, all interior edges oriented the same way, and a real perimeter.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.max_shared == 2 && self.oriented && self.boundary_edges > 0
    }

    fn of(mesh: &TerrainMesh) -> Self {
        let mut undirected: BTreeMap<(u32, u32), usize> = BTreeMap::new();
        let mut directed: BTreeMap<(u32, u32), usize> = BTreeMap::new();
        let mut degenerate = 0usize;
        for tri in mesh.indices.as_chunks::<3>().0 {
            let (a, b, c) = (tri[0], tri[1], tri[2]);
            for &(p, q) in &[(a, b), (b, c), (c, a)] {
                if p == q {
                    degenerate += 1;
                    continue;
                }
                *directed.entry((p, q)).or_insert(0) += 1;
                let key = if p < q { (p, q) } else { (q, p) };
                *undirected.entry(key).or_insert(0) += 1;
            }
        }
        let max_shared = undirected.values().copied().max().unwrap_or(0);
        let interior_edges = undirected.values().filter(|&&n| n == 2).count();
        let boundary_edges = undirected.values().filter(|&&n| n == 1).count();
        // Orientability: an edge shared by exactly two triangles must be
        // traversed once in each direction. A *boundary* edge (count 1) has no
        // partner and is not evidence of a flip, so it must not be required to
        // have a reverse — requiring that made every cut-off boundary node
        // report the mesh as non-orientable while the winding was in fact
        // consistent. No edge may be shared by three or more.
        let oriented = degenerate == 0
            && undirected.values().all(|&n| n <= 2)
            && undirected.iter().all(|(&(p, q), &n)| {
                if n != 2 {
                    return true;
                }
                directed.get(&(p, q)) == Some(&1) && directed.get(&(q, p)) == Some(&1)
            });
        Self {
            max_shared,
            interior_edges,
            boundary_edges,
            oriented,
        }
    }
}

impl std::fmt::Display for ManifoldReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "manifold(max_shared={}, interior={}, boundary={}, oriented={})",
            self.max_shared, self.interior_edges, self.boundary_edges, self.oriented
        )
    }
}

/// One unit normal per corner, from the grid's own finite differences.
///
/// `dx` is the east tangent and `dy` the north tangent, so `cross(dx, dy)`
/// points up. A perfectly flat grid would give `(0, 0, 1)`; the degenerate
/// case (a flat area repeated, which cannot happen on a corner grid with any
/// height variation) is guarded so a caller never sees a zero normal.
fn compute_normals(positions: &[[f32; 3]], width: usize, height: usize) -> Vec<[f32; 3]> {
    let stride = width + 1;
    let at = |x: usize, y: usize| positions[y * stride + x];
    let sub = |a: [f32; 3], b: [f32; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let cross = |a: [f32; 3], b: [f32; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };

    let mut normals = vec![[0.0f32, 0.0, 1.0]; positions.len()];
    for y in 0..=height {
        for x in 0..=width {
            let east = sub(at((x + 1).min(width), y), at(x.saturating_sub(1), y));
            let north = sub(at(x, (y + 1).min(height)), at(x, y.saturating_sub(1)));
            let n = cross(east, north);
            let len2 = n[0] * n[0] + n[1] * n[1] + n[2] * n[2];
            normals[y * stride + x] = if len2 > 0.0 {
                let s = len2.sqrt();
                [n[0] / s, n[1] / s, n[2] / s]
            } else {
                [0.0, 0.0, 1.0]
            };
        }
    }
    normals
}
