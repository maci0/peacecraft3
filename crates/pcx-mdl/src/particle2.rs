//! PRE2 state layout proved by native binary/keyword readers in format-mdx-particles.md.
use super::{PARTICLE2_FIXED, Particle2};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Particle2Properties {
    pub speed: f32,
    pub variation: f32,
    pub latitude: f32,
    pub gravity: f32,
    pub life_span: f32,
    pub emission_rate: f32,
    pub length: f32,
    pub width: f32,
    pub filter_mode: u32,
    pub rows: u32,
    pub columns: u32,
    /// Native enum: Head=0, Tail=1, Both=2. Kept authored, never defaulted.
    pub head_or_tail: u32,
    pub tail_length: f32,
    pub time: f32,
    /// Three segment colours, components in authored file order.
    pub segment_colours: [[f32; 3]; 3],
    pub segment_alpha: [u8; 3],
    pub particle_scaling: [f32; 3],
    pub life_span_uv_anim: [u32; 3],
    pub decay_uv_anim: [u32; 3],
    pub tail_uv_anim: [u32; 3],
    pub tail_decay_uv_anim: [u32; 3],
    pub texture_id: u32,
    pub squirt: u32,
    pub priority_plane: i32,
    pub replaceable_id: u32,
}

impl Particle2Properties {
    pub fn from_bytes(bytes: &[u8; PARTICLE2_FIXED]) -> Self {
        let u = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let f = |at| f32::from_bits(u(at));
        let vector = |at| [f(at), f(at + 4), f(at + 8)];
        let integers = |at| [u(at), u(at + 4), u(at + 8)];
        Self {
            speed: f(0),
            variation: f(4),
            latitude: f(8),
            gravity: f(12),
            life_span: f(16),
            emission_rate: f(20),
            length: f(24),
            width: f(28),
            filter_mode: u(32),
            rows: u(36),
            columns: u(40),
            head_or_tail: u(44),
            tail_length: f(48),
            time: f(52),
            segment_colours: [vector(56), vector(68), vector(80)],
            segment_alpha: bytes[92..95].try_into().unwrap(),
            particle_scaling: vector(95),
            life_span_uv_anim: integers(107),
            decay_uv_anim: integers(119),
            tail_uv_anim: integers(131),
            tail_decay_uv_anim: integers(143),
            texture_id: u(155),
            squirt: u(159),
            priority_plane: u(163) as i32,
            replaceable_id: u(167),
        }
    }

    /// Reassemble authored fields exactly; useful for local original-fixture checks.
    pub fn to_bytes(self) -> [u8; PARTICLE2_FIXED] {
        let mut out = Vec::with_capacity(PARTICLE2_FIXED);
        for value in [
            self.speed,
            self.variation,
            self.latitude,
            self.gravity,
            self.life_span,
            self.emission_rate,
            self.length,
            self.width,
        ] {
            out.extend(value.to_bits().to_le_bytes());
        }
        for value in [self.filter_mode, self.rows, self.columns, self.head_or_tail] {
            out.extend(value.to_le_bytes());
        }
        for value in [self.tail_length, self.time] {
            out.extend(value.to_bits().to_le_bytes());
        }
        for colour in self.segment_colours {
            for value in colour {
                out.extend(value.to_bits().to_le_bytes());
            }
        }
        out.extend(self.segment_alpha);
        for value in self.particle_scaling {
            out.extend(value.to_bits().to_le_bytes());
        }
        for triplet in [
            self.life_span_uv_anim,
            self.decay_uv_anim,
            self.tail_uv_anim,
            self.tail_decay_uv_anim,
        ] {
            for value in triplet {
                out.extend(value.to_le_bytes());
            }
        }
        for value in [
            self.texture_id,
            self.squirt,
            self.priority_plane as u32,
            self.replaceable_id,
        ] {
            out.extend(value.to_le_bytes());
        }
        out.try_into().expect("native PRE2 fixed-state width")
    }
}

impl Particle2 {
    pub fn properties(&self) -> Particle2Properties {
        Particle2Properties::from_bytes(&self.state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unaligned_segment_fields_and_signed_priority_are_preserved() {
        let bytes = std::array::from_fn(|index| (index.wrapping_mul(37)) as u8);
        let parsed = Particle2Properties::from_bytes(&bytes);
        assert_eq!(parsed.segment_alpha, bytes[92..95]);
        assert_eq!(
            parsed.particle_scaling[0].to_bits(),
            u32::from_le_bytes(bytes[95..99].try_into().unwrap())
        );
        assert_eq!(
            parsed.priority_plane,
            i32::from_le_bytes(bytes[163..167].try_into().unwrap())
        );
        assert_eq!(parsed.to_bytes(), bytes);
    }
}

#[cfg(test)]
mod native_track_regressions {
    use super::*;
    use crate::{Mdlx, NO_ID};

    #[test]
    fn native_variation_and_gravity_tracks_are_supported() {
        let mut record = vec![0; 4 + 96 + PARTICLE2_FIXED];
        record[4..8].copy_from_slice(&96u32.to_le_bytes());
        for magic in [b"KP2R", b"KP2G"] {
            record.extend(magic);
            for word in [1u32, 1, NO_ID, 250] {
                record.extend(word.to_le_bytes());
            }
            record.extend(1.25f32.to_le_bytes());
        }
        let length = record.len() as u32;
        record[..4].copy_from_slice(&length.to_le_bytes());
        let mut data = b"MDLXVERS".to_vec();
        data.extend(4u32.to_le_bytes());
        data.extend(800u32.to_le_bytes());
        data.extend(b"PRE2");
        data.extend(length.to_le_bytes());
        data.extend(record);
        let emitters = Mdlx::parse(&data).unwrap().particles2(&data).unwrap();
        assert_eq!(emitters[0].tracks.len(), 2);
        assert_eq!(emitters[0].tracks[0].magic, *b"KP2R");
        assert_eq!(emitters[0].tracks[1].magic, *b"KP2G");
        assert_eq!(emitters[0].tracks[1].scalar(0), Some(1.25));
    }
}
