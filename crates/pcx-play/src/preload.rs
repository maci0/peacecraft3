//! Source-bound preload entry and bounded asset warming on the retained resolver.
use crate::{neutral_player::sha256, pe_image::PeSections};
use pcx_assets::{AssetPath, Resolver};
use std::{collections::VecDeque, sync::Arc};

#[derive(Clone, Debug)]
pub struct Rules {
    pub image_sha256: [u8; 32],
    common: String,
    entry: String,
    extensions: [String; 2],
    path_bytes: usize,
    refresh_assets: usize,
    start_timeout: f32,
}
impl Rules {
    pub fn read(image: &[u8]) -> Result<Self, String> {
        if image.len() > 32 << 20 || sha256(image) != crate::menu_lighting::REVIEWED_IMAGE {
            return Err("unsupported preload client".into());
        }
        let pe = PeSections::parse(image)?;
        Self::decode(sha256(image), |va, n| {
            pe.va_bytes(va, n)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| format!("preload image {va:#x}"))
        })
    }
    fn decode(
        image_sha256: [u8; 32],
        read: impl Fn(u32, usize) -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        let exact = |va, n| {
            let bytes = read(va, n)?;
            if bytes.len() != n {
                return Err(format!("truncated preload window {va:#x}"));
            }
            Ok(bytes)
        };
        let pointer = |va, opcode| -> Result<u32, String> {
            let code = exact(va, 5)?;
            if code[0] != opcode {
                return Err("preload source pointer opcode".into());
            }
            Ok(u32::from_le_bytes(code[1..].try_into().unwrap()))
        };
        let string = |va| {
            let bytes = exact(va, 256)?;
            let end = bytes
                .iter()
                .position(|&v| v == 0)
                .filter(|&v| v > 0)
                .ok_or("preload string termination")?;
            std::str::from_utf8(&bytes[..end])
                .map(str::to_owned)
                .map_err(|e| e.to_string())
        };
        let common = string(pointer(0x4a46f1, 0x68)?)?;
        AssetPath::new(&common).map_err(|e| e.to_string())?;
        let entry = string(pointer(0x4a47a8, 0x68)?)?;
        if !entry
            .bytes()
            .all(|v| v.is_ascii_alphanumeric() || v == b'_')
        {
            return Err("preload entry identifier".into());
        }
        let extensions = [
            string(pointer(0x4a4615, 0xba)?)?,
            string(pointer(0x4a464a, 0xb8)?)?,
        ];
        if extensions[0] == extensions[1]
            || extensions.iter().any(|s| {
                s.len() != 4
                    || !s.starts_with('.')
                    || !s[1..].bytes().all(|v| v.is_ascii_alphanumeric())
            })
        {
            return Err("preload extension predicate".into());
        }
        let path_bytes = pointer(0x4a45fb, 0x68)? as usize;
        if !(4..=16 << 10).contains(&path_bytes) {
            return Err("preload path ceiling".into());
        }
        let refresh = exact(0x4a4580, 15)?;
        let call = |code: &[u8], at: usize, base: u32, target: u32| {
            code.get(at) == Some(&0xe8)
                && code.get(at + 1..at + 5).is_some_and(|d| {
                    i64::from(base)
                        + at as i64
                        + 5
                        + i64::from(i32::from_le_bytes(d.try_into().unwrap()))
                        == i64::from(target)
                })
        };
        if refresh[0] != 0x6a
            || !call(&refresh, 2, 0x4a4580, 0x4fd040)
            || refresh[7..10] != [0x8b, 0xc8, 0xe8]
            || !call(&refresh, 9, 0x4a4580, 0x4fd430)
            || refresh[14] != 0xc3
            || refresh[1] & 0x80 != 0
        {
            return Err("preload refresh branch".into());
        }
        let bound = exact(0x4fd516, 12)?;
        let increment = exact(0x4fd67f, 3)?;
        if bound[..5] != [0x8b, 0x4d, 8, 0x39, 0x4d]
            || bound[6..8] != [0x0f, 0x87]
            || increment != [0xff, 0x45, bound[5]]
            || i64::from(0x4fd516)
                + 12
                + i64::from(i32::from_le_bytes(bound[8..12].try_into().unwrap()))
                != i64::from(0x4fd6a5)
        {
            return Err("preload refresh inclusive work bound".into());
        }
        let start = exact(0x4a459e, 12)?;
        if !call(&start, 0, 0x4a459e, 0x4fd6d0) || start[5..7] != [0xc7, 0x46] || start[7] != 0x10 {
            return Err("preload start timeout".into());
        }
        let timeout = u32::from_le_bytes(start[8..12].try_into().unwrap());
        let scale = exact(0x4a4459, 12)?;
        if scale[..8] != [0xf3, 0x0f, 0x10, 0x01, 0xf3, 0x0f, 0x59, 0x05] {
            return Err("preload timeout conversion".into());
        }
        let address = u32::from_le_bytes(scale[8..12].try_into().unwrap());
        let factor = f32::from_le_bytes(exact(address, 4)?.try_into().unwrap());
        let start_timeout = timeout as f32 / factor;
        if !factor.is_finite() || factor <= 0. || !start_timeout.is_finite() || start_timeout < 0. {
            return Err("invalid preload timeout conversion".into());
        }
        Ok(Self {
            image_sha256,
            common,
            entry,
            extensions,
            path_bytes,
            refresh_assets: refresh[1] as usize + 1,
            start_timeout,
        })
    }
    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn fixture() -> Self {
        Self {
            image_sha256: [0x42; 32],
            common: "Source/Shared.j".into(),
            entry: "WarmFiles".into(),
            extensions: [".hot".into(), ".txt".into()],
            path_bytes: 131,
            refresh_assets: 2,
            start_timeout: 4.125,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    pub queued: usize,
    /// Successful requests, including cache hits and duplicate paths.
    pub loaded: usize,
    /// Bytes visited by successful requests, not unique retained bytes.
    pub loaded_bytes: u64,
    pub missing: Vec<String>,
    pub timeout: Option<f32>,
    pub scripts: Vec<String>,
}

pub(crate) struct State {
    pub(crate) rules: Rules,
    pub(crate) reader: Arc<Resolver>,
    pub(crate) report: Report,
    pub(crate) depth: usize,
    pending: VecDeque<String>,
}
impl State {
    pub(crate) fn new(rules: Rules, reader: Arc<Resolver>) -> Self {
        Self {
            rules,
            reader,
            report: Report::default(),
            depth: 0,
            pending: VecDeque::new(),
        }
    }
    pub(crate) fn queue(&mut self, path: &str) -> Result<(), String> {
        if path.is_empty() {
            return Ok(());
        }
        if path.len() > 16 << 10 || self.report.queued >= 4096 {
            return Err("Preload: path/count budget".into());
        }
        AssetPath::new(path).map_err(|e| format!("Preload: {e}"))?;
        self.pending
            .try_reserve(1)
            .map_err(|_| "Preload: allocation")?;
        self.pending.push_back(path.into());
        self.report.queued += 1;
        Ok(())
    }
    pub(crate) fn drain(&mut self, limit: usize, timeout: Option<f32>) -> Result<(), String> {
        if timeout.is_some_and(|n| !n.is_finite() || n < 0.) {
            return Err("PreloadEnd: invalid timeout".into());
        }
        self.report.timeout = timeout;
        // shortcut: bounded synchronous warming; add incremental decoding when loading UI supports native asynchronous progress.
        let completed = self.report.loaded;
        while self.report.loaded - completed < limit {
            let Some(path) = self.pending.front() else {
                break;
            };
            let remaining = (128u64 << 20)
                .checked_sub(self.report.loaded_bytes)
                .ok_or("Preload: byte budget")?;
            match self.reader.preload_bounded(path, remaining.min(32 << 20)) {
                Ok(bytes) => {
                    self.report.loaded += 1;
                    self.report.loaded_bytes += bytes as u64;
                }
                Err(pcx_assets::Error::MissingAsset { .. }) => {
                    self.report.missing.push(path.clone())
                }
                Err(error) => return Err(format!("Preload {path}: {error}")),
            }
            self.pending.pop_front();
        }
        Ok(())
    }
    pub(crate) fn filename(&self, path: &str) -> Result<bool, String> {
        if path.is_empty() {
            return Ok(false);
        }
        if path.len() > self.rules.path_bytes {
            return Err("Preloader: path ceiling".into());
        }
        if !self.rules.extensions.iter().any(|ext| path.ends_with(ext)) {
            return Ok(false);
        }
        AssetPath::new(path).map_err(|e| format!("Preloader: {e}"))?;
        Ok(true)
    }
    pub(crate) fn common(&self) -> &str {
        &self.rules.common
    }
    pub(crate) fn entry(&self) -> &str {
        &self.rules.entry
    }
    pub(crate) fn refresh_assets(&self) -> usize {
        self.rules.refresh_assets
    }
    pub(crate) fn start_timeout(&self) -> f32 {
        self.rules.start_timeout
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    fn windows() -> BTreeMap<u32, Vec<u8>> {
        let mut out = BTreeMap::new();
        for (va, opcode, ptr, s) in [
            (0x4a46f1, 0x68, 0x1000, "Custom/Shared.j"),
            (0x4a47a8, 0x68, 0x2000, "LoadAssets"),
            (0x4a4615, 0xba, 0x3000, ".foo"),
            (0x4a464a, 0xb8, 0x4000, ".bar"),
        ] {
            let mut code = vec![opcode];
            code.extend_from_slice(&u32::to_le_bytes(ptr));
            out.insert(va, code);
            let mut text = vec![0; 256];
            text[..s.len()].copy_from_slice(s.as_bytes());
            out.insert(ptr, text);
        }
        out.insert(0x4a45fb, [&[0x68], &197u32.to_le_bytes()[..]].concat());
        out.insert(
            0x4a4580,
            vec![
                0x6a, 3, 0xe8, 0, 0, 0, 0, 0x8b, 0xc8, 0xe8, 0, 0, 0, 0, 0xc3,
            ],
        );
        out.insert(
            0x4a459e,
            [
                &[0xe8, 0, 0, 0, 0, 0xc7, 0x46, 0x10],
                &25u32.to_le_bytes()[..],
            ]
            .concat(),
        );
        out.insert(
            0x4a4459,
            [
                &[0xf3, 0x0f, 0x10, 1, 0xf3, 0x0f, 0x59, 5],
                &0x5000u32.to_le_bytes()[..],
            ]
            .concat(),
        );
        out.insert(0x5000, 2f32.to_le_bytes().to_vec());
        out.insert(
            0x4fd516,
            [
                &[0x8b, 0x4d, 8, 0x39, 0x4d, 0xec, 0x0f, 0x87],
                &0x183i32.to_le_bytes()[..],
            ]
            .concat(),
        );
        out.insert(0x4fd67f, vec![0xff, 0x45, 0xec]);
        for (va, at, target) in [
            (0x4a4580u32, 2, 0x4fd040u32),
            (0x4a4580, 9, 0x4fd430),
            (0x4a459e, 0, 0x4fd6d0),
        ] {
            out.get_mut(&va).unwrap()[at + 1..at + 5].copy_from_slice(
                &((i64::from(target) - i64::from(va) - at as i64 - 5) as i32).to_le_bytes(),
            );
        }
        out
    }
    fn decode(w: &BTreeMap<u32, Vec<u8>>) -> Result<Rules, String> {
        Rules::decode([7; 32], |va, _| w.get(&va).cloned().ok_or("missing".into()))
    }
    #[test]
    fn nonstock_entry_extensions_and_timeouts_come_from_checked_operands() {
        let r = decode(&windows()).unwrap();
        assert_eq!(r.common, "Custom/Shared.j");
        assert_eq!(r.entry, "LoadAssets");
        assert_eq!(r.extensions, [".foo", ".bar"]);
        assert_eq!(r.path_bytes, 197);
        assert_eq!(r.refresh_assets, 4);
        assert_eq!(r.start_timeout, 12.5);
        let mut zero = windows();
        zero.get_mut(&0x4a4580).unwrap()[1] = 0;
        assert_eq!(decode(&zero).unwrap().refresh_assets, 1);
    }
    #[test]
    fn malformed_and_truncated_windows_refuse() {
        let w = windows();
        for (&va, b) in &w {
            for n in 0..b.len() {
                let mut bad = w.clone();
                bad.get_mut(&va).unwrap().truncate(n);
                assert!(decode(&bad).is_err(), "{va:x} {n}");
            }
        }
        for (va, at, value) in [
            (0x4a4615, 0, 0xb8),
            (0x4a4580, 1, 0x80),
            (0x4a4580, 3, 0),
            (0x4a4580, 10, 0),
            (0x4a459e, 1, 0),
            (0x4a459e, 7, 0x14),
            (0x4a4459, 4, 0),
            (0x4fd516, 7, 0x83),
            (0x4fd67f, 2, 0xe8),
        ] {
            let mut bad = w.clone();
            bad.get_mut(&va).unwrap()[at] = value;
            assert!(decode(&bad).is_err());
        }
    }
    #[test]
    #[ignore = "requires PCX_WAR3_EXE from the selected local install"]
    fn selected_original_preload_rules_decode() {
        let image = std::fs::read(std::env::var_os("PCX_WAR3_EXE").unwrap()).unwrap();
        let r = Rules::read(&image).unwrap();
        assert_eq!(r.entry, "PreloadFiles");
        assert_eq!(r.common, "scripts\\common.j");
        assert_eq!(r.extensions, [".pld", ".txt"]);
        assert_eq!(r.start_timeout, 15.);
        assert_eq!(r.refresh_assets, 5);
    }
}
