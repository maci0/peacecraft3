//! The Gem of True Seeing ability class (`Adet`, RTTI `CAbilityDetector`). The registration and
//! the name of the radius field come from the reviewed image and the row values from the ability
//! table; the decode is in `docs/re-notes/passive-held-items.md`, "`Adet`".
//!
//! A held `Adet` makes the holder's owner a detector of every unit within the radius of the
//! holder. The radius is `Rng1`, not `Area1`. The detection types are the two low bits of
//! `DataA1`. The recreation has no invisible units and no per-player detection state to
//! write them to, so the decoded values are retained on the held item and nothing consumes them.
use super::*;
use spell_orb::row_real;

/// Registration call of the class (source `Adet`, computed base `APas`).
pub(super) const DETECTOR_WINDOW: u32 = 0xb80a20;
/// The instruction in the attach slot (`0xb7c560`) that pushes the pointer to the name of the
/// field the radius is read from; the field key is that name followed by the level `1`.
pub(super) const RADIUS_NAME_PUSH: u32 = 0xb7c5a8;
const PUSH_MEM32: [u8; 2] = [0xff, 0x35];
const PUSH_MEM32_LEN: usize = 6;
/// Longest field name the image read accepts; the shipped names are three to seven bytes.
const FIELD_NAME_MAX: usize = 16;
/// The detection types the client counts per player (`0x694750`): bit 0 and bit 1 of `DataA1`.
/// Higher bits are ignored by the client and refused here.
const DETECTION_TYPE_MASK: i64 = 0b11;

/// Bytes of the reviewed image the detector rules are read from.
#[derive(Clone, Debug)]
pub(crate) struct DetectorImage {
    pub(crate) window: Vec<u8>,
    pub(crate) base: Vec<u8>,
    /// The name the attach slot builds the radius key from.
    pub(crate) radius_field: String,
}

impl DetectorImage {
    pub(crate) fn read(pe: &Pe<'_>) -> Result<Self, String> {
        let bytes = |va: u32, len: usize, code: bool| -> Result<Vec<u8>, String> {
            pe.read_from(va, code)
                .map_err(|e| e.to_string())?
                .get(..len)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| format!("truncated image bytes at {va:#x}"))
        };
        let window = bytes(DETECTOR_WINDOW, HEAL_WINDOW_LEN, true)?;
        let base_va = call_target(&window, DETECTOR_WINDOW, 5)?;
        let push = bytes(RADIUS_NAME_PUSH, PUSH_MEM32_LEN, true)?;
        if push[..2] != PUSH_MEM32 {
            return Err("the radius field name push is not a memory push".into());
        }
        let pointer_va = u32::from_le_bytes([push[2], push[3], push[4], push[5]]);
        let name_va = u32::from_le_bytes(
            bytes(pointer_va, 4, false)?
                .try_into()
                .map_err(|_| "truncated radius field name pointer".to_string())?,
        );
        let text = pe.read_from(name_va, false).map_err(|e| e.to_string())?;
        let text = &text[..text.len().min(FIELD_NAME_MAX)];
        let end = text
            .iter()
            .position(|&b| b == 0)
            .ok_or("unterminated radius field name")?;
        Ok(Self {
            base: bytes(base_va, BASE_FN_LEN, true)?,
            window,
            radius_field: String::from_utf8(text[..end].to_vec())
                .map_err(|_| "radius field name is not text".to_string())?,
        })
    }
}

/// The registered class and the row column the radius is read from.
#[derive(Clone, Debug)]
pub(super) struct DetectorClass {
    pub(super) class: String,
    radius_column: String,
}

/// One `Adet` row at the effect level.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Detector {
    /// `Rng1`: the distance from the holder within which units are detected.
    pub radius: f32,
    /// `DataA1`: bit 0 and bit 1 select the two per-player detection counters.
    pub types: u8,
}

impl Rules {
    /// Attaches the detector class, which must be distinct from every registered class. The
    /// radius field name must be letters only, so it cannot name another column.
    pub(crate) fn with_detector(mut self, image: &DetectorImage) -> Result<Self, String> {
        let (class, _) = class_from_call_window_at(&image.window, DETECTOR_WINDOW, &image.base)?;
        if self.detector.is_some() || self.is_registered(&class) {
            return Err("the detector registration must name a distinct class".into());
        }
        if image.radius_field.is_empty()
            || !image.radius_field.bytes().all(|b| b.is_ascii_alphabetic())
        {
            return Err("the detector radius field name is not letters".into());
        }
        self.detector = Some(DetectorClass {
            class,
            radius_column: format!("{}{EFFECT_LEVEL}", image.radius_field),
        });
        Ok(self)
    }

    /// One `Adet` row. The client reads the radius at level 1 whatever the ability level, so a
    /// multi-level row refuses with the other attribute abilities.
    pub(super) fn detector_row(
        &self,
        alias: &str,
        class: &DetectorClass,
    ) -> Result<Detector, String> {
        self.single_level(alias)?;
        let row = self.effect_row(alias, "detector")?;
        let radius = row_real(row, alias, &class.radius_column)?;
        let types = row_real(row, alias, "DataA1")?;
        if !radius.is_finite() || radius < 0.0 {
            return Err(format!(
                "detector {alias} {} is not a number or is negative",
                class.radius_column
            ));
        }
        // The client truncates the float to an integer (`0x9a07b0`).
        let types = types.trunc();
        if !types.is_finite() || !(0.0..=DETECTION_TYPE_MASK as f32).contains(&types) {
            return Err(format!(
                "detector {alias} DataA1 is outside the two decoded detection types"
            ));
        }
        Ok(Detector {
            radius,
            types: types as u8,
        })
    }
}

/// Synthetic image bytes with the shapes read above, for tests. Nothing here is a client value.
#[cfg(test)]
pub(crate) mod fixture {
    use super::spell_orb::fixture::computed_at;
    use super::*;

    /// An image with `Adet` over `Base` and a radius field named `Rng`.
    pub(crate) fn image() -> DetectorImage {
        let mut base = vec![MOV_EAX_IMM32];
        base.extend(b"Base".iter().rev());
        base.push(RET);
        DetectorImage {
            window: computed_at("Adet", DETECTOR_WINDOW),
            base,
            radius_field: "Rng".into(),
        }
    }
}
