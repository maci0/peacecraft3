use super::{Entry, Error, Failure, ObjectHost};
use crate::terrain_pathing::TerrainPathing;
use pcx_script::Value;

const QUERY: &str = "IsTerrainPathable";
const CONVERT: &str = "ConvertPathingType";

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
        self.terrain_pathing = Some(input);
        Ok(self)
    }

    pub(super) fn terrain_pathing_native(
        &self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        let input = self.terrain_pathing.as_ref()?;
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
            let flags = raster
                .cell(x as u32, y as u32)
                .ok_or_else(|| fail("missing authored cell"))?;
            Ok(Value::Bool(flags & input.mask(enumerand) != 0))
        })())
    }
}
