//! Bounded read access to one selected Warcraft III install: the MPQ archives of its
//! edition, with names listed once and reads capped by the caller's budget.
use pcx_assets::{ContentSource, LayerKind, SourceContent, SourceError, SourceInfo};
use std::path::Path;
use std::sync::Mutex;

/// Local resource ceilings for one session, not original measurements.
pub const MAX_LISTED_PATHS: usize = 100_000;
pub const MAX_ASSET_BYTES: usize = 64 << 20;
pub const MAX_AGGREGATE_BYTES: usize = 512 << 20;

/// One bounded MPQ set over the edition's archives.
pub struct InstallSource {
    info: SourceInfo,
    set: Mutex<pcx_mpq::MpqSet>,
    names: Vec<String>,
    huffman: pcx_mpq::huffman::HuffmanTable,
}

impl InstallSource {
    pub fn open(dir: &Path) -> Result<Self, String> {
        let archives = pcx_mpq::DEFAULT_PRIORITY;
        let mut set = pcx_mpq::MpqSet::open_named(dir, &archives).map_err(|e| e.to_string())?;
        let mut names = set.file_list().map_err(|e| e.to_string())?;
        if names.len() > MAX_LISTED_PATHS {
            return Err(format!(
                "{}: archive listing exceeds {MAX_LISTED_PATHS}",
                dir.display()
            ));
        }
        // Listings repeat names under other spellings; keep one per normalized
        // path, and only when every spelling resolves to the same archive block.
        let mut canonical = std::collections::BTreeMap::<pcx_assets::AssetPath, String>::new();
        for name in names.drain(..) {
            let Some((archive, block)) = set.locate(&name).map(|(a, b)| (a, *b)) else {
                continue;
            };
            let key = pcx_assets::AssetPath::new(&name).map_err(|e| e.to_string())?;
            match canonical.get(&key) {
                Some(prior) => {
                    let (a, b) = set.locate(prior).map(|(a, b)| (a, *b)).expect("located");
                    if (a, b.file_pos, b.file_size) != (archive, block.file_pos, block.file_size) {
                        return Err(format!("{name} and {prior} name different archive data"));
                    }
                }
                None => {
                    canonical.insert(key, name);
                }
            }
        }
        let names: Vec<String> = canonical.into_values().collect();
        let huffman = pcx_mpq::huffman::HuffmanTable::open(dir.join("Warcraft III.exe"))
            .map_err(|e| e.to_string())?;
        Ok(Self {
            info: SourceInfo {
                id: format!("selected install {}", archives.join(",")),
                kind: LayerKind::Mpq,
                origin: dir.display().to_string(),
                version: Some("selected install; client default archive priority".into()),
            },
            set: Mutex::new(set),
            names,
            huffman,
        })
    }
}

impl ContentSource for InstallSource {
    fn info(&self) -> &SourceInfo {
        &self.info
    }
    fn paths(&self) -> Result<Vec<String>, SourceError> {
        Ok(self.names.clone())
    }
    fn read(&self, name: &str) -> Result<Option<SourceContent>, SourceError> {
        self.read_bounded(name, MAX_ASSET_BYTES as u64)
    }
    fn read_bounded(&self, name: &str, limit: u64) -> Result<Option<SourceContent>, SourceError> {
        let mut set = self.set.lock().map_err(|e| SourceError::Unavailable {
            detail: e.to_string(),
        })?;
        let Some((at, block)) = set.locate(name) else {
            return Ok(None);
        };
        let declared_length = u64::from(block.file_size);
        let budget = pcx_mpq::ExtractionBudget {
            max_raw_packed_bytes: MAX_ASSET_BYTES,
            max_decoded_bytes: usize::try_from(limit.min(MAX_ASSET_BYTES as u64))
                .unwrap_or(MAX_ASSET_BYTES),
        };
        let archive = &mut set.members_mut()[at];
        let locator = format!("{}::{name}", archive.path().display());
        let bytes = archive
            .extract_bounded(name, budget, Some(&self.huffman))
            .map_err(|e| SourceError::MpqExtraction(Box::new(e)))?;
        Ok(Some(SourceContent {
            bytes,
            declared_length,
            locator,
        }))
    }
}
