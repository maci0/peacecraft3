//! In-game minimap anchors and image lookup order from the reviewed client.
//! The mapping rectangle expands executed camera bounds by the selected profile margins.
//! Visibility-grid sampling, fog filtering and marker drawing remain separate.
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
    /// D3DTEXF min/mag/mip and D3DTADDRESS U/V from dynamic texture creation.
    pub texture_filters: [u32; 3],
    pub texture_address: [u32; 2],
    mapping_section: u32,
    mapping_keys: [u32; 4],
    mapping_scale: f32,
    mapping_corners: [usize; 4],
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

    /// The native map rectangle uses camera bounds plus margins, not the entire terrain.
    pub fn world_bounds(
        &self,
        camera: &crate::playercolor_host::CameraState,
    ) -> Result<[[f32; 2]; 2], String> {
        let binding = camera.margins.binding();
        if binding.image_sha256 != self.image_sha256
            || binding.section.va != self.mapping_section
            || binding.scale.digest != sha256(&self.mapping_scale.to_le_bytes())
        {
            return Err("minimap camera-margin source differs from the selected client".into());
        }
        let mut margins = [0.; 4];
        for (out, key) in margins.iter_mut().zip(self.mapping_keys) {
            let index = binding
                .keys
                .iter()
                .position(|window| window.va == key)
                .ok_or("minimap camera-margin key is not admitted")?;
            *out = camera.margins.values()[index];
        }
        expanded_bounds(
            camera.bounds.ok_or("minimap camera bounds are unset")?,
            margins,
            self.mapping_corners,
        )
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
        // Join profile references to their arithmetic destinations in the map rectangle.
        let mut mapping_keys = [0; 4];
        let mut mapping_section = None;
        for (index, key_va, section_va, call_va, arithmetic_va, arithmetic, store_va, store) in [
            (
                0,
                0x6370c7,
                0x6370cc,
                0x6370d4,
                0x6370f9,
                &[0xf3, 0x0f, 0x5c, 0xc1][..],
                0x6370fd,
                &[0xf3, 0x0f, 0x11, 0x87, 0x78, 1, 0, 0][..],
            ),
            (
                1,
                0x6370e3,
                0x6370ef,
                0x637105,
                0x637122,
                &[0xf3, 0x0f, 0x58, 0x87, 0x80, 1, 0, 0][..],
                0x63712a,
                &[0xf3, 0x0f, 0x11, 0x87, 0x80, 1, 0, 0][..],
            ),
            (
                2,
                0x637141,
                0x63714d,
                0x637163,
                0x637189,
                &[0xf3, 0x0f, 0x58, 0x87, 0x7c, 1, 0, 0][..],
                0x637191,
                &[0xf3, 0x0f, 0x11, 0x87, 0x7c, 1, 0, 0][..],
            ),
            (
                3,
                0x63710c,
                0x637111,
                0x637132,
                0x637157,
                &[0xf3, 0x0f, 0x5c, 0xc1][..],
                0x63715b,
                &[0xf3, 0x0f, 0x11, 0x87, 0x74, 1, 0, 0][..],
            ),
        ] {
            mapping_keys[index] = word(operand(&mut read, key_va, &[0x68], 4)?)?;
            let section = word(operand(&mut read, section_va, &[0x68], 4)?)?;
            if mapping_section
                .replace(section)
                .is_some_and(|old| old != section)
            {
                return Err("minimap camera-margin sections differ".into());
            }
            if word(operand(&mut read, call_va, &[0xe8], 4)?)?
                != 0x4304c0u32.wrapping_sub(call_va + 5)
            {
                return Err("minimap camera-margin reader differs".into());
            }
            operand(&mut read, arithmetic_va, arithmetic, 0)?;
            operand(&mut read, store_va, store, 0)?;
        }
        // Getter outputs a,b,c,d are stored camera pairs 0,2,3,1 in this image.
        let mut fields = [0u32; 4];
        for (field, va) in fields
            .iter_mut()
            .zip([0x5e070c, 0x5e0731, 0x5e0745, 0x5e071d])
        {
            *field = word(operand(&mut read, va, &[0x8b, 0x86], 4)?)?;
        }
        let base = *fields.iter().min().ok_or("missing minimap camera fields")?;
        let mut mapping_corners = [0; 4];
        for (corner, field) in mapping_corners.iter_mut().zip(fields) {
            let offset = field - base;
            if offset % 8 != 0 || offset / 8 >= 4 {
                return Err("unsupported minimap camera fields".into());
            }
            *corner = offset as usize / 8;
        }
        for (index, corner) in mapping_corners.iter().enumerate() {
            if mapping_corners[..index].contains(corner) {
                return Err("duplicate minimap camera fields".into());
            }
        }
        let scale_global = word(operand(&mut read, 0x63706c, &[0x68], 4)?)?;
        if word(operand(&mut read, 0x40a865, &[0x68], 4)?)? != scale_global {
            return Err("minimap camera-margin scale initializer differs".into());
        }
        for (va, target) in [
            (0x40a86au32, 0x9a1620u32),
            (0x6370c0, 0x65d3d0),
            (0x636f9c, 0x5e0700),
        ] {
            if word(operand(&mut read, va, &[0xe8], 4)?)? != target.wrapping_sub(va + 5) {
                return Err("minimap camera input call differs".into());
            }
        }
        let mapping_scale = word(operand(&mut read, 0x40a860, &[0x68], 4)?)? as i32 as f32;
        if mapping_scale <= 0. {
            return Err("invalid minimap camera-margin scale".into());
        }
        // Dynamic minimap textures use their own flag constructor, not the file-image loader.
        for (va, target) in [(0x637881u32, 0x61dfb0u32), (0x6378a8, 0x7d11f0)] {
            if word(operand(&mut read, va, &[0xe8], 4)?)? != target.wrapping_sub(va + 5) {
                return Err("minimap dynamic texture call differs".into());
            }
        }
        operand(&mut read, 0x63788d, &[0xff, 0x30], 0)?;
        for va in [0x63786e, 0x637870, 0x637872, 0x637874] {
            operand(&mut read, va, &[0x6a, 0], 0)?;
        }
        let flags = operand(&mut read, 0x637876, &[0x6a], 1)?[0];
        let low_mask = operand(&mut read, 0x61dfe4, &[0x83, 0xe0], 1)?[0];
        let selector = operand(&mut read, 0x7def4a, &[0x83, 0xe0], 1)?[0];
        operand(&mut read, 0x7def4d, &[0x8d, 0x34, 0x40], 0)?;
        if flags & 0x80 != 0 || low_mask >> 2 != 0 || selector >> 2 != 0 {
            return Err("unsupported minimap sampler selector".into());
        }
        let texture_filters = crate::command_layout::texture_filter_record(
            &mut crate::unit_panel_layout::Reader(&mut read),
            u32::from(flags & low_mask & selector),
        )?;
        let mut texture_address = [0; 2];
        for (out, (shift_va, shift, mask_va, table_va)) in texture_address.iter_mut().zip([
            (0x7def8f, 2, 0x7def92, 0x7def95),
            (0x7defab, 3, 0x7defae, 0x7defb1),
        ]) {
            operand(&mut read, shift_va, &[0xc1, 0xe8, shift], 0)?;
            operand(&mut read, mask_va, &[0x83, 0xe0, 1], 0)?;
            let table = word(operand(&mut read, table_va, &[0xff, 0x34, 0x85], 4)?)?;
            *out = word(read(table, 4, false)?)?;
        }
        if texture_address.iter().any(|value| !matches!(value, 1 | 3)) {
            return Err("unsupported minimap texture address mode".into());
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
            texture_filters,
            texture_address,
            mapping_section: mapping_section.ok_or("missing minimap camera-margin section")?,
            mapping_keys,
            mapping_scale,
            mapping_corners,
        })
    }
}

fn expanded_bounds(
    bounds: [f32; 8],
    [left, right, top, bottom]: [f32; 4],
    indices: [usize; 4],
) -> Result<[[f32; 2]; 2], String> {
    let corners = bounds.as_chunks::<2>().0;
    let [a, b, c, d] = indices.map(|index| corners[index]);
    let low_x = a[0].min(b[0]) - left;
    let high_x = c[0].max(d[0]) + right;
    let low_y = a[1].min(c[1]) - bottom;
    let high_y = b[1].max(d[1]) + top;
    let expanded = [[low_x, low_y], [high_x, high_y]];
    if bounds
        .iter()
        .chain([left, right, top, bottom].iter())
        .any(|v| !v.is_finite())
        || expanded.iter().flatten().any(|v| !v.is_finite())
        || low_x >= high_x
        || low_y >= high_y
    {
        return Err("invalid minimap camera rectangle".into());
    }
    Ok(expanded)
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
        for (index, key_va, section_va, call_va, arithmetic_va, arithmetic, store_va, store) in [
            (
                0,
                0x6370c7,
                0x6370cc,
                0x6370d4,
                0x6370f9,
                &[0xf3, 0x0f, 0x5c, 0xc1][..],
                0x6370fd,
                &[0xf3, 0x0f, 0x11, 0x87, 0x78, 1, 0, 0][..],
            ),
            (
                1,
                0x6370e3,
                0x6370ef,
                0x637105,
                0x637122,
                &[0xf3, 0x0f, 0x58, 0x87, 0x80, 1, 0, 0][..],
                0x63712a,
                &[0xf3, 0x0f, 0x11, 0x87, 0x80, 1, 0, 0][..],
            ),
            (
                2,
                0x637141,
                0x63714d,
                0x637163,
                0x637189,
                &[0xf3, 0x0f, 0x58, 0x87, 0x7c, 1, 0, 0][..],
                0x637191,
                &[0xf3, 0x0f, 0x11, 0x87, 0x7c, 1, 0, 0][..],
            ),
            (
                3,
                0x63710c,
                0x637111,
                0x637132,
                0x637157,
                &[0xf3, 0x0f, 0x5c, 0xc1][..],
                0x63715b,
                &[0xf3, 0x0f, 0x11, 0x87, 0x74, 1, 0, 0][..],
            ),
        ] {
            op(&mut d, key_va, &[0x68], 0x5000 + index as u32 * 16);
            op(&mut d, section_va, &[0x68], 0x5100);
            op(
                &mut d,
                call_va,
                &[0xe8],
                0x4304c0u32.wrapping_sub(call_va + 5),
            );
            put(&mut d, arithmetic_va, arithmetic);
            put(&mut d, store_va, store);
        }
        for (va, field) in [
            (0x5e070c, 0x100),
            (0x5e0731, 0x110),
            (0x5e0745, 0x118),
            (0x5e071d, 0x108),
        ] {
            op(&mut d, va, &[0x8b, 0x86], field);
        }
        op(&mut d, 0x63706c, &[0x68], 0x5200);
        op(&mut d, 0x40a865, &[0x68], 0x5200);
        op(&mut d, 0x40a860, &[0x68], 64);
        for (va, target) in [
            (0x40a86au32, 0x9a1620u32),
            (0x6370c0, 0x65d3d0),
            (0x636f9c, 0x5e0700),
        ] {
            op(&mut d, va, &[0xe8], target.wrapping_sub(va + 5));
        }
        for (va, target) in [(0x637881u32, 0x61dfb0u32), (0x6378a8, 0x7d11f0)] {
            op(&mut d, va, &[0xe8], target.wrapping_sub(va + 5));
        }
        put(&mut d, 0x63788d, &[0xff, 0x30]);
        for va in [0x63786e, 0x637870, 0x637872, 0x637874, 0x637876] {
            put(&mut d, va, &[0x6a, 0]);
        }
        put(&mut d, 0x61dfe4, &[0x83, 0xe0, 3]);
        put(&mut d, 0x7def4a, &[0x83, 0xe0, 3]);
        put(&mut d, 0x7def4d, &[0x8d, 0x34, 0x40]);
        for (site, table) in [(0x7def50, 0x6000), (0x7def61, 0x6004), (0x7def74, 0x6008)] {
            op(&mut d, site, &[0xff, 0x34, 0xb5], table);
        }
        put(&mut d, 0x6000, &1u32.to_le_bytes());
        put(&mut d, 0x6004, &2u32.to_le_bytes());
        put(&mut d, 0x6008, &0u32.to_le_bytes());
        for (shift_va, shift, mask_va, table_va) in [
            (0x7def8f, 2, 0x7def92, 0x7def95),
            (0x7defab, 3, 0x7defae, 0x7defb1),
        ] {
            put(&mut d, shift_va, &[0xc1, 0xe8, shift]);
            put(&mut d, mask_va, &[0x83, 0xe0, 1]);
            op(&mut d, table_va, &[0xff, 0x34, 0x85], 0x6100);
        }
        put(&mut d, 0x6100, &3u32.to_le_bytes());
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
        assert_eq!(r.mapping_keys, [0x5000, 0x5010, 0x5020, 0x5030]);
        assert_eq!(r.mapping_section, 0x5100);
        assert_eq!(r.mapping_scale, 64.);
        assert_eq!(r.mapping_corners, [0, 2, 3, 1]);
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
    fn dynamic_sampler_follows_its_selector_tables_and_refuses_unsupported_modes() {
        let mut d = fixture();
        assert_eq!(decode(&d).unwrap().texture_filters, [1, 2, 0]);
        assert_eq!(decode(&d).unwrap().texture_address, [3, 3]);
        put(&mut d, 0x637877, &[1]);
        for (va, value) in [(0x600c, 2u32), (0x6010, 1), (0x6014, 0)] {
            put(&mut d, va, &value.to_le_bytes());
        }
        put(&mut d, 0x6100, &1u32.to_le_bytes());
        assert_eq!(decode(&d).unwrap().texture_filters, [2, 1, 0]);
        assert_eq!(decode(&d).unwrap().texture_address, [1, 1]);
        put(&mut d, 0x6014, &1u32.to_le_bytes());
        assert!(decode(&d).is_err());
        put(&mut d, 0x6014, &0u32.to_le_bytes());
        put(&mut d, 0x6100, &2u32.to_le_bytes());
        assert!(decode(&d).is_err());
    }
    #[test]
    fn malformed_geometry_relationships_and_loader_paths_refuse() {
        let base = fixture();
        for (va, bytes) in [
            (0x637881, vec![0x90]),
            (0x63786e, vec![0x90]),
            (0x637877, vec![0xff]),
            (0x7def4c, vec![0xff]),
            (0x7def4d, vec![0x90]),
            (0x7def8f, vec![0x90]),
            (0x6370f9, vec![0x90]),
            (0x6370fd, vec![0x90]),
            (0x6370d4, vec![0x90]),
            (0x6370ce, vec![0]),
            (0x40a866, vec![1]),
            (0x40a861, vec![0, 0, 0, 0]),
            (0x636f9c, vec![0x90]),
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
    fn camera_rectangle_uses_native_corner_pairs_and_refuses_invalid_inputs() {
        let bounds = [-10., -20., 30., 40., -10., 40., 30., -20.];
        assert_eq!(
            expanded_bounds(bounds, [1., 2., 3., 4.], [0, 2, 3, 1]).unwrap(),
            [[-11., -24.], [32., 43.]]
        );
        assert!(expanded_bounds(bounds, [f32::NAN, 2., 3., 4.], [0, 2, 3, 1]).is_err());
        assert!(expanded_bounds(bounds, [-100., 2., 3., 4.], [0, 2, 3, 1]).is_err());
        let mut invalid = bounds;
        invalid[0] = f32::INFINITY;
        assert!(expanded_bounds(invalid, [1., 2., 3., 4.], [0, 2, 3, 1]).is_err());
        assert_eq!(
            expanded_bounds(
                [100., 0., 10., 10., 0., 10., 10., 0.],
                [0.; 4],
                [0, 2, 3, 1]
            )
            .unwrap(),
            [[0., 0.], [10., 10.]]
        );
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
