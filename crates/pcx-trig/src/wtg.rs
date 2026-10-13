//! `war3map.wtg` — the trigger graph.
//!
//! The decoder lives at the crate root ([`crate::Wtg`]) next to WTS and WCT as
//! one scripting-side family sharing a framed record walk; this module
//! re-exports it under a `wtg::` path and holds the `wtg`-scoped tests. See
//! the crate docs for the measured offset table and the framing invariant
//! `file_len == 12 + Σ category + 8 + Σ variable + 4 + Σ trigger`.
//!
//! Every shipped parameter is decoded — no WTG parameter on either build is
//! refused. The four measured parameter types (preset / variable / function /
//! string) and the three sub-record kinds are all proved; a parameter outside
//! them, an ECA kind outside `{0,1,2}`, or a sub flag other than 1 is a typed
//! error (see [`crate::Error`]) rather than a guess.

pub use crate::{
    Category, Eca, EcaKind, Error, Param, ParamKind, Sub, SubKind, Trigger, TriggerArity, Variable,
    Wtg, WtgFraming,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;
    use std::path::Path;

    /// Parse the `war3map.wtg` of the one shipped map whose name contains
    /// `stem` (matched on the lowercased basename, so the `(N)` player-count
    /// prefix and the extension do not matter).
    fn wtg_of(dir: &Path, stem: &str) -> Option<Wtg<'static>> {
        let mut set = pcx_mpq::MpqSet::open_wc3(dir).ok()?;
        let map = crate::shipped_maps(&mut set).ok()?.into_iter().find(|m| {
            m.rsplit(['\\', '/'])
                .next()
                .unwrap()
                .to_ascii_lowercase()
                .contains(stem)
        })?;
        let bytes: &'static [u8] = Box::leak(
            crate::extract_trig_file(&mut set, &map, "war3map.wtg")
                .ok()?
                .into_boxed_slice(),
        );
        let arity = crate::install_trigger_arity(dir).unwrap();
        Wtg::parse(bytes, &arity).ok()
    }

    // Hand-read from the bytes: `(10)DustwallowKeys.w3m`, the melee template.
    #[test]
    fn dustwallow_wtg_fields_are_exact() {
        let mut ran = 0;
        for (_label, dir) in crate::builds() {
            let Some(w) = wtg_of(&dir, "dustwallowkeys") else {
                continue;
            };
            ran += 1;
            assert_eq!(w.version(), crate::WTG_VERSION);
            assert_eq!(w.categories().len(), 1);
            assert_eq!(w.categories()[0].id, 0);
            assert_eq!(w.categories()[0].name, b"Initialization");
            assert_eq!(w.unknown_root(), 1);
            assert!(w.variables().is_empty());
            assert_eq!(w.triggers().len(), 1);
            let t = &w.triggers()[0];
            assert_eq!(t.name, b"Melee Initialization");
            assert_eq!(
                t.description,
                b"Default melee game initialization for all players"
            );
            assert_eq!((t.is_enabled, t.is_custom), (1, 0));
            assert_eq!(t.category_id, 0);
            // 9 events, all parameterless game events, then nothing else.
            assert_eq!(t.ecas.len(), 9);
            assert_eq!(t.ecas[0].kind, EcaKind::Event);
            assert_eq!(t.ecas[0].name, b"MapInitializationEvent");
            assert!(t.ecas.iter().all(|e| e.params.is_empty()));
            assert!(w.framing().sums());
            assert_eq!(w.canonical_bytes(), w.raw());
        }
        assert!(
            ran >= 1,
            "dustwallow should be present on at least one build"
        );
    }

    // Hand-read from the bytes: `(1)TheDeathSheep.w3m`, a custom map with eight
    // categories, 40 variables and a rich parameter tree.
    #[test]
    fn thedeathsheep_wtg_structure_is_exact() {
        let mut ran = 0;
        for (_label, dir) in crate::builds() {
            let Some(w) = wtg_of(&dir, "thedeathsheep") else {
                continue;
            };
            ran += 1;
            assert_eq!(w.categories().len(), 8);
            assert!(w.categories().iter().any(|c| c.name == b"Unit Control"));
            assert_eq!(w.variables().len(), 40);
            let started = w
                .variables()
                .iter()
                .find(|v| v.name == b"GameStarted")
                .expect("GameStarted variable");
            assert_eq!(started.type_name, b"boolean");
            assert_eq!(started.initial_value, b"false");
            // A location variable with no initial value sits first.
            let first = &w.variables()[0];
            assert_eq!(first.name, b"TargetDest");
            assert_eq!(first.type_name, b"location");
            assert_eq!((first.is_array, first.is_initialized), (0, 0));
            assert_eq!(w.triggers().len(), 54);
            // Framing relation the decoder stands on, and byte-for-byte round
            // trip, on a 77 KB parameter-rich file.
            assert!(w.framing().sums(), "{:?}", w.framing().violations());
            assert_eq!(w.canonical_bytes(), w.raw());
            // Every trigger names a category that has a record.
            assert!(
                w.triggers()
                    .iter()
                    .all(|t| w.categories().iter().any(|c| c.id == t.category_id))
            );
        }
        assert!(
            ran >= 1,
            "thedeathsheep should be present on at least one build"
        );
    }

    /// Every `.wtg` on every shipped build is framed exactly and round-trips
    /// byte-for-byte; a per-file census and any failures land in
    /// `/tmp/opencode/wtg-sweep.txt`.
    #[test]
    #[ignore = "opens every shipped install on both builds; run explicitly"]
    fn sweep_every_shipped_wtg() {
        let mut total = 0usize;
        let mut failed = 0usize;
        let mut misses = 0usize;
        let mut ecas = 0usize;
        let mut params = 0usize;
        let mut per_type: std::collections::BTreeMap<&'static str, usize> =
            std::collections::BTreeMap::new();
        let mut log = String::new();
        for (label, dir) in crate::builds() {
            let Ok(mut set) = pcx_mpq::MpqSet::open_wc3(&dir) else {
                continue;
            };
            let arity = crate::install_trigger_arity(&dir).unwrap();
            let maps = crate::shipped_maps(&mut set).unwrap();
            for map in &maps {
                if crate::extract_trig_file(&mut set, map, "war3map.wtg").is_err() {
                    misses += 1;
                    continue;
                }
                // (re-extract to own; the miss check above already failed soft)
                let bytes = crate::extract_trig_file(&mut set, map, "war3map.wtg").unwrap();
                total += 1;
                match Wtg::parse(&bytes, &arity) {
                    Ok(w) => {
                        let mut v = w.framing().violations();
                        for e in w.triggers().iter().flat_map(|t| &t.ecas) {
                            ecas += 1;
                            for p in &e.params {
                                params += 1;
                                *per_type.entry(param_name(p.kind)).or_default() += 1;
                            }
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
            "wtgs={total} lacking={misses} failed={failed} ecas={ecas} params={params} \
             per_type={per_type:?}\n"
        );
        log = census + &log;
        print!("{log}");
        let _ = std::fs::create_dir_all("/tmp/opencode");
        if let Ok(mut f) = std::fs::File::create("/tmp/opencode/wtg-sweep.txt") {
            let _ = f.write_all(log.as_bytes());
        }
        assert_eq!(
            failed, 0,
            "every shipped wtg must frame; see /tmp/opencode/wtg-sweep.txt"
        );
    }

    fn param_name(k: ParamKind) -> &'static str {
        match k {
            ParamKind::Preset => "preset",
            ParamKind::Variable => "variable",
            ParamKind::Function => "function",
            ParamKind::Str => "string",
        }
    }
}
