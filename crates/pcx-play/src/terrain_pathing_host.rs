use super::{Entry, Error, Failure, ObjectHost};
use crate::terrain_pathing::{CliffOverlay, TerrainPathing};
use pcx_script::Value;

const QUERY: &str = "IsTerrainPathable";
const CONVERT: &str = "ConvertPathingType";
const CLIFF_QUERY: &str = "GetTerrainCliffLevel";

impl ObjectHost {
    /// Uses this session's already admitted map raster. It neither updates that
    /// raster nor substitutes static terrain flags for live unit occupancy.
    pub fn with_terrain_pathing(mut self, input: TerrainPathing) -> Result<Self, Error> {
        let fail = |reason| self.inner.error(QUERY, Failure::TerrainPathing(reason));
        if self.terrain_pathing.is_some() {
            return Err(fail("already admitted"));
        }
        Self::contract(
            &self.inner,
            QUERY,
            &["real", "real", "pathingtype"],
            "boolean",
            false,
            None,
        )?;
        Self::contract(
            &self.inner,
            CONVERT,
            &["integer"],
            "pathingtype",
            true,
            None,
        )?;
        let conversion = self
            .inner
            .conversions
            .get(CONVERT)
            .filter(|c| c.kind == "pathingtype")
            .ok_or_else(|| fail("source conversion is not admitted"))?;
        let query = self
            .inner
            .program
            .natives()
            .get(QUERY)
            .ok_or_else(|| self.inner.error(QUERY, Failure::Signature))?;
        if query.source != conversion.source {
            return Err(self.inner.error(QUERY, Failure::Source));
        }
        let context = self
            .player_context()
            .ok_or_else(|| fail("admitted map context required"))?;
        if !std::sync::Arc::ptr_eq(&self.inner.program, &context.scripts.shared_program()) {
            return Err(fail("Program differs from admitted map"));
        }
        let (x, y) = context.terrain.origin();
        if !x.is_finite() || !y.is_finite() {
            return Err(fail("finite map origin required"));
        }
        // Cliff level reads the authored W3E corner layer (`byte6` low nibble)
        // at the nearest corner, combined with the maximum over the authored
        // spatial-rect overlay the selected `0x7044b0` lookup also samples.
        // The overlay is populated from the admitted terrain below: one rect
        // per tile whose corners disagree on the layer. Dynamic runtime
        // registrations behind that layer are unestablished in either original
        // probe, so only the authored tiles participate here. See
        // `docs/re-notes/ai-construction.md` and
        // `docs/re-notes/script-unit-world.md`.
        if self.inner.program.natives().get(CLIFF_QUERY).is_some() {
            Self::contract(
                &self.inner,
                CLIFF_QUERY,
                &["real", "real"],
                "integer",
                false,
                None,
            )?;
            let cliff = self
                .inner
                .program
                .natives()
                .get(CLIFF_QUERY)
                .expect("checked cliff contract");
            if cliff.source != query.source {
                return Err(self.inner.error(CLIFF_QUERY, Failure::Source));
            }
        }
        let overlay = CliffOverlay::of_terrain(&context.terrain).map_err(fail)?;
        self.terrain_pathing = Some(input);
        self.cliff_overlay = Some(overlay);
        Ok(self)
    }

    /// Authored cliff overlay populating the spatial-rect layer, if admitted.
    pub fn cliff_overlay(&self) -> Option<&CliffOverlay> {
        self.cliff_overlay.as_ref()
    }

    pub(super) fn terrain_pathing_native(
        &self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        let input = self.terrain_pathing.as_ref()?;
        if name == CLIFF_QUERY {
            self.inner.program.natives().get(CLIFF_QUERY)?;
            return Some(self.cliff_native(args));
        }
        if name != QUERY {
            return None;
        }
        Some((|| {
            let fail = |reason| self.inner.error(name, Failure::TerrainPathing(reason));
            let [x, y, Value::Handle(kind)] = args else {
                return Err(fail("two finite coordinates and a pathingtype required"));
            };
            let (Some(x), Some(y)) = (
                x.as_real().filter(|v| v.is_finite()),
                y.as_real().filter(|v| v.is_finite()),
            ) else {
                return Err(fail("two finite coordinates required"));
            };
            let enumerand = match self.inner.entry(*kind)? {
                Entry::Symbolic { kind, enumerand } if kind == "pathingtype" => *enumerand,
                _ => return Err(self.inner.error(name, Failure::WrongFamily)),
            };
            Ok(Value::Bool(
                self.terrain_cell_flags(name, input, x, y)? & input.mask(enumerand) != 0,
            ))
        })())
    }

    /// The raster flags of the cell that holds the point.
    fn terrain_cell_flags(
        &self,
        name: &str,
        input: &TerrainPathing,
        x: f32,
        y: f32,
    ) -> Result<u8, Error> {
        let fail = |reason| self.inner.error(name, Failure::TerrainPathing(reason));
        let context = self
            .player_context()
            .ok_or_else(|| fail("admitted map context required"))?;
        let (ox, oy) = context.terrain.origin();
        let cell = f64::from(input.cell_size());
        let coordinate = |value, origin| {
            use crate::script_random::real;
            let delta = real::sub(real::from_f32(value)?, real::from_f32(origin)?)?;
            Ok::<_, real::MathError>((f64::from(f32::from_bits(delta)) / cell).floor())
        };
        let x = coordinate(x, ox).map_err(|_| fail("unsupported coordinate arithmetic"))?;
        let y = coordinate(y, oy).map_err(|_| fail("unsupported coordinate arithmetic"))?;
        let raster = &context.world.pathing;
        // shortcut: out-of-raster native behavior is unqualified; admit it
        // only after explicit boundary observations rather than clamping.
        if x < 0.0 || y < 0.0 || x >= f64::from(raster.width) || y >= f64::from(raster.height) {
            return Err(fail("out-of-raster query is unsupported"));
        }
        raster
            .cell(x as u32, y as u32)
            .ok_or_else(|| fail("missing authored cell"))
    }

    /// Whether the walkability mask covers the cell that holds `point`: the static terrain part
    /// of the test `0x7f6a00` makes with mask 2 (`IsTerrainPathable` for walkability). The
    /// dynamic spatial entries the client also walks are not read.
    pub(crate) fn terrain_unwalkable(&self, point: [f32; 2]) -> Result<bool, Error> {
        const NAME: &str = "terrain walkability";
        let input = self.terrain_pathing.as_ref().ok_or_else(|| {
            self.inner.error(
                NAME,
                Failure::TerrainPathing("terrain pathing is not admitted"),
            )
        })?;
        let walkability =
            self.source_constant_enumerand("PATHING_TYPE_WALKABILITY", "pathingtype")?;
        Ok(
            self.terrain_cell_flags(NAME, input, point[0], point[1])? & input.mask(walkability)
                != 0,
        )
    }

    /// Authored corner elevation layer at the nearest tile corner, combined
    /// with the maximum over the authored spatial-rect overlay covering the
    /// tile that contains the query point. Uniform tiles carry no overlay
    /// rect, so flat ground still reads the corner alone. Tile size is the
    /// admitted pathing cell size scaled by the map-derived
    /// pathing-per-tile ratio; no world scale is invented.
    fn cliff_native(&self, args: &[Value]) -> Result<Value, Error> {
        let name = CLIFF_QUERY;
        let fail = |reason| self.inner.error(name, Failure::TerrainPathing(reason));
        let [x, y] = args else {
            return Err(fail("two finite coordinates required"));
        };
        let (Some(x), Some(y)) = (
            x.as_real().filter(|v| v.is_finite()),
            y.as_real().filter(|v| v.is_finite()),
        ) else {
            return Err(fail("two finite coordinates required"));
        };
        let input = self
            .terrain_pathing
            .as_ref()
            .ok_or_else(|| fail("terrain pathing not admitted"))?;
        let overlay = self
            .cliff_overlay
            .as_ref()
            .ok_or_else(|| fail("cliff overlay not admitted"))?;
        let context = self
            .player_context()
            .ok_or_else(|| fail("admitted map context required"))?;
        let (ox, oy) = context.terrain.origin();
        let tile = f64::from(input.cell_size()) * f64::from(context.alignment.uniform_scale);
        if !tile.is_finite() || tile <= 0.0 {
            return Err(fail("unusable tile scale"));
        }
        let coordinate = |value: f32, origin: f32, round: bool| {
            use crate::script_random::real;
            let delta = real::sub(real::from_f32(value)?, real::from_f32(origin)?)?;
            let cell = f64::from(f32::from_bits(delta)) / tile;
            Ok::<_, real::MathError>(if round { cell.round() } else { cell.floor() })
        };
        let nearest = |value: f32, origin: f32| {
            coordinate(value, origin, true).map_err(|_| fail("unsupported coordinate arithmetic"))
        };
        let containing = |value: f32, origin: f32| {
            coordinate(value, origin, false).map_err(|_| fail("unsupported coordinate arithmetic"))
        };
        let (nx, ny) = (nearest(x, ox)?, nearest(y, oy)?);
        let (fx, fy) = (containing(x, ox)?, containing(y, oy)?);
        let width = context.terrain.width();
        let height = context.terrain.height();
        if nx < 0.0 || ny < 0.0 || nx > f64::from(width) || ny > f64::from(height) {
            return Err(fail("out-of-raster query is unsupported"));
        }
        let corner = context
            .terrain
            .corner(nx as u32, ny as u32)
            .ok_or_else(|| fail("missing authored corner"))?;
        let mut level = corner.layer_height();
        // Tile-area fidelity beyond the nearest corner: the overlay rect of
        // the containing tile carries that tile's maximum layer, matching the
        // native maximum over the spatial-rect layer.
        if fx >= 0.0 && fy >= 0.0 {
            let (fx, fy) = (fx as u32, fy as u32);
            if fx < width
                && fy < height
                && let Some(tile_level) = overlay.level_at_tile([fx, fy])
            {
                level = level.max(tile_level);
            }
        }
        Ok(Value::Int(i32::from(level)))
    }
}
