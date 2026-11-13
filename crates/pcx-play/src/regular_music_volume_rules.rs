//! Source-derived regular stream gain. Thematic playback has its own volume path.
use crate::{
    menu_lighting::REVIEWED_IMAGE, neutral_player::sha256, pe_image::PeSections,
    scripts::AssetIdentity,
};

const REVIEWED_BACKEND: [u8; 32] = [
    0xcc, 0x14, 0x2b, 0x9d, 0x8b, 0x52, 0x23, 0xe2, 0x72, 0x0c, 0x64, 0x40, 0xcb, 0x7a, 0x12, 0x4c,
    0x0a, 0x80, 0xd2, 0xfb, 0x04, 0xec, 0xf5, 0x9a, 0xd7, 0x33, 0x1d, 0xfd, 0x6e, 0x3c, 0xb5, 0x1f,
];

type Read<'a> = dyn Fn(u32, usize) -> Result<Vec<u8>, String> + 'a;

#[derive(Clone, Debug)]
pub struct Rules {
    pub image_sha256: [u8; 32],
    pub backend_sha256: [u8; 32],
    pub backend_origin: AssetIdentity,
    initial: u32,
    maximum: u32,
    channel: i32,
    unity: f32,
    curve: Vec<u32>,
    shift: u8,
    threshold: u32,
    mixer_unity: u32,
}

fn bytes(read: &Read<'_>, at: u32, size: usize) -> Result<Vec<u8>, String> {
    let data = read(at, size)?;
    if data.len() != size {
        return Err(format!("truncated regular music source window {at:#x}"));
    }
    Ok(data)
}
fn operand(read: &Read<'_>, at: u32, prefix: &[u8], size: usize) -> Result<Vec<u8>, String> {
    let data = bytes(read, at, prefix.len() + size)?;
    if !data.starts_with(prefix) {
        return Err(format!("regular music instruction differs at {at:#x}"));
    }
    Ok(data[prefix.len()..].to_vec())
}
fn word(read: &Read<'_>, at: u32) -> Result<u32, String> {
    Ok(u32::from_le_bytes(bytes(read, at, 4)?.try_into().unwrap()))
}
fn immediate(read: &Read<'_>, at: u32, prefix: &[u8]) -> Result<u32, String> {
    Ok(u32::from_le_bytes(
        operand(read, at, prefix, 4)?.try_into().unwrap(),
    ))
}
fn scalar(read: &Read<'_>, at: u32, prefix: &[u8]) -> Result<f32, String> {
    Ok(f32::from_bits(word(read, immediate(read, at, prefix)?)?))
}

impl Rules {
    /// The transport derives the module owner from this source-anchored import slot.
    pub fn backend_iat(image: &[u8]) -> Result<u32, String> {
        if image.len() > 32 << 20 || sha256(image) != REVIEWED_IMAGE {
            return Err("unsupported regular music client".into());
        }
        let pe = PeSections::parse(image)?;
        immediate(
            &|va, len| {
                pe.va_bytes(va, len)
                    .map(<[u8]>::to_vec)
                    .ok_or_else(|| format!("regular music image window {va:#x}"))
            },
            0x7c9b51,
            &[0xff, 0x15],
        )
    }

    /// Both local images are pinned by the caller's source generation; no DLL is executed.
    pub fn read(
        image: &[u8],
        backend: &[u8],
        backend_origin: AssetIdentity,
    ) -> Result<Self, String> {
        Self::backend_iat(image)?;
        if backend.len() > 4 << 20
            || sha256(backend) != REVIEWED_BACKEND
            || [
                &backend_origin.source,
                &backend_origin.edition,
                &backend_origin.version,
            ]
            .iter()
            .any(|s| s.trim().is_empty())
        {
            return Err("unsupported regular music backend source".into());
        }
        let main = PeSections::parse(image)?;
        let dll = PeSections::parse(backend)?;
        Self::decode(
            &|va, len| {
                main.va_bytes(va, len)
                    .map(<[u8]>::to_vec)
                    .ok_or_else(|| format!("regular music image window {va:#x}"))
            },
            &|va, len| {
                dll.va_bytes(va, len)
                    .map(<[u8]>::to_vec)
                    .ok_or_else(|| format!("regular music backend window {va:#x}"))
            },
            backend_origin,
        )
    }

    fn decode(
        main: &Read<'_>,
        dll: &Read<'_>,
        backend_origin: AssetIdentity,
    ) -> Result<Self, String> {
        let maximum = immediate(main, 0x5da426, &[0xb8])?;
        let saved = immediate(main, 0x5da430, &[0xa3])?;
        if saved != immediate(main, 0x5d8dd1, &[0xa1])?
            || bytes(main, 0x5da42b, 5)? != [0x3b, 0xc8, 0x0f, 0x42, 0xc1]
        {
            return Err("regular music saved-volume relationship differs".into());
        }
        let initial = word(main, saved)?;
        let count = immediate(main, 0x7c6392, &[0x81, 0xfe])?;
        let offset = scalar(main, 0x7c6333, &[0x0f, 0x28, 0x15])?;
        let bias = scalar(main, 0x7c636a, &[0x0f, 0x28, 0x15])?;
        let peak = scalar(main, 0x7c639a, &[0xf3, 0x0f, 0x10, 0x15])?;
        let channel = i32::from(operand(main, 0x5da5c2, &[0x6a], 1)?[0] as i8);
        let unity = scalar(main, 0x79fa44, &[0xf3, 0x0f, 0x10, 0x0d])?;
        let backend_cap = immediate(dll, 0x21114513, &[0xba])?;
        let master = immediate(dll, 0x21113708, &[0xc7, 0x46, 0x10])?;
        let shift = operand(dll, 0x211145c2, &[0xc1, 0xe6], 1)?[0];
        let threshold = immediate(dll, 0x211145a9, &[0xb8])?;
        let mixer_unity = immediate(dll, 0x211145b0, &[0xba])?;
        let copy_shift = operand(dll, 0x2113d29e, &[0xc1, 0xf8], 1)?[0];
        let output_channels = operand(main, 0x79db4c, &[0x6a], 1)?[0];
        let pan = immediate(dll, 0x2111448e, &[0xc7, 0x46, 0x68])?;
        let left_end = immediate(dll, 0x21114577, &[0xba])?;
        let right_start = immediate(dll, 0x21114590, &[0x0f, 0xb6, 0x87])?;
        let pan_division = operand(dll, 0x2111458a, &[0xc1, 0xf8], 1)?[0];
        let left = bytes(
            dll,
            left_end.checked_sub(pan).ok_or("regular music pan range")?,
            1,
        )?[0];
        let right = bytes(
            dll,
            right_start
                .checked_add(pan)
                .ok_or("regular music pan range")?,
            1,
        )?[0];
        // Collection and shift bounds are recreation resource guards, not authored gain values.
        if maximum == 0
            || maximum > 4096
            || initial > maximum
            || count
                != maximum
                    .checked_add(1)
                    .ok_or("regular music table size overflow")?
            || peak != maximum as f32
            || !offset.is_finite()
            || offset <= 0.0
            || !bias.is_finite()
            || !unity.is_finite()
            || unity <= 0.0
            || channel < 0
            || backend_cap != maximum
            || master != backend_cap
            || shift >= 32
            || copy_shift >= 32
            || maximum.checked_mul(1u32.checked_shl(u32::from(shift)).unwrap_or(0))
                != Some(threshold)
            || 1u32.checked_shl(u32::from(copy_shift)) != Some(mixer_unity)
            || threshold == 0
            || threshold > mixer_unity
            || output_channels <= 1
            || pan > backend_cap
            || pan_division >= 8
            || left_end.checked_sub(right_start) != Some(backend_cap)
            || left != right
            || u32::from(left)
                != 1u32
                    .checked_shl(u32::from(shift) + u32::from(pan_division))
                    .unwrap_or(0)
        {
            return Err("unsupported regular music curve or mixer profile".into());
        }
        let mut log = Vec::new();
        log.try_reserve_exact(count as usize)
            .map_err(|_| "regular music table capacity")?;
        for i in 0..count {
            let input = i as f32 + offset;
            log.push(f64::from(input).log10() as f32 - bias);
        }
        let scale = peak / log.last().copied().ok_or("empty regular music table")?;
        let mut curve = Vec::new();
        curve
            .try_reserve_exact(count as usize)
            .map_err(|_| "regular music table capacity")?;
        for value in log.into_iter().rev() {
            let mapped = peak - value * scale;
            if !mapped.is_finite() || mapped < 0.0 || mapped > maximum as f32 {
                return Err("unsupported regular music generated level".into());
            }
            curve.push(mapped as u32);
        }
        Ok(Self {
            image_sha256: REVIEWED_IMAGE,
            backend_sha256: REVIEWED_BACKEND,
            backend_origin,
            initial,
            maximum,
            channel,
            unity,
            curve,
            shift,
            threshold,
            mixer_unity,
        })
    }

    pub fn channel(&self) -> i32 {
        self.channel
    }
    pub fn initial_level(&self) -> u32 {
        self.initial
    }
    pub fn maximum_level(&self) -> u32 {
        self.maximum
    }

    pub fn level(&self, level: i32) -> Result<u32, String> {
        u32::try_from(level)
            .ok()
            .filter(|v| *v <= self.maximum)
            .ok_or_else(|| "regular music volume outside the qualified normal domain".into())
    }

    /// User/group channel gain precedes integer conversion and the source table lookup.
    pub fn gain(&self, level: u32, group: f32, user: f32) -> Result<f32, String> {
        if level > self.maximum
            || !group.is_finite()
            || !user.is_finite()
            || group < 0.0
            || user < 0.0
        {
            return Err("unsupported regular music volume input".into());
        }
        let channel = (group * user).clamp(0.0, self.unity);
        let index = ((level as f32 * channel) as u32).min(self.maximum);
        let mapped = self.curve[index as usize];
        let coefficient = mapped
            .checked_mul(1u32 << self.shift)
            .ok_or("regular music mixer coefficient overflow")?;
        Ok(if coefficient >= self.threshold {
            1.0
        } else {
            coefficient as f32 / self.mixer_unity as f32
        })
    }
}

#[cfg(test)]
#[path = "regular_music_volume_rules_tests.rs"]
pub(crate) mod tests;
