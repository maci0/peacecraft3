//! Ground shadow images named by UnitUI, without stock defaults.
//!
//! `unitShadow` names a soft image placed through `shadowW`, `shadowH`, `shadowX`
//! and `shadowY`; `buildingShadow` names a cell mask stamped at the building.
//! Both resolve to `ReplaceableTextures\Shadows\<name>.blp`. The placement
//! rules below are recreation choices, not decoded native behaviour; the
//! evidence for each is in `docs/re-notes/campaign-unit-shadows.md`.
use pcx_objects::editor::FieldKind;
use pcx_sim::world::{Definition, FieldValue};

/// Directory holding every shadow image in the shipped archives.
const TEXTURE_DIR: &str = "ReplaceableTextures\\Shadows";

/// A `unitShadow` image and its authored placement, in world units.
#[derive(Debug, Clone, PartialEq)]
pub struct UnitImage {
    pub texture: String,
    pub width: f32,
    pub height: f32,
    /// Distance from the image's lower-left corner to the unit origin.
    pub origin_x: f32,
    pub origin_y: f32,
}

/// The shadow images one unit definition names.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Shadows {
    pub unit: Option<UnitImage>,
    /// Texture path of the `buildingShadow` cell mask.
    pub building: Option<String>,
}

/// World rectangle covered by one shadow image, and how its rows run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Footprint {
    pub min: [f32; 2],
    pub max: [f32; 2],
    /// Image row 0 is the rectangle's south edge instead of its north edge.
    pub row_zero_south: bool,
}

impl UnitImage {
    /// Recreation choice: the origin sits `origin_x`, `origin_y` from the
    /// image's lower-left corner, and row 0 is the north edge.
    pub fn footprint(&self, position: [f32; 2]) -> Footprint {
        let min = [position[0] - self.origin_x, position[1] - self.origin_y];
        Footprint {
            min,
            max: [min[0] + self.width, min[1] + self.height],
            row_zero_south: false,
        }
    }
}

/// Recreation choice: a mask texel is one shadow-map cell of `cell` world
/// units, the mask is centred on the building origin and row 0 is the south
/// edge, as in `war3map.shd`.
pub fn building_footprint(position: [f32; 2], texels: [u32; 2], cell: f32) -> Footprint {
    let half = texels.map(|n| n as f32 * cell / 2.0);
    Footprint {
        min: [position[0] - half[0], position[1] - half[1]],
        max: [position[0] + half[0], position[1] + half[1]],
        row_zero_south: true,
    }
}

impl Footprint {
    /// Centre and side of the smallest axis-aligned square containing the
    /// rectangle.
    pub fn enclosing_square(&self) -> ([f32; 2], f32) {
        let size = [self.max[0] - self.min[0], self.max[1] - self.min[1]];
        (
            [
                (self.min[0] + self.max[0]) / 2.0,
                (self.min[1] + self.max[1]) / 2.0,
            ],
            size[0].max(size[1]),
        )
    }

    /// Texture coordinate of a world point; outside `0..=1` beyond the rectangle.
    pub fn uv(&self, point: [f32; 2]) -> [f32; 2] {
        let u = (point[0] - self.min[0]) / (self.max[0] - self.min[0]);
        let from_south = (point[1] - self.min[1]) / (self.max[1] - self.min[1]);
        [
            u,
            if self.row_zero_south {
                from_south
            } else {
                1.0 - from_south
            },
        ]
    }
}

/// The unit's UnitUI shadow images; an authored empty marker names none.
pub fn read(definition: &Definition) -> Result<Shadows, String> {
    let unit = crate::unit_model::unit_ui_text(definition, "unitShadow")?
        .map(|name| -> Result<_, String> {
            let width = positive(real(definition, "shadowW")?, "shadowW")?;
            let height = positive(real(definition, "shadowH")?, "shadowH")?;
            Ok(UnitImage {
                texture: texture_path(&name)?,
                width,
                height,
                origin_x: real(definition, "shadowX")?,
                origin_y: real(definition, "shadowY")?,
            })
        })
        .transpose()?;
    let building = crate::unit_model::unit_ui_text(definition, "buildingShadow")?
        .map(|name| texture_path(&name))
        .transpose()?;
    Ok(Shadows { unit, building })
}

fn positive(value: f32, column: &str) -> Result<f32, String> {
    if value > 0.0 {
        Ok(value)
    } else {
        Err(format!("UnitUI/{column} must be positive"))
    }
}

fn texture_path(name: &str) -> Result<String, String> {
    if name.contains(['\\', '/', ',', '.']) || name.chars().any(char::is_control) {
        return Err(format!("unsupported shadow image name {name:?}"));
    }
    let path = format!("{TEXTURE_DIR}\\{name}.blp");
    pcx_assets::AssetPath::new(&path).map_err(|e| e.to_string())?;
    Ok(path)
}

/// A finite `real` UnitUI column through the selected metadata, including map edits.
fn real(definition: &Definition, column: &str) -> Result<f32, String> {
    let field = crate::unit_model::unit_ui_field(definition, column, FieldKind::Real)?;
    let value = match &field.value {
        Ok(FieldValue::Cell(text)) => text
            .trim()
            .parse::<f32>()
            .map_err(|_| format!("invalid UnitUI/{column} {text:?}"))?,
        Ok(FieldValue::Override(pcx_objects::Value::Real(v))) => *v,
        _ => return Err(format!("unsupported UnitUI/{column} value")),
    };
    if value.is_finite() {
        Ok(value)
    } else {
        Err(format!("nonfinite UnitUI/{column}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pcx_objects::editor::{FieldDef, FieldSource, ObjectKind, SlkId};
    use pcx_sim::world::ObjectField;
    use std::collections::BTreeMap;

    fn field(id: [u8; 4], column: &str, kind: FieldKind, value: FieldValue) -> ObjectField {
        ObjectField {
            metadata: Some(FieldDef {
                fourcc: id,
                from: SlkId::UnitMetaData,
                field: column.into(),
                slk: "UnitUI".into(),
                index: -1,
                declared: "synthetic".into(),
                kind,
                category: "art".into(),
                source: FieldSource::Column {
                    slk: SlkId::UnitUi,
                    column: column.into(),
                },
                measured: true,
            }),
            value: Ok(value),
        }
    }

    fn cell(text: &str) -> FieldValue {
        FieldValue::Cell(text.into())
    }

    fn definition(unit: &str, building: &str, numbers: [&str; 4]) -> Definition {
        let [w, h, x, y] = numbers;
        Definition {
            rawcode: *b"U001",
            base_rawcode: "U001".into(),
            kind: ObjectKind::Unit,
            fields: BTreeMap::from([
                (
                    *b"s001",
                    field(*b"s001", "unitShadow", FieldKind::Token, cell(unit)),
                ),
                (
                    *b"s002",
                    field(*b"s002", "buildingShadow", FieldKind::Token, cell(building)),
                ),
                (
                    *b"s003",
                    field(*b"s003", "shadowW", FieldKind::Real, cell(w)),
                ),
                (
                    *b"s004",
                    field(*b"s004", "shadowH", FieldKind::Real, cell(h)),
                ),
                (
                    *b"s005",
                    field(*b"s005", "shadowX", FieldKind::Real, cell(x)),
                ),
                (
                    *b"s006",
                    field(*b"s006", "shadowY", FieldKind::Real, cell(y)),
                ),
            ]),
            modifications: Vec::new(),
        }
    }

    #[test]
    fn named_images_resolve_to_textures_and_keep_authored_placement() {
        let shadows = read(&definition("Zshade", "ZCells", ["90", "60.5", "20", "45"])).unwrap();
        assert_eq!(
            shadows,
            Shadows {
                unit: Some(UnitImage {
                    texture: "ReplaceableTextures\\Shadows\\Zshade.blp".into(),
                    width: 90.0,
                    height: 60.5,
                    origin_x: 20.0,
                    origin_y: 45.0,
                }),
                building: Some("ReplaceableTextures\\Shadows\\ZCells.blp".into()),
            }
        );
    }

    #[test]
    fn empty_markers_name_no_image_and_need_no_placement() {
        for marker in ["_", "-", ""] {
            let shadows = read(&definition(marker, marker, ["_", "_", "_", "_"])).unwrap();
            assert_eq!(shadows, Shadows::default());
        }
    }

    #[test]
    fn map_edited_real_overrides_replace_source_cells() {
        let mut definition = definition("Zshade", "_", ["90", "60", "20", "45"]);
        definition.fields.get_mut(b"s003").unwrap().value =
            Ok(FieldValue::Override(pcx_objects::Value::Real(33.25)));
        assert_eq!(read(&definition).unwrap().unit.unwrap().width, 33.25);
        definition.fields.get_mut(b"s003").unwrap().value =
            Ok(FieldValue::Override(pcx_objects::Value::Int(33)));
        assert!(read(&definition).is_err());
    }

    #[test]
    fn malformed_names_and_placement_are_refused() {
        for (unit, building, numbers, reason) in [
            ("Zshade", "_", ["0", "60", "20", "45"], "shadowW"),
            ("Zshade", "_", ["90", "-5", "20", "45"], "shadowH"),
            ("Zshade", "_", ["90", "60", "x", "45"], "shadowX"),
            ("Zshade", "_", ["90", "60", "20", "inf"], "shadowY"),
            ("Zshade", "_", ["90", "60", "20", "-"], "shadowY"),
            (
                "Dir\\Zshade",
                "_",
                ["90", "60", "20", "45"],
                "shadow image name",
            ),
            ("_", "Zcells.blp", ["_", "_", "_", "_"], "shadow image name"),
        ] {
            let error = read(&definition(unit, building, numbers)).unwrap_err();
            assert!(error.contains(reason), "{error}");
        }
        let mut definition = definition("Zshade", "_", ["90", "60", "20", "45"]);
        definition
            .fields
            .get_mut(b"s003")
            .unwrap()
            .metadata
            .as_mut()
            .unwrap()
            .kind = FieldKind::Unreal;
        assert!(read(&definition).unwrap_err().contains("metadata"));
    }

    #[test]
    fn unit_image_footprint_puts_the_origin_at_the_authored_offset_from_the_lower_left() {
        let image = UnitImage {
            texture: String::new(),
            width: 90.0,
            height: 60.0,
            origin_x: 20.0,
            origin_y: 45.0,
        };
        let footprint = image.footprint([1000.0, -200.0]);
        assert_eq!(footprint.min, [980.0, -245.0]);
        assert_eq!(footprint.max, [1070.0, -185.0]);
        assert_eq!(footprint.enclosing_square(), ([1025.0, -215.0], 90.0));
        // Row 0 is the north edge: the south-west corner is the last row.
        assert_eq!(footprint.uv([980.0, -245.0]), [0.0, 1.0]);
        assert_eq!(footprint.uv([1070.0, -185.0]), [1.0, 0.0]);
        assert_eq!(footprint.uv([1025.0, -215.0]), [0.5, 0.5]);
    }

    #[test]
    fn building_footprint_centres_whole_cells_on_the_origin_with_south_first_rows() {
        let footprint = building_footprint([500.0, 700.0], [7, 4], 24.0);
        assert_eq!(footprint.min, [416.0, 652.0]);
        assert_eq!(footprint.max, [584.0, 748.0]);
        assert_eq!(footprint.enclosing_square(), ([500.0, 700.0], 168.0));
        assert_eq!(footprint.uv([416.0, 652.0]), [0.0, 0.0]);
        assert_eq!(footprint.uv([584.0, 748.0]), [1.0, 1.0]);
        // One cell east of the south-west corner is one texel of seven.
        assert_eq!(footprint.uv([440.0, 652.0]), [1.0 / 7.0, 0.0]);
    }
}
