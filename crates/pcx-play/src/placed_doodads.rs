//! Authored decorative instances and models, shared by script animation and drawing.
use super::*;
use crate::unit_model::{Animation, Model};

#[derive(Clone, Debug)]
pub struct PlacedDoodad {
    pub source: pcx_map::Doodad,
    pub shown: bool,
    pub point_selection: bool,
    pub appearance: Result<DoodadAppearance, String>,
}

#[derive(Clone, Debug)]
pub struct DoodadAppearance {
    /// Source colour-channel values; conversion uses the admitted client colour scale.
    pub tint: [f32; 3],
    pub animation: Animation,
}

impl Catalog {
    pub fn decorative_models(
        &self,
        reader: &Resolver,
        edition: &str,
        now: f64,
    ) -> Result<Vec<PlacedDoodad>, String> {
        let tables = self.objects.tables();
        let mut rows = Vec::new();
        rows.try_reserve_exact(self.doodads.len())
            .map_err(|_| "doodad instance allocation")?;
        let mut models = BTreeMap::<String, Arc<Model>>::new();
        let mut remaining = 128usize << 20;
        for instance in &self.doodads {
            if self.role(instance)? != PlacedRole::Decoration {
                continue;
            }
            let appearance = (|| {
                let (member, tint) = appearance_source(
                    &tables,
                    &self.decorations,
                    instance,
                    reader.paths().map(|p| p.as_str()),
                )?;
                let model = if let Some(model) = models.get(&member) {
                    Arc::clone(model)
                } else {
                    if models.len() >= 512 {
                        return Err("doodad model count budget".into());
                    }
                    let asset = reader
                        .resolve_bounded(&member, remaining.min(16 << 20) as u64)
                        .map_err(|e| format!("{member}: {e}"))?;
                    let (_, origin) = crate::campaign_assets::identity(&asset, edition, 16 << 10)?;
                    let size = asset.bytes.len();
                    let model = Arc::new(Model::read(
                        origin,
                        member.clone(),
                        asset.bytes.into(),
                        16 << 20,
                    )?);
                    remaining = remaining
                        .checked_sub(size)
                        .ok_or("doodad model aggregate byte budget")?;
                    models.insert(member, Arc::clone(&model));
                    model
                };
                Ok(DoodadAppearance {
                    tint,
                    animation: Animation::new(model, now)?,
                })
            })();
            let point_selection = self
                .path(&tables, instance.type_id, instance.life == 0)
                .is_ok_and(|p| matches!(p.as_str(), "" | "_" | "none"));
            rows.push(PlacedDoodad {
                source: *instance,
                // shortcut: documented low flag states drive visibility; extra flag bits still need a native render comparison.
                shown: instance.flags & 3 != 0,
                point_selection,
                appearance,
            });
        }
        Ok(rows)
    }
}

fn appearance_source<'a>(
    tables: &DefaultTables,
    overlay: &Overlay,
    instance: &pcx_map::Doodad,
    paths: impl Iterator<Item = &'a str>,
) -> Result<(String, [f32; 3]), String> {
    if instance
        .position
        .iter()
        .chain(&instance.scale)
        .any(|v| !v.is_finite())
        || instance.scale.iter().any(|v| *v <= 0.)
        || !instance.angle.is_finite()
    {
        return Err("invalid placed doodad pose".into());
    }
    let name = std::str::from_utf8(&instance.type_id).map_err(|_| "non-UTF8 doodad id")?;
    let (base, mods) = overlay.new_object(name).unwrap_or((name, overlay.on(name)));
    if overlay.new_object(base).is_some() || mods.len() > 4096 {
        return Err(format!("{name}: doodad base/modification bound"));
    }
    let object = tables
        .object(Family::Doodad, base)
        .map_err(|e| e.to_string())?;
    if object.rawcode() != base {
        return Err(format!("{name}: ambiguous doodad rawcode case"));
    }
    let field = |column: &str, repeated: bool| -> Result<String, String> {
        let mut defs = tables.editor().fields().iter().filter(|f| f.from == SlkId::DoodadMetaData
            && matches!(&f.source, FieldSource::Column { slk: SlkId::Doodads, column: c } if c == column));
        let def = defs
            .next()
            .ok_or_else(|| format!("{name}: missing doodad {column} metadata"))?;
        if defs.next().is_some()
            || (!repeated && !def.measured)
            || def.index != 0
            || def.kind
                != if column == "file" {
                    FieldKind::Token
                } else {
                    FieldKind::Int
                }
        {
            return Err(format!("{name}: unsupported doodad {column} metadata"));
        }
        let level = if repeated {
            instance
                .variation
                .checked_add(1)
                .ok_or("doodad variation overflow")?
        } else {
            0
        };
        let repeat = tables
            .metadata(def.fourcc)
            .and_then(|m| m.attribute("repeat"));
        if repeated
            && !repeat
                .as_ref()
                .is_some_and(|n| n.text.parse::<u32>().is_ok_and(|n| level <= n))
        {
            return Err(format!(
                "{name}: doodad tint variation exceeds metadata repeat"
            ));
        }
        if repeated
            && mods.iter().any(|m| {
                m.field == def.fourcc
                    && !m.selectors.is_some_and(|[n, data]| {
                        data == 0
                            && n > 0
                            && repeat
                                .as_ref()
                                .is_some_and(|r| r.text.parse::<u32>().is_ok_and(|max| n <= max))
                    })
            })
        {
            return Err(format!("{name}: unsupported doodad {column} selector"));
        }
        let edit = mods.iter().rev().find(|m| {
            m.field == def.fourcc && (!repeated || m.selectors.is_some_and(|s| s == [level, 0]))
        });
        if let Some(edit) = edit {
            if (!repeated && edit.selectors.is_some_and(|s| s != [0, 0]))
                || edit.value_type.word() != def.kind.wire_word()
            {
                return Err(format!("{name}: unsupported doodad {column} edit"));
            }
            return match (&edit.value, def.kind) {
                (pcx_objects::Value::Str(s), FieldKind::Token) => std::str::from_utf8(s)
                    .map(str::to_owned)
                    .map_err(|e| e.to_string()),
                (pcx_objects::Value::Int(n), FieldKind::Int) => Ok((*n as i32).to_string()),
                _ => Err(format!("{name}: invalid doodad {column} edit type")),
            };
        }
        let column = if repeated {
            let mut columns =
                object
                    .cells()
                    .into_iter()
                    .filter_map(|c| match c.provenance.location {
                        pcx_objects::editor::defaults::Location::Slk { column: c, .. }
                            if c.strip_prefix(column)
                                .is_some_and(|n| n.parse::<u32>() == Ok(level)) =>
                        {
                            Some(c)
                        }
                        _ => None,
                    });
            let c = columns
                .next()
                .ok_or_else(|| format!("{name}: missing {column} variation {level}"))?;
            if columns.next().is_some() {
                return Err(format!("{name}: ambiguous {column} variation {level}"));
            }
            c
        } else {
            column.to_owned()
        };
        object
            .cell(SlkId::Doodads, &column)
            .map(|c| c.text.to_owned())
            .map_err(|e| e.to_string())
    };
    let file = field("file", false)?;
    let count = field("numVar", false)?
        .parse::<u32>()
        .map_err(|_| "invalid doodad numVar")?;
    if instance.variation >= count {
        return Err(format!("{name}: placed variation outside numVar"));
    }
    let mut tint = [0.; 3];
    for (out, column) in tint.iter_mut().zip(["vertR", "vertG", "vertB"]) {
        *out = field(column, true)?
            .parse::<f32>()
            .map_err(|_| "invalid doodad colour")?;
        if !out.is_finite() || *out < 0. {
            return Err("nonfinite/negative doodad colour".into());
        }
    }
    Ok((model_member(&file, instance.variation, count, paths)?, tint))
}

fn model_member<'a>(
    file: &str,
    variation: u32,
    count: u32,
    paths: impl Iterator<Item = &'a str>,
) -> Result<String, String> {
    let binary = pcx_mdl::binary_model_path(file);
    let path = AssetPath::new(&binary).map_err(|e| e.to_string())?;
    let stem = path.as_str().strip_suffix(".mdx").unwrap_or(path.as_str());
    let varied = format!("{stem}{variation}.mdx");
    let direct = format!("{stem}.mdx");
    let mut found =
        paths.filter(|p| *p == varied || (count == 1 && variation == 0 && *p == direct));
    let member = found
        .next()
        .ok_or_else(|| format!("{file}: no source model for variation {variation}"))?;
    if found.next().is_some() {
        return Err(format!("{file}: ambiguous direct/varied model"));
    }
    Ok(member.to_owned())
}

#[cfg(test)]
#[path = "placed_doodads_tests.rs"]
mod tests;
