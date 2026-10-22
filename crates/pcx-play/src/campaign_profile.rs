//! Two independent sections in the existing local profile. Cache-only bytes remain readable.
use super::BUDGET;
use crate::game_cache::Storage;
use std::sync::{Arc, Mutex};

const MAGIC: &[u8] = b"PCXPROFILE1\n";

#[derive(Clone, Copy)]
enum Part {
    Cache,
    Progress,
}

struct Envelope<'a> {
    cache: Option<&'a [u8]>,
    progress: Option<&'a [u8]>,
}

impl<'a> Envelope<'a> {
    fn parse(bytes: &'a [u8]) -> Result<Self, String> {
        if bytes.len() > BUDGET.storage_bytes {
            return Err("campaign profile byte policy exceeded".into());
        }
        if !bytes.starts_with(MAGIC) {
            return Ok(Self {
                cache: Some(bytes),
                progress: None,
            });
        }
        let header = bytes
            .get(MAGIC.len()..MAGIC.len() + 8)
            .ok_or("truncated campaign profile header")?;
        let cache = usize::try_from(u32::from_le_bytes(header[..4].try_into().unwrap()))
            .map_err(|e| e.to_string())?;
        let progress = usize::try_from(u32::from_le_bytes(header[4..].try_into().unwrap()))
            .map_err(|e| e.to_string())?;
        let begin = MAGIC.len() + 8;
        let middle = begin
            .checked_add(cache)
            .ok_or("campaign profile offset overflow")?;
        let end = middle
            .checked_add(progress)
            .ok_or("campaign profile offset overflow")?;
        if end != bytes.len() {
            return Err("campaign profile section length mismatch".into());
        }
        Ok(Self {
            cache: (cache != 0).then(|| &bytes[begin..middle]),
            progress: (progress != 0).then(|| &bytes[middle..end]),
        })
    }
    fn validate(&self) -> Result<(), String> {
        if let Some(bytes) = self.cache {
            crate::game_cache::Profile::decode(bytes, BUDGET)?;
        }
        if let Some(bytes) = self.progress {
            crate::campaign_progress::Progress::decode(bytes)?;
        }
        Ok(())
    }
    fn encode(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        if self.progress.is_none() {
            return copy(self.cache.unwrap_or(&[]));
        }
        let cache = self.cache.unwrap_or(&[]);
        let progress = self.progress.unwrap_or(&[]);
        let length = (MAGIC.len() + 8)
            .checked_add(cache.len())
            .and_then(|n| n.checked_add(progress.len()))
            .filter(|n| *n <= BUDGET.storage_bytes)
            .ok_or("campaign profile byte policy exceeded")?;
        let mut out = Vec::new();
        out.try_reserve_exact(length).map_err(|e| e.to_string())?;
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(
            &u32::try_from(cache.len())
                .map_err(|e| e.to_string())?
                .to_le_bytes(),
        );
        out.extend_from_slice(
            &u32::try_from(progress.len())
                .map_err(|e| e.to_string())?
                .to_le_bytes(),
        );
        out.extend_from_slice(cache);
        out.extend_from_slice(progress);
        Ok(out)
    }
}

fn copy(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if bytes.len() > BUDGET.storage_bytes {
        return Err("campaign profile byte policy exceeded".into());
    }
    let mut out = Vec::new();
    out.try_reserve_exact(bytes.len())
        .map_err(|e| e.to_string())?;
    out.extend_from_slice(bytes);
    Ok(out)
}

pub(super) fn validate(bytes: &[u8]) -> Result<(), String> {
    Envelope::parse(bytes)?.validate()
}

struct Shared {
    storage: Arc<dyn Storage>,
    lock: Mutex<()>,
}
struct Channel {
    shared: Arc<Shared>,
    part: Part,
}

pub(super) fn split(storage: Arc<dyn Storage>) -> (Arc<dyn Storage>, Arc<dyn Storage>) {
    let shared = Arc::new(Shared {
        storage,
        lock: Mutex::new(()),
    });
    (
        Arc::new(Channel {
            shared: Arc::clone(&shared),
            part: Part::Cache,
        }),
        Arc::new(Channel {
            shared,
            part: Part::Progress,
        }),
    )
}

impl Storage for Channel {
    fn load(&self, limit: usize) -> Result<Option<Vec<u8>>, String> {
        let _lock = self.shared.lock.lock().map_err(|e| e.to_string())?;
        let Some(raw) = self.shared.storage.load(BUDGET.storage_bytes)? else {
            return Ok(None);
        };
        if matches!(self.part, Part::Cache) && !raw.starts_with(MAGIC) {
            if raw.len() > limit || raw.len() > BUDGET.storage_bytes {
                return Err("campaign profile section read policy exceeded".into());
            }
            return Ok(Some(raw));
        }
        let envelope = Envelope::parse(&raw)?;
        let bytes = match self.part {
            Part::Cache => envelope.cache,
            Part::Progress => envelope.progress,
        };
        if bytes.is_some_and(|bytes| bytes.len() > limit) {
            return Err("campaign profile section read policy exceeded".into());
        }
        // Payload consumers validate their own section; storage only frames bounded bytes.
        bytes.map(copy).transpose()
    }
    fn save(&self, bytes: &[u8]) -> Result<(), String> {
        let _lock = self.shared.lock.lock().map_err(|e| e.to_string())?;
        let raw = self.shared.storage.load(BUDGET.storage_bytes)?;
        let mut envelope = match raw.as_deref() {
            Some(bytes) => Envelope::parse(bytes)?,
            None => Envelope {
                cache: None,
                progress: None,
            },
        };
        match self.part {
            Part::Cache => envelope.cache = Some(bytes),
            Part::Progress => envelope.progress = Some(bytes),
        }
        self.shared.storage.save(&envelope.encode()?)
    }
}

#[cfg(test)]
#[path = "campaign_profile_tests.rs"]
mod tests;
