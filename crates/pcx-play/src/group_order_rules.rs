//! Selected-client bounds for the script group-order consumers.
use crate::{
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    scripts::AssetIdentity,
};

#[derive(Clone, Debug)]
pub struct Rules {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    point_member_limit: usize,
}
impl Rules {
    pub fn reviewed_legacy(image: &[u8], origin: AssetIdentity) -> Result<Self, String> {
        let context = format!("group order rules {}", origin.source);
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
                .ok_or_else(|| "truncated group order instruction window".into())
        })
        .map_err(|e| fail(&e))
    }
    fn decode(
        origin: AssetIdentity,
        image_sha256: Sha256Digest,
        mut read: impl FnMut(u32, usize) -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        // Both preparations and the point delivery pass a positive signed-byte
        // member bound. Immediate delivery instead passes the format's -1 sentinel.
        let immediate_prepare = read(0x502da2, 8)?;
        let point_prepare = read(0x5030c2, 13)?;
        let point_delivery = read(0x5030ee, 13)?;
        let immediate_delivery = read(0x502dd0, 13)?;
        let immediate = bounded_push(&immediate_prepare, &[])?;
        let prepared = bounded_push(&point_prepare, &POINT_PREPARE_MOVES)?;
        let delivered = bounded_push(&point_delivery, &[])?;
        if immediate.limit != prepared.limit
            || immediate.limit != delivered.limit
            || immediate.callback != prepared.callback
            || immediate_delivery.len() != 13
            || immediate_delivery[..4] != [0x50, 0x6a, 0xff, 0x68]
            || immediate_delivery[8] != 0xe8
            || point_delivery[8] != 0xe8
        {
            return Err("inconsistent group preparation/delivery bounds".into());
        }
        let target = |va: u32, window: &[u8]| -> Result<u32, String> {
            let displacement = i32::from_le_bytes(
                window[9..13]
                    .try_into()
                    .map_err(|_| "truncated group enumeration call")?,
            );
            Ok(va.wrapping_add(13).wrapping_add_signed(displacement))
        };
        if target(0x502dd0, &immediate_delivery)? != target(0x5030ee, &point_delivery)? {
            return Err("group delivery enumeration targets differ".into());
        }
        Ok(Self {
            origin,
            image_sha256,
            point_member_limit: usize::from(delivered.limit),
        })
    }
    pub fn point_member_limit(&self) -> usize {
        self.point_member_limit
    }
}

/// Authored `mov [ebp-0x20], ecx; mov ecx, edx` between the point preparation's
/// bound push and its callback push.
const POINT_PREPARE_MOVES: [u8; 5] = [0x89, 0x4d, 0xe0, 0x8b, 0xca];

struct BoundedPush {
    limit: u8,
    callback: [u8; 4],
}

/// `push eax; push n; <intervening>; push callback`, where the intervening bytes
/// must match exactly at their authored offset. Nothing is searched for.
fn bounded_push(window: &[u8], intervening: &[u8]) -> Result<BoundedPush, String> {
    let unsupported = || "unsupported bounded group enumeration instruction".to_string();
    let callback_at = 3 + intervening.len();
    if window.get(..2) != Some(&[0x50, 0x6a])
        || !matches!(window.get(2), Some(1..=127))
        || window.get(3..callback_at) != Some(intervening)
        || window.get(callback_at) != Some(&0x68)
    {
        return Err(unsupported());
    }
    let callback = window
        .get(callback_at + 1..callback_at + 5)
        .ok_or_else(unsupported)?;
    Ok(BoundedPush {
        limit: window[2],
        callback: callback.try_into().map_err(|_| unsupported())?,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::order_registry::Binding;
    use std::collections::BTreeMap;
    fn windows(limit: u8) -> BTreeMap<u32, Vec<u8>> {
        let prepare = vec![0x50, 0x6a, limit, 0x68, 0x44, 0x33, 0x22, 0x11];
        let point_prepare = vec![
            0x50, 0x6a, limit, 0x89, 0x4d, 0xe0, 0x8b, 0xca, 0x68, 0x44, 0x33, 0x22, 0x11,
        ];
        let delivery = |va: u32, cap: u8| {
            let mut bytes = vec![0x50, 0x6a, cap, 0x68, 0x88, 0x77, 0x66, 0x55, 0xe8];
            bytes.extend_from_slice(&(0x12345678u32.wrapping_sub(va + 13) as i32).to_le_bytes());
            bytes
        };
        BTreeMap::from([
            (0x502da2, prepare),
            (0x5030c2, point_prepare),
            (0x502dd0, delivery(0x502dd0, 0xff)),
            (0x5030ee, delivery(0x5030ee, limit)),
        ])
    }
    fn decode_with(
        windows: &BTreeMap<u32, Vec<u8>>,
        origin: AssetIdentity,
        digest: Sha256Digest,
    ) -> Result<Rules, String> {
        Rules::decode(origin, digest, |va, n| {
            windows
                .get(&va)
                .filter(|v| v.len() == n)
                .cloned()
                .ok_or_else(|| "missing synthetic window".into())
        })
    }
    pub(crate) fn rules(binding: Binding, limit: u8) -> Rules {
        decode_with(&windows(limit), binding.origin, binding.image_sha256).unwrap()
    }
    fn origin() -> AssetIdentity {
        AssetIdentity {
            source: "synthetic:group-order-image".into(),
            edition: "synthetic".into(),
            version: "test".into(),
        }
    }
    #[test]
    fn group_order_limit_is_decoded_and_preparation_relationships_are_checked() {
        for limit in [1, 3, 19] {
            assert_eq!(
                decode_with(&windows(limit), origin(), [0; 32])
                    .unwrap()
                    .point_member_limit(),
                usize::from(limit)
            );
        }
        for at in [0x502da2, 0x5030c2, 0x5030ee] {
            let mut data = windows(3);
            data.get_mut(&at).unwrap()[2] = 4;
            assert!(decode_with(&data, origin(), [0; 32]).is_err());
        }
        let mut data = windows(3);
        data.get_mut(&0x502dd0).unwrap()[2] = 3;
        assert!(decode_with(&data, origin(), [0; 32]).is_err());
        let mut data = windows(3);
        data.get_mut(&0x5030ee).unwrap()[9] ^= 1;
        assert!(decode_with(&data, origin(), [0; 32]).is_err());
        let mut data = windows(3);
        data.get_mut(&0x5030c2).unwrap()[9] ^= 1;
        assert!(decode_with(&data, origin(), [0; 32]).is_err());
        assert!(decode_with(&windows(0), origin(), [0; 32]).is_err());
        assert!(decode_with(&windows(255), origin(), [0; 32]).is_err());
    }
    #[test]
    fn bounded_push_requires_authored_intervening_instructions_at_their_offset() {
        let moves = POINT_PREPARE_MOVES;
        let mut point = vec![0x50, 0x6a, 3];
        point.extend_from_slice(&moves);
        point.extend_from_slice(&[0x68, 1, 2, 3, 4]);
        let pushed = bounded_push(&point, &moves).unwrap();
        assert_eq!((pushed.limit, pushed.callback), (3, [1, 2, 3, 4]));
        let mut changed = point.clone();
        changed[4] ^= 1;
        assert!(bounded_push(&changed, &moves).is_err());
        // A callback-looking byte later in the window is not a search hit.
        let mut shifted = vec![
            0x50, 0x6a, 3, 0x90, 0x90, 0x90, 0x90, 0x90, 0x68, 1, 2, 3, 4,
        ];
        assert!(bounded_push(&shifted, &[]).is_err());
        shifted.truncate(7);
        assert!(bounded_push(&shifted, &[]).is_err());
        assert!(bounded_push(&[0x50, 0x6a, 3, 0x68, 1, 2, 3], &[]).is_err());
        assert!(bounded_push(&[0x51, 0x6a, 3, 0x68, 1, 2, 3, 4], &[]).is_err());
        assert!(bounded_push(&[0x50, 0x6a, 0, 0x68, 1, 2, 3, 4], &[]).is_err());
        assert!(bounded_push(&[0x50, 0x6a, 128, 0x68, 1, 2, 3, 4], &[]).is_err());
    }
    #[test]
    fn point_preparation_intervening_moves_and_callback_are_checked_in_decode() {
        let mut data = windows(3);
        data.get_mut(&0x5030c2).unwrap()[3] = 0x88;
        assert!(decode_with(&data, origin(), [0; 32]).is_err());
        let mut data = windows(3);
        data.get_mut(&0x5030c2).unwrap().truncate(12);
        assert!(decode_with(&data, origin(), [0; 32]).is_err());
        let mut data = windows(3);
        data.get_mut(&0x5030c2).unwrap()[9] ^= 1;
        assert!(decode_with(&data, origin(), [0; 32]).is_err());
    }
    #[test]
    #[ignore = "requires locally supplied selected Warcraft III.exe"]
    fn supplied_group_order_image_qualifies_native_bounds() {
        let path = std::env::var("PCX_GROUP_ORDER_IMAGE")
            .expect("set PCX_GROUP_ORDER_IMAGE to the selected local executable");
        let image = std::fs::read(&path).unwrap();
        let rules = Rules::reviewed_legacy(
            &image,
            AssetIdentity {
                source: path,
                edition: "selected Legacy executable".into(),
                version: "VERSIONINFO 1.29.2.9231".into(),
            },
        )
        .unwrap();
        assert!(rules.point_member_limit() > 0);
        assert_eq!(rules.image_sha256, sha256(&image));
    }
}
