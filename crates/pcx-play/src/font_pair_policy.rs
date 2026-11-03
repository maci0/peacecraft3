//! Additive font-pair exceptions derived from the reviewed Legacy client.
use crate::{
    neutral_player::{Sha256Digest, sha256},
    pe_image::PeSections,
    scripts::AssetIdentity,
};

/// Metrics for the qualified static TrueType/horizontal-kern path.
/// Bitmap widths must come from the rasterizer, not outline bounds.
pub struct FontMetrics<'a> {
    face: ttf_parser::Face<'a>,
    pub origin: AssetIdentity,
    pub font_sha256: Sha256Digest,
}

impl<'a> FontMetrics<'a> {
    pub fn read(bytes: &'a [u8], origin: AssetIdentity) -> Result<Self, String> {
        let source = &origin.source;
        let fail = |reason| format!("font metrics {source}: {reason}");
        if [&origin.source, &origin.edition, &origin.version]
            .iter()
            .any(|value| value.trim().is_empty())
            || bytes.len() > 16 << 20
        {
            return Err(fail("missing provenance or font byte budget exceeded"));
        }
        let face = ttf_parser::Face::parse(bytes, 0).map_err(|_| fail("invalid font face"))?;
        if face
            .raw_face()
            .table(ttf_parser::Tag::from_bytes(b"fvar"))
            .is_some()
        {
            return Err(fail("variable font metrics are unqualified"));
        }
        if face
            .raw_face()
            .table(ttf_parser::Tag::from_bytes(b"kern"))
            .is_some()
            && face.tables().kern.is_none()
        {
            return Err(fail("malformed kern table"));
        }
        if let Some(kern) = face.tables().kern {
            let expected = kern.subtables.len();
            let mut count = 0;
            for table in kern.subtables {
                count += 1;
                if count > 1
                    || !table.horizontal
                    || table.variable
                    || table.has_cross_stream
                    || table.has_state_machine
                    || !matches!(table.format, ttf_parser::kern::Format::Format0(_))
                {
                    return Err(fail("unsupported kerning subtable combination"));
                }
            }
            if count != expected {
                return Err(fail("truncated kerning subtables"));
            }
        }
        Ok(Self {
            face,
            origin,
            font_sha256: sha256(bytes),
        })
    }

    /// Unhinted 26.6 advance and nonpositive unscaled kerning, scaled by source UPEM.
    pub fn pair_metrics(
        &self,
        previous: char,
        current: char,
        pixels: f32,
    ) -> Result<(f32, f32), String> {
        let fail = |reason| format!("font metrics {}: {reason}", self.origin.source);
        if !pixels.is_finite() || pixels <= 0. {
            return Err(fail("invalid pixel size"));
        }
        let left = self
            .face
            .glyph_index(previous)
            .ok_or_else(|| fail("missing previous glyph"))?;
        let right = self
            .face
            .glyph_index(current)
            .ok_or_else(|| fail("missing current glyph"))?;
        let advance = self
            .face
            .glyph_hor_advance(left)
            .ok_or_else(|| fail("missing horizontal advance"))?;
        let scale = pixels / f32::from(self.face.units_per_em());
        // 26.6 fixed point is the qualified glyph-metrics format, not authored data.
        let advance = (f32::from(advance) * scale * 64.).round() / 64.;
        let kerning = self
            .face
            .tables()
            .kern
            .into_iter()
            .flat_map(|table| table.subtables)
            .find_map(|table| table.glyphs_kerning(left, right))
            .unwrap_or(0)
            .min(0);
        let kerning = f32::from(kerning) * scale;
        if !advance.is_finite() || !kerning.is_finite() {
            return Err(fail("scaled metrics overflow"));
        }
        Ok((advance, kerning))
    }

    pub fn baseline(&self, pixels: f32) -> Result<f32, String> {
        let table = self.face.tables().hhea;
        let ascender = f32::from(table.ascender);
        let descender = f32::from(table.descender).abs();
        if !pixels.is_finite() || pixels <= 0. || ascender <= 0. {
            return Err(format!(
                "font metrics {}: invalid baseline metrics",
                self.origin.source
            ));
        }
        Ok((pixels * (ascender / (ascender + descender))).round())
    }
}

#[derive(Clone, Debug)]
struct RasterSizing {
    minimum: f32,
    maximum: u32,
    empty_bias: u32,
    empty_shift: u32,
}
impl RasterSizing {
    fn read(mut read: impl FnMut(u32, usize) -> Option<Vec<u8>>) -> Result<Self, &'static str> {
        let minimum = read(0x7f0be9, 8).ok_or("missing font minimum instruction")?;
        let maximum = read(0x7f0c3a, 5).ok_or("missing font cap instruction")?;
        let empty = read(0x7f1d8d, 9).ok_or("missing empty glyph instructions")?;
        if minimum.len() != 8
            || minimum[..4] != [0xf3, 0x0f, 0x10, 0x0d]
            || maximum.len() != 5
            || maximum[0] != 0xb9
            || empty.len() != 9
            || empty[..2] != [0x8d, 0x41]
            || empty[3..8] != [0x89, 0x4d, 0xf8, 0xc1, 0xe8]
        {
            return Err("unsupported font sizing instructions");
        }
        let address = u32::from_le_bytes(minimum[4..].try_into().unwrap());
        let minimum = read(address, 4).ok_or("missing font minimum operand")?;
        let minimum = f32::from_le_bytes(minimum.try_into().map_err(|_| "truncated font minimum")?);
        let maximum = u32::from_le_bytes(maximum[1..].try_into().unwrap());
        let empty_bias = u32::from(empty[2]);
        let empty_shift = u32::from(empty[8]);
        if !minimum.is_finite()
            || minimum <= 0.
            || maximum == 0
            || maximum > 65535
            || minimum > maximum as f32
            || empty_shift > 31
        {
            return Err("invalid font sizing operands");
        }
        Ok(Self {
            minimum,
            maximum,
            empty_bias,
            empty_shift,
        })
    }
}

fn simple_integer_strings(
    mut read: impl FnMut(u32, usize) -> Option<Vec<u8>>,
) -> Result<bool, &'static str> {
    fn operand(
        read: &mut impl FnMut(u32, usize) -> Option<Vec<u8>>,
        va: u32,
        prefix: &[u8],
        count: usize,
    ) -> Result<u32, &'static str> {
        let bytes = read(va, prefix.len() + count).ok_or("missing simple-string flag operand")?;
        if bytes.len() != prefix.len() + count || !bytes.starts_with(prefix) {
            return Err("unsupported simple-string flag instruction");
        }
        let value = &bytes[prefix.len()..];
        Ok(match count {
            1 if prefix[0] == 0x83 => value[0] as i8 as i32 as u32,
            1 => u32::from(value[0]),
            4 => u32::from_le_bytes(value.try_into().unwrap()),
            _ => return Err("unsupported simple-string operand width"),
        })
    }
    let widget = operand(&mut read, 0x8864ac, &[0xc7, 0x86, 0xc4, 0, 0, 0], 4)?;
    let shift = operand(&mut read, 0x95f699, &[0xc1, 0xe9], 1)?;
    let mask = operand(&mut read, 0x95f69e, &[0x83, 0xe1], 1)?;
    let mut flags = (widget >> (shift & 31)) & mask;
    let mut possible = mask;
    for (out_va, out_prefix, out_size, input_va, input_prefix, input_size) in [
        (0x95f6a3, &[0x83, 0xca][..], 1, 0x95f6a6, &[0x25][..], 4),
        (0x95f6b2, &[0x83, 0xc9][..], 1, 0x95f6b5, &[0x25][..], 4),
        (0x95f6c1, &[0x83, 0xca][..], 1, 0x95f6c4, &[0x25][..], 4),
        (0x95f6d0, &[0x83, 0xc9][..], 1, 0x95f6d3, &[0x24][..], 1),
        (0x95f6dc, &[0x81, 0xca][..], 4, 0x95f6e2, &[0x25][..], 4),
        (0x95f6ee, &[0x81, 0xc9][..], 4, 0x95f6f4, &[0x25][..], 4),
        (0x95f700, &[0x81, 0xca][..], 4, 0x95f706, &[0x25][..], 4),
        (0x95f710, &[0x0d][..], 4, 0x95f715, &[0x81, 0xe6][..], 4),
    ] {
        let output = operand(&mut read, out_va, out_prefix, out_size)?;
        let input = operand(&mut read, input_va, input_prefix, input_size)?;
        possible |= output;
        if widget & input != 0 {
            flags |= output;
        }
    }
    let alternate_pairs = operand(&mut read, 0x7ec12a, &[0x83, 0xe1], 1)?;
    let fractional = operand(&mut read, 0x7ec1e0, &[0x25], 4)?;
    let pixel_coordinates = operand(&mut read, 0x7ec01a, &[0xf6, 0x45, 0x2c], 1)?;
    Ok(
        flags & alternate_pairs == 0
            && possible & fractional == 0
            && flags & pixel_coordinates != 0,
    )
}

fn default_font_point_sampling(
    mut read: impl FnMut(u32, usize) -> Option<Vec<u8>>,
) -> Result<bool, &'static str> {
    let mut filters = [0; 3];
    for (slot, va) in filters.iter_mut().zip([0x7def50, 0x7def61, 0x7def74]) {
        let code = read(va, 7).ok_or("missing font sampler lookup")?;
        if code.len() != 7 || code[..3] != [0xff, 0x34, 0xb5] {
            return Err("unsupported font sampler lookup");
        }
        let table = u32::from_le_bytes(code[3..].try_into().unwrap());
        let value = read(table, 4).ok_or("missing default sampler operand")?;
        *slot = u32::from_le_bytes(
            value
                .try_into()
                .map_err(|_| "truncated font sampler operand")?,
        );
    }
    // D3DTEXF enum identifiers, not measured sampler values.
    match filters {
        [1, 1, 0] => Ok(true),
        [2, 2, 0] => Ok(false),
        _ => Err("unsupported default font min/mag/mip combination"),
    }
}

fn d3d9_glyph_offset(
    mut read: impl FnMut(u32, usize) -> Option<Vec<u8>>,
) -> Result<[f32; 2], &'static str> {
    let mut offsets = [0.; 2];
    for (slot, va) in offsets.iter_mut().zip([0x7ec3ac, 0x7ec36a]) {
        let code = read(va, 10).ok_or("missing D3D9 font pixel correction")?;
        if code.len() != 10 || code[..2] != [0xc7, 0x05] {
            return Err("unsupported D3D9 font pixel correction");
        }
        *slot = f32::from_le_bytes(code[6..].try_into().unwrap());
        if !slot.is_finite() || slot.abs() > 4. {
            return Err("invalid D3D9 font pixel correction");
        }
    }
    // D3D9 samples at integer coordinates; Bevy's raster centers are half-integers.
    Ok([offsets[0] + 0.5, -offsets[1] + 0.5])
}

fn font_alpha_reference(
    mut read: impl FnMut(u32, usize) -> Option<Vec<u8>>,
) -> Result<u8, &'static str> {
    let selector = read(0x7ed64a, 10).ok_or("missing font blend selector")?;
    let lookup = read(0x7d3386, 7).ok_or("missing alpha reference lookup")?;
    if selector.len() != 10
        || selector[..2] != [0xc7, 0x05]
        || lookup.len() != 7
        || lookup[..3] != [0x8b, 0x04, 0x85]
    {
        return Err("unsupported font alpha instructions");
    }
    let selector = u32::from_le_bytes(selector[6..].try_into().unwrap());
    if selector > 64 {
        return Err("font alpha selector exceeds inspection budget");
    }
    let table = u32::from_le_bytes(lookup[3..].try_into().unwrap());
    let at = table
        .checked_add(
            selector
                .checked_mul(4)
                .ok_or("alpha reference index overflow")?,
        )
        .ok_or("alpha reference address overflow")?;
    let value = read(at, 4).ok_or("missing font alpha reference")?;
    let value = u32::from_le_bytes(
        value
            .try_into()
            .map_err(|_| "truncated font alpha reference")?,
    );
    u8::try_from(value).map_err(|_| "font alpha reference exceeds byte range")
}

#[derive(Clone, Debug, PartialEq)]
pub struct PairAdjustment {
    pub previous: u32,
    pub current: u32,
    pub addend: f32,
}

#[derive(Clone, Debug)]
pub struct PairPolicy {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    adjustments: Vec<PairAdjustment>,
    fallback: f32,
    sizing: RasterSizing,
    alpha_reference: u8,
    point_sampling: bool,
    glyph_offset: [f32; 2],
    widget_flags: u32,
}

impl PairPolicy {
    pub fn read(image: &[u8], origin: AssetIdentity) -> Result<Self, String> {
        let fail = |reason: &str| format!("font pair policy {}: {reason}", origin.source);
        if image.len() > 32 << 20 {
            return Err(fail("image exceeds byte budget"));
        }
        if [&origin.source, &origin.edition, &origin.version]
            .iter()
            .any(|value| value.trim().is_empty())
        {
            return Err(fail("missing source/edition/version provenance"));
        }
        let digest = sha256(image);
        if digest != crate::menu_lighting::REVIEWED_IMAGE {
            return Err(fail("unsupported client image; no pair values substituted"));
        }
        let pe = PeSections::parse(image).map_err(fail)?;
        let adjustments = decode(|va, len| {
            pe.va_bytes(va, len)
                .map(<[u8]>::to_vec)
                .ok_or("missing instruction or table range")
        })
        .map_err(fail)?;
        let sizing =
            RasterSizing::read(|va, len| pe.va_bytes(va, len).map(<[u8]>::to_vec)).map_err(fail)?;
        let alpha_reference =
            font_alpha_reference(|va, len| pe.va_bytes(va, len).map(<[u8]>::to_vec))
                .map_err(fail)?;
        let point_sampling =
            default_font_point_sampling(|va, len| pe.va_bytes(va, len).map(<[u8]>::to_vec))
                .map_err(fail)?;
        if !simple_integer_strings(|va, len| pe.va_bytes(va, len).map(<[u8]>::to_vec))
            .map_err(fail)?
        {
            return Err(fail(
                "default simple-string integer spacing path is unsupported",
            ));
        }
        let glyph_offset =
            d3d9_glyph_offset(|va, len| pe.va_bytes(va, len).map(<[u8]>::to_vec)).map_err(fail)?;
        // The integer-path admission above already validates this constructor operand.
        let widget_flags = u32::from_le_bytes(
            pe.va_bytes(0x8864b2, 4)
                .ok_or_else(|| fail("missing simple-string default flags"))?
                .try_into()
                .unwrap(),
        );
        Ok(Self {
            origin,
            image_sha256: digest,
            sizing,
            alpha_reference,
            point_sampling,
            glyph_offset,
            widget_flags,
            adjustments: adjustments.0,
            fallback: adjustments.1,
        })
    }

    pub fn default_widget_flags(&self) -> u32 {
        self.widget_flags
    }

    /// Only the observed default font-flag branch is qualified by this profile.
    pub fn default_font_point_sampling(&self) -> bool {
        self.point_sampling
    }

    /// Qualified default-font D3D9 quads, translated to Bevy's downward-Y raster space.
    pub fn d3d9_glyph_offset(&self) -> [f32; 2] {
        self.glyph_offset
    }

    pub fn font_alpha_reference(&self) -> u8 {
        self.alpha_reference
    }

    /// The source backend compares combined texture/material alpha with GreaterEqual.
    pub fn font_alpha_cutoff(&self) -> f32 {
        f32::from(self.alpha_reference) / f32::from(u8::MAX)
    }

    pub fn pixel_height(&self, normalized: f32, viewport_height: f32) -> Result<f32, &'static str> {
        if !normalized.is_finite()
            || normalized <= 0.
            || !viewport_height.is_finite()
            || viewport_height <= 0.
        {
            return Err("invalid source font height or viewport");
        }
        Ok((normalized * viewport_height)
            .max(self.sizing.minimum)
            .round()
            .min(self.sizing.maximum as f32))
    }

    pub fn empty_bitmap_width(&self, pixels: u32) -> Result<u32, &'static str> {
        pixels
            .checked_add(self.sizing.empty_bias)
            .map(|n| n >> self.sizing.empty_shift)
            .map(|width| if width == 0 { pixels } else { width })
            .ok_or("empty glyph width overflow")
    }

    /// Qualified integer-width branch; other native modes remain unsupported here.
    pub fn integer_step(
        &self,
        metrics: &FontMetrics<'_>,
        previous: char,
        current: char,
        pixels: f32,
        previous_bitmap_width: u32,
    ) -> Result<f32, String> {
        let (advance, kerning) = metrics.pair_metrics(previous, current, pixels)?;
        self.finish_step(
            previous.into(),
            current.into(),
            (previous_bitmap_width as f32).min(advance) + kerning,
        )
        .map_err(|error| format!("font pair policy {}: {error}", self.origin.source))
    }

    pub fn adjustments(&self) -> &[PairAdjustment] {
        &self.adjustments
    }

    /// The native routine adds the exception after ceiling the ordinary pair step.
    pub fn finish_step(
        &self,
        previous: u32,
        current: u32,
        ordinary: f32,
    ) -> Result<f32, &'static str> {
        if !ordinary.is_finite() {
            return Err("non-finite ordinary font pair step");
        }
        let addend = self
            .adjustments
            .iter()
            .find(|pair| pair.previous == previous && pair.current == current)
            .map_or(self.fallback, |pair| pair.addend);
        let step = ordinary.ceil() + addend;
        step.is_finite()
            .then_some(step)
            .ok_or("font pair step overflow")
    }
}

fn decode(
    mut read: impl FnMut(u32, usize) -> Result<Vec<u8>, &'static str>,
) -> Result<(Vec<PairAdjustment>, f32), &'static str> {
    // Reviewed instruction locations identify the format; operands supply its data.
    let code = read(0x7e960b, 32)?;
    if code.len() != 32
        || code[0] != 0xb8
        || code[5..24]
            != [
                0x33, 0xc9, 0x39, 0x38, 0x75, 5, 0x39, 0x58, 4, 0x74, 0x4f, 0x41, 0x83, 0xc0,
                code[19], 0x83, 0xf9, code[22], 0x72,
            ]
        || code[24..28] != [0xee, 0xc7, 0x45, 8]
    {
        return Err("unsupported pair-table instruction shape");
    }
    let address = u32::from_le_bytes(code[1..5].try_into().unwrap());
    let stride = usize::from(code[19]);
    let count = usize::from(code[22]);
    // Twelve bytes is this qualified record format; the count remains source-derived.
    if stride != 12 || count == 0 || count > 64 {
        return Err("invalid pair-table extent");
    }
    let fallback = f32::from_le_bytes(code[28..32].try_into().unwrap());
    if !fallback.is_finite() {
        return Err("non-finite pair fallback");
    }
    let bytes = read(address, count * stride)?;
    if bytes.len() != count * stride {
        return Err("truncated pair table");
    }
    let mut pairs = Vec::with_capacity(count);
    for record in bytes.chunks_exact(stride) {
        let pair = PairAdjustment {
            previous: u32::from_le_bytes(record[..4].try_into().unwrap()),
            current: u32::from_le_bytes(record[4..8].try_into().unwrap()),
            addend: f32::from_le_bytes(record[8..12].try_into().unwrap()),
        };
        if char::from_u32(pair.previous).is_none()
            || char::from_u32(pair.current).is_none()
            || !pair.addend.is_finite()
            || pairs.iter().any(|old: &PairAdjustment| {
                old.previous == pair.previous && old.current == pair.current
            })
        {
            return Err("invalid or repeated pair-table record");
        }
        pairs.push(pair);
    }
    Ok((pairs, fallback))
}

/// Synthetic policies for dependent-crate component tests; never source game data.
#[cfg(any(test, feature = "test-fixtures"))]
pub mod fixtures {
    use super::*;
    pub fn synthetic() -> PairPolicy {
        PairPolicy {
            origin: AssetIdentity {
                source: "synthetic font policy".into(),
                edition: "fixture".into(),
                version: "test".into(),
            },
            image_sha256: [0x42; 32],
            adjustments: Vec::new(),
            fallback: 0.,
            sizing: RasterSizing {
                minimum: 2.,
                maximum: 60,
                empty_bias: 3,
                empty_shift: 2,
            },
            alpha_reference: 23,
            point_sampling: true,
            glyph_offset: [0.; 2],
            widget_flags: 2 | 16,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_string_mode_is_derived_from_constructor_mapper_and_emitter_operands() {
        use std::collections::BTreeMap;
        let mut data = BTreeMap::new();
        let put =
            |data: &mut BTreeMap<u32, u8>, va: u32, prefix: &[u8], value: u32, size: usize| {
                for (i, b) in prefix
                    .iter()
                    .chain(value.to_le_bytes()[..size].iter())
                    .enumerate()
                {
                    data.insert(va + i as u32, *b);
                }
            };
        put(&mut data, 0x8864ac, &[0xc7, 0x86, 0xc4, 0, 0, 0], 2, 4);
        put(&mut data, 0x95f699, &[0xc1, 0xe9], 8, 1);
        put(&mut data, 0x95f69e, &[0x83, 0xe1], 1, 1);
        for (a, p, n, b, q, m) in [
            (0x95f6a3, &[0x83, 0xca][..], 1, 0x95f6a6, &[0x25][..], 4),
            (0x95f6b2, &[0x83, 0xc9][..], 1, 0x95f6b5, &[0x25][..], 4),
            (0x95f6c1, &[0x83, 0xca][..], 1, 0x95f6c4, &[0x25][..], 4),
            (0x95f6d0, &[0x83, 0xc9][..], 1, 0x95f6d3, &[0x24][..], 1),
            (0x95f6dc, &[0x81, 0xca][..], 4, 0x95f6e2, &[0x25][..], 4),
            (0x95f6ee, &[0x81, 0xc9][..], 4, 0x95f6f4, &[0x25][..], 4),
            (0x95f700, &[0x81, 0xca][..], 4, 0x95f706, &[0x25][..], 4),
            (0x95f710, &[0x0d][..], 4, 0x95f715, &[0x81, 0xe6][..], 4),
        ] {
            put(&mut data, a, p, 0, n);
            put(&mut data, b, q, 0, m);
        }
        put(&mut data, 0x95f6a3, &[0x83, 0xca], 64, 1);
        put(&mut data, 0x95f6a6, &[0x25], 2, 4);
        put(&mut data, 0x7ec12a, &[0x83, 0xe1], 8, 1);
        put(&mut data, 0x7ec1e0, &[0x25], 32, 4);
        put(&mut data, 0x7ec01a, &[0xf6, 0x45, 0x2c], 64, 1);
        let run = |data: &BTreeMap<u32, u8>| {
            simple_integer_strings(|va, n| {
                (0..n)
                    .map(|i| data.get(&(va + i as u32)).copied())
                    .collect()
            })
        };
        assert_eq!(run(&data), Ok(true));
        put(&mut data, 0x8864ac, &[0xc7, 0x86, 0xc4, 0, 0, 0], 0, 4);
        assert_eq!(run(&data), Ok(false));
        put(&mut data, 0x8864ac, &[0xc7, 0x86, 0xc4, 0, 0, 0], 2, 4);
        put(&mut data, 0x95f6b2, &[0x83, 0xc9], 32, 1);
        assert_eq!(run(&data), Ok(false));
        data.remove(&0x95f6b2);
        assert!(run(&data).is_err());
    }
    #[test]
    #[ignore = "requires PCX_FONT_METRICS_REFERENCE with a local native-advance receipt and source font"]
    fn supplied_native_advances_match_source_font_metrics() {
        let path = std::env::var("PCX_FONT_METRICS_REFERENCE").expect("PCX_FONT_METRICS_REFERENCE");
        let reference: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let font_path = reference["font"].as_str().unwrap();
        let bytes = std::fs::read(font_path).unwrap();
        assert_eq!(serde_json::json!(sha256(&bytes)), reference["font_sha256"]);
        let metrics = FontMetrics::read(
            &bytes,
            AssetIdentity {
                source: font_path.into(),
                edition: reference["edition"].as_str().unwrap().into(),
                version: reference["version"].as_str().unwrap().into(),
            },
        )
        .unwrap();
        let pixels = reference["pixels"].as_f64().unwrap() as f32;
        assert_eq!(
            metrics.baseline(pixels).unwrap(),
            reference["baseline"]
                .as_f64()
                .expect("native baseline in receipt") as f32
        );
        let glyphs = reference["glyphs"].as_array().unwrap();
        assert!(!glyphs.is_empty());
        for glyph in glyphs {
            let text = glyph["character"].as_str().unwrap();
            let mut chars = text.chars();
            let c = chars.next().unwrap();
            assert!(chars.next().is_none());
            let (advance, _) = metrics.pair_metrics(c, c, pixels).unwrap();
            assert_eq!(advance, glyph["advance"].as_f64().unwrap() as f32, "{c}");
        }
    }

    #[test]
    fn d3d9_offsets_follow_source_operands_and_flip_y() {
        for (x, y) in [(-0.25f32, -0.75f32), (0., 0.)] {
            let read = |va, _| {
                let value = match va {
                    0x7ec3ac => x,
                    0x7ec36a => y,
                    _ => return None,
                };
                let mut bytes = vec![0xc7, 0x05, 0, 0x40, 0, 0];
                bytes.extend(value.to_le_bytes());
                Some(bytes)
            };
            assert_eq!(d3d9_glyph_offset(read), Ok([x + 0.5, -y + 0.5]));
        }
        assert!(d3d9_glyph_offset(|_, _| None).is_err());
        assert!(d3d9_glyph_offset(|_, _| Some(vec![0; 10])).is_err());
    }

    #[test]
    fn baseline_uses_font_vertical_metrics() {
        let bytes = synthetic_font(0);
        let metrics = FontMetrics::read(&bytes, synthetic_origin()).unwrap();
        assert_eq!(metrics.baseline(10.).unwrap(), 8.);
        assert_eq!(metrics.baseline(20.).unwrap(), 16.);
        assert!(metrics.baseline(f32::NAN).is_err());
        assert!(metrics.baseline(0.).is_err());
    }

    #[test]
    fn default_sampler_values_are_read_and_unsupported_combinations_refuse() {
        for filter in [1u32, 2] {
            let read = |va, _| match va {
                0x7def50 => Some(vec![0xff, 0x34, 0xb5, 0, 0x40, 0, 0]),
                0x7def61 => Some(vec![0xff, 0x34, 0xb5, 4, 0x40, 0, 0]),
                0x7def74 => Some(vec![0xff, 0x34, 0xb5, 8, 0x40, 0, 0]),
                0x4000 | 0x4004 => Some(filter.to_le_bytes().to_vec()),
                0x4008 => Some(0u32.to_le_bytes().to_vec()),
                _ => None,
            };
            assert_eq!(default_font_point_sampling(read), Ok(filter == 1));
            assert!(
                default_font_point_sampling(|va, n| if va == 0x4008 {
                    Some(1u32.to_le_bytes().to_vec())
                } else {
                    read(va, n)
                })
                .is_err()
            );
        }
        assert!(default_font_point_sampling(|_, _| None).is_err());
    }

    #[test]
    fn font_alpha_reference_follows_selector_and_table_operands() {
        let read = |va, _| match va {
            0x7ed64a => Some(vec![0xc7, 0x05, 0, 0, 0, 0, 1, 0, 0, 0]),
            0x7d3386 => Some(vec![0x8b, 0x04, 0x85, 0, 0x40, 0, 0]),
            0x4004 => Some(13u32.to_le_bytes().to_vec()),
            _ => None,
        };
        assert_eq!(font_alpha_reference(read), Ok(13));
        assert!(
            font_alpha_reference(|va, len| if va == 0x4004 {
                Some(256u32.to_le_bytes().to_vec())
            } else {
                read(va, len)
            })
            .is_err()
        );
        assert!(font_alpha_reference(|_, _| None).is_err());
    }

    #[test]
    fn sizing_and_empty_glyph_width_follow_source_operands() {
        let read = |va, _| match va {
            0x7f0be9 => Some(vec![0xf3, 0x0f, 0x10, 0x0d, 0, 0x40, 0, 0]),
            0x7f0c3a => Some(vec![0xb9, 40, 0, 0, 0]),
            0x7f1d8d => Some(vec![0x8d, 0x41, 7, 0x89, 0x4d, 0xf8, 0xc1, 0xe8, 3]),
            0x4000 => Some(3f32.to_le_bytes().to_vec()),
            _ => None,
        };
        let sizing = RasterSizing::read(read).unwrap();
        let policy = PairPolicy {
            origin: synthetic_origin(),
            image_sha256: [0; 32],
            alpha_reference: 13,
            point_sampling: false,
            glyph_offset: [0.; 2],
            widget_flags: 2 | 16,
            adjustments: vec![],
            fallback: 0.,
            sizing,
        };
        assert_eq!(policy.pixel_height(0.01, 960.), Ok(10.));
        assert_eq!(policy.pixel_height(0.001, 960.), Ok(3.));
        assert_eq!(policy.pixel_height(1., 960.), Ok(40.));
        assert_eq!(policy.empty_bitmap_width(16), Ok(2));
        assert!(policy.pixel_height(f32::NAN, 960.).is_err());
        assert!(RasterSizing::read(|_, _| None).is_err());
    }

    fn synthetic_origin() -> AssetIdentity {
        AssetIdentity {
            source: "synthetic".into(),
            edition: "fixture".into(),
            version: "test".into(),
        }
    }

    fn synthetic_font(kerning: i16) -> Vec<u8> {
        let mut head = vec![0; 54];
        head[18..20].copy_from_slice(&1000u16.to_be_bytes());
        let mut hhea = vec![0; 36];
        hhea[4..6].copy_from_slice(&800i16.to_be_bytes());
        hhea[6..8].copy_from_slice(&(-200i16).to_be_bytes());
        hhea[34..].copy_from_slice(&4u16.to_be_bytes());
        let mut maxp = vec![0, 0, 0x50, 0];
        maxp.extend(4u16.to_be_bytes());
        let mut hmtx = Vec::new();
        for advance in [0u16, 500, 700, 600] {
            hmtx.extend(advance.to_be_bytes());
            hmtx.extend(0i16.to_be_bytes());
        }
        let mut cmap = vec![0, 0, 0, 1, 0, 3, 0, 1, 0, 0, 0, 12, 0, 0, 1, 6, 0, 0];
        cmap.extend([0; 256]);
        cmap[18 + 65] = 1;
        cmap[18 + 86] = 2;
        cmap[18 + 84] = 3;
        let mut kern = vec![
            0, 0, 0, 1, 0, 0, 0, 20, 0, 1, 0, 1, 0, 6, 0, 0, 0, 0, 0, 1, 0, 2,
        ];
        kern.extend(kerning.to_be_bytes());
        let tables = [
            (b"cmap", cmap),
            (b"head", head),
            (b"hhea", hhea),
            (b"hmtx", hmtx),
            (b"kern", kern),
            (b"maxp", maxp),
        ];
        let mut bytes = vec![0; 12 + tables.len() * 16];
        bytes[..4].copy_from_slice(&0x10000u32.to_be_bytes());
        bytes[4..6].copy_from_slice(&(tables.len() as u16).to_be_bytes());
        for (index, (tag, data)) in tables.into_iter().enumerate() {
            let offset = bytes.len() as u32;
            let length = data.len() as u32;
            let at = 12 + index * 16;
            bytes[at..at + 4].copy_from_slice(tag);
            bytes[at + 8..at + 12].copy_from_slice(&offset.to_be_bytes());
            bytes[at + 12..at + 16].copy_from_slice(&length.to_be_bytes());
            bytes.extend(data);
        }
        bytes
    }

    #[test]
    fn font_tables_supply_metrics_and_positive_kerning_is_discarded() {
        let bytes = synthetic_font(-40);
        let metrics = FontMetrics::read(&bytes, synthetic_origin()).unwrap();
        let (advance, kerning) = metrics.pair_metrics('A', 'V', 10.).unwrap();
        let (code, table) = fixture();
        let (adjustments, fallback) = parse(&code, &table).unwrap();
        let policy = PairPolicy {
            origin: AssetIdentity {
                source: "synthetic".into(),
                edition: "fixture".into(),
                version: "test".into(),
            },
            image_sha256: [0; 32],
            alpha_reference: 13,
            point_sampling: false,
            glyph_offset: [0.; 2],
            widget_flags: 2 | 16,
            sizing: RasterSizing {
                minimum: 1.,
                maximum: 48,
                empty_bias: 1,
                empty_shift: 1,
            },
            adjustments,
            fallback,
        };
        assert_eq!(policy.integer_step(&metrics, 'A', 'V', 10., 4), Ok(3.5));
        assert_eq!(advance, 5.);
        assert!((kerning + 0.4).abs() < 1e-6);
        assert!(metrics.pair_metrics('A', '?', 10.).is_err());
        assert!(metrics.pair_metrics('A', 'V', f32::NAN).is_err());
        assert!(FontMetrics::read(&bytes[..20], synthetic_origin()).is_err());
        let bytes = synthetic_font(40);
        let metrics = FontMetrics::read(&bytes, synthetic_origin()).unwrap();
        assert_eq!(metrics.pair_metrics('A', 'V', 10.).unwrap(), (5., 0.));
    }

    fn fixture() -> (Vec<u8>, Vec<u8>) {
        let mut code = vec![
            0xb8, 0, 0x40, 0, 0, 0x33, 0xc9, 0x39, 0x38, 0x75, 5, 0x39, 0x58, 4, 0x74, 0x4f, 0x41,
            0x83, 0xc0, 12, 0x83, 0xf9, 2, 0x72, 0xee, 0xc7, 0x45, 8, 0, 0, 0, 0,
        ];
        code[28..].copy_from_slice(&0.25f32.to_le_bytes());
        let mut table = Vec::new();
        for (a, b, n) in [(65u32, 86u32, -0.5f32), (84, 111, 1.5)] {
            table.extend(a.to_le_bytes());
            table.extend(b.to_le_bytes());
            table.extend(n.to_le_bytes());
        }
        (code, table)
    }
    fn parse(code: &[u8], table: &[u8]) -> Result<(Vec<PairAdjustment>, f32), &'static str> {
        decode(|va, _| match va {
            0x7e960b => Ok(code.to_vec()),
            0x4000 => Ok(table.to_vec()),
            _ => Err("unexpected address"),
        })
    }
    #[test]
    fn instruction_operands_and_table_values_drive_additive_post_ceiling_steps() {
        let (code, table) = fixture();
        let (adjustments, fallback) = parse(&code, &table).unwrap();
        let policy = PairPolicy {
            origin: AssetIdentity {
                source: "synthetic".into(),
                edition: "fixture".into(),
                version: "test".into(),
            },
            image_sha256: [0; 32],
            alpha_reference: 13,
            point_sampling: false,
            glyph_offset: [0.; 2],
            widget_flags: 2 | 16,
            sizing: RasterSizing {
                minimum: 1.,
                maximum: 48,
                empty_bias: 1,
                empty_shift: 1,
            },
            adjustments,
            fallback,
        };
        assert_eq!(policy.adjustments().len(), 2);
        assert_eq!(policy.finish_step(65, 86, 3.2), Ok(3.5));
        assert_eq!(policy.finish_step(84, 111, 3.2), Ok(5.5));
        assert_eq!(policy.finish_step(65, 65, 3.2), Ok(4.25));
        assert!(policy.finish_step(65, 65, f32::NAN).is_err());
    }
    #[test]
    fn malformed_shapes_ranges_and_records_refuse() {
        let (mut code, mut table) = fixture();
        assert!(parse(&code[..31], &table).is_err());
        code[19] = 8;
        assert!(parse(&code, &table).is_err());
        code[19] = 12;
        code[22] = 65;
        assert!(parse(&code, &table).is_err());
        code[22] = 2;
        assert!(parse(&code, &table[..23]).is_err());
        table[8..12].copy_from_slice(&f32::INFINITY.to_le_bytes());
        assert!(parse(&code, &table).is_err());
    }
    #[test]
    #[ignore = "requires PCX_WAR3_EXE selecting the reviewed local Legacy executable"]
    fn supplied_original_policy_is_read_without_copying_its_records() {
        let path = std::env::var("PCX_WAR3_EXE").expect("PCX_WAR3_EXE");
        let image = std::fs::read(&path).unwrap();
        let policy = PairPolicy::read(
            &image,
            AssetIdentity {
                source: path,
                edition: "Legacy".into(),
                version: "1.29.2.9231".into(),
            },
        )
        .unwrap();
        assert!(!policy.adjustments().is_empty());
        for pair in policy.adjustments() {
            assert_eq!(
                policy.finish_step(pair.previous, pair.current, 2.25),
                Ok(3. + pair.addend)
            );
        }
    }
}
