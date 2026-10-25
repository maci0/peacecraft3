//! Script and renderer share the placed decorative instances; no script handles are invented.
use super::{Error, Failure, ObjectHost};
use crate::campaign_placement::PlacedDoodad;
use pcx_script::Value;

const NATIVE: &str = "SetDoodadAnimationRect";

impl ObjectHost {
    pub fn admit_placed_doodads(&mut self, rows: Vec<PlacedDoodad>) -> Result<(), Error> {
        if self.doodads.is_some() || self.timer_game_time().is_none() || rows.len() > 100000 {
            return Err(self.inner.error(
                NATIVE,
                Failure::Doodad("duplicate admission, absent clock or instance bound".into()),
            ));
        }
        if self.inner.program.natives().get(NATIVE).is_some() {
            Self::contract(
                &self.inner,
                NATIVE,
                &["rect", "integer", "string", "boolean"],
                "nothing",
                false,
                None,
            )?;
            if !self.rectangles
                || self.inner.program.natives().get(NATIVE).map(|n| &n.source)
                    != self.inner.program.natives().get("Rect").map(|n| &n.source)
            {
                return Err(self.inner.error(NATIVE, Failure::Source));
            }
        }
        let mut ids = std::collections::BTreeSet::new();
        if rows
            .iter()
            .any(|row| !ids.insert(row.source.creation_number))
        {
            return Err(self.inner.error(
                NATIVE,
                Failure::Doodad("duplicate placed creation number".into()),
            ));
        }
        self.doodads = Some(rows);
        Ok(())
    }

    pub fn placed_doodads(&self) -> impl Iterator<Item = &PlacedDoodad> {
        self.doodads.iter().flatten()
    }

    pub(super) fn doodad_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        self.doodads.as_ref()?;
        if name != NATIVE {
            return None;
        }
        Some((|| {
            let [
                rect,
                Value::Int(kind),
                Value::Str(clip),
                Value::Bool(random),
            ] = args
            else {
                return Err(self.inner.error(name, Failure::Signature));
            };
            let rect = match rect {
                Value::Handle(rect) => *rect,
                Value::Null => return Ok(Value::Null),
                _ => return Err(self.inner.error(name, Failure::Signature)),
            };
            if ["soundon", "soundoff"]
                .iter()
                .any(|word| clip.eq_ignore_ascii_case(word))
            {
                return Err(self.inner.error(
                    name,
                    Failure::Doodad("doodad loop-sound control is not admitted".into()),
                ));
            }
            if *random {
                return Err(self.inner.error(
                    name,
                    Failure::Doodad("randomized doodad animation is not admitted".into()),
                ));
            }
            if clip.len() > 16 << 10 || clip.chars().any(char::is_control) {
                return Err(self
                    .inner
                    .error(name, Failure::Doodad("invalid animation name".into())));
            }
            let [x0, y0, x1, y1] = self.rectangle_bounds(rect)?;
            let now = self.timer_game_time().expect("admitted clock");
            let rows = self.doodads.as_ref().expect("admitted");
            let mut changes = Vec::new();
            changes.try_reserve(rows.len()).map_err(|_| {
                self.inner
                    .error(name, Failure::Doodad("animation batch allocation".into()))
            })?;
            for (i, row) in rows.iter().enumerate() {
                let selected_type = *kind == -1 || row.source.type_id == kind.to_be_bytes();
                // shortcut: footprint-overlap selection refuses until its source rectangle mapping is admitted.
                if selected_type && !row.point_selection {
                    return Err(self.inner.error(
                        name,
                        Failure::Doodad(
                            "footprint-overlap doodad rectangle selection is not admitted".into(),
                        ),
                    ));
                }
                let [x, y, _] = row.source.position;
                if selected_type && (x0..=x1).contains(&x) && (y0..=y1).contains(&y) {
                    if clip.eq_ignore_ascii_case("show") || clip.eq_ignore_ascii_case("hide") {
                        changes.push((i, None, Some(clip.eq_ignore_ascii_case("show"))));
                        continue;
                    }
                    let mut appearance = row
                        .appearance
                        .clone()
                        .map_err(|e| self.inner.error(name, Failure::Doodad(e)))?;
                    appearance
                        .animation
                        .select(clip, now, None)
                        .map_err(|e| self.inner.error(name, Failure::Doodad(e)))?;
                    changes.push((i, Some(appearance), None));
                }
            }
            for (i, appearance, shown) in changes {
                let row = &mut self.doodads.as_mut().expect("admitted")[i];
                if let Some(appearance) = appearance {
                    row.appearance = Ok(appearance);
                }
                if let Some(shown) = shown {
                    row.shown = shown;
                }
            }
            Ok(Value::Null)
        })())
    }
}

#[cfg(test)]
#[path = "doodad_host_tests.rs"]
mod tests;
