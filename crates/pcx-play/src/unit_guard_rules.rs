//! Qualified selected-client AI enrollment gates and the internal release order.
use crate::{
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    scripts::AssetIdentity,
};
#[derive(Clone, Debug)]
pub struct Rules {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    stop_order: i32,
    dead_mask: u32,
    excluded_type_mask: u8,
}
impl Rules {
    pub fn reviewed_legacy(image: &[u8], origin: AssetIdentity) -> Result<Self, String> {
        let context = format!("unit guard rules {}", origin.source);
        let fail = |why: &str| format!("{context}: {why}");
        if image.len() > 32 << 20
            || sha256(image) != crate::menu_lighting::REVIEWED_IMAGE
            || [&origin.source, &origin.edition, &origin.version]
                .iter()
                .any(|s| s.trim().is_empty())
        {
            return Err(fail("unsupported client identity"));
        }
        let pe = Pe::parse(
            image,
            RuleInputBudget {
                image_bytes: 32 << 20,
                profile_bytes: 64 << 10,
                sections: 96,
            },
        )
        .map_err(|e| fail(&e.to_string()))?;
        Self::decode(origin, sha256(image), |va, n| {
            pe.read_from(va, true)
                .map_err(|e| e.to_string())?
                .get(..n)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| "truncated guard instruction window".into())
        })
        .map_err(|e| fail(&e))
    }
    fn decode(
        origin: AssetIdentity,
        image_sha256: Sha256Digest,
        mut read: impl FnMut(u32, usize) -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        let gate = read(0xab455f, 10)?;
        let kind = read(0xab4579, 2)?;
        let dead = read(0x667050, 13)?;
        let getter = read(0x66e7d0, 7)?;
        let order = read(0x6906e1, 7)?;
        if gate.len() != 10
            || gate[..2] != [0xf7, 0x86]
            || kind.len() != 2
            || kind[0] != 0xa8
            || dead.len() != 13
            || dead[..2] != [0x8b, 0x81]
            || dead[6..8] != [0xc1, 0xe8]
            || dead[9..] != [0x83, 0xe0, 1, 0xc3]
            || getter.len() != 7
            || getter[..2] != [0x8b, 0x81]
            || getter[6] != 0xc3
            || order.len() != 7
            || order[..3] != [0xc7, 0x46, 0x24]
            || gate[2..6] != dead[2..6]
        {
            return Err("unsupported guard enrollment/dead/order instruction shape".into());
        }
        let dead_mask =
            u32::from_le_bytes(gate[6..10].try_into().map_err(|_| "truncated dead mask")?);
        if dead_mask == 0 || Some(dead_mask) != 1u32.checked_shl(u32::from(dead[8])) || kind[1] == 0
        {
            return Err("inconsistent guard enrollment masks".into());
        }
        Ok(Self {
            origin,
            image_sha256,
            stop_order: i32::from_le_bytes(
                order[3..]
                    .try_into()
                    .map_err(|_| "truncated guard stop order")?,
            ),
            dead_mask,
            excluded_type_mask: kind[1],
        })
    }
    pub fn stop_order(&self) -> i32 {
        self.stop_order
    }
    /// Native masks are retained without inventing equivalent catalog bits.
    pub fn enrollment_masks(&self) -> (u32, u8) {
        (self.dead_mask, self.excluded_type_mask)
    }
    pub fn native_enrollment_allowed(&self, unit_flags: u32, type_flags: u32) -> bool {
        unit_flags & self.dead_mask == 0 && type_flags & u32::from(self.excluded_type_mask) == 0
    }
}
#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::collections::BTreeMap;
    fn windows(shift: u8, kind: u8, stop: i32) -> BTreeMap<u32, Vec<u8>> {
        let mut gate = vec![0xf7, 0x86];
        gate.extend(0x1234u32.to_le_bytes());
        gate.extend((1u32 << shift).to_le_bytes());
        let mut dead = vec![0x8b, 0x81];
        dead.extend(0x1234u32.to_le_bytes());
        dead.extend([0xc1, 0xe8, shift, 0x83, 0xe0, 1, 0xc3]);
        let mut getter = vec![0x8b, 0x81];
        getter.extend(0x5678u32.to_le_bytes());
        getter.push(0xc3);
        let mut order = vec![0xc7, 0x46, 0x24];
        order.extend(stop.to_le_bytes());
        BTreeMap::from([
            (0xab455f, gate),
            (0xab4579, vec![0xa8, kind]),
            (0x667050, dead),
            (0x66e7d0, getter),
            (0x6906e1, order),
        ])
    }
    fn decode(w: &BTreeMap<u32, Vec<u8>>) -> Result<Rules, String> {
        Rules::decode(
            AssetIdentity {
                source: "synthetic:guard".into(),
                edition: "synthetic".into(),
                version: "fixture".into(),
            },
            sha256(&[]),
            |va, n| {
                w.get(&va)
                    .and_then(|b| b.get(..n))
                    .map(<[u8]>::to_vec)
                    .ok_or_else(|| "truncated synthetic guard".into())
            },
        )
    }
    pub(crate) fn fixture(stop: i32) -> Rules {
        decode(&windows(7, 0x40, stop)).unwrap()
    }
    #[test]
    fn enrollment_masks_and_stop_order_follow_changed_source_operands() {
        for (shift, kind, stop) in [(7, 0x40, 913), (11, 0x20, 1777)] {
            let r = decode(&windows(shift, kind, stop)).unwrap();
            assert_eq!(r.stop_order(), stop);
            assert!(r.native_enrollment_allowed(0, 0));
            assert!(!r.native_enrollment_allowed(1 << shift, 0));
            assert!(!r.native_enrollment_allowed(0, u32::from(kind)));
        }
    }
    #[test]
    fn malformed_and_inconsistent_guard_windows_refuse() {
        for va in [0xab455f, 0xab4579, 0x667050, 0x66e7d0, 0x6906e1] {
            let mut w = windows(7, 0x40, 913);
            w.get_mut(&va).unwrap().pop();
            assert!(decode(&w).is_err());
            let mut w = windows(7, 0x40, 913);
            w.get_mut(&va).unwrap()[0] ^= 1;
            assert!(decode(&w).is_err());
        }
        let mut w = windows(7, 0x40, 913);
        w.get_mut(&0x667050).unwrap()[8] = 8;
        assert!(decode(&w).is_err());
    }
    #[test]
    #[ignore = "requires explicit PEACECRAFT3_GUARD_IMAGE qualified original client"]
    fn local_original_guard_enrollment_and_stop_order_are_qualified() {
        let p = std::env::var_os("PEACECRAFT3_GUARD_IMAGE").expect("explicit original image");
        let b = std::fs::read(&p).unwrap();
        let r = Rules::reviewed_legacy(
            &b,
            AssetIdentity {
                source: std::path::PathBuf::from(p).display().to_string(),
                edition: "selected legacy".into(),
                version: "PE VERSIONINFO 1.29.2.9231".into(),
            },
        )
        .unwrap();
        assert_eq!(r.image_sha256, sha256(&b));
        assert_ne!(r.enrollment_masks().0, 0);
    }
}
