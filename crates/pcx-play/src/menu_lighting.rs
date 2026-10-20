//! Fixed-function menu light and particle appearance parameters from a reviewed client image.
use crate::{
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    scripts::AssetIdentity,
};

// Identity and instruction locations bind the reviewed 1.29.2.9231 profile;
// the parameter values themselves are read from the supplied image.
pub(crate) const REVIEWED_IMAGE: Sha256Digest = [
    0x3f, 0x2e, 0xd0, 0x12, 0x0d, 0x80, 0x57, 0x8b, 0xf0, 0x7e, 0x44, 0x23, 0x29, 0x6d, 0xad, 0xe1,
    0xad, 0xfb, 0x95, 0x9d, 0x59, 0xa2, 0xd2, 0x0a, 0x75, 0x84, 0x22, 0x45, 0x59, 0x57, 0x0e, 0xed,
];

#[derive(Clone, Debug)]
pub struct Parameters {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    pub linear: f32,
    pub quadratic: f32,
    pub constant: f32,
    pub range: f32,
    pub channel_scale: f32,
    pub max_lights: usize,
    pub particle_appearance: ParticleAppearance,
    pub cutout_reference: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct ParticleAppearance {
    pub phase_scale: f32,
    pub phase_bias: f32,
    pub colour_scale: f32,
    pub packing_bias: f32,
    pub packing_shift: u32,
}

#[derive(Debug, thiserror::Error)]
#[error("menu lighting {source_name}: {reason}")]
pub struct Error {
    source_name: String,
    reason: &'static str,
}

impl Parameters {
    /// The uncapped D3D9 profile, independent of the recreation's host adapter.
    /// The original AMD-specific coefficient cap is not selected by this profile.
    pub fn read(image: &[u8], origin: AssetIdentity) -> Result<Self, Error> {
        let fail = |reason| Error {
            source_name: origin.source.clone(),
            reason,
        };
        if image.len() > 32 * 1024 * 1024 {
            return Err(fail("client image exceeds budget"));
        }
        if [&origin.source, &origin.edition, &origin.version]
            .iter()
            .any(|value| value.trim().is_empty())
        {
            return Err(fail("missing source identity"));
        }
        let digest = sha256(image);
        if digest != REVIEWED_IMAGE {
            return Err(fail(
                "unsupported client image; no lighting values substituted",
            ));
        }
        let pe = Pe::parse(
            image,
            RuleInputBudget {
                image_bytes: 32 * 1024 * 1024,
                profile_bytes: 128,
                sections: 96,
            },
        )
        .map_err(|_| fail("invalid PE image"))?;
        let read = |va, code| -> Result<f32, Error> {
            let bytes = pe
                .read_from(va, code)
                .map_err(|_| fail("missing reviewed parameter operand"))?;
            let word = bytes
                .get(..4)
                .ok_or_else(|| fail("truncated parameter operand"))?;
            Ok(f32::from_le_bytes(word.try_into().unwrap()))
        };
        let values = [
            read(0x42e9b7, true)?,
            read(0x42e9b0, true)?,
            read(0x7de4ca, true)?,
            read(0x7de4ba, true)?,
            read(0xebc5ac, false)?,
        ];
        let max_lights = usize::from(
            *pe.read_from(0x7de519, true)
                .map_err(|_| fail("missing reviewed light slot operand"))?
                .first()
                .ok_or_else(|| fail("truncated light slot operand"))?,
        );
        let particle_appearance = ParticleAppearance {
            phase_scale: read(0xed59e4, false)?,
            phase_bias: read(0xe8df70, false)?,
            colour_scale: read(0xe7834c, false)?,
            packing_bias: read(0xe8dfe8, false)?,
            packing_shift: u32::from(
                *pe.read_from(0x97ecfb, true)
                    .map_err(|_| fail("missing reviewed particle packing operand"))?
                    .first()
                    .ok_or_else(|| fail("truncated particle packing operand"))?,
            ),
        };
        let reference = pe
            .read_from(0x10df970, false)
            .map_err(|_| fail("missing reviewed cutout reference"))?;
        let reference = reference
            .get(..4)
            .ok_or_else(|| fail("truncated cutout reference"))?;
        let reference = u8::try_from(u32::from_le_bytes(reference.try_into().unwrap()))
            .map_err(|_| fail("cutout reference exceeds byte range"))?;
        Self::checked(
            origin,
            digest,
            values,
            max_lights,
            particle_appearance,
            f32::from(reference) / f32::from(u8::MAX),
        )
    }

    fn checked(
        origin: AssetIdentity,
        image_sha256: Sha256Digest,
        values: [f32; 5],
        max_lights: usize,
        particle_appearance: ParticleAppearance,
        cutout_reference: f32,
    ) -> Result<Self, Error> {
        let [linear, quadratic, constant, range, channel_scale] = values;
        if values
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0)
            || constant == 0.0
            || range == 0.0
            || channel_scale == 0.0
            || max_lights == 0
            || !cutout_reference.is_finite()
            || !(0.0..=1.0).contains(&cutout_reference)
            || [
                particle_appearance.phase_scale,
                particle_appearance.phase_bias,
                particle_appearance.colour_scale,
                particle_appearance.packing_bias,
            ]
            .into_iter()
            .any(|value| !value.is_finite() || value < 0.)
            || particle_appearance.phase_scale + particle_appearance.phase_bias > 1.
            || particle_appearance.colour_scale == 0.
            || particle_appearance.packing_bias == 0.
            || particle_appearance.packing_shift >= u32::BITS
        {
            return Err(Error {
                source_name: origin.source,
                reason: "unusable lighting parameter",
            });
        }
        Ok(Self {
            origin,
            image_sha256,
            linear,
            quadratic,
            constant,
            range,
            channel_scale,
            max_lights,
            particle_appearance,
            cutout_reference,
        })
    }

    /// D3D9 attenuation: Range excludes the light before the denominator.
    pub fn attenuation(&self, distance: f32) -> f32 {
        if !distance.is_finite() || distance < 0.0 || distance > self.range {
            return 0.0;
        }
        (self.constant + self.linear * distance + self.quadratic * (distance * distance)).recip()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn origin() -> AssetIdentity {
        AssetIdentity {
            source: "synthetic-light".into(),
            edition: "fixture".into(),
            version: "fixture".into(),
        }
    }
    fn particle() -> ParticleAppearance {
        ParticleAppearance {
            phase_scale: 0.5,
            phase_bias: 0.25,
            colour_scale: 128.,
            packing_bias: 1024.,
            packing_shift: 13,
        }
    }
    #[test]
    fn attenuation_uses_all_source_parameters_and_inclusive_range() {
        let p = Parameters::checked(
            origin(),
            [73; 32],
            [0.25, 0.125, 2.0, 12.0, 0.01],
            4,
            particle(),
            0.6,
        )
        .unwrap();
        assert_eq!(p.attenuation(0.0), 0.5);
        assert_eq!(p.attenuation(4.0), 0.2);
        assert_eq!(p.attenuation(12.0), 1.0 / 23.0);
        assert_eq!(p.attenuation(12.001), 0.0);
        for d in [-1.0, f32::NAN, f32::INFINITY] {
            assert_eq!(p.attenuation(d), 0.0);
        }
    }
    #[test]
    fn quadratic_term_uses_squared_distance_before_its_coefficient() {
        let p = Parameters::checked(
            origin(),
            [19; 32],
            [0.0023, 0.003, 1.2, 100.0, 0.01],
            4,
            particle(),
            0.6,
        )
        .unwrap();
        // Independent f32 reference with synthetic operands; the left-associated mutant differs.
        assert_eq!(p.attenuation(77.7).to_bits(), 0x3d52_2720);
        assert!(
            Parameters::checked(
                origin(),
                [0; 32],
                [0.0, 0.0, 1.0, 9.0, 0.01],
                0,
                particle(),
                0.6
            )
            .is_err()
        );
    }

    #[test]
    fn invalid_parameters_and_unreviewed_images_refuse() {
        for index in 0..5 {
            for bad in [f32::NAN, f32::INFINITY, -0.1] {
                let mut values = [0.1, 0.2, 1.0, 9.0, 0.01];
                values[index] = bad;
                assert!(
                    Parameters::checked(origin(), [0; 32], values, 4, particle(), 0.6).is_err()
                );
            }
        }
        assert!(Parameters::read(b"MZ unknown", origin()).is_err());
        assert!(
            Parameters::checked(
                origin(),
                [0; 32],
                [0.0, 0.0, 0.0, 9.0, 0.01],
                4,
                particle(),
                0.6
            )
            .is_err()
        );
    }
    #[test]
    fn invalid_cutout_references_refuse() {
        for reference in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
            assert!(
                Parameters::checked(
                    origin(),
                    [0; 32],
                    [0., 0., 1., 10., 0.01],
                    4,
                    particle(),
                    reference
                )
                .is_err()
            );
        }
    }
    #[test]
    fn invalid_particle_coefficients_refuse() {
        for index in 0..4 {
            for bad in [f32::NAN, f32::INFINITY, -1.] {
                let mut appearance = particle();
                match index {
                    0 => appearance.phase_scale = bad,
                    1 => appearance.phase_bias = bad,
                    2 => appearance.colour_scale = bad,
                    _ => appearance.packing_bias = bad,
                }
                assert!(
                    Parameters::checked(
                        origin(),
                        [0; 32],
                        [0., 0., 1., 10., 0.01],
                        4,
                        appearance,
                        0.6
                    )
                    .is_err()
                );
            }
        }
        let mut appearance = particle();
        appearance.packing_shift = u32::BITS;
        assert!(
            Parameters::checked(
                origin(),
                [0; 32],
                [0., 0., 1., 10., 0.01],
                4,
                appearance,
                0.6
            )
            .is_err()
        );
    }
    #[test]
    #[ignore = "requires explicitly supplied reviewed Legacy client image"]
    fn selected_original_image_reads_reviewed_operands() {
        let path = std::env::var("PCX_LIGHTING_CLIENT_IMAGE").expect("PCX_LIGHTING_CLIENT_IMAGE");
        let image = std::fs::read(&path).unwrap();
        let mut source = origin();
        source.source = path;
        source.edition = "Legacy target".into();
        source.version = "1.29.2.9231".into();
        let p = Parameters::read(&image, source).unwrap();
        assert_eq!(p.image_sha256, sha256(&image));
        let pe = Pe::parse(
            &image,
            RuleInputBudget {
                image_bytes: 32 * 1024 * 1024,
                profile_bytes: 128,
                sections: 96,
            },
        )
        .unwrap();
        let word = pe.read_from(0x10df970, false).unwrap();
        assert_eq!(
            p.cutout_reference * f32::from(u8::MAX),
            u32::from_le_bytes(word[..4].try_into().unwrap()) as f32
        );
        // Relations, not copied original parameter values.
        assert!(p.constant > p.quadratic && p.quadratic > p.linear);
        assert!(p.attenuation(p.range / 2.0) > p.attenuation(p.range));
        assert_eq!(
            p.particle_appearance.phase_scale + 2. * p.particle_appearance.phase_bias,
            1.
        );
        assert_eq!(p.particle_appearance.colour_scale * p.channel_scale, 1.);
        let mut changed = image;
        changed[0] ^= 1;
        assert!(Parameters::read(&changed, p.origin).is_err());
    }
}
