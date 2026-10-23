//! Selected-unit title binding and portrait readouts from the reviewed client.
use crate::{
    inventory_layout::Rect,
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    scripts::AssetIdentity,
};

#[derive(Clone, Debug)]
pub struct Rules {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    pub frame: String,
    pub title: String,
    /// Native TOPLEFT attached to logical-root BOTTOMLEFT; y is the top edge.
    pub health: Rect,
    pub mana: Rect,
    pub font: String,
    pub section: String,
    pub height: String,
    pub shadow: u32,
    pub shadow_offset: [f32; 2],
    pub mana_colour: u32,
    pub justification: u8,
    pub portrait: Portrait,
    format: [String; 3],
    health_colour: u32,
    high: f32,
    low: f32,
    offset: f32,
    scale: f32,
    low_divisor: f32,
    full_channel: u8,
}

#[derive(Clone, Debug)]
pub struct Portrait {
    pub light: crate::day_night::Sun,
    base: Rect,
    aspect: f32,
    center: f32,
    numerator: f32,
    logical_width: f32,
    model_format: String,
    path_bytes: usize,
}
impl Portrait {
    pub fn rect(&self, aspect: f32) -> Result<Rect, String> {
        if !aspect.is_finite() || aspect <= 0. {
            return Err("unit portrait: invalid target aspect".into());
        }
        let mut rect = self.base;
        if aspect > self.aspect {
            rect.width *= self.aspect / aspect;
            rect.x = (self.center - self.numerator / aspect) * self.logical_width;
        }
        Ok(rect)
    }
    /// The selected unit model has already resolved its edition/variation.
    pub fn member(&self, unit_model: &str) -> Result<String, String> {
        let (stem, extension) = unit_model
            .rsplit_once('.')
            .ok_or("unit portrait: model extension missing")?;
        if !extension.eq_ignore_ascii_case("mdx")
            || stem.is_empty()
            || extension.contains(['\\', '/'])
        {
            return Err("unit portrait: unsupported source model path".into());
        }
        let member = self.model_format.replacen("%s", stem, 1);
        if member.len() >= self.path_bytes || member.chars().any(char::is_control) {
            return Err("unit portrait: model path budget".into());
        }
        Ok(member)
    }
}

struct Reader<F>(F);
impl<F: FnMut(u32, usize, bool) -> Result<Vec<u8>, String>> Reader<F> {
    fn bytes(&mut self, va: u32, n: usize, code: bool) -> Result<Vec<u8>, String> {
        let bytes = (self.0)(va, n, code)?;
        if bytes.len() != n {
            return Err(format!("unit panel: truncated window {va:#x}"));
        }
        Ok(bytes)
    }
    fn word(&mut self, va: u32, prefix: &[u8]) -> Result<u32, String> {
        let bytes = self.bytes(va, prefix.len() + 4, true)?;
        if !bytes.starts_with(prefix) {
            return Err(format!("unit panel: instruction {va:#x}"));
        }
        Ok(u32::from_le_bytes(
            bytes[prefix.len()..].try_into().unwrap(),
        ))
    }
    fn number(&mut self, va: u32, prefix: &[u8]) -> Result<f32, String> {
        let value = f32::from_bits(self.word(va, prefix)?);
        if !value.is_finite() {
            return Err(format!("unit panel: nonfinite number {va:#x}"));
        }
        Ok(value)
    }
    fn referenced(&mut self, va: u32, prefix: &[u8]) -> Result<f32, String> {
        let pointer = self.word(va, prefix)?;
        let bytes = self.bytes(pointer, 4, false)?;
        let value = f32::from_le_bytes(bytes.try_into().unwrap());
        if !value.is_finite() {
            return Err("unit panel: nonfinite referenced value".into());
        }
        Ok(value)
    }
    fn string_at(&mut self, pointer: u32) -> Result<String, String> {
        let mut bytes = Vec::new();
        for offset in 0..4096 {
            let va = pointer
                .checked_add(offset)
                .ok_or("unit panel: string overflow")?;
            let byte = self.bytes(va, 1, false)?[0];
            if byte == 0 {
                return String::from_utf8(bytes).map_err(|e| e.to_string());
            }
            bytes.push(byte);
        }
        Err("unit panel: string budget".into())
    }
    fn string(&mut self, va: u32) -> Result<String, String> {
        let pointer = self.word(va, &[0x68])?;
        self.string_at(pointer)
    }
    fn rect(&mut self, xywh: [u32; 4], anchor: u32) -> Result<Rect, String> {
        if self.bytes(anchor, 5, true)? != [0x6a, 6, 0x56, 0x6a, 0] {
            return Err("unit panel: unsupported portrait anchor".into());
        }
        let rect = Rect {
            x: self.number(xywh[0], &[0xc7, 4, 0x24])?,
            y: self.number(xywh[1], &[0xc7, 0x44, 0x24, 4])?,
            width: self.number(xywh[2], &[0xc7, 4, 0x24])?,
            height: self.number(xywh[3], &[0xc7, 4, 0x24])?,
        };
        if rect.width <= 0. || rect.height <= 0. {
            return Err("unit panel: invalid text extent".into());
        }
        Ok(rect)
    }
}
impl Rules {
    pub fn reviewed_legacy(image: &[u8], origin: AssetIdentity) -> Result<Self, String> {
        let digest = sha256(image);
        if image.len() > 32 << 20
            || digest != REVIEWED_IMAGE
            || [&origin.source, &origin.edition, &origin.version]
                .iter()
                .any(|s| s.trim().is_empty())
        {
            return Err("unit panel: unsupported client identity".into());
        }
        let pe = Pe::parse(
            image,
            RuleInputBudget {
                image_bytes: 32 << 20,
                profile_bytes: 64 << 10,
                sections: 96,
            },
        )
        .map_err(|e| e.to_string())?;
        Self::decode(origin, digest, |va, n, code| {
            pe.read_from(va, code)
                .map_err(|e| e.to_string())?
                .get(..n)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| "unit panel: truncated image".into())
        })
    }
    fn decode(
        origin: AssetIdentity,
        image_sha256: Sha256Digest,
        read: impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        let mut r = Reader(read);
        let model_format = r.string(0x695503)?;
        if model_format.matches("%s").count() != 1 || model_format.replace("%s", "").contains('%') {
            return Err("unit portrait: unsupported path format".into());
        }
        let portrait = Portrait {
            light: crate::day_night::Sun {
                ambient_intensity: r.number(0x6210c9, &[0xc7, 4, 0x24])?,
                intensity: r.number(0x6210fe, &[0xc7, 4, 0x24])?,
                ambient_colour: [
                    r.number(0x6210dd, &[0xc7, 0x45, 0xd8])?,
                    r.number(0x6210e8, &[0xc7, 0x45, 0xdc])?,
                    r.number(0x6210ef, &[0xc7, 0x45, 0xe0])?,
                ],
                colour: [
                    r.number(0x621110, &[0xc7, 0x45, 0xd8])?,
                    r.number(0x62111b, &[0xc7, 0x45, 0xdc])?,
                    r.number(0x621122, &[0xc7, 0x45, 0xe0])?,
                ],
                direction: [
                    r.number(0x621133, &[0xc7, 0x45, 0xd8])?,
                    r.number(0x62113a, &[0xc7, 0x45, 0xdc])?,
                    r.number(0x621141, &[0xc7, 0x45, 0xe0])?,
                ],
            },
            base: Rect {
                x: r.referenced(0x6416cb, &[0xf3, 0x0f, 0x10, 5])?,
                y: r.number(0x641781, &[0xc7, 0x44, 0x24, 4])?,
                width: r.referenced(0x6416b6, &[0xf3, 0x0f, 0x10, 5])?,
                height: r.number(0x641758, &[0xc7, 4, 0x24])?,
            },
            aspect: r.referenced(0x6416f7, &[0xf3, 0x0f, 0x10, 5])?,
            center: r.referenced(0x641721, &[0xf3, 0x0f, 0x10, 0x0d])?,
            numerator: r.referenced(0x641715, &[0xf3, 0x0f, 0x10, 5])?,
            logical_width: r.referenced(0x64172d, &[0xf3, 0x0f, 0x59, 0x0d])?,
            path_bytes: r.word(0x69550e, &[0x68])? as usize,
            model_format,
        };
        if r.bytes(0x641702, 2, true)? != [0x76, 0x36]
            || r.bytes(0x641790, 5, true)? != [0x6a, 6, 0x52, 0x6a, 6]
            || [
                portrait.base.width,
                portrait.base.height,
                portrait.aspect,
                portrait.logical_width,
            ]
            .iter()
            .any(|v| *v <= 0.)
            || portrait.path_bytes == 0
            || portrait.path_bytes > 64 << 10
            || portrait.light.direction.iter().all(|v| *v == 0.)
            || portrait.light.ambient_intensity < 0.
            || portrait.light.intensity < 0.
            || portrait
                .light
                .ambient_colour
                .iter()
                .chain(&portrait.light.colour)
                .any(|v| !(0.0..=1.0).contains(v))
        {
            return Err("unit portrait: unsupported placement/path bounds".into());
        }
        let frame = r.string(0x61e690)?;
        let title = r.string(0x632bcb)?;
        let health = r.rect([0x659d33, 0x659d2b, 0x659d4b, 0x659d5e], 0x659d3a)?;
        let mana = r.rect([0x659e76, 0x659e6e, 0x659e8e, 0x659ea1], 0x659e7d)?;
        let alignment = r.bytes(0x659d70, 2, true)?;
        if alignment[0] != 0x6a
            || !matches!(alignment[1], 1 | 2 | 4)
            || alignment != r.bytes(0x659eb3, 2, true)?
        {
            return Err("unit panel: unsupported portrait justification".into());
        }
        let font = r.string(0x659e04)?;
        let height = r.string(0x659dd9)?;
        let pointer = r.word(0x659dde, &[0xff, 0x35])?;
        let section_pointer = u32::from_le_bytes(r.bytes(pointer, 4, false)?.try_into().unwrap());
        let section = r.string_at(section_pointer)?;
        if font != r.string(0x659f47)? || height != r.string(0x659f1c)? {
            return Err("unit panel: mismatched portrait fonts".into());
        }
        let shadow = r.word(0x659dc8, &[0xc7, 0x85, 0xe4, 0xfe, 0xff, 0xff])?;
        let shadow_offset = [
            r.number(0x659db3, &[0xc7, 0x85, 0xd8, 0xfe, 0xff, 0xff])?,
            r.number(0x659dbe, &[0xc7, 0x85, 0xdc, 0xfe, 0xff, 0xff])?,
        ];
        if shadow != r.word(0x659f0b, &[0xc7, 0x85, 0xe4, 0xfe, 0xff, 0xff])?
            || shadow_offset
                != [
                    r.number(0x659ef6, &[0xc7, 0x85, 0xd8, 0xfe, 0xff, 0xff])?,
                    r.number(0x659f01, &[0xc7, 0x85, 0xdc, 0xfe, 0xff, 0xff])?,
                ]
        {
            return Err("unit panel: mismatched portrait shadows".into());
        }
        let format = r.string(0x65a081)?;
        if format != r.string(0x65a277)? {
            return Err("unit panel: differing vitals format".into());
        }
        let parts = format.split("%u").map(str::to_owned).collect::<Vec<_>>();
        if parts.len() != 3 || parts.iter().any(|s| s.contains('%')) {
            return Err("unit panel: unsupported vitals format".into());
        }
        let high = r.referenced(0x65a0f1, &[0xf3, 0x0f, 0x10, 5])?;
        let low = r.referenced(0x65a131, &[0x0f, 0x2f, 0x0d])?;
        let offset = r.referenced(0x65a102, &[0xf3, 0x0f, 0x5c, 0x0d])?;
        let scale = r.referenced(0x65a10a, &[0xf3, 0x0f, 0x10, 5])?;
        let low_divisor = r.referenced(0x65a145, &[0xf3, 0x0f, 0x5e, 0x0d])?;
        if !(0. < low
            && low < high
            && high < 1.
            && offset >= 0.
            && offset < high
            && scale > 0.
            && scale <= 255.
            && low_divisor > 0.)
        {
            return Err("unit panel: health colour bounds".into());
        }
        let channel = r.bytes(0x65a112, 7, true)?;
        let red = r.bytes(0x65a155, 7, true)?;
        if channel[..6] != [0xc6, 0x85, 0xe5, 0xfe, 0xff, 0xff]
            || red[..6] != [0xc6, 0x85, 0xe6, 0xfe, 0xff, 0xff]
            || channel[6] != red[6]
            || r.bytes(0x65a11d, 4, true)? != [0xf3, 0x0f, 0x58, 0xc9]
        {
            return Err("unit panel: health colour channels".into());
        }
        Ok(Self {
            portrait,
            origin,
            image_sha256,
            frame,
            title,
            health,
            mana,
            font,
            section,
            height,
            shadow,
            shadow_offset,
            mana_colour: r.word(0x659ed4, &[0xc7, 0x85, 0xe4, 0xfe, 0xff, 0xff])?,
            justification: alignment[1],
            health_colour: r.word(0x65a0e2, &[0xc7, 0x85, 0xe4, 0xfe, 0xff, 0xff])?,
            format: parts.try_into().unwrap(),
            high,
            low,
            offset,
            scale,
            low_divisor,
            full_channel: channel[6],
        })
    }
    /// Positive health rounds upward; mana truncates, as in the portrait updater.
    pub fn readout(&self, current: f32, maximum: f32, health: bool) -> Result<String, String> {
        let number = |value: f32| -> Result<u32, String> {
            if !value.is_finite() || value < 0. || f64::from(value.ceil()) > f64::from(u32::MAX) {
                return Err("unit panel: invalid vitals".into());
            }
            Ok(if health { value.ceil() } else { value.trunc() } as u32)
        };
        Ok(format!(
            "{}{}{}{}{}",
            self.format[0],
            number(current)?,
            self.format[1],
            number(maximum)?,
            self.format[2]
        ))
    }
    pub fn health_colour(&self, fraction: f32) -> Result<u32, String> {
        if !fraction.is_finite() || !(0.0..=1.0).contains(&fraction) {
            return Err("unit panel: invalid life fraction".into());
        }
        let mut bytes = self.health_colour.to_le_bytes();
        if fraction > self.high {
            let scaled = (fraction - self.offset) * self.scale;
            bytes[1] = self.full_channel;
            bytes[2] = (self.scale - (scaled + scaled)).clamp(0., 255.) as u8;
        } else {
            bytes[2] = self.full_channel;
            bytes[1] = (fraction
                / if fraction > self.low {
                    self.high
                } else {
                    self.low_divisor
                }
                * self.scale)
                .clamp(0., 255.) as u8;
        }
        Ok(u32::from_le_bytes(bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    fn origin() -> AssetIdentity {
        AssetIdentity {
            source: "synthetic".into(),
            edition: "test".into(),
            version: "fixture".into(),
        }
    }
    fn put(map: &mut BTreeMap<u32, u8>, va: u32, bytes: &[u8]) {
        for (i, byte) in bytes.iter().enumerate() {
            map.insert(va + i as u32, *byte);
        }
    }
    fn op(map: &mut BTreeMap<u32, u8>, va: u32, prefix: &[u8], value: u32) {
        put(map, va, prefix);
        put(map, va + prefix.len() as u32, &value.to_le_bytes());
    }
    fn fixture() -> BTreeMap<u32, u8> {
        let mut map = BTreeMap::new();
        let mut pointer = 0x1000;
        for (sites, value) in [
            (&[0x695503][..], "%s_head.mdx"),
            (&[0x61e690][..], "UnitFrame"),
            (&[0x632bcb][..], "UnitTitle"),
            (&[0x659e04, 0x659f47][..], "TextFont"),
            (&[0x659dd9, 0x659f1c][..], "PortraitHeight"),
            (&[0x65a081, 0x65a277][..], "[%u of %u]"),
        ] {
            for va in sites {
                op(&mut map, *va, &[0x68], pointer);
            }
            put(&mut map, pointer, value.as_bytes());
            put(&mut map, pointer + value.len() as u32, &[0]);
            pointer += 64;
        }
        op(&mut map, 0x659dde, &[0xff, 0x35], 0x2000);
        put(&mut map, 0x2000, &0x2100u32.to_le_bytes());
        put(&mut map, 0x2100, b"Sizes\0");
        for xywh in [
            [0x659d33, 0x659d2b, 0x659d4b, 0x659d5e],
            [0x659e76, 0x659e6e, 0x659e8e, 0x659ea1],
        ] {
            for (i, va) in xywh.into_iter().enumerate() {
                op(
                    &mut map,
                    va,
                    if i == 1 {
                        &[0xc7, 0x44, 0x24, 4]
                    } else {
                        &[0xc7, 4, 0x24]
                    },
                    [0.19f32, 0.03, 0.09, 0.015][i].to_bits(),
                );
            }
        }
        for va in [0x659d3a, 0x659e7d] {
            put(&mut map, va, &[0x6a, 6, 0x56, 0x6a, 0]);
        }
        for va in [0x659d70, 0x659eb3] {
            put(&mut map, va, &[0x6a, 1]);
        }
        for va in [0x659dc8, 0x659f0b] {
            op(
                &mut map,
                va,
                &[0xc7, 0x85, 0xe4, 0xfe, 0xff, 0xff],
                0xab010203,
            );
        }
        for (va, field, value) in [
            (0x659db3, 0xd8, 0.003f32),
            (0x659ef6, 0xd8, 0.003),
            (0x659dbe, 0xdc, -0.003),
            (0x659f01, 0xdc, -0.003),
        ] {
            op(
                &mut map,
                va,
                &[0xc7, 0x85, field, 0xfe, 0xff, 0xff],
                value.to_bits(),
            );
        }
        op(
            &mut map,
            0x659ed4,
            &[0xc7, 0x85, 0xe4, 0xfe, 0xff, 0xff],
            0xff765432,
        );
        op(
            &mut map,
            0x65a0e2,
            &[0xc7, 0x85, 0xe4, 0xfe, 0xff, 0xff],
            0xaa00bb12,
        );
        for (va, prefix, value, address) in [
            (0x65a0f1, &[0xf3, 0x0f, 0x10, 5][..], 0.7f32, 0x2200),
            (0x65a131, &[0x0f, 0x2f, 0x0d][..], 0.2, 0x2210),
            (0x65a102, &[0xf3, 0x0f, 0x5c, 0x0d][..], 0.5, 0x2220),
            (0x65a10a, &[0xf3, 0x0f, 0x10, 5][..], 200., 0x2230),
            (0x65a145, &[0xf3, 0x0f, 0x5e, 0x0d][..], 0.9, 0x2240),
        ] {
            op(&mut map, va, prefix, address);
            put(&mut map, address, &value.to_le_bytes());
        }
        put(
            &mut map,
            0x65a112,
            &[0xc6, 0x85, 0xe5, 0xfe, 0xff, 0xff, 230],
        );
        put(
            &mut map,
            0x65a155,
            &[0xc6, 0x85, 0xe6, 0xfe, 0xff, 0xff, 230],
        );
        put(&mut map, 0x65a11d, &[0xf3, 0x0f, 0x58, 0xc9]);
        op(&mut map, 0x69550e, &[0x68], 777);
        op(
            &mut map,
            0x641781,
            &[0xc7, 0x44, 0x24, 4],
            0.02f32.to_bits(),
        );
        op(&mut map, 0x641758, &[0xc7, 4, 0x24], 0.11f32.to_bits());
        for (va, prefix, value, address) in [
            (0x6416cb, &[0xf3, 0x0f, 0x10, 5][..], 0.25f32, 0x2400),
            (0x6416b6, &[0xf3, 0x0f, 0x10, 5][..], 0.13, 0x2410),
            (0x6416f7, &[0xf3, 0x0f, 0x10, 5][..], 1.4, 0x2420),
            (0x641721, &[0xf3, 0x0f, 0x10, 0x0d][..], 0.5, 0x2430),
            (0x641715, &[0xf3, 0x0f, 0x10, 5][..], 0.3, 0x2440),
            (0x64172d, &[0xf3, 0x0f, 0x59, 0x0d][..], 0.9, 0x2450),
        ] {
            op(&mut map, va, prefix, address);
            put(&mut map, address, &value.to_le_bytes());
        }
        put(&mut map, 0x641702, &[0x76, 0x36]);
        put(&mut map, 0x641790, &[0x6a, 6, 0x52, 0x6a, 6]);
        for (va, prefix, value) in [
            (0x6210c9, &[0xc7, 4, 0x24][..], 0.2f32),
            (0x6210fe, &[0xc7, 4, 0x24][..], 0.7),
            (0x6210dd, &[0xc7, 0x45, 0xd8][..], 0.3),
            (0x6210e8, &[0xc7, 0x45, 0xdc][..], 0.4),
            (0x6210ef, &[0xc7, 0x45, 0xe0][..], 0.5),
            (0x621110, &[0xc7, 0x45, 0xd8][..], 0.6),
            (0x62111b, &[0xc7, 0x45, 0xdc][..], 0.7),
            (0x621122, &[0xc7, 0x45, 0xe0][..], 0.8),
            (0x621133, &[0xc7, 0x45, 0xd8][..], -0.5),
            (0x62113a, &[0xc7, 0x45, 0xdc][..], 0.3),
            (0x621141, &[0xc7, 0x45, 0xe0][..], -0.2),
        ] {
            op(&mut map, va, prefix, value.to_bits());
        }
        map
    }
    fn decode(map: &BTreeMap<u32, u8>) -> Result<Rules, String> {
        Rules::decode(origin(), [0x42; 32], |va, n, _| {
            (0..n)
                .map(|i| {
                    map.get(&(va + i as u32))
                        .copied()
                        .ok_or_else(|| "missing fixture byte".into())
                })
                .collect()
        })
    }
    #[test]
    fn source_geometry_format_rounding_and_channels_drive_readouts() {
        let r = decode(&fixture()).unwrap();
        assert_eq!(r.frame, "UnitFrame");
        assert_eq!(r.health.x, 0.19);
        assert_eq!(r.font, "TextFont");
        assert_eq!(r.section, "Sizes");
        assert_eq!(r.justification, 1);
        assert_eq!(r.portrait.light.ambient_intensity, 0.2);
        assert_eq!(r.portrait.light.intensity, 0.7);
        assert_eq!(r.portrait.light.ambient_colour, [0.3, 0.4, 0.5]);
        assert_eq!(r.portrait.light.colour, [0.6, 0.7, 0.8]);
        assert_eq!(r.portrait.light.direction, [-0.5, 0.3, -0.2]);
        assert_eq!(
            r.portrait.member("Models/Unit.mdx").unwrap(),
            "Models/Unit_head.mdx"
        );
        assert_eq!(
            r.portrait.rect(1.).unwrap(),
            Rect {
                x: 0.25,
                y: 0.02,
                width: 0.13,
                height: 0.11
            }
        );
        let wide = r.portrait.rect(2.).unwrap();
        assert!((wide.x - 0.315).abs() < 1e-6);
        assert!((wide.width - 0.091).abs() < 1e-6);
        assert!(r.portrait.rect(f32::NAN).is_err());
        assert!(r.portrait.member("bad.blp").is_err());
        assert_eq!(r.readout(4.2, 10.1, true).unwrap(), "[5 of 11]");
        assert_eq!(r.readout(4.9, 10.9, false).unwrap(), "[4 of 10]");
        assert_eq!(r.health_colour(1.).unwrap(), 0xaa00e612);
        assert_eq!(r.health_colour(0.35).unwrap(), 0xaae66412);
        assert_eq!(r.health_colour(0.09).unwrap(), 0xaae61412);
        for value in [f32::NAN, f32::INFINITY, -1., f32::MAX] {
            assert!(r.readout(value, 10., true).is_err());
        }
        assert!(r.health_colour(1.1).is_err());
    }
    #[test]
    fn malformed_operands_anchors_fonts_and_truncated_windows_refuse() {
        let base = fixture();
        for va in [
            0x61e690, 0x632bcb, 0x659d33, 0x659d3b, 0x659e7e, 0x659f47, 0x65a11d, 0x65a155,
        ] {
            let mut map = base.clone();
            *map.get_mut(&va).unwrap() ^= 0x20;
            assert!(decode(&map).is_err(), "{va:#x}");
        }
        let mut map = base.clone();
        put(&mut map, 0x2230, &f32::NAN.to_le_bytes());
        assert!(decode(&map).is_err());
        for va in base.keys() {
            let mut map = base.clone();
            map.remove(va);
            assert!(decode(&map).is_err(), "missing {va:#x}");
        }
        assert!(Rules::reviewed_legacy(b"not an image", origin()).is_err());
    }
    #[test]
    #[ignore = "requires PCX_WAR3_EXE pointing to the reviewed local original client"]
    fn reviewed_original_image_decodes() {
        let path = std::env::var("PCX_WAR3_EXE").unwrap();
        let bytes = std::fs::read(path).unwrap();
        let r = Rules::reviewed_legacy(&bytes, origin()).unwrap();
        assert_eq!(r.image_sha256, REVIEWED_IMAGE);
        assert_eq!(r.frame, "SimpleInfoPanelUnitDetail");
        assert_eq!(r.title, "SimpleNameValue");
        assert_eq!(r.readout(7.5, 20., true).unwrap(), "8 / 20");
        assert_eq!(r.readout(7.5, 20., false).unwrap(), "7 / 20");
    }
}
