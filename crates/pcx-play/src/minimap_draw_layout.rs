//! Minimap CPU-image drawing facts from the reviewed client: the map-edge border lines,
//! the unit marker block sizes and the camera outline colour.
//!
//! Sites are in the texture updater (`0x658ac0`), the marker pass (`0x65ba90`), the draw
//! method (`0x63fd30`) and the constructor (`0x61f930`). Resource blit geometry,
//! textures, classes and tint operands are read too. Hero/neutral icons, creep camps
//! and pings are not decoded here.
use crate::{
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    scripts::AssetIdentity,
};

/// A row or column of the CPU image, relative to the map padding on that axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    /// `pad + n`
    PadPlus(i32),
    /// `edge - pad - n`
    EdgeMinusPad(i32),
}

impl Edge {
    /// The image index, or `None` outside `0..edge`.
    pub fn index(self, edge: u32, pad: u32) -> Option<u32> {
        let index = match self {
            Self::PadPlus(n) => i64::from(pad) + i64::from(n),
            Self::EdgeMinusPad(n) => i64::from(edge) - i64::from(pad) - i64::from(n),
        };
        u32::try_from(index).ok().filter(|i| *i < edge)
    }
}

/// Lines written over the whole image after the fog alpha, where the map is narrower or
/// shorter than the square image.
#[derive(Clone, Debug)]
pub struct Border {
    /// Profile section and key of the packed ARGB colour.
    pub section: String,
    pub key: String,
    /// The lines are drawn only when the two paddings differ by more than this.
    pub minimum_pad_difference: u32,
    /// Vertical lines, used when the map is taller than wide. Padding is the left padding.
    pub columns: Vec<Edge>,
    /// Horizontal lines, used when the map is wider than tall. Padding is the top padding.
    pub rows: Vec<Edge>,
}

/// Terrain-cell to CPU-texel conversion shared by ordinary and resource markers.
#[derive(Clone, Debug)]
pub struct PointProjection {
    pub grid_shift: u8,
    pub bias: f32,
    pub bit_shift: u8,
}
impl PointProjection {
    pub fn scales(
        &self,
        bounds: [[i32; 2]; 2],
        edge: u32,
        pads: [u32; 2],
    ) -> Result<[f32; 2], String> {
        if edge == 0 || edge > u32::from(u8::MAX) + 1 {
            return Err("unsupported minimap marker image edge".into());
        }
        let mut result = [0.; 2];
        for axis in 0..2 {
            let extent = i64::from(bounds[1][axis]) - i64::from(bounds[0][axis]);
            let span = edge
                .checked_sub(1)
                .and_then(|v| v.checked_sub(pads[axis].checked_mul(2)?))
                .ok_or("minimap marker image span")?;
            if extent <= 0 || extent > i64::from(u32::MAX) || span == 0 {
                return Err("invalid minimap marker grid bounds".into());
            }
            result[axis] = span as f32 / extent as f32;
        }
        Ok(result)
    }
    pub fn project(
        &self,
        bounds: [[i32; 2]; 2],
        edge: u32,
        pads: [u32; 2],
        point: [i32; 2],
    ) -> Result<Option<[f64; 2]>, String> {
        let scales = self.scales(bounds, edge, pads)?;
        if (0..2).any(|i| point[i] < bounds[0][i] || point[i] > bounds[1][i]) {
            return Ok(None);
        }
        let pixel: [u32; 2] = std::array::from_fn(|i| {
            let delta = (i64::from(point[i]) - i64::from(bounds[0][i])) as u32;
            ((delta as f32 * scales[i] + self.bias).to_bits() >> self.bit_shift)
                & u32::from(u8::MAX)
        });
        Ok(Some([
            f64::from(pixel[0]) + f64::from(pads[0]),
            f64::from(edge) - f64::from(pixel[1]) - f64::from(pads[1]),
        ]))
    }
}

/// The textured hero pass, separate from CPU marker blocks.
#[derive(Clone, Debug)]
pub struct Hero {
    pub key: String,
    pub filters: [u32; 3],
    pub addresses: [u32; 2],
    pub uv: [f32; 4],
    pub initial_phase: f32,
    pub reset_phase: f32,
    pub midpoint: f32,
    pub period: f32,
    pub alpha_scale: f32,
    pub alpha_floor: f32,
    pub geometry_edge: f32,
    pub world_scale: f32,
    pub size_scale: f32,
    pub half: f32,
}
impl Hero {
    pub fn half_size(&self, world: [[f32; 2]; 2]) -> Result<f32, String> {
        let extent = [world[1][0] - world[0][0], world[1][1] - world[0][1]];
        if !world.into_iter().flatten().all(f32::is_finite) || extent.iter().any(|v| *v <= 0.) {
            return Err("invalid minimap hero world bounds".into());
        }
        let scale = extent.map(|v| self.geometry_edge / (v * self.world_scale));
        let result = scale[0].min(scale[1]) * self.size_scale * self.half;
        if !result.is_finite() || result <= 0. {
            return Err("invalid minimap hero quad size".into());
        }
        Ok(result)
    }
    /// Local frame scale and translation, preserving the native float operation order.
    pub fn projection(
        &self,
        frame: crate::inventory_layout::Rect,
        world: [[f32; 2]; 2],
    ) -> Result<[[f32; 2]; 2], String> {
        let extent = [world[1][0] - world[0][0], world[1][1] - world[0][1]];
        if !world.into_iter().flatten().all(f32::is_finite) || extent.iter().any(|v| *v <= 0.) {
            return Err("invalid minimap hero projection bounds".into());
        }
        let maximum = extent[0].max(extent[1]);
        let dimensions = [frame.width, frame.height];
        let scale = std::array::from_fn(|i| (dimensions[i] * (extent[i] / maximum)) / extent[i]);
        let bias = std::array::from_fn(|i| {
            dimensions[i] * self.half - (world[0][i] + extent[i] * self.half) * scale[i]
        });
        if !scale.into_iter().chain(bias).all(f32::is_finite) {
            return Err("invalid minimap hero projection".into());
        }
        Ok([scale, bias])
    }
    pub fn quad(
        &self,
        frame: crate::inventory_layout::Rect,
        bounds: [[f32; 2]; 2],
        full_world: [[f32; 2]; 2],
        point: [f32; 2],
    ) -> Result<crate::inventory_layout::Rect, String> {
        if !point.into_iter().all(f32::is_finite) {
            return Err("invalid minimap hero point".into());
        }
        let [scale, bias] = self.projection(frame, bounds)?;
        let half = self.half_size(full_world)?;
        Ok(crate::inventory_layout::Rect {
            x: frame.x + scale[0] * point[0] + bias[0] - half,
            y: frame.y + scale[1] * point[1] + bias[1] - half,
            width: half * 2.,
            height: half * 2.,
        })
    }

    /// The native UI update clamps at the period; drawing then resets that phase.
    pub fn advance(&self, phase: &mut f32, delta: f32, drawn: bool) -> Result<u8, String> {
        if !phase.is_finite() || *phase < 0. || !delta.is_finite() || delta < 0. {
            return Err("invalid minimap hero UI clock".into());
        }
        *phase = (*phase + delta).min(self.period);
        let triangle = if *phase > self.midpoint {
            self.period - *phase
        } else {
            *phase
        };
        let value = triangle * self.alpha_scale + self.alpha_floor;
        if !value.is_finite() || !(0. ..=f32::from(u8::MAX)).contains(&value) {
            return Err("invalid minimap hero alpha".into());
        }
        if drawn && *phase >= self.period {
            *phase = self.reset_phase;
        }
        Ok(value as u8)
    }
}

/// Resource textures and CPU blit operands, in normal/entangled/haunted order.
#[derive(Clone, Debug)]
pub struct ResourceIcons {
    pub keys: [String; 3],
    pub classes: [[u8; 4]; 3],
    pub side: u32,
    pub half: u32,
    pub clamp_inset: u32,
    pub luminance: [u32; 3],
    pub color_shift: u8,
    pub tint_bias: u32,
}

impl ResourceIcons {
    pub fn tint(&self, rgba: [u8; 4], owner: [u8; 4]) -> [u8; 4] {
        let gray = rgba[..3]
            .iter()
            .zip(self.luminance)
            .map(|(c, w)| u32::from(*c) * w)
            .sum::<u32>()
            >> self.color_shift;
        let mut result = rgba;
        for channel in 0..3 {
            result[channel] =
                ((gray * u32::from(owner[channel]) + self.tint_bias) >> self.color_shift) as u8;
        }
        result
    }
}

#[derive(Clone, Debug)]
pub struct Draw {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    /// Edge of the square CPU image the client allocates.
    pub edge: u32,
    pub resources: ResourceIcons,
    pub fog_fraction_bits: u8,
    pub hero: Hero,
    pub points: PointProjection,
    pub border: Border,
    /// Side in image pixels of the unit marker block for class 0 and class 1.
    pub marker_sides: [u32; 2],
    /// Camera outline RGB; its alpha is the control's alpha.
    pub outline_rgb: [u8; 3],
}

impl Draw {
    pub fn reviewed_legacy(image: &[u8], origin: AssetIdentity) -> Result<Self, String> {
        let fail = |reason: String| format!("minimap draw layout {}: {reason}", origin.source);
        if image.len() > 32 << 20
            || [&origin.source, &origin.edition, &origin.version]
                .iter()
                .any(|s| s.trim().is_empty())
            || sha256(image) != REVIEWED_IMAGE
        {
            return Err(fail("unsupported client identity".into()));
        }
        let pe = Pe::parse(
            image,
            RuleInputBudget {
                image_bytes: 32 << 20,
                profile_bytes: 64 << 10,
                sections: 96,
            },
        )
        .map_err(|e| fail(e.to_string()))?;
        let mut remaining = 64usize << 10;
        Self::decode(origin.clone(), REVIEWED_IMAGE, |va, len, code| {
            remaining = remaining.checked_sub(len).ok_or("profile read budget")?;
            pe.read_from(va, code)
                .map_err(|e| e.to_string())?
                .get(..len)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| format!("truncated operand at {va:#x}"))
        })
        .map_err(fail)
    }

    fn decode(
        origin: AssetIdentity,
        image_sha256: Sha256Digest,
        mut read: impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        type Read<'a> = &'a mut dyn FnMut(u32, usize, bool) -> Result<Vec<u8>, String>;
        fn operand(read: Read, va: u32, prefix: &[u8], len: usize) -> Result<Vec<u8>, String> {
            let bytes = read(va, prefix.len() + len, true)?;
            if bytes.len() != prefix.len() + len || !bytes.starts_with(prefix) {
                return Err(format!("unsupported minimap draw instruction at {va:#x}"));
            }
            Ok(bytes[prefix.len()..].to_vec())
        }
        fn word(bytes: Vec<u8>) -> Result<u32, String> {
            Ok(u32::from_le_bytes(
                bytes
                    .try_into()
                    .map_err(|_| "truncated minimap draw word")?,
            ))
        }
        fn imm(read: Read, va: u32, prefix: &[u8]) -> Result<u32, String> {
            word(operand(read, va, prefix, 4)?)
        }
        fn referenced(read: Read, va: u32, prefix: &[u8]) -> Result<f32, String> {
            let address = imm(read, va, prefix)?;
            let value = f32::from_bits(word(read(address, 4, false)?)?);
            if !value.is_finite() {
                return Err(format!("nonfinite minimap float at {va:#x}"));
            }
            Ok(value)
        }
        fn string(read: Read, start: u32) -> Result<String, String> {
            let mut bytes = Vec::new();
            for offset in 0..256 {
                let va = start.checked_add(offset).ok_or("minimap string overflow")?;
                let value = read(va, 1, false)?;
                let [byte] = value.as_slice() else {
                    return Err("truncated minimap string".into());
                };
                if *byte == 0 {
                    let text = String::from_utf8(bytes).map_err(|e| e.to_string())?;
                    return if text.is_empty() || text.chars().any(char::is_control) {
                        Err("invalid minimap string".into())
                    } else {
                        Ok(text)
                    };
                }
                bytes.push(*byte);
            }
            Err("overlong minimap string".into())
        }
        let read: Read = &mut read;

        // The image edge, from the updater's row count and its row stride shift.
        let edge = imm(read, 0x658afd, &[0xbe])?;
        if imm(read, 0x658d67, &[0xbe])? != edge
            || edge == 0
            || edge > 1 << 12
            || 1u32 << operand(read, 0x658dc6, &[0xc1, 0xe7], 1)?[0] != edge * 4
            || 1u32 << operand(read, 0x658ded, &[0xc1, 0xe6], 1)?[0] != edge * 4
        {
            return Err("minimap image edge relation".into());
        }
        let stride = edge * 4;

        // Border colour: the constructor stores one ARGB profile value, the updater loads it.
        operand(read, 0x620480, &[0x8d, 0x45, 0xf0], 0)?;
        operand(read, 0x620488, &[0x50], 0)?;
        let call = 0x620489u32;
        if imm(read, call, &[0xe8])? != 0x430390u32.wrapping_sub(call + 5) {
            return Err("minimap border colour parser".into());
        }
        let key_start = imm(read, 0x620473, &[0x6a, 0, 0x68])?;
        let key = string(read, key_start)?;
        let section_start = imm(read, 0x620483, &[0x68])?;
        let section = string(read, section_start)?;
        let field = imm(read, 0x62049d, &[0x89, 0x87])?;
        if imm(read, 0x658d59, &[0x8b, 0x83])? != field {
            return Err("minimap border colour field relation".into());
        }

        // Axis paddings: both are read from the fields the border lines are placed by.
        let width_fraction = imm(read, 0x658d1d, &[0xf3, 0x0f, 0x10, 0x83])?;
        let height_fraction = imm(read, 0x658d25, &[0xf3, 0x0f, 0x10, 0x8b])?;
        let pad_x = imm(read, 0x658d3a, &[0x8b, 0x83])?;
        let pad_y = imm(read, 0x658d40, &[0x8b, 0xbb])?;
        if width_fraction == height_fraction || pad_x == pad_y {
            return Err("minimap border axis fields".into());
        }
        operand(
            read,
            0x658d2d,
            &[0x0f, 0x2e, 0xc1, 0x9f, 0xf6, 0xc4, 0x44, 0x0f, 0x8b],
            4,
        )?;
        let threshold = operand(
            read,
            0x658d46,
            &[0x2b, 0xc7, 0x99, 0x33, 0xc2, 0x2b, 0xc2, 0x83, 0xf8],
            1,
        )?[0];
        operand(read, 0x658d50, &[0x0f, 0x8e], 4)?;
        // Taller than wide takes the column branch, otherwise the row branch.
        operand(read, 0x658d56, &[0x0f, 0x2f, 0xc8], 0)?;
        imm(read, 0x658d5f, &[0x8b, 0x93])?;
        operand(read, 0x658d65, &[0x76, 0x5f], 0)?;

        // Columns: four single-pixel stores per row, all through the left padding field.
        // The row pointer is advanced by one stride before the stores, which address back.
        if imm(read, 0x658d75, &[0x8d, 0x92])? != stride {
            return Err("minimap border column stride".into());
        }
        let store_base = (stride as i32).wrapping_neg();
        let mut columns = Vec::new();
        for (load, sub, store) in [
            (0x658d70u32, 0x658d7bu32, 0x658d81u32),
            (0x658d88, 0x658d8d, 0x658d93),
        ] {
            let last = i32::try_from(imm(read, load, &[0xb9])?).map_err(|_| "border column")?;
            if imm(read, sub, &[0x2b, 0x8b])? != pad_x {
                return Err("minimap border column padding".into());
            }
            if imm(read, store, &[0x89, 0x84, 0x8a])? as i32 != store_base {
                return Err("minimap border column store".into());
            }
            columns.push(Edge::EdgeMinusPad(edge as i32 - last));
        }
        for (load, store, n) in [(0x658d9au32, 0x658da0u32, 0i32), (0x658da7, 0x658dad, -1)] {
            if imm(read, load, &[0x8b, 0x8b])? != pad_x {
                return Err("minimap border column padding".into());
            }
            let displacement = imm(read, store, &[0x89, 0x84, 0x8a])? as i32;
            if displacement != store_base + 4 * n {
                return Err("minimap border column store".into());
            }
            columns.push(Edge::PadPlus(n));
        }
        operand(read, 0x658db4, &[0x83, 0xee, 0x01, 0x75, 0xb7], 0)?;

        // Rows: two runs of two rows, by the top padding.
        let first = imm(read, 0x658dc9, &[0x81, 0xc7])? as i32;
        let run = imm(read, 0x658dd1, &[0xbf])?;
        if first % stride as i32 != 0 || run != imm(read, 0x658de8, &[0xba])? || run % edge != 0 {
            return Err("minimap border row runs".into());
        }
        if imm(read, 0x658de2, &[0x2b, 0xb3])? != pad_y {
            return Err("minimap border row padding".into());
        }
        operand(read, 0x658dcf, &[0x03, 0xd7], 0)?;
        operand(read, 0x658df0, &[0x03, 0xb3], 4)?;
        let height = run / edge;
        let mut rows = Vec::new();
        for n in 0..height as i32 {
            rows.push(Edge::PadPlus(first / stride as i32 + n));
        }
        for n in 0..height as i32 {
            rows.push(Edge::EdgeMinusPad(-n));
        }

        // Marker block: half = 1 << (class + increments); the block is two halves wide.
        if imm(read, 0x65c0f5, &[0xbb])? != 1 {
            return Err("minimap marker base".into());
        }
        operand(read, 0x65c0fd, &[0x8b, 0x4d, 0xd4, 0x41], 0)?;
        operand(read, 0x65c107, &[0xd3, 0xe3], 0)?;
        if imm(read, 0x65c109, &[0xb9])? != edge + 1 {
            return Err("minimap marker clamp".into());
        }
        operand(read, 0x65c15b, &[0x8d, 0x04, 0x1b, 0x2b, 0xc8], 0)?;
        operand(read, 0x65c1e9, &[0x83, 0x7d, 0xd4, 0x00], 0)?;
        operand(read, 0x65c1fa, &[0x0f, 0x84, 0xf0, 0, 0, 0], 0)?;
        // The class is the structure test's 0 or 1 result.
        if imm(read, 0x696b72, &[0xb8])? != 1 {
            return Err("minimap marker class value".into());
        }
        operand(read, 0x696b7b, &[0x33, 0xc0], 0)?;
        let marker_sides = [0, 1].map(|class: u32| 2 * (1u32 << (class + 1)));

        let fog_fraction_bits = operand(read, 0x657496, &[0xc1, 0xe0], 1)?[0];
        if fog_fraction_bits == 0 || fog_fraction_bits >= 31 {
            return Err("minimap fog fixed-point width".into());
        }
        let unit = 1u32 << fog_fraction_bits;
        if operand(read, 0x6574d0, &[0xc1, 0xe0], 1)?[0] != fog_fraction_bits
            || operand(read, 0x65751a, &[0xc1, 0xe9], 1)?[0] != fog_fraction_bits
            || operand(read, 0x65758a, &[0xc1, 0xee], 1)?[0] != fog_fraction_bits
            || imm(read, 0x657551, &[0xba])? != unit
            || imm(read, 0x657510, &[0x81, 0xfb])? != unit
            || imm(read, 0x657580, &[0x81, 0xfa])? != unit
            || imm(read, 0x65751d, &[0x69, 0xc1])? != unit.wrapping_neg()
            || imm(read, 0x65758d, &[0x69, 0xc6])? != unit.wrapping_neg()
            || imm(read, 0x657459, &[0xbe])? != edge - 1
            || imm(read, 0x6574a3, &[0xbe])? != edge - 1
            || imm(read, 0x6574da, &[0xb9])? != edge - 1
        {
            return Err("minimap fog fixed-point relationships".into());
        }
        operand(read, 0x657530, &[0x32, 0xdb], 0)?;
        operand(read, 0x6575d9, &[0xd1, 0xe9], 0)?;

        let keys = [0x6376ae, 0x6376e2, 0x637716]
            .map(|va| imm(read, va, &[0x68]).and_then(|address| string(read, address)));
        let [normal, entangled, haunted] = keys;
        let keys = [normal?, entangled?, haunted?];
        let mut classes = [[0; 4]; 3];
        for (class, va) in classes.iter_mut().zip([0xb62690, 0xb941a0, 0xb263d0]) {
            *class = imm(read, va, &[0xb8])?.to_be_bytes();
            operand(read, va + 5, &[0xc3], 0)?;
        }
        let side = imm(read, 0x659720, &[0xbb])?;
        if side == 0
            || side > edge
            || imm(read, 0x659711, &[0xc7, 0x45, 0xe8])? != side
            || imm(read, 0x659776, &[0x81, 0xc6])? != (edge - side) * 4
        {
            return Err("minimap resource blit dimensions".into());
        }
        let offset = operand(read, 0x659584, &[0x83, 0xc0], 1)?[0] as i8;
        let half = u32::try_from(-i32::from(offset)).map_err(|_| "resource centre offset")?;
        if half * 2 != side || imm(read, 0x659519, &[0xba])? != edge - half {
            return Err("minimap resource centre relation".into());
        }
        let clamp = imm(read, 0x659378, &[0xb8])?;
        let clamp_inset = edge.checked_sub(clamp).ok_or("resource clamp")?;
        if clamp_inset <= side || clamp_inset > edge {
            return Err("minimap resource clamp relation".into());
        }
        let green = imm(read, 0x62d0ce, &[0x69, 0xd0])?;
        let red = u32::from(operand(read, 0x62d0df, &[0x6b, 0xc0], 1)?[0]);
        let first_scale = operand(read, 0x62d0ea, &[0x8d, 0x04], 5)?;
        operand(read, 0x62d0f1, &[0x2b, 0xc1], 0)?;
        let second_scale = operand(read, 0x62d0f3, &[0x8d, 0x04], 1)?[0];
        if first_scale[0] & 0x3f != 0x0d || first_scale[1..] != [0; 4] || second_scale & 0x3f != 2 {
            return Err("minimap resource luminance registers".into());
        }
        let blue = ((1u32 << (first_scale[0] >> 6)) - 1) * (1u32 << (second_scale >> 6));
        let color_shift = operand(read, 0x62d0f6, &[0xc1, 0xe8], 1)?[0];
        if color_shift >= 16
            || red.checked_add(green).and_then(|sum| sum.checked_add(blue))
                != Some(1u32 << color_shift)
        {
            return Err("minimap resource luminance relation".into());
        }
        let tint_bias = imm(read, 0x63a873, &[0x81, 0xc2])?;
        for (add, prefix, shift, reg) in [
            (0x63a873, &[0x81, 0xc2][..], 0x63a879, 0xea),
            (0x63a87c, &[0x81, 0xc1][..], 0x63a885, 0xe9),
            (0x63a8a2, &[0x81, 0xc1][..], 0x63a8a8, 0xe9),
        ] {
            if imm(read, add, prefix)? != tint_bias
                || operand(read, shift, &[0xc1, reg], 1)?[0] != color_shift
            {
                return Err("minimap resource tint relation".into());
            }
        }
        if tint_bias != (1u32 << color_shift) - 1 {
            return Err("minimap resource tint rounding".into());
        }
        let resources = ResourceIcons {
            keys,
            classes,
            side,
            half,
            clamp_inset,
            luminance: [red, green, blue],
            color_shift,
            tint_bias,
        };

        let points = PointProjection {
            grid_shift: operand(read, 0x654104, &[0xc1, 0x3e], 1)?[0],
            bias: referenced(read, 0x659550, &[0xf3, 0x0f, 0x58, 5])?,
            bit_shift: operand(read, 0x659574, &[0xc1, 0xe8], 1)?[0],
        };
        if points.grid_shift >= 31
            || points.bit_shift >= 31
            || points.bias <= 0.
            || operand(read, 0x654109, &[0xc1, 0x7e, 4], 1)?[0] != points.grid_shift
            || operand(read, 0x6595a1, &[0xc1, 0xe8], 1)?[0] != points.bit_shift
            || operand(read, 0x65c163, &[0xc1, 0xe8], 1)?[0] != points.bit_shift
            || referenced(read, 0x659591, &[0xf3, 0x0f, 0x58, 5])? != points.bias
            || referenced(read, 0x65c13a, &[0xf3, 0x0f, 0x58, 5])? != points.bias
        {
            return Err("minimap marker grid/float encoding relationships".into());
        }
        operand(read, 0x659577, &[0x0f, 0xb6, 0xc8], 0)?;
        if (points.bias.to_bits() >> points.bit_shift) & u32::from(u8::MAX) != 0
            || ((points.bias + (edge - 1) as f32).to_bits() >> points.bit_shift)
                & u32::from(u8::MAX)
                != edge - 1
        {
            return Err("unsupported minimap marker float encoding".into());
        }

        let hero_key_address = imm(read, 0x637651, &[0x68])?;
        let hero_key = string(read, hero_key_address)?;
        operand(read, 0x63764d, &[0x6a, 0], 0)?;
        let preserved = u32::from(operand(read, 0x71b7fa, &[0x83, 0xe6], 1)?[0]);
        let forced = u32::from(operand(read, 0x71b7fd, &[0x83, 0xce], 1)?[0]);
        let selector = u32::from(operand(read, 0x7def4a, &[0x83, 0xe0], 1)?[0]);
        if selector > 3 || preserved & selector & !forced != 0 {
            return Err("minimap hero texture filter selector".into());
        }
        let filters = crate::command_layout::texture_filter_record_mipmapped(
            &mut crate::unit_panel_layout::Reader(&mut *read),
            forced & selector,
        )?;
        let mut addresses = [0; 2];
        for (address, (shift, mask, table)) in addresses.iter_mut().zip([
            (0x7def8f, 0x7def92, 0x7def95),
            (0x7defab, 0x7defae, 0x7defb1),
        ]) {
            let shift = operand(read, shift, &[0xc1, 0xe8], 1)?[0];
            let mask = u32::from(operand(read, mask, &[0x83, 0xe0], 1)?[0]);
            if shift >= 8 || mask > 1 || ((preserved >> shift) & mask & !(forced >> shift)) != 0 {
                return Err("minimap hero texture address selector".into());
            }
            let index = (forced >> shift) & mask;
            let table = imm(read, table, &[0xff, 0x34, 0x85])?;
            *address = word(read(
                table
                    .checked_add(index * 4)
                    .ok_or("hero address table overflow")?,
                4,
                false,
            )?)?;
            if !matches!(*address, 1 | 3) {
                return Err("unsupported minimap hero address mode".into());
            }
        }
        let uv_address = imm(read, 0x640341, &[0x68])?;
        let bytes = read(uv_address, 32, false)?;
        if bytes.len() != 32 {
            return Err("truncated minimap hero UVs".into());
        }
        let uv: Vec<f32> = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| f32::from_le_bytes(*b))
            .collect();
        if uv.iter().any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
            || uv[0] == uv[6]
            || uv[1] == uv[7]
            || uv[2] != uv[0]
            || uv[3] != uv[7]
            || uv[4] != uv[6]
            || uv[5] != uv[1]
        {
            return Err("unsupported minimap hero UV rectangle".into());
        }
        let hero = Hero {
            key: hero_key,
            filters,
            addresses,
            uv: [uv[0], uv[1], uv[6], uv[7]],
            initial_phase: f32::from_bits(imm(read, 0x61fc3d, &[0xc7, 0x87, 0, 5, 0, 0])?),
            reset_phase: f32::from_bits(imm(read, 0x640659, &[0xc7, 0x87, 0, 5, 0, 0])?),
            midpoint: referenced(read, 0x64038c, &[0x0f, 0x2f, 0x0d])?,
            period: referenced(read, 0x64039b, &[0xf3, 0x0f, 0x10, 5])?,
            alpha_scale: referenced(read, 0x6403ac, &[0xf3, 0x0f, 0x59, 5])?,
            alpha_floor: referenced(read, 0x6403b9, &[0xf3, 0x0f, 0x58, 5])?,
            geometry_edge: referenced(read, 0x637986, &[0xf3, 0x0f, 0x10, 0x1d])?,
            world_scale: referenced(read, 0x63797b, &[0xf3, 0x0f, 0x59, 5])?,
            size_scale: referenced(read, 0x6379b2, &[0xf3, 0x0f, 0x59, 0x15])?,
            half: referenced(read, 0x6379ba, &[0xf3, 0x0f, 0x59, 0x15])?,
        };
        if hero.geometry_edge != edge as f32
            || hero.world_scale <= 0.
            || hero.size_scale <= 0.
            || hero.half <= 0.
            || hero.midpoint <= 0.
            || hero.period != hero.midpoint * 2.
            || hero.alpha_scale < 0.
            || hero.alpha_floor < 0.
            || hero.alpha_floor + hero.alpha_scale * hero.midpoint > f32::from(u8::MAX)
            || !hero.initial_phase.is_finite()
            || !(0. ..=hero.period).contains(&hero.initial_phase)
            || !hero.reset_phase.is_finite()
            || !(0. ..=hero.period).contains(&hero.reset_phase)
        {
            return Err("minimap hero geometry or pulse relationships".into());
        }
        if referenced(read, 0x641311, &[0xf3, 0x0f, 0x5d, 5])? != hero.period {
            return Err("minimap hero UI clock clamp".into());
        }

        // Outline colour: the control's alpha shifted up, or'd with a constant RGB.
        operand(
            read,
            0x640676,
            &[0x0f, 0xb6, 0x87, 0xb4, 0, 0, 0, 0xc1, 0xe0, 0x18],
            0,
        )?;
        let rgb = imm(read, 0x640680, &[0x0d])?;
        if rgb >> 24 != 0 {
            return Err("minimap outline alpha operand".into());
        }
        let [_, red, green, blue] = rgb.to_be_bytes();

        Ok(Self {
            origin,
            image_sha256,
            edge,
            resources,
            fog_fraction_bits,
            hero,
            points,
            border: Border {
                section,
                key,
                minimum_pad_difference: u32::from(threshold),
                columns,
                rows,
            },
            marker_sides,
            outline_rgb: [red, green, blue],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    const EDGE: u32 = 64;
    const STRIDE: u32 = EDGE * 4;

    fn origin() -> AssetIdentity {
        AssetIdentity {
            source: "synthetic minimap draw image".into(),
            edition: "fixture".into(),
            version: "fixture".into(),
        }
    }
    fn put(data: &mut BTreeMap<u32, u8>, va: u32, bytes: &[u8]) {
        for (index, byte) in bytes.iter().enumerate() {
            data.insert(va + index as u32, *byte);
        }
    }
    fn op(data: &mut BTreeMap<u32, u8>, va: u32, prefix: &[u8], value: u32) {
        put(data, va, prefix);
        put(data, va + prefix.len() as u32, &value.to_le_bytes());
    }
    /// A client with a 64 pixel image, other field offsets, other keys and other constants.
    fn fixture() -> BTreeMap<u32, u8> {
        let mut d = BTreeMap::new();
        let (pad_x, pad_y) = (0x2f0u32, 0x2f4u32);
        op(&mut d, 0x658afd, &[0xbe], EDGE);
        op(&mut d, 0x658d67, &[0xbe], EDGE);
        put(&mut d, 0x658dc6, &[0xc1, 0xe7, 8]);
        put(&mut d, 0x658ded, &[0xc1, 0xe6, 8]);
        put(&mut d, 0x620480, &[0x8d, 0x45, 0xf0]);
        put(&mut d, 0x620488, &[0x50]);
        op(
            &mut d,
            0x620489,
            &[0xe8],
            0x430390u32.wrapping_sub(0x620489 + 5),
        );
        op(&mut d, 0x620473, &[0x6a, 0, 0x68], 0x1000);
        op(&mut d, 0x620483, &[0x68], 0x1010);
        put(&mut d, 0x1000, b"RimColor\0");
        put(&mut d, 0x1010, b"Frame\0");
        op(&mut d, 0x62049d, &[0x89, 0x87], 0xa44);
        op(&mut d, 0x658d59, &[0x8b, 0x83], 0xa44);
        op(&mut d, 0x658d1d, &[0xf3, 0x0f, 0x10, 0x83], 0x2e8);
        op(&mut d, 0x658d25, &[0xf3, 0x0f, 0x10, 0x8b], 0x2ec);
        op(&mut d, 0x658d3a, &[0x8b, 0x83], pad_x);
        op(&mut d, 0x658d40, &[0x8b, 0xbb], pad_y);
        op(
            &mut d,
            0x658d2d,
            &[0x0f, 0x2e, 0xc1, 0x9f, 0xf6, 0xc4, 0x44, 0x0f, 0x8b],
            0xc8,
        );
        put(
            &mut d,
            0x658d46,
            &[0x2b, 0xc7, 0x99, 0x33, 0xc2, 0x2b, 0xc2, 0x83, 0xf8, 3],
        );
        op(&mut d, 0x658d50, &[0x0f, 0x8e], 0xac);
        put(&mut d, 0x658d56, &[0x0f, 0x2f, 0xc8]);
        op(&mut d, 0x658d5f, &[0x8b, 0x93], 0x1e4);
        put(&mut d, 0x658d65, &[0x76, 0x5f]);
        op(&mut d, 0x658d75, &[0x8d, 0x92], STRIDE);
        let back = STRIDE.wrapping_neg();
        op(&mut d, 0x658d70, &[0xb9], EDGE - 3);
        op(&mut d, 0x658d7b, &[0x2b, 0x8b], pad_x);
        op(&mut d, 0x658d81, &[0x89, 0x84, 0x8a], back);
        op(&mut d, 0x658d88, &[0xb9], EDGE - 5);
        op(&mut d, 0x658d8d, &[0x2b, 0x8b], pad_x);
        op(&mut d, 0x658d93, &[0x89, 0x84, 0x8a], back);
        op(&mut d, 0x658d9a, &[0x8b, 0x8b], pad_x);
        op(&mut d, 0x658da0, &[0x89, 0x84, 0x8a], back);
        op(&mut d, 0x658da7, &[0x8b, 0x8b], pad_x);
        op(&mut d, 0x658dad, &[0x89, 0x84, 0x8a], back.wrapping_sub(4));
        put(&mut d, 0x658db4, &[0x83, 0xee, 0x01, 0x75, 0xb7]);
        op(&mut d, 0x658dc9, &[0x81, 0xc7], back.wrapping_sub(STRIDE));
        op(&mut d, 0x658dd1, &[0xbf], EDGE * 3);
        op(&mut d, 0x658de2, &[0x2b, 0xb3], pad_y);
        op(&mut d, 0x658de8, &[0xba], EDGE * 3);
        put(&mut d, 0x658dcf, &[0x03, 0xd7]);
        op(&mut d, 0x658df0, &[0x03, 0xb3], 0x1e4);
        op(&mut d, 0x65c0f5, &[0xbb], 1);
        put(&mut d, 0x65c0fd, &[0x8b, 0x4d, 0xd4, 0x41]);
        put(&mut d, 0x65c107, &[0xd3, 0xe3]);
        op(&mut d, 0x65c109, &[0xb9], EDGE + 1);
        put(&mut d, 0x65c15b, &[0x8d, 0x04, 0x1b, 0x2b, 0xc8]);
        put(&mut d, 0x65c1e9, &[0x83, 0x7d, 0xd4, 0x00]);
        put(&mut d, 0x65c1fa, &[0x0f, 0x84, 0xf0, 0, 0, 0]);
        op(&mut d, 0x696b72, &[0xb8], 1);
        put(&mut d, 0x696b7b, &[0x33, 0xc0]);
        put(
            &mut d,
            0x640676,
            &[0x0f, 0xb6, 0x87, 0xb4, 0, 0, 0, 0xc1, 0xe0, 0x18],
        );
        op(&mut d, 0x640680, &[0x0d], 0x0012_3456);
        for (site, reg) in [
            (0x657496, 0xe0),
            (0x6574d0, 0xe0),
            (0x65751a, 0xe9),
            (0x65758a, 0xee),
        ] {
            put(&mut d, site, &[0xc1, reg, 12]);
        }
        op(&mut d, 0x657551, &[0xba], 1 << 12);
        for (site, reg) in [(0x657510, 0xfb), (0x657580, 0xfa)] {
            op(&mut d, site, &[0x81, reg], 1 << 12);
        }
        for (site, reg) in [(0x65751d, 0xc1), (0x65758d, 0xc6)] {
            op(&mut d, site, &[0x69, reg], (1u32 << 12).wrapping_neg());
        }
        op(&mut d, 0x657459, &[0xbe], EDGE - 1);
        op(&mut d, 0x6574a3, &[0xbe], EDGE - 1);
        op(&mut d, 0x6574da, &[0xb9], EDGE - 1);
        put(&mut d, 0x657530, &[0x32, 0xdb]);
        put(&mut d, 0x6575d9, &[0xd1, 0xe9]);
        for (site, ptr, value) in [
            (0x6376ae, 0x1100, "Mine"),
            (0x6376e2, 0x1120, "Roots"),
            (0x637716, 0x1140, "Ghost"),
        ] {
            op(&mut d, site, &[0x68], ptr);
            put(&mut d, ptr, value.as_bytes());
            put(&mut d, ptr + value.len() as u32, &[0]);
        }
        for (site, value) in [
            (0xb62690, *b"Mine"),
            (0xb941a0, *b"Root"),
            (0xb263d0, *b"Ghst"),
        ] {
            op(&mut d, site, &[0xb8], u32::from_be_bytes(value));
            put(&mut d, site + 5, &[0xc3]);
        }
        op(&mut d, 0x659720, &[0xbb], 8);
        op(&mut d, 0x659711, &[0xc7, 0x45, 0xe8], 8);
        op(&mut d, 0x659776, &[0x81, 0xc6], (EDGE - 8) * 4);
        put(&mut d, 0x659584, &[0x83, 0xc0, 0xfc]);
        op(&mut d, 0x659519, &[0xba], EDGE - 4);
        op(&mut d, 0x659378, &[0xb8], EDGE - 9);
        op(&mut d, 0x62d0ce, &[0x69, 0xd0], 160);
        put(&mut d, 0x62d0df, &[0x6b, 0xc0, 68]);
        put(
            &mut d,
            0x62d0ea,
            &[0x8d, 0x04, 0xcd, 0, 0, 0, 0, 0x2b, 0xc1, 0x8d, 0x04, 0x82],
        );
        put(&mut d, 0x62d0f6, &[0xc1, 0xe8, 8]);
        for (site, reg) in [(0x63a873, 0xc2), (0x63a87c, 0xc1), (0x63a8a2, 0xc1)] {
            op(&mut d, site, &[0x81, reg], 255);
        }
        for (site, reg) in [(0x63a879, 0xea), (0x63a885, 0xe9), (0x63a8a8, 0xe9)] {
            put(&mut d, site, &[0xc1, reg, 8]);
        }
        put(&mut d, 0x654104, &[0xc1, 0x3e, 1]);
        put(&mut d, 0x654109, &[0xc1, 0x7e, 4, 1]);
        for site in [0x659550, 0x659591, 0x65c13a] {
            op(&mut d, site, &[0xf3, 0x0f, 0x58, 5], 0x2120);
        }
        put(&mut d, 0x2120, &1024f32.to_le_bytes());
        for site in [0x659574, 0x6595a1, 0x65c163] {
            put(&mut d, site, &[0xc1, 0xe8, 13]);
        }
        put(&mut d, 0x659577, &[0x0f, 0xb6, 0xc8]);
        op(&mut d, 0x637651, &[0x68], 0x1160);
        put(&mut d, 0x1160, b"Champion\0");
        put(&mut d, 0x63764d, &[0x6a, 0]);
        put(&mut d, 0x71b7fa, &[0x83, 0xe6, 0xce]);
        put(&mut d, 0x71b7fd, &[0x83, 0xce, 0x0e]);
        put(&mut d, 0x7def4a, &[0x83, 0xe0, 3]);
        for (site, table, value) in [
            (0x7def50, 0x2000, 2u32),
            (0x7def61, 0x2020, 1),
            (0x7def74, 0x2040, 2),
        ] {
            op(&mut d, site, &[0xff, 0x34, 0xb5], table);
            put(&mut d, table + 24, &value.to_le_bytes());
        }
        for (shift, mask, site, amount) in [
            (0x7def8f, 0x7def92, 0x7def95, 2),
            (0x7defab, 0x7defae, 0x7defb1, 3),
        ] {
            put(&mut d, shift, &[0xc1, 0xe8, amount]);
            put(&mut d, mask, &[0x83, 0xe0, 1]);
            op(&mut d, site, &[0xff, 0x34, 0x85], 0x2060);
        }
        put(&mut d, 0x2064, &3u32.to_le_bytes());
        op(&mut d, 0x640341, &[0x68], 0x2080);
        for (i, value) in [0.1f32, 0.2, 0.1, 0.8, 0.9, 0.2, 0.9, 0.8]
            .into_iter()
            .enumerate()
        {
            put(&mut d, 0x2080 + i as u32 * 4, &value.to_le_bytes());
        }
        op(
            &mut d,
            0x61fc3d,
            &[0xc7, 0x87, 0, 5, 0, 0],
            0.25f32.to_bits(),
        );
        op(&mut d, 0x640659, &[0xc7, 0x87, 0, 5, 0, 0], 0f32.to_bits());
        for (site, prefix, ptr, value) in [
            (0x64038c, &[0x0f, 0x2f, 0x0d][..], 0x2100, 0.5f32),
            (0x64039b, &[0xf3, 0x0f, 0x10, 5][..], 0x2104, 1.),
            (0x6403ac, &[0xf3, 0x0f, 0x59, 5][..], 0x2108, 100.),
            (0x6403b9, &[0xf3, 0x0f, 0x58, 5][..], 0x210c, 80.),
            (0x637986, &[0xf3, 0x0f, 0x10, 0x1d][..], 0x2110, EDGE as f32),
            (0x63797b, &[0xf3, 0x0f, 0x59, 5][..], 0x2114, 0.25),
            (0x6379b2, &[0xf3, 0x0f, 0x59, 0x15][..], 0x2118, 0.02),
            (0x6379ba, &[0xf3, 0x0f, 0x59, 0x15][..], 0x211c, 0.5),
            (0x641311, &[0xf3, 0x0f, 0x5d, 5][..], 0x2104, 1.),
        ] {
            op(&mut d, site, prefix, ptr);
            put(&mut d, ptr, &value.to_le_bytes());
        }
        d
    }
    fn decode(d: &BTreeMap<u32, u8>) -> Result<Draw, String> {
        Draw::decode(origin(), sha256(b"fixture"), |va, n, _| {
            (0..n)
                .map(|i| {
                    d.get(&(va + i as u32))
                        .copied()
                        .ok_or_else(|| format!("fixture missing {:#x}", va + i as u32))
                })
                .collect()
        })
    }

    #[test]
    fn border_markers_and_outline_come_from_the_operands() {
        let draw = decode(&fixture()).unwrap();
        assert_eq!(draw.edge, EDGE);
        assert_eq!(draw.fog_fraction_bits, 12);
        assert_eq!(draw.resources.keys, ["Mine", "Roots", "Ghost"]);
        assert_eq!(draw.resources.classes, [*b"Mine", *b"Root", *b"Ghst"]);
        assert_eq!(
            (
                draw.resources.side,
                draw.resources.half,
                draw.resources.clamp_inset
            ),
            (8, 4, 9)
        );
        assert_eq!(draw.resources.luminance, [68, 160, 28]);
        assert_eq!(
            draw.resources
                .tint([255, 255, 255, 128], [100, 200, 50, 255]),
            [100, 200, 50, 128]
        );
        assert_eq!(draw.border.section, "Frame");
        assert_eq!(draw.border.key, "RimColor");
        assert_eq!(draw.border.minimum_pad_difference, 3);
        assert_eq!(
            draw.border.columns,
            [
                Edge::EdgeMinusPad(3),
                Edge::EdgeMinusPad(5),
                Edge::PadPlus(0),
                Edge::PadPlus(-1)
            ]
        );
        assert_eq!(
            draw.border.rows,
            [
                Edge::PadPlus(-2),
                Edge::PadPlus(-1),
                Edge::PadPlus(0),
                Edge::EdgeMinusPad(0),
                Edge::EdgeMinusPad(-1),
                Edge::EdgeMinusPad(-2)
            ]
        );
        assert_eq!(draw.marker_sides, [4, 8]);
        assert_eq!(draw.outline_rgb, [0x12, 0x34, 0x56]);
    }

    #[test]
    fn marker_projection_uses_source_grid_scale_and_float_bit_encoding() {
        let points = decode(&fixture()).unwrap().points;
        assert_eq!(points.grid_shift, 1);
        assert_eq!(
            points
                .project([[2, 4], [10, 8]], 64, [0, 16], [6, 6])
                .unwrap(),
            Some([31., 33.])
        );
        assert_eq!(
            points
                .project([[2, 4], [10, 8]], 64, [0, 16], [1, 6])
                .unwrap(),
            None
        );
        assert!(
            points
                .project([[2, 4], [2, 8]], 64, [0, 16], [2, 6])
                .is_err()
        );
    }

    #[test]
    fn hero_geometry_uv_sampler_and_clock_are_source_driven() {
        let hero = decode(&fixture()).unwrap().hero;
        assert_eq!(hero.key, "Champion");
        assert_eq!(hero.filters, [2, 1, 2]);
        assert_eq!(hero.addresses, [3, 3]);
        assert_eq!(hero.uv, [0.1, 0.2, 0.9, 0.8]);
        assert_eq!(hero.half_size([[0., 0.], [256., 128.]]).unwrap(), 0.01);
        let mut phase = hero.initial_phase;
        assert_eq!(hero.advance(&mut phase, 0.25, true).unwrap(), 130);
        assert_eq!(hero.advance(&mut phase, 0.75, false).unwrap(), 80);
        assert_eq!(phase, 1.);
        assert_eq!(hero.advance(&mut phase, 0., true).unwrap(), 80);
        assert_eq!(phase, 0.);
        assert!(hero.advance(&mut phase, -1., true).is_err());
        assert!(hero.half_size([[0.; 2]; 2]).is_err());
    }

    #[test]
    fn resource_luminance_scales_are_read_from_instruction_operands() {
        let mut data = fixture();
        put(&mut data, 0x62d0ec, &[0x8d]);
        op(&mut data, 0x62d0ce, &[0x69, 0xd0], 176);
        assert_eq!(decode(&data).unwrap().resources.luminance, [68, 176, 12]);
    }

    #[test]
    fn edges_index_relative_to_padding_and_drop_outside_the_image() {
        assert_eq!(Edge::PadPlus(-1).index(64, 10), Some(9));
        assert_eq!(Edge::PadPlus(-1).index(64, 0), None);
        assert_eq!(Edge::EdgeMinusPad(1).index(64, 10), Some(53));
        assert_eq!(Edge::EdgeMinusPad(-1).index(64, 0), None);
        assert_eq!(Edge::EdgeMinusPad(0).index(64, 64), Some(0));
    }

    #[test]
    fn changed_instructions_and_truncations_refuse() {
        let base = fixture();
        for (va, bytes) in [
            (0x658d67, vec![0xbd]),
            (0x658dc8, vec![9]),
            (0x620489, vec![0xe9]),
            (0x62049f, vec![0x01]),
            (0x658d5b, vec![0x11]),
            (0x658d4f, vec![0x83, 0xf9]),
            (0x658d81, vec![0x88]),
            (0x658d7d, vec![0x12]),
            (0x658dad, vec![0x00, 0xfd]),
            (0x658dd2, vec![0xff]),
            (0x65c100, vec![0x40]),
            (0x65c0f6, vec![2]),
            (0x65c109, vec![0xba]),
            (0x696b73, vec![2]),
            (0x640684, vec![1]),
            (0x654106, vec![31]),
            (0x659576, vec![14]),
            (0x2120, f32::NAN.to_le_bytes().to_vec()),
            (0x63764e, vec![1]),
            (0x71b7fc, vec![0xff]),
            (0x2110, 0f32.to_le_bytes().to_vec()),
            (0x2104, f32::NAN.to_le_bytes().to_vec()),
            (0x2080, (-1f32).to_le_bytes().to_vec()),
            (0x657498, vec![31]),
            (0x6574d2, vec![16]),
            (0x657552, vec![1]),
            (0x657531, vec![0xc9]),
            (0x6575da, vec![0xea]),
            (0x659721, vec![7]),
            (0x659585, vec![0xc1]),
            (0xb941a5, vec![0xc2]),
            (0x62d0d0, vec![159]),
            (0x62d0d0, u32::MAX.to_le_bytes().to_vec()),
            (0x63a875, vec![0]),
        ] {
            let mut d = base.clone();
            put(&mut d, va, &bytes);
            assert!(decode(&d).is_err(), "mutant {va:#x}");
        }
        for va in base.keys() {
            let mut d = base.clone();
            d.remove(va);
            assert!(decode(&d).is_err(), "truncation {va:#x}");
        }
        assert!(Draw::reviewed_legacy(b"not a client", origin()).is_err());
    }

    #[test]
    #[ignore = "requires PCX_PLAYER_RESOURCE_IMAGE selecting the reviewed local client"]
    fn supplied_client_draw_operands_are_admitted() {
        let path = std::env::var("PCX_PLAYER_RESOURCE_IMAGE").unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let draw = Draw::reviewed_legacy(
            &bytes,
            AssetIdentity {
                source: path,
                edition: "explicit selected local client".into(),
                version: "reviewed client".into(),
            },
        )
        .unwrap();
        assert_eq!(draw.image_sha256, REVIEWED_IMAGE);
        assert_eq!(draw.border.key, "MinimapColorBorder");
        assert_eq!(draw.border.section, "Minimap");
        assert_eq!(draw.marker_sides, [4, 8]);
        assert_eq!(draw.edge, 256);
    }
}
