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
    pub experience: ExperienceBar,
    pub armor_icons: CombatIcons,
    pub damage_icons: CombatIcons,
    pub status: StatusLabel,
    format: [String; 3],
    health_colour: u32,
    high: f32,
    low: f32,
    offset: f32,
    scale: f32,
    low_divisor: f32,
    full_channel: u8,
    damage_range: String,
    bonus_positive: String,
    bonus_negative: String,
    bonus_number: String,
    whole_number: String,
    tenth: String,
    half: f32,
    damage_dy: f32,
    damage_width: f32,
}

#[derive(Clone, Debug)]
pub struct StatusLabel {
    pub text: String,
    pub font: String,
    pub height: f32,
    pub colour: u32,
    pub shadow: u32,
    pub shadow_offset: [f32; 2],
    bar_size: [f32; 2],
    bar_offset: [f32; 2],
    label_offset: [f32; 2],
}
impl StatusLabel {
    pub fn rect(&self, info: Rect) -> Result<Rect, String> {
        let right =
            info.x + info.width + self.bar_offset[0] - self.bar_size[0] + self.label_offset[0];
        let center = info.y + self.bar_offset[1] + self.bar_size[1] / 2. + self.label_offset[1];
        let rect = Rect {
            x: info.x,
            y: center - self.height / 2.,
            width: right - info.x,
            height: self.height,
        };
        if [
            info.x,
            info.y,
            info.width,
            info.height,
            rect.x,
            rect.y,
            rect.width,
            rect.height,
        ]
        .iter()
        .any(|v| !v.is_finite())
            || info.width <= 0.
            || info.height <= 0.
            || rect.width <= 0.
        {
            return Err("unit panel: invalid status-label frame".into());
        }
        Ok(rect)
    }
}

#[derive(Clone, Debug)]
pub struct CombatIcons {
    pub keys: Vec<String>,
    types: Vec<(String, usize)>,
    classes: Vec<String>,
}
impl CombatIcons {
    pub fn index(&self, kind: &str, classes: &[&str]) -> Result<usize, String> {
        let index = self
            .types
            .iter()
            .find(|(name, _)| name == kind)
            .map(|(_, index)| *index)
            .ok_or_else(|| format!("unit panel: unknown combat type {kind}"))?;
        Ok(index
            + if classes
                .iter()
                .any(|class| self.classes.iter().any(|s| s.eq_ignore_ascii_case(class)))
            {
                0
            } else {
                self.keys.len() / 2
            })
    }
}

#[derive(Clone, Debug)]
pub struct ExperienceBar {
    pub frame: String,
    pub fill_key: String,
    pub border_key: String,
    pub background: u32,
    pub tint: u32,
    pub inset: f32,
    pub width: f32,
    minimum: f32,
    maximum: f32,
    fallback: f32,
}
impl ExperienceBar {
    pub fn fraction(&self, [xp, lower, upper]: [i32; 3]) -> Result<f32, String> {
        if xp < 0 || lower < 0 {
            return Err("unit panel: negative hero XP range".into());
        }
        let (xp, lower, upper) = (xp as f32, lower as f32, upper as f32);
        if lower == 0. && upper == 0. {
            return Err("unit panel: zero initial XP threshold".into());
        }
        let value = if upper > lower || lower == 0. {
            (xp - lower) / (upper - lower)
        } else {
            self.fallback
        };
        Ok(
            (value.clamp(self.minimum, self.maximum) - self.minimum)
                / (self.maximum - self.minimum),
        )
    }
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

pub(crate) struct Reader<F>(pub(crate) F);
impl<F: FnMut(u32, usize, bool) -> Result<Vec<u8>, String>> Reader<F> {
    pub(crate) fn bytes(&mut self, va: u32, n: usize, code: bool) -> Result<Vec<u8>, String> {
        let bytes = (self.0)(va, n, code)?;
        if bytes.len() != n {
            return Err(format!("unit panel: truncated window {va:#x}"));
        }
        Ok(bytes)
    }
    pub(crate) fn word(&mut self, va: u32, prefix: &[u8]) -> Result<u32, String> {
        let bytes = self.bytes(va, prefix.len() + 4, true)?;
        if !bytes.starts_with(prefix) {
            return Err(format!("unit panel: instruction {va:#x}"));
        }
        Ok(u32::from_le_bytes(
            bytes[prefix.len()..].try_into().unwrap(),
        ))
    }
    pub(crate) fn number(&mut self, va: u32, prefix: &[u8]) -> Result<f32, String> {
        let value = f32::from_bits(self.word(va, prefix)?);
        if !value.is_finite() {
            return Err(format!("unit panel: nonfinite number {va:#x}"));
        }
        Ok(value)
    }
    pub(crate) fn referenced(&mut self, va: u32, prefix: &[u8]) -> Result<f32, String> {
        let pointer = self.word(va, prefix)?;
        let bytes = self.bytes(pointer, 4, false)?;
        let value = f32::from_le_bytes(bytes.try_into().unwrap());
        if !value.is_finite() {
            return Err("unit panel: nonfinite referenced value".into());
        }
        Ok(value)
    }
    pub(crate) fn string_at(&mut self, pointer: u32) -> Result<String, String> {
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
    pub(crate) fn string(&mut self, va: u32) -> Result<String, String> {
        self.prefixed_string(va, &[0x68])
    }
    fn prefixed_string(&mut self, va: u32, prefix: &[u8]) -> Result<String, String> {
        let pointer = self.word(va, prefix)?;
        self.string_at(pointer)
    }
    pub(crate) fn call_target(&mut self, va: u32) -> Result<u32, String> {
        let bytes = self.bytes(va, 5, true)?;
        if bytes[0] != 0xe8 {
            return Err(format!("unit panel: call {va:#x}"));
        }
        let rel = i32::from_le_bytes(bytes[1..5].try_into().unwrap());
        Ok(va
            .checked_add(5)
            .ok_or_else(|| format!("unit panel: call {va:#x}"))?
            .wrapping_add(rel as u32))
    }
    fn f32_at(&mut self, va: u32) -> Result<f32, String> {
        let bytes = self.bytes(va, 4, false)?;
        let value = f32::from_le_bytes(bytes.try_into().unwrap());
        if !value.is_finite() {
            return Err(format!("unit panel: nonfinite number {va:#x}"));
        }
        Ok(value)
    }
    fn icon_keys(&mut self, sites: [u32; 2], count: usize) -> Result<Vec<String>, String> {
        let mut keys = Vec::new();
        for site in sites {
            let table = self.word(site, &[0xff, 0x34, 0x85])?;
            for index in 0..count {
                let at = table
                    .checked_add(index as u32 * 4)
                    .ok_or("unit panel: icon table overflow")?;
                let pointer = u32::from_le_bytes(self.bytes(at, 4, false)?.try_into().unwrap());
                let key = self.string_at(pointer)?;
                if key.is_empty() {
                    return Err("unit panel: empty combat icon key".into());
                }
                keys.push(key);
            }
        }
        Ok(keys)
    }
    fn upgrade_classes(&mut self, getters: &[u32]) -> Result<Vec<String>, String> {
        let fields = getters
            .iter()
            .map(|&site| self.word(site, &[0x8b, 0x80]))
            .collect::<Result<Vec<_>, _>>()?;
        let cases = self.word(0x67d3c6, &[0xff, 0x24, 0x8d])?;
        let domain = self.bytes(0x67d3c0, 4, true)?;
        if domain[..3] != [0x49, 0x83, 0xf9] || domain[3] > 31 {
            return Err("unit panel: upgrade-class dispatch".into());
        }
        let mut ids = Vec::new();
        for index in 0..=u32::from(domain[3]) {
            let at = cases
                .checked_add(index * 4)
                .ok_or("unit panel: class dispatch overflow")?;
            let target = u32::from_le_bytes(self.bytes(at, 4, false)?.try_into().unwrap());
            if fields.contains(&self.word(target, &[0x89, 0x83])?) {
                ids.push(index + 1);
            }
        }
        if ids.len() != fields.len() {
            return Err("unit panel: upgrade-class getter binding".into());
        }
        let table = self.word(0x6dd31e, &[0xff, 0x34, 0xf5])?;
        let count = self.word(0x6dd311, &[0xbe])?;
        if count > 32 {
            return Err("unit panel: upgrade-class table budget".into());
        }
        let mut classes = Vec::new();
        for id in ids {
            let mut matches = Vec::new();
            for index in 0..count {
                let at = table
                    .checked_add(index * 8)
                    .ok_or("unit panel: class table overflow")?;
                let pair = self.bytes(at, 8, false)?;
                if u32::from_le_bytes(pair[4..].try_into().unwrap()) == id {
                    matches
                        .push(self.string_at(u32::from_le_bytes(pair[..4].try_into().unwrap()))?);
                }
            }
            let [name] = matches.as_slice() else {
                return Err("unit panel: ambiguous upgrade class".into());
            };
            if name.is_empty() {
                return Err("unit panel: empty upgrade class".into());
            }
            classes.push(name.clone());
        }
        Ok(classes)
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
        if r.call_target(0x632cfa)? != 0x61b9b0
            || r.bytes(0x632cf5, 5, true)? != [0x6a, 1, 0x57, 0x8b, 0xc8]
            || r.bytes(0x632d11, 6, true)? != [0x89, 0x87, 0x70, 1, 0, 0]
            || r.bytes(0x632fee, 6, true)? != [0x8b, 0x8f, 0x70, 1, 0, 0]
            || r.bytes(0x633008, 5, true)? != [0x6a, 8, 0x57, 0x6a, 8]
            || r.call_target(0x63300d)? != 0x873a10
            || r.bytes(0x61bbac, 5, true)? != [0x6a, 3, 0x53, 0x6a, 5]
            || r.call_target(0x61bbb1)? != 0x873a10
        {
            return Err("unit panel: status-label parent/anchor chain".into());
        }
        let status = StatusLabel {
            text: r.string(0x61bb4e)?,
            font: r.string(0x61bbc2)?,
            height: r.number(0x61bbb9, &[0xc7, 4, 0x24])?,
            colour: r.word(0x61bbe5, &[0xc7, 0x45, 0xac])?,
            shadow: r.word(0x61bc0d, &[0xc7, 0x45, 0xac])?,
            shadow_offset: [
                r.number(0x61bbfe, &[0xc7, 0x45, 0x9c])?,
                r.number(0x61bc06, &[0xc7, 0x45, 0xa0])?,
            ],
            bar_size: [
                r.number(0x61ba0d, &[0xc7, 4, 0x24])?,
                r.number(0x61ba1c, &[0xc7, 4, 0x24])?,
            ],
            bar_offset: [
                r.number(0x633001, &[0xc7, 4, 0x24])?,
                r.number(0x632ff9, &[0xc7, 0x44, 0x24, 4])?,
            ],
            label_offset: [
                r.number(0x61bba5, &[0xc7, 4, 0x24])?,
                r.number(0x61bb9d, &[0xc7, 0x44, 0x24, 4])?,
            ],
        };
        if status.text.is_empty()
            || status.font.is_empty()
            || status.height <= 0.
            || status.bar_size.iter().any(|v| *v <= 0.)
        {
            return Err("unit panel: invalid status-label style".into());
        }
        let armor_icons = CombatIcons {
            keys: r.icon_keys(
                [0x654cd2, 0x654ce5],
                pcx_objects::editor::misc::DEFENSE_TYPES.len(),
            )?,
            types: pcx_objects::editor::misc::DEFENSE_TYPES
                .iter()
                .enumerate()
                .map(|(i, name)| (name.to_string(), i))
                .collect(),
            classes: r.upgrade_classes(&[0x67b013])?,
        };
        for (site, target) in [
            (0x6552b7, 0x697450),
            (0x6552c5, 0x69a1d0),
            (0x6552d3, 0x67b020),
        ] {
            if r.call_target(site)? != target {
                return Err("unit panel: damage upgrade getter".into());
            }
        }
        // The source parser returns zero for spells, explicit integers for the
        // middle branches, and the final ADD value for a matching hero token.
        let zero = r.bytes(0x6aca35, 6, true)?;
        if zero[..2] != [0x0f, 0x84]
            || 0x6aca3bu32.wrapping_add(i32::from_le_bytes(zero[2..].try_into().unwrap()) as u32)
                != 0x6acae4
            || r.bytes(0x6acae4, 2, true)? != [0x33, 0xc0]
        {
            return Err("unit panel: spell attack-type branch".into());
        }
        let mut types = vec![(r.string(0x6aca25)?, 0)];
        for (name, value) in [
            (0x6aca3b, 0x6aca4d),
            (0x6aca57, 0x6aca69),
            (0x6aca73, 0x6aca85),
            (0x6aca8f, 0x6acaa1),
            (0x6acaab, 0x6acabd),
        ] {
            types.push((r.string(name)?, r.word(value, &[0xb8])? as usize));
        }
        let hero = r.bytes(0x6acadc, 3, true)?;
        if hero[..2] != [0x83, 0xc0] {
            return Err("unit panel: hero attack-type branch".into());
        }
        let count = usize::from(hero[2]) + 1;
        types.push((r.string(0x6acac7)?, usize::from(hero[2])));
        let mut indices = types.iter().map(|(_, i)| *i).collect::<Vec<_>>();
        indices.sort_unstable();
        if count > 32
            || indices != (0..count).collect::<Vec<_>>()
            || types.iter().any(|(name, _)| name.is_empty())
        {
            return Err("unit panel: attack-type domain".into());
        }
        let damage_icons = CombatIcons {
            keys: r.icon_keys([0x655387, 0x65539a], count)?,
            types,
            classes: r.upgrade_classes(&[0x697473, 0x69a1f3, 0x67b043])?,
        };
        let dispatch = r.word(0x65b845, &[0xff, 0x24, 0x85])?;
        let count = usize::from(r.bytes(0x65b83c, 3, true)?[2]) + 1;
        if count > 32 || r.bytes(0x65b83c, 2, true)? != [0x83, 0xf8] {
            return Err("unit panel: status-bar dispatch domain".into());
        }
        let entries = r.bytes(dispatch, count * 4, false)?;
        let styles = entries
            .as_chunks::<4>()
            .0
            .iter()
            .enumerate()
            .filter(|(_, bytes)| u32::from_le_bytes(**bytes) == 0x65b90d)
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let [style] = styles.as_slice() else {
            return Err("unit panel: ambiguous XP status-bar style".into());
        };
        let constructors = r.word(0x621af3, &[0xff, 0x24, 0x85])?;
        let constructor = u32::from_le_bytes(
            r.bytes(
                constructors
                    .checked_add((*style as u32) * 4)
                    .ok_or("unit panel: XP constructor table overflow")?,
                4,
                false,
            )?
            .try_into()
            .unwrap(),
        );
        if constructor != 0x621b2c {
            return Err("unit panel: XP tint style differs from updater".into());
        }
        let table = r.word(0x621ac2, &[0xff, 0x34, 0x85])?;
        let pointer = u32::from_le_bytes(
            r.bytes(
                table
                    .checked_add((*style as u32) * 4)
                    .ok_or("unit panel: XP texture table overflow")?,
                4,
                false,
            )?
            .try_into()
            .unwrap(),
        );
        let red = r.bytes(0x621b38, 4, true)?;
        let green_blue = r.bytes(0x621b32, 6, true)?;
        let alpha = r.bytes(0x621ae6, 4, true)?;
        if red[..3] != [0xc6, 0x45, 8]
            || green_blue[..4] != [0x66, 0xc7, 0x45, 9]
            || alpha[..3] != [0xc6, 0x45, 0xb]
        {
            return Err("unit panel: XP tint instructions".into());
        }
        if r.call_target(0x61e482)? != 0x6218d0
            || r.bytes(0x61e47b, 2, true)? != [0x6a, *style as u8]
            || r.call_target(0x61e49e)? != 0x873b40
        {
            return Err("unit panel: XP frame constructor/width setter".into());
        }
        let experience = ExperienceBar {
            width: r.number(0x61e497, &[0xc7, 4, 0x24])?,
            frame: r.string(0x632c1a)?,
            fill_key: r.string_at(pointer)?,
            border_key: r.string(0x621bd2)?,
            background: r.word(0x6219f4, &[0xc7, 0x45, 0x10])?,
            tint: u32::from_le_bytes([red[3], green_blue[4], green_blue[5], alpha[3]]),
            inset: r.referenced(0x621a00, &[0xf3, 0x0f, 0x10, 5])?,
            minimum: r.number(0x621ca4, &[0xc7, 4, 0x24])?,
            maximum: r.number(0x621c9c, &[0xc7, 0x44, 0x24, 4])?,
            fallback: r.referenced(0x65b97d, &[0xf3, 0x0f, 0x10, 0x0d])?,
        };
        if experience.minimum >= experience.maximum
            || experience.inset < 0.
            || experience.width <= 0.
        {
            return Err("unit panel: XP status-bar range/inset".into());
        }

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
        // Damage value text is sprintf("%d - %d") plus, when the bonus integer
        // is nonzero, a sign-coloured suffix. cmovle keeps the green prefix for
        // a positive bonus. The first icon row is TopLeft on the detail frame.
        if r.bytes(0x6553f5, 2, true)? != [0x0f, 0x4e]
            || r.bytes(0x632eb0, 6, true)? != [0x8b, 0x8f, 0x50, 0x01, 0x00, 0x00]
            || r.bytes(0x632eca, 2, true)? != [0x6a, 0x00]
            || r.bytes(0x632ecd, 2, true)? != [0x6a, 0x00]
            || r.bytes(0x632f17, 2, true)? != [0x6a, 0x02]
            || r.bytes(0x632f19, 6, true)? != [0xff, 0xb7, 0x50, 0x01, 0x00, 0x00]
            || r.bytes(0x632fb6, 2, true)? != [0x6a, 0x02]
            || r.bytes(0x632fb8, 6, true)? != [0xff, 0xb7, 0x50, 0x01, 0x00, 0x00]
        {
            return Err("unit panel: stat text or row anchor".into());
        }
        let damage_range = r.string(0x6553c7)?;
        let bonus_positive = r.prefixed_string(0x6553f0, &[0xb8])?;
        let bonus_negative = r.prefixed_string(0x6553eb, &[0xba])?;
        let bonus_number = r.string(0x655413)?;
        let whole_number = r.string(0x655b43)?;
        let tenth = r.string(0x628c51)?;
        let half = r.f32_at(0xe6957c)?;
        if r.bytes(0x654c40, 8, true)? != [0xf3, 0x0f, 0x58, 0x0d, 0x7c, 0x95, 0xe6, 0x00]
            || r.bytes(0x654c54, 8, true)? != [0xf3, 0x0f, 0x10, 0x05, 0x7c, 0x95, 0xe6, 0x00]
            || r.string(0x654d1c)? != whole_number
            || r.call_target(0x654d24)? != 0x433bb0
            || r.call_target(0x654d4f)? != 0x628bd0
        {
            return Err("unit panel: armor integer/bonus format relation".into());
        }

        let damage_dy = r.number(0x632ebb, &[0xc7, 0x44, 0x24, 0x04])?;
        // The constructor stores one width on the damage, armor, and hero frames
        // before it anchors armor and the hero block to the damage frame's top-right.
        let damage_width = r.number(0x632e1f, &[0xc7, 0x04, 0x24])?;
        let armor_width = r.number(0x632e45, &[0xc7, 0x04, 0x24])?;
        let hero_width = r.number(0x632e91, &[0xc7, 0x04, 0x24])?;
        if r.bytes(0x632da0, 6, true)? != [0x8b, 0x8f, 0x50, 0x01, 0x00, 0x00]
            || r.bytes(0x632e3f, 6, true)? != [0x8b, 0x8f, 0x58, 0x01, 0x00, 0x00]
            || r.bytes(0x632e8b, 6, true)? != [0x8b, 0x8f, 0x68, 0x01, 0x00, 0x00]
            || r.call_target(0x632e26)? != 0x873b40
            || r.call_target(0x632e4c)? != 0x873b40
            || r.call_target(0x632e98)? != 0x873b40
            || !(damage_width > 0. && damage_width == armor_width && damage_width == hero_width)
        {
            return Err("unit panel: stat row width".into());
        }
        if half <= 0. || !tenth.contains("%0.1f") || tenth.matches('%').count() != 1 {
            return Err("unit panel: stat number format".into());
        }
        Ok(Self {
            armor_icons,
            damage_icons,
            status,
            experience,
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
            damage_range,
            bonus_positive,
            bonus_negative,
            bonus_number,
            whole_number,
            tenth,
            half,
            damage_dy,
            damage_width,
        })
    }
    /// Top of the first stat row, relative to the detail frame's top. Negative
    /// moves down. Armor and the hero block are TopLeft on this row's top-right.
    pub fn damage_row_dy(&self) -> f32 {
        self.damage_dy
    }
    /// Width of the damage, armor, and hero icon frames. Their top-right anchors use it.
    pub fn damage_row_width(&self) -> f32 {
        self.damage_width
    }
    pub fn damage_text(
        &self,
        minimum: i32,
        maximum: i32,
        natural: f32,
        bonus: f32,
    ) -> Result<String, String> {
        if minimum < 0 || maximum < 0 {
            return Err("unit panel: negative damage".into());
        }
        let natural = self.whole(natural)?;
        let minimum = minimum
            .checked_add(natural)
            .ok_or("unit panel: damage display overflow")?;
        let maximum = maximum
            .checked_add(natural)
            .ok_or("unit panel: damage display overflow")?;
        let mut text = self.integers(&self.damage_range, &[minimum.max(0), maximum.max(0)])?;
        text.push_str(&self.bonus_suffix(self.whole(bonus)?)?);
        Ok(text)
    }
    pub fn armor_text(&self, base: f32, bonus: f32) -> Result<String, String> {
        let mut text = self.integers(&self.whole_number, &[self.whole(base)?])?;
        text.push_str(&self.bonus_suffix_float(bonus)?);
        Ok(text)
    }
    pub fn attribute_text(&self, base: f32, bonus: f32) -> Result<String, String> {
        let mut text = self.integers(&self.whole_number, &[self.integral(base)?])?;
        text.push_str(&self.bonus_suffix(self.integral(bonus)?)?);
        Ok(text)
    }
    fn whole(&self, value: f32) -> Result<i32, String> {
        if !value.is_finite() {
            return Err("unit panel: non-finite stat".into());
        }
        let shifted = if value > 0. {
            value + self.half
        } else {
            self.half - value
        };
        let truncated = shifted.trunc();
        let signed = if value > 0. { truncated } else { -truncated };
        i32::try_from(signed as i64).map_err(|_| "unit panel: stat exceeds integer".into())
    }
    fn integral(&self, value: f32) -> Result<i32, String> {
        if !value.is_finite() || value.trunc() != value {
            return Err("unit panel: non-integral stat".into());
        }
        i32::try_from(value as i64).map_err(|_| "unit panel: stat exceeds integer".into())
    }
    fn integers(&self, template: &str, values: &[i32]) -> Result<String, String> {
        let parts = template.split("%d").collect::<Vec<_>>();
        if parts.len() != values.len() + 1 || parts.iter().any(|part| part.contains('%')) {
            return Err("unit panel: stat template".into());
        }
        let mut text = String::new();
        for (index, part) in parts.iter().enumerate() {
            text.push_str(part);
            if let Some(value) = values.get(index) {
                text.push_str(&value.to_string());
            }
        }
        Ok(text)
    }
    fn bonus_suffix(&self, bonus: i32) -> Result<String, String> {
        self.bonus_suffix_float(bonus as f32)
    }
    fn bonus_suffix_float(&self, bonus: f32) -> Result<String, String> {
        if !bonus.is_finite() {
            return Err("unit panel: non-finite stat".into());
        }
        if bonus == 0. {
            return Ok(String::new());
        }
        let prefix = if bonus > 0. {
            &self.bonus_positive
        } else {
            &self.bonus_negative
        };
        let body = if bonus.trunc() == bonus {
            self.integers(&self.bonus_number, &[self.integral(bonus)?])?
        } else {
            let token = "%0.1f";
            let Some((head, tail)) = self.tenth.split_once(token) else {
                return Err("unit panel: tenth format".into());
            };
            if head.contains('%') || tail.contains('%') {
                return Err("unit panel: tenth format".into());
            }
            format!("{head}{}{tail}", self.tenth_text(bonus)?)
        };
        Ok(format!("{prefix}{body}"))
    }
    fn tenth_text(&self, value: f32) -> Result<String, String> {
        let scaled = value * 10.;
        if !scaled.is_finite() {
            return Err("unit panel: non-finite stat".into());
        }
        let rounded = self.whole(scaled)?;
        let negative = rounded < 0;
        let abs = rounded.unsigned_abs();
        Ok(format!(
            "{}{}.{}",
            if negative { "-" } else { "" },
            abs / 10,
            abs % 10
        ))
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
    #[test]
    fn armor_icons_use_source_tables_and_effective_upgrade_class() {
        let rules = decode(&fixture()).unwrap().armor_icons;
        assert_eq!(rules.index("large", &["SyntheticClass0"]).unwrap(), 2);
        assert_eq!(rules.index("large", &["other"]).unwrap(), 10);
        assert_eq!(rules.keys[10], "SyntheticIcon20544_2");
        assert!(rules.index("unknown", &[]).is_err());
        let mut bad = fixture();
        put(&mut bad, 0x67b013, &[0x8b, 0x80, 0x80, 2, 0, 0]);
        assert!(decode(&bad).is_err());
    }
    #[test]
    fn damage_icons_follow_source_attack_indices_and_three_upgrade_classes() {
        let rules = decode(&fixture()).unwrap().damage_icons;
        assert_eq!(
            rules
                .index("SyntheticAttack0", &["SyntheticClass2"])
                .unwrap(),
            5
        );
        assert_eq!(
            rules
                .index("SyntheticAttack4", &["SyntheticClass3"])
                .unwrap(),
            1
        );
        assert_eq!(
            rules
                .index("SyntheticAttack0", &["SyntheticClass1"])
                .unwrap(),
            5
        );
        assert_eq!(
            rules
                .index("SyntheticAttack0", &["SyntheticClass0"])
                .unwrap(),
            12
        );
        assert_eq!(rules.index("SyntheticAttack6", &[]).unwrap(), 7);
        assert!(rules.index("normal", &[]).is_err());
        let mut bad = fixture();
        op(&mut bad, 0x6aca4d, &[0xb8], 2);
        assert!(decode(&bad).is_err());
    }
    #[test]
    fn status_label_follows_source_buff_bar_anchor_and_style() {
        let rules = decode(&fixture()).unwrap().status;
        assert_eq!(rules.text, "SYNTHETIC_STATUS");
        assert_eq!(rules.font, "SyntheticStatusFont");
        assert_eq!((rules.colour, rules.shadow), (0x8a123456, 0x5a654321));
        assert_eq!(rules.shadow_offset, [0.003, -0.004]);
        let info = Rect {
            x: 0.23,
            y: 0.04,
            width: 0.31,
            height: 0.18,
        };
        let rect = rules.rect(info).unwrap();
        assert!((rect.width - 0.141).abs() < f32::EPSILON);
        assert!((rect.y - 0.06685).abs() < f32::EPSILON);
        assert_eq!(rect.height, 0.0093);
        assert!(
            rules
                .rect(Rect {
                    width: 0.01,
                    ..info
                })
                .is_err()
        );
        assert!(
            rules
                .rect(Rect {
                    y: f32::NAN,
                    ..info
                })
                .is_err()
        );
        let mut bad = fixture();
        put(&mut bad, 0x632cf5, &[0x6a, 0, 0x57, 0x8b, 0xc8]);
        assert!(decode(&bad).is_err());
    }
    #[test]
    fn xp_bar_uses_bound_art_tint_and_float_thresholds() {
        let rules = decode(&fixture()).unwrap().experience;
        assert_eq!(rules.frame, "SyntheticHeroBar");
        assert_eq!(rules.fill_key, "SyntheticXpFill");
        assert_eq!(rules.border_key, "SyntheticXpBorder");
        assert_eq!(rules.tint, 0x2c21160b);
        assert_eq!(rules.inset, 0.0023);
        assert_eq!(rules.width, 0.173);
        for (range, expected) in [
            ([47, 23, 71], 0.5),
            ([0, 23, 71], 0.),
            ([100, 23, 71], 1.),
            ([23, 23, 23], 1.),
        ] {
            assert_eq!(rules.fraction(range).unwrap(), expected);
        }
        assert!(rules.fraction([-1, 0, 23]).is_err());
        assert!(rules.fraction([0, 0, 0]).is_err());
        assert_eq!(rules.fraction([5, 0, -7]).unwrap(), 0.);
        let mut bad = fixture();
        put(&mut bad, 0x65b83c, &[0x83, 0xf8, 255]);
        assert!(decode(&bad).is_err());
    }
    fn fixture() -> BTreeMap<u32, u8> {
        let mut map = BTreeMap::new();
        let mut pointer = 0x1000;
        for (site, target) in [
            (0x632cfau32, 0x61b9b0u32),
            (0x63300d, 0x873a10),
            (0x61bbb1, 0x873a10),
        ] {
            op(&mut map, site, &[0xe8], target.wrapping_sub(site + 5));
        }
        put(&mut map, 0x632cf5, &[0x6a, 1, 0x57, 0x8b, 0xc8]);
        put(&mut map, 0x632d11, &[0x89, 0x87, 0x70, 1, 0, 0]);
        put(&mut map, 0x632fee, &[0x8b, 0x8f, 0x70, 1, 0, 0]);
        put(&mut map, 0x633008, &[0x6a, 8, 0x57, 0x6a, 8]);
        put(&mut map, 0x61bbac, &[0x6a, 3, 0x53, 0x6a, 5]);
        op(&mut map, 0x61bbe5, &[0xc7, 0x45, 0xac], 0x8a123456);
        op(&mut map, 0x61bc0d, &[0xc7, 0x45, 0xac], 0x5a654321);
        op(&mut map, 0x61bbfe, &[0xc7, 0x45, 0x9c], 0.003f32.to_bits());
        op(
            &mut map,
            0x61bc06,
            &[0xc7, 0x45, 0xa0],
            (-0.004f32).to_bits(),
        );
        for (site, prefix, value) in [
            (0x61bbb9, &[0xc7, 4, 0x24][..], 0.0093f32),
            (0x61ba0d, &[0xc7, 4, 0x24][..], 0.13),
            (0x61ba1c, &[0xc7, 4, 0x24][..], 0.021),
            (0x633001, &[0xc7, 4, 0x24][..], -0.012),
            (0x632ff9, &[0xc7, 0x44, 0x24, 4][..], 0.017),
            (0x61bba5, &[0xc7, 4, 0x24][..], -0.027),
            (0x61bb9d, &[0xc7, 0x44, 0x24, 4][..], 0.004),
        ] {
            op(&mut map, site, prefix, value.to_bits());
        }
        for (site, table) in [(0x654cd2, 0x5000), (0x654ce5, 0x5040)] {
            op(&mut map, site, &[0xff, 0x34, 0x85], table);
            for index in 0..8u32 {
                let at = 0x5100 + (table - 0x5000) * 32 + index * 64;
                put(&mut map, table + index * 4, &at.to_le_bytes());
                put(
                    &mut map,
                    at,
                    format!("SyntheticIcon{table}_{index}\0").as_bytes(),
                );
            }
        }
        put(&mut map, 0x67d3c0, &[0x49, 0x83, 0xf9, 4]);
        op(&mut map, 0x67d3c6, &[0xff, 0x24, 0x8d], 0x6080);
        for (i, field) in [0x24cu32, 0x254, 0x248, 0x250, 0x258]
            .into_iter()
            .enumerate()
        {
            let target = 0x6200 + i as u32 * 16;
            put(&mut map, 0x6080 + i as u32 * 4, &target.to_le_bytes());
            op(&mut map, target, &[0x89, 0x83], field);
            let label = 0x6300 + i as u32 * 64;
            put(&mut map, 0x6000 + i as u32 * 8, &label.to_le_bytes());
            put(
                &mut map,
                0x6004 + i as u32 * 8,
                &(i as u32 + 1).to_le_bytes(),
            );
            // Caster is outside this panel's requested upgrade classes.
            if i < 4 {
                put(&mut map, label, format!("SyntheticClass{i}\0").as_bytes());
            }
        }
        for (site, field) in [
            (0x67b013, 0x24cu32),
            (0x697473, 0x248),
            (0x69a1f3, 0x250),
            (0x67b043, 0x254),
        ] {
            op(&mut map, site, &[0x8b, 0x80], field);
        }
        op(&mut map, 0x6dd31e, &[0xff, 0x34, 0xf5], 0x6000);
        op(&mut map, 0x6dd311, &[0xbe], 5);
        for (site, target) in [
            (0x6552b7u32, 0x697450u32),
            (0x6552c5, 0x69a1d0),
            (0x6552d3, 0x67b020),
        ] {
            op(&mut map, site, &[0xe8], target.wrapping_sub(site + 5));
        }
        for (i, (name, value, index)) in [
            (0x6aca3b, Some(0x6aca4d), 5),
            (0x6aca57, Some(0x6aca69), 2),
            (0x6aca73, Some(0x6aca85), 3),
            (0x6aca8f, Some(0x6acaa1), 4),
            (0x6acaab, Some(0x6acabd), 1),
            (0x6acac7, None, 6),
            (0x6aca25, None, 0),
        ]
        .into_iter()
        .enumerate()
        {
            let at = 0x6600 + i as u32 * 64;
            op(&mut map, name, &[0x68], at);
            put(&mut map, at, format!("SyntheticAttack{i}\0").as_bytes());
            if let Some(value) = value {
                op(&mut map, value, &[0xb8], index);
            }
        }
        op(&mut map, 0x6aca35, &[0x0f, 0x84], 0x6acae4 - 0x6aca3b);
        put(&mut map, 0x6acae4, &[0x33, 0xc0]);
        put(&mut map, 0x6acadc, &[0x83, 0xc0, 6]);
        for (site, table) in [(0x655387, 0x6800), (0x65539a, 0x6840)] {
            op(&mut map, site, &[0xff, 0x34, 0x85], table);
            for index in 0..7u32 {
                let at = 0x6900 + (table - 0x6800) * 32 + index * 64;
                put(&mut map, table + index * 4, &at.to_le_bytes());
                put(
                    &mut map,
                    at,
                    format!("SyntheticDamage{table}_{index}\0").as_bytes(),
                );
            }
        }
        for (site, target) in [(0x61e482u32, 0x6218d0u32), (0x61e49e, 0x873b40)] {
            op(&mut map, site, &[0xe8], target.wrapping_sub(site + 5));
        }
        put(&mut map, 0x61e47b, &[0x6a, 0]);
        op(&mut map, 0x61e497, &[0xc7, 4, 0x24], 0.173f32.to_bits());
        op(&mut map, 0x65b845, &[0xff, 0x24, 0x85], 0x4000);
        put(&mut map, 0x65b83c, &[0x83, 0xf8, 0]);
        put(&mut map, 0x4000, &0x65b90du32.to_le_bytes());
        op(&mut map, 0x621af3, &[0xff, 0x24, 0x85], 0x4060);
        put(&mut map, 0x4060, &0x621b2cu32.to_le_bytes());
        op(&mut map, 0x621ac2, &[0xff, 0x34, 0x85], 0x4010);
        put(&mut map, 0x4010, &0x4020u32.to_le_bytes());
        put(&mut map, 0x4020, b"SyntheticXpFill\0");
        put(&mut map, 0x621b38, &[0xc6, 0x45, 8, 11]);
        put(&mut map, 0x621b32, &[0x66, 0xc7, 0x45, 9, 22, 33]);
        put(&mut map, 0x621ae6, &[0xc6, 0x45, 0xb, 44]);
        op(&mut map, 0x6219f4, &[0xc7, 0x45, 0x10], 0x44332211);
        op(&mut map, 0x621a00, &[0xf3, 0x0f, 0x10, 5], 0x4040);
        put(&mut map, 0x4040, &0.0023f32.to_le_bytes());
        op(&mut map, 0x621ca4, &[0xc7, 4, 0x24], 0f32.to_bits());
        op(&mut map, 0x621c9c, &[0xc7, 0x44, 0x24, 4], 1f32.to_bits());
        op(&mut map, 0x65b97d, &[0xf3, 0x0f, 0x10, 0x0d], 0x4050);
        put(&mut map, 0x4050, &2f32.to_le_bytes());

        for (sites, value) in [
            (&[0x61bb4e][..], "SYNTHETIC_STATUS"),
            (&[0x61bbc2][..], "SyntheticStatusFont"),
            (&[0x632c1a][..], "SyntheticHeroBar"),
            (&[0x621bd2][..], "SyntheticXpBorder"),
            (&[0x695503][..], "%s_head.mdx"),
            (&[0x61e690][..], "UnitFrame"),
            (&[0x632bcb][..], "UnitTitle"),
            (&[0x659e04, 0x659f47][..], "TextFont"),
            (&[0x659dd9, 0x659f1c][..], "PortraitHeight"),
            (&[0x65a081, 0x65a277][..], "[%u of %u]"),
            (&[0x6553c7][..], "%d - %d"),
            (&[0x655413][..], "%d|R"),
            (&[0x655b43, 0x654d1c][..], "%d"),
            (&[0x628c51][..], "%0.1f|R"),
        ] {
            for va in sites {
                op(&mut map, *va, &[0x68], pointer);
            }
            put(&mut map, pointer, value.as_bytes());
            put(&mut map, pointer + value.len() as u32, &[0]);
            pointer += 64;
        }
        for (va, prefix, value) in [
            (0x6553f0u32, &[0xb8u8][..], " |+"),
            (0x6553eb, &[0xba], " |"),
        ] {
            op(&mut map, va, prefix, pointer);
            put(&mut map, pointer, value.as_bytes());
            put(&mut map, pointer + value.len() as u32, &[0]);
            pointer += 64;
        }
        put(&mut map, 0xe6957c, &0.5f32.to_le_bytes());
        put(
            &mut map,
            0x654c40,
            &[0xf3, 0x0f, 0x58, 0x0d, 0x7c, 0x95, 0xe6, 0x00],
        );
        put(
            &mut map,
            0x654c54,
            &[0xf3, 0x0f, 0x10, 0x05, 0x7c, 0x95, 0xe6, 0x00],
        );
        for (site, target) in [(0x654d24_u32, 0x433bb0_u32), (0x654d4f, 0x628bd0)] {
            put(
                &mut map,
                site,
                &[
                    vec![0xe8],
                    target.wrapping_sub(site + 5).to_le_bytes().to_vec(),
                ]
                .concat(),
            );
        }

        op(
            &mut map,
            0x632ebb,
            &[0xc7, 0x44, 0x24, 0x04],
            (-0.03125f32).to_bits(),
        );
        for (va, bytes) in [
            (0x6553f5u32, &[0x0fu8, 0x4e][..]),
            (0x632eb0, &[0x8b, 0x8f, 0x50, 0x01, 0x00, 0x00]),
            (0x632eca, &[0x6a, 0x00]),
            (0x632ecd, &[0x6a, 0x00]),
            (0x632f17, &[0x6a, 0x02]),
            (0x632f19, &[0xff, 0xb7, 0x50, 0x01, 0x00, 0x00]),
            (0x632fb6, &[0x6a, 0x02]),
            (0x632fb8, &[0xff, 0xb7, 0x50, 0x01, 0x00, 0x00]),
            (0x632da0, &[0x8b, 0x8f, 0x50, 0x01, 0x00, 0x00]),
            (0x632e3f, &[0x8b, 0x8f, 0x58, 0x01, 0x00, 0x00]),
            (0x632e8b, &[0x8b, 0x8f, 0x68, 0x01, 0x00, 0x00]),
        ] {
            put(&mut map, va, bytes);
        }
        let width = 0.11f32;
        for va in [0x632e1fu32, 0x632e45, 0x632e91] {
            op(&mut map, va, &[0xc7, 0x04, 0x24], width.to_bits());
        }
        for site in [0x632e26u32, 0x632e4c, 0x632e98] {
            let rel = 0x873b40u32.wrapping_sub(site.wrapping_add(5)) as i32;
            let mut call = [0xe8, 0, 0, 0, 0];
            call[1..].copy_from_slice(&rel.to_le_bytes());
            put(&mut map, site, &call);
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
        assert!((r.damage_row_dy() - -0.03125).abs() < 1e-6);
        assert!((r.damage_row_width() - 0.11).abs() < 1e-6);
        assert_eq!(r.damage_text(3, 5, 0., 2.2).unwrap(), "3 - 5 |+2|R");
        assert_eq!(r.damage_text(3, 5, 10.2, 0.).unwrap(), "13 - 15");
        assert_eq!(r.damage_text(3, 5, 10.2, 2.).unwrap(), "13 - 15 |+2|R");
        assert_eq!(r.damage_text(3, 5, 0., -1.2).unwrap(), "3 - 5 |-1|R");
        assert_eq!(r.armor_text(2., 1.25).unwrap(), "2 |+1.3|R");
        assert_eq!(r.armor_text(4.2, 0.).unwrap(), "4");
        assert_eq!(r.armor_text(2., 0.).unwrap(), "2");
        assert_eq!(r.attribute_text(18., 2.).unwrap(), "18 |+2|R");
        assert_eq!(r.armor_text(1.5, 0.).unwrap(), "2");
        assert_eq!(r.armor_text(-1.5, 0.).unwrap(), "-2");
        assert!(r.attribute_text(1.5, 0.).is_err());
        assert!(r.damage_text(-1, 2, 0., 0.).is_err());
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
        assert_eq!(
            r.damage_text(12, 18, 0., 3.).unwrap(),
            "12 - 18 |CFF00FF00+3|R"
        );
        assert_eq!(r.attribute_text(21., -2.).unwrap(), "21 |CFFFF0000-2|R");
        assert!((r.damage_row_dy() - -0.03125).abs() < 1e-6);
        assert!((r.damage_row_width() - f32::from_bits(0x3dc0a3d7)).abs() < 1e-6);
    }
}
