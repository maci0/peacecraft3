//! In-game minimap anchors and image lookup order from the reviewed client.
//! This does not qualify world-to-map projection, fog, markers or input.
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
    /// Logical frame coordinates, positive Y upwards.
    pub viewport: [f32; 2],
    pub frame: Rect,
    /// Map members in the texture loader's source-defined attempt order.
    pub image_members: Vec<String>,
    /// RGBA from the minimap texture updater's packed clear pixel.
    pub clear_color: [u8; 4],
}

impl Rules {
    pub fn reviewed_legacy(image: &[u8], origin: AssetIdentity) -> Result<Self, String> {
        let fail = |reason: String| format!("minimap layout {}: {reason}", origin.source);
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
        fn operand(
            read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
            va: u32,
            prefix: &[u8],
            len: usize,
        ) -> Result<Vec<u8>, String> {
            let bytes = read(va, prefix.len() + len, true)?;
            if bytes.len() != prefix.len() + len || !bytes.starts_with(prefix) {
                return Err(format!("unsupported minimap instruction at {va:#x}"));
            }
            Ok(bytes[prefix.len()..].to_vec())
        }
        fn word(bytes: Vec<u8>) -> Result<u32, String> {
            Ok(u32::from_le_bytes(
                bytes.try_into().map_err(|_| "truncated minimap word")?,
            ))
        }
        fn number(bytes: Vec<u8>) -> Result<f32, String> {
            let n = f32::from_bits(word(bytes)?);
            if !n.is_finite() {
                return Err("nonfinite minimap geometry".into());
            }
            Ok(n)
        }
        fn string(
            read: &mut impl FnMut(u32, usize, bool) -> Result<Vec<u8>, String>,
            start: u32,
        ) -> Result<String, String> {
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
        for (va, target) in [
            (0x5e7d93u32, 0x876630u32),
            (0x5e7dc9, 0x873750),
            (0x5e7e7e, 0x61f930),
            (0x5e7ec4, 0x873a10),
            (0x5e7f00, 0x873a10),
            (0x637688, 0x71ba80),
            (0x71bb31, 0x9727f0),
        ] {
            if word(operand(&mut read, va, &[0xe8], 4)?)? != target.wrapping_sub(va + 5) {
                return Err(format!("minimap call target at {va:#x}"));
            }
        }
        let vtable = word(operand(&mut read, 0x61f968, &[0xc7, 0x07], 4)?)?;
        let method = vtable.checked_add(0xec).ok_or("minimap vtable overflow")?;
        if word(read(method, 4, false)?)? != 0x636db0 {
            return Err("minimap image method belongs to another class".into());
        }
        // The intermediate frame takes all points of GameUI, independently of ConsoleUI art.
        let parent = operand(&mut read, 0x5e7d9d, &[0x89, 0x8f], 4)?;
        for (va, prefix) in [
            (0x5e7dbd, &[0x8b, 0x8f][..]),
            (0x5e7e89, &[0x8b, 0x8f][..]),
            (0x5e7ec9, &[0x8b, 0x8f][..]),
        ] {
            if operand(&mut read, va, prefix, 4)? != parent {
                return Err("minimap parent field relation".into());
            }
        }
        operand(&mut read, 0x5e7db0, &[0x8d, 0x87, 0xbc, 0, 0, 0], 0)?;
        operand(&mut read, 0x5e7db6, &[0x6a, 1], 0)?;
        operand(&mut read, 0x5e7dbc, &[0x51], 0)?;
        operand(&mut read, 0x5e7dc3, &[0x81, 0xc1, 0xbc, 0, 0, 0], 0)?;
        let field = operand(&mut read, 0x5e7e98, &[0x89, 0x97], 4)?;
        if operand(&mut read, 0x5e7ef2, &[0x8b, 0x8f], 4)? != field {
            return Err("minimap corner receiver relation".into());
        }
        for (va, bytes) in [
            (0x5e7e8f, &[0x6a, 1, 0x83, 0xec, 8][..]),
            (0x5e7eb9, &[0x6a, 6, 0x51, 0x6a, 6][..]),
            (0x5e7ebe, &[0x8d, 0x8a, 0xbc, 0, 0, 0][..]),
            (0x5e7ecf, &[0x6a, 1, 0x83, 0xec, 8][..]),
            (0x5e7eef, &[0x6a, 6, 0x51][..]),
            (0x5e7ef8, &[0x6a, 2, 0x81, 0xc1, 0xbc, 0, 0, 0][..]),
        ] {
            operand(&mut read, va, bytes, 0)?;
        }
        let x = number(operand(&mut read, 0x5e7eb0, &[0xc7, 4, 0x24], 4)?)?;
        let y = number(operand(&mut read, 0x5e7ea6, &[0xc7, 0x44, 0x24, 4], 4)?)?;
        let right = number(operand(&mut read, 0x5e7ee6, &[0xc7, 4, 0x24], 4)?)?;
        let top = number(operand(&mut read, 0x5e7edc, &[0xc7, 0x44, 0x24, 4], 4)?)?;
        let mut viewport = [0.; 2];
        for (index, va, prefix) in [
            (0, 0x640ea8, &[0xf3, 0x0f, 0x10, 5][..]),
            (1, 0x640e96, &[0xf3, 0x0f, 0x10, 0x1d][..]),
        ] {
            let pointer = word(operand(&mut read, va, prefix, 4)?)?;
            viewport[index] = number(read(pointer, 4, false)?)?;
        }
        if x < 0. || y < 0. || right <= x || top <= y || right > viewport[0] || top > viewport[1] {
            return Err("minimap corners exceed logical viewport".into());
        }
        let name = word(operand(&mut read, 0x637683, &[0x68], 4)?)?;
        let name = string(&mut read, name)?;
        let (stem, _) = name
            .rsplit_once('.')
            .ok_or("minimap image has no extension")?;
        if stem.is_empty() || stem.contains(['/', '\\', '.']) {
            return Err("unsupported minimap image member".into());
        }
        let start = word(operand(&mut read, 0x71bb0c, &[0xbe], 4)?)?;
        let end = word(operand(&mut read, 0x71bb40, &[0x81, 0xfe], 4)?)?;
        let stride = operand(&mut read, 0x71bb3d, &[0x83, 0xc6], 1)?[0];
        let size = end
            .checked_sub(start)
            .ok_or("minimap extension table order")?;
        if stride == 0
            || stride & 0x80 != 0
            || size == 0
            || size > 256
            || size % u32::from(stride) != 0
        {
            return Err("unsupported minimap extension table extent".into());
        }
        operand(&mut read, 0x71bb39, &[0x85, 0xc0, 0x75, 0x0b], 0)?;
        operand(&mut read, 0x71bb46, &[0x7c, 0xc9], 0)?;
        let mut image_members = Vec::new();
        for offset in (0..size).step_by(usize::from(stride)) {
            let ext = read(start + offset, usize::from(stride), false)?;
            let [b'.', a, b, c, 0] = ext.as_slice() else {
                return Err("unsupported minimap extension record".into());
            };
            let extension = std::str::from_utf8(&[*a, *b, *c])
                .map_err(|e| e.to_string())?
                .to_owned();
            if !matches!(extension.as_str(), "tga" | "blp") {
                return Err("unsupported minimap image encoding".into());
            }
            let member = format!("{stem}.{extension}");
            if image_members.contains(&member) {
                return Err("duplicate minimap image attempt".into());
            }
            image_members.push(member);
        }
        let clear = operand(&mut read, 0x658bae, &[0xc7, 0x45, 0xe8], 4)?;
        if operand(&mut read, 0x658bb5, &[0xc7, 0x45, 0xec], 4)? != clear {
            return Err("minimap paired clear pixels differ".into());
        }
        let [blue, green, red, alpha]: [u8; 4] =
            clear.try_into().map_err(|_| "minimap clear pixel")?;
        Ok(Self {
            origin,
            image_sha256,
            viewport,
            frame: Rect {
                x,
                y,
                width: right - x,
                height: top - y,
            },
            image_members,
            clear_color: [red, green, blue, alpha],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn origin() -> AssetIdentity {
        AssetIdentity {
            source: "synthetic minimap image".into(),
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
    fn fixture() -> BTreeMap<u32, u8> {
        let mut d = BTreeMap::new();
        for (va, target) in [
            (0x5e7d93u32, 0x876630u32),
            (0x5e7dc9, 0x873750),
            (0x5e7e7e, 0x61f930),
            (0x5e7ec4, 0x873a10),
            (0x5e7f00, 0x873a10),
            (0x637688, 0x71ba80),
            (0x71bb31, 0x9727f0),
        ] {
            op(&mut d, va, &[0xe8], target.wrapping_sub(va + 5));
        }
        op(&mut d, 0x61f968, &[0xc7, 7], 0x1000);
        put(&mut d, 0x10ec, &0x636db0u32.to_le_bytes());
        op(&mut d, 0x5e7d9d, &[0x89, 0x8f], 0x450);
        for va in [0x5e7dbd, 0x5e7e89, 0x5e7ec9] {
            op(&mut d, va, &[0x8b, 0x8f], 0x450);
        }
        op(&mut d, 0x5e7e98, &[0x89, 0x97], 0x410);
        op(&mut d, 0x5e7ef2, &[0x8b, 0x8f], 0x410);
        for (va, bytes) in [
            (0x5e7db0, &[0x8d, 0x87, 0xbc, 0, 0, 0][..]),
            (0x5e7db6, &[0x6a, 1][..]),
            (0x5e7dbc, &[0x51][..]),
            (0x5e7dc3, &[0x81, 0xc1, 0xbc, 0, 0, 0][..]),
            (0x5e7e8f, &[0x6a, 1, 0x83, 0xec, 8][..]),
            (0x5e7eb9, &[0x6a, 6, 0x51, 0x6a, 6][..]),
            (0x5e7ebe, &[0x8d, 0x8a, 0xbc, 0, 0, 0][..]),
            (0x5e7ecf, &[0x6a, 1, 0x83, 0xec, 8][..]),
            (0x5e7eef, &[0x6a, 6, 0x51][..]),
            (0x5e7ef8, &[0x6a, 2, 0x81, 0xc1, 0xbc, 0, 0, 0][..]),
            (0x71bb39, &[0x85, 0xc0, 0x75, 0x0b][..]),
            (0x71bb46, &[0x7c, 0xc9][..]),
        ] {
            put(&mut d, va, bytes);
        }
        for (va, prefix, n) in [
            (0x5e7eb0, &[0xc7, 4, 0x24][..], 0.1f32),
            (0x5e7ea6, &[0xc7, 0x44, 0x24, 4][..], 0.2),
            (0x5e7ee6, &[0xc7, 4, 0x24][..], 0.4),
            (0x5e7edc, &[0xc7, 0x44, 0x24, 4][..], 0.5),
        ] {
            op(&mut d, va, prefix, n.to_bits());
        }
        op(&mut d, 0x640ea8, &[0xf3, 0x0f, 0x10, 5], 0x2000);
        op(&mut d, 0x640e96, &[0xf3, 0x0f, 0x10, 0x1d], 0x2004);
        put(&mut d, 0x2000, &1.2f32.to_le_bytes());
        put(&mut d, 0x2004, &0.9f32.to_le_bytes());
        op(&mut d, 0x637683, &[0x68], 0x3000);
        put(&mut d, 0x3000, b"fixtureMap.blp\0");
        op(&mut d, 0x71bb0c, &[0xbe], 0x4000);
        op(&mut d, 0x71bb40, &[0x81, 0xfe], 0x400a);
        put(&mut d, 0x71bb3d, &[0x83, 0xc6, 5]);
        put(&mut d, 0x4000, b".blp\0.tga\0");
        op(&mut d, 0x658bae, &[0xc7, 0x45, 0xe8], 0x6d53412b);
        op(&mut d, 0x658bb5, &[0xc7, 0x45, 0xec], 0x6d53412b);
        d
    }
    fn decode(d: &BTreeMap<u32, u8>) -> Result<Rules, String> {
        Rules::decode(origin(), sha256(b"fixture"), |va, n, _| {
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
    fn geometry_and_lookup_order_are_operand_driven() {
        let mut d = fixture();
        let r = decode(&d).unwrap();
        assert_eq!(r.viewport, [1.2, 0.9]);
        assert_eq!(r.frame.x, 0.1);
        assert_eq!(r.frame.y, 0.2);
        assert!((r.frame.width - 0.3).abs() < 0.000001);
        assert!((r.frame.height - 0.3).abs() < 0.000001);
        assert_eq!(r.image_members, ["fixtureMap.blp", "fixtureMap.tga"]);
        assert_eq!(r.clear_color, [83, 65, 43, 109]);
        op(&mut d, 0x5e7ee6, &[0xc7, 4, 0x24], 0.8f32.to_bits());
        put(&mut d, 0x4000, b".tga\0.blp\0");
        let other = decode(&d).unwrap();
        assert!((other.frame.width - 0.7).abs() < 0.000001);
        assert_eq!(other.image_members, ["fixtureMap.tga", "fixtureMap.blp"]);
    }
    #[test]
    fn malformed_geometry_relationships_and_loader_paths_refuse() {
        let base = fixture();
        for (va, bytes) in [
            (0x5e7ec4, vec![0xe9]),
            (0x5e7dbe, vec![0]),
            (0x5e7ef4, vec![0]),
            (0x5e7ef9, vec![8]),
            (0x10ec, vec![0]),
            (0x71bb3f, vec![0]),
            (0x71bb47, vec![0]),
            (0x71bb3c, vec![0]),
            (0x658bae, vec![0x90]),
            (0x658bb8, vec![0]),
            (0x4004, vec![b'x']),
            (0x4000, b".png\0".to_vec()),
            (0x4005, b".blp\0".to_vec()),
            (0x5e7ee9, 0.05f32.to_le_bytes().to_vec()),
            (0x5e7ee9, f32::NAN.to_le_bytes().to_vec()),
            (0x2004, 0.1f32.to_le_bytes().to_vec()),
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
        assert!(Rules::reviewed_legacy(b"not a client", origin()).is_err());
    }
    #[test]
    #[ignore = "requires PCX_PLAYER_RESOURCE_IMAGE selecting the reviewed local client"]
    fn supplied_client_minimap_operands_are_admitted() {
        let path = std::env::var("PCX_PLAYER_RESOURCE_IMAGE").unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let r = Rules::reviewed_legacy(
            &bytes,
            AssetIdentity {
                source: path,
                edition: "explicit selected local client".into(),
                version: "reviewed client".into(),
            },
        )
        .unwrap();
        assert_eq!(r.image_sha256, REVIEWED_IMAGE);
        assert_eq!(r.image_members, ["war3mapMap.tga", "war3mapMap.blp"]);
        assert!(r.frame.width > 0. && r.frame.height > 0.);
    }
}
