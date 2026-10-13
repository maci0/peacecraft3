//! `war3map.wct` — the trigger custom-text sidecar.
//!
//! The decoder lives at the crate root ([`crate::Wct`]) with WTS/WTG as one
//! family; this module re-exports it under a `wct::` path and holds the
//! `wct`-scoped tests. See the crate docs for the measured layout
//! (`version=0`, a block count, then `count × (u32 len + len NUL-terminated
//! bytes)`) and the framing invariant
//! `file_len == 8 + 4·count + Σ len`, which holds on all 132 shipped files.

pub use crate::{Error, Wct, WctBlock, WctFraming};

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;
    use std::path::Path;

    fn wct_of(dir: &Path, stem: &str) -> Option<Vec<u8>> {
        let mut set = pcx_mpq::MpqSet::open_wc3(dir).ok()?;
        let map = crate::shipped_maps(&mut set).ok()?.into_iter().find(|m| {
            m.rsplit(['\\', '/'])
                .next()
                .unwrap()
                .to_ascii_lowercase()
                .contains(stem)
        })?;
        crate::extract_trig_file(&mut set, &map, "war3map.wct").ok()
    }

    // Hand-read from the bytes: `(10)DustwallowKeys.w3m`, whose triggers keep
    // their GUI bodies, so the whole file is the header plus one empty slot.
    #[test]
    fn dustwallow_wct_is_one_empty_block() {
        let mut ran = 0;
        for (_label, dir) in crate::builds() {
            let Some(bytes) = wct_of(&dir, "dustwallowkeys") else {
                continue;
            };
            ran += 1;
            let w = Wct::parse(&bytes).unwrap();
            assert_eq!(w.unknown0(), 0);
            assert_eq!(w.blocks().len(), 1);
            assert!(w.blocks()[0].text.is_empty());
            assert!(w.framing().sums());
            assert_eq!(w.canonical_bytes(), w.raw());
        }
        assert!(
            ran >= 1,
            "dustwallow should be present on at least one build"
        );
    }

    // Hand-read from the bytes: a campaign map (`Human06`) with real custom
    // scripts — three NUL-terminated JASS blocks on 1.28c, the count matching
    // the trigger graph.
    #[test]
    fn human06_wct_custom_blocks_are_exact() {
        let mut ran = 0;
        for (_label, dir) in crate::builds() {
            let Some(bytes) = wct_of(&dir, "human06") else {
                continue;
            };
            ran += 1;
            let w = Wct::parse(&bytes).unwrap();
            assert_eq!(w.unknown0(), 0);
            assert_eq!(w.blocks().len(), 97, "one slot per trigger");
            // Every non-empty block is NUL-terminated JASS; the whole file
            // frames and round-trips.
            assert!(w.framing().sums());
            assert_eq!(w.canonical_bytes(), w.raw());
            let scripts: Vec<&[u8]> = w
                .blocks()
                .iter()
                .map(|b| b.text.as_slice())
                .filter(|t| !t.is_empty())
                .collect();
            assert!(!scripts.is_empty());
            for s in &scripts {
                assert_eq!(*s.last().unwrap(), 0, "custom JASS is NUL-terminated");
                let body = &s[..s.len() - 1];
                assert!(
                    body.windows(9).any(|w| w == b"function "),
                    "custom text holds JASS"
                );
            }
        }
        assert!(ran >= 1, "human06 should be present on at least one build");
    }

    /// Every `.wct` on every shipped build is framed exactly and round-trips
    /// byte-for-byte; a per-file census and any failures land in
    /// `/tmp/opencode/wct-sweep.txt`.
    #[test]
    #[ignore = "opens every shipped install on both builds; run explicitly"]
    fn sweep_every_shipped_wct() {
        let mut total = 0usize;
        let mut failed = 0usize;
        let mut misses = 0usize;
        let mut blocks = 0usize;
        let mut nonempty = 0usize;
        let mut log = String::new();
        for (label, dir) in crate::builds() {
            let Ok(mut set) = pcx_mpq::MpqSet::open_wc3(&dir) else {
                continue;
            };
            let maps = crate::shipped_maps(&mut set).unwrap();
            for map in &maps {
                let Ok(bytes) = crate::extract_trig_file(&mut set, map, "war3map.wct") else {
                    misses += 1;
                    continue;
                };
                total += 1;
                match Wct::parse(&bytes) {
                    Ok(w) => {
                        blocks += w.blocks().len();
                        nonempty += w.framing().nonempty;
                        let mut v = w.framing().violations();
                        if w.unknown0() != 0 {
                            v.push(format!("version word is {}", w.unknown0()));
                        }
                        if w.canonical_bytes().as_slice() != w.raw() {
                            v.push("canonical bytes differ from the raw file".into());
                        }
                        if !v.is_empty() {
                            failed += 1;
                            log.push_str(&format!("{label} {map}: {v:?}\n"));
                        }
                    }
                    Err(e) => {
                        failed += 1;
                        log.push_str(&format!("{label} {map}: {e}\n"));
                    }
                }
            }
        }
        let census = format!(
            "wcts={total} lacking={misses} failed={failed} blocks={blocks} nonempty={nonempty}\n"
        );
        log = census + &log;
        print!("{log}");
        let _ = std::fs::create_dir_all("/tmp/opencode");
        if let Ok(mut f) = std::fs::File::create("/tmp/opencode/wct-sweep.txt") {
            let _ = f.write_all(log.as_bytes());
        }
        assert_eq!(
            failed, 0,
            "every shipped wct must frame; see /tmp/opencode/wct-sweep.txt"
        );
    }
}
