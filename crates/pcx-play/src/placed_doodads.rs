//! Authored decorative instances and models, shared by script animation and drawing.
use super::*;
use crate::unit_model::{Animation, Model};

#[derive(Clone, Debug)]
pub struct PlacedDoodad {
    pub source: pcx_map::Doodad,
    pub shown: bool,
    pub selection: RectangleSelection,
    pub appearance: Result<DoodadAppearance, String>,
}

/// How `SetDoodadAnimationRect` tests an instance against its rectangle. Native
/// selector `0x751d90`; evidence in `docs/re-notes/script-doodads.md`.
#[derive(Clone, Debug, PartialEq)]
pub enum RectangleSelection {
    /// Instance record `+0xa0` is not positive: the position is tested with
    /// inclusive rectangle edges.
    Point,
    /// Pathing texture size, record `+0xa0` (TGA width) and `+0xa4` (TGA
    /// height): the footprint box must overlap the rectangle strictly.
    Footprint { width: u16, height: u16 },
    /// The footprint size could not be read; selecting this instance refuses.
    Unresolved(String),
}

/// Half a pathing cell in world units: `0x10dd5b8` (32.0) times `0xe6957c` (0.5).
const HALF_PATHING_CELL: f32 = 16.;
/// A quarter turn, `0xe9231c`, and a full turn, `0xe92324` (f32 bit patterns
/// `0x3fc90fdb` and `0x40c90fdb`).
const QUARTER_TURN: f32 = std::f32::consts::FRAC_PI_2;
const FULL_TURN: f32 = std::f32::consts::TAU;
/// Bound on the native `while facing < 0 { facing += 2 pi }` loop, which does
/// not terminate for huge negative angles.
const MAX_TURN_STEPS: u32 = 1 << 16;

impl PlacedDoodad {
    /// The native selection test against `[min x, min y, max x, max y]`.
    pub fn rectangle_selects(&self, bounds: [f32; 4]) -> Result<bool, String> {
        let [min_x, min_y, max_x, max_y] = bounds;
        let [x, y, _] = self.source.position;
        let (width, height) = match &self.selection {
            RectangleSelection::Point => {
                return Ok((min_x..=max_x).contains(&x) && (min_y..=max_y).contains(&y));
            }
            RectangleSelection::Unresolved(reason) => return Err(reason.clone()),
            RectangleSelection::Footprint { width, height } => (*width, *height),
        };
        if width == 0 {
            return Ok((min_x..=max_x).contains(&x) && (min_y..=max_y).contains(&y));
        }
        let mut facing = self.source.angle;
        if !facing.is_finite() {
            return Err("non-finite doodad facing".into());
        }
        let mut steps = 0;
        while facing < 0. {
            steps += 1;
            if steps > MAX_TURN_STEPS || facing + FULL_TURN == facing {
                return Err("doodad facing outside the normalizable range".into());
            }
            facing += FULL_TURN;
        }
        let turns = facing / QUARTER_TURN + 0.5;
        // x86 `cvttss2si` yields i32::MIN when the value does not fit.
        let quarter = if turns.abs() < i32::MAX as f32 {
            turns as i32
        } else {
            i32::MIN
        };
        let (mut half_x, mut half_y) = (
            f32::from(width) * HALF_PATHING_CELL,
            f32::from(height) * HALF_PATHING_CELL,
        );
        if quarter & 1 != 0 {
            std::mem::swap(&mut half_x, &mut half_y);
        }
        let low_x = (x - half_x).max(min_x);
        let high_x = (x + half_x).min(max_x);
        let low_y = (y - half_y).max(min_y);
        let high_y = (y + half_y).min(max_y);
        Ok(high_x > low_x && high_y > low_y)
    }
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
        let mut footprints = BTreeMap::<String, Result<(u16, u16), String>>::new();
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
            let selection = self.selection(&tables, reader, instance, &mut footprints);
            rows.push(PlacedDoodad {
                source: *instance,
                // shortcut: documented low flag states drive visibility; extra flag bits still need a native render comparison.
                shown: instance.flags & 3 != 0,
                selection,
                appearance,
            });
        }
        Ok(rows)
    }
}

impl Catalog {
    /// Record `+0xa0`/`+0xa4`: `0x753240` takes the type's pathing texture path
    /// (`0x75b150`, `pathTex`; empty, `none` and `_` mean no footprint), strips
    /// its extension and reads the size of `<stem>.tga` (`0x71abc0`, `0x71a7c0`).
    fn selection(
        &self,
        tables: &DefaultTables,
        reader: &Resolver,
        instance: &pcx_map::Doodad,
        sizes: &mut BTreeMap<String, Result<(u16, u16), String>>,
    ) -> RectangleSelection {
        let path = match self.path(tables, instance.type_id, false) {
            Ok(path) => path,
            Err(error) => return RectangleSelection::Unresolved(error),
        };
        if matches!(path.as_str(), "" | "_" | "none") {
            return RectangleSelection::Point;
        }
        let size = sizes
            .entry(path.clone())
            .or_insert_with(|| footprint_size(reader, &path))
            .clone();
        match size {
            Ok((width, height)) => RectangleSelection::Footprint { width, height },
            Err(error) => RectangleSelection::Unresolved(error),
        }
    }
}

/// The member the native size reader opens: the path up to its last `.` after
/// the final separator (`0x71abc0`), then `.tga`.
fn footprint_member(path: &str) -> String {
    let stem = path
        .rfind(['.', '\\', '/'])
        .filter(|&at| path.as_bytes()[at] == b'.')
        .map_or(path, |at| &path[..at]);
    format!("{stem}.tga")
}

fn footprint_size(reader: &Resolver, path: &str) -> Result<(u16, u16), String> {
    let member = footprint_member(path);
    let asset = reader
        .resolve_bounded(&member, FOOTPRINT_READ_BYTES)
        .map_err(|e| format!("doodad pathing texture {member}: {e}"))?;
    let header = pcx_tga::Header::parse(&asset.bytes)
        .map_err(|e| format!("doodad pathing texture {member}: {e}"))?;
    Ok((header.width, header.height))
}

/// Upper bound for reading one pathing texture, the limit placement uses.
const FOOTPRINT_READ_BYTES: u64 = 1 << 20;

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
        let repeat_max = repeat
            .as_ref()
            .and_then(|n| n.text.parse::<u32>().ok())
            .unwrap_or(0);
        if repeated
            && mods.iter().any(|m| {
                is_repeated_edit(m, def.fourcc) && repeated_variation(m, repeat_max).is_none()
            })
        {
            return Err(format!("{name}: unsupported doodad {column} selector"));
        }
        let edit = mods.iter().rev().find(|m| {
            if repeated {
                is_repeated_edit(m, def.fourcc) && repeated_variation(m, repeat_max) == Some(level)
            } else {
                m.field == def.fourcc
            }
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

/// Version-1 doodad edits spell the variation as the last character of the
/// 4-character id, so the tenth variation is `0` (`dvr0`, after `dvr9`).
const TENTH_VARIATION_SUFFIX: u8 = b'0';
const TENTH_VARIATION: u32 = 10;

/// Whether `edit` belongs to a repeated column whose metadata row is `field`.
/// Indexed edits must carry the exact id; version-1 edits share its first three
/// characters and differ in the variation suffix.
fn is_repeated_edit(edit: &pcx_objects::Mod, field: [u8; 4]) -> bool {
    match edit.selectors {
        Some(_) => edit.field == field,
        None => edit.field[..3] == field[..3],
    }
}

/// The variation a repeated-column edit names, within `1..=repeat`. Indexed edits
/// name it as their first selector with data `0`.
fn repeated_variation(edit: &pcx_objects::Mod, repeat: u32) -> Option<u32> {
    let variation = match edit.selectors {
        Some([variation, 0]) => variation,
        Some(_) => return None,
        None => match edit.field[3] {
            digit @ b'1'..=b'9' => u32::from(digit - b'0'),
            TENTH_VARIATION_SUFFIX => TENTH_VARIATION,
            _ => return None,
        },
    };
    (1..=repeat).contains(&variation).then_some(variation)
}

pub(crate) fn model_member<'a>(
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
    // The exact variation name wins; the direct name is only a fallback for
    // single-variation decorations shipped without suffixed files. Shipped
    // archives carry both spellings for at least one single-variation
    // doodad, and the original loads the map, so an ambiguous pair must
    // resolve instead of refusing; which spelling the original prefers when
    // both exist is unmeasured.
    let mut fallback = None;
    for candidate in paths {
        if candidate == varied {
            return Ok(varied);
        }
        if fallback.is_none() && count == 1 && variation == 0 && candidate == direct {
            fallback = Some(direct.clone());
        }
    }
    fallback.ok_or_else(|| format!("{file}: no source model for variation {variation}"))
}

#[cfg(test)]
#[path = "placed_doodads_tests.rs"]
mod tests;
