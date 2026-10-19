//! Recreation-owned campaign profile storage. Original W3V import is not implied.
use std::collections::BTreeMap;

/// A profile store must distinguish an absent profile from a failed read, and
/// replace the saved bytes atomically or leave the previous save intact.
pub trait Storage: Send + Sync {
    fn load(&self, limit: usize) -> Result<Option<Vec<u8>>, String>;
    fn save(&self, bytes: &[u8]) -> Result<(), String>;
}

/// Application work/memory ceilings, not original engine limits.
#[derive(Clone, Copy, Debug)]
pub struct Budget {
    pub caches: usize,
    pub entries: usize,
    pub text_bytes: usize,
    pub storage_bytes: usize,
    pub handle_bytes: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Kind {
    Integer,
    Real,
    Boolean,
    String,
}

#[derive(Debug, PartialEq)]
pub(crate) enum Stored {
    Integer(i32),
    Real(u32),
    Boolean(bool),
    String(String),
}
impl Stored {
    pub fn kind(&self) -> Kind {
        match self {
            Self::Integer(_) => Kind::Integer,
            Self::Real(_) => Kind::Real,
            Self::Boolean(_) => Kind::Boolean,
            Self::String(_) => Kind::String,
        }
    }
    fn text_bytes(&self) -> usize {
        match self {
            Self::String(s) => s.len(),
            _ => 0,
        }
    }
}

type Key = (String, String, Kind);
type Values = BTreeMap<Key, Stored>;
#[derive(Default, Debug, PartialEq)]
pub(crate) struct Profile {
    files: BTreeMap<String, Values>,
    entries: usize,
    text_bytes: usize,
}

pub(crate) fn key(text: &str, limit: usize) -> Result<String, String> {
    if text.len() > limit {
        return Err("game-cache text byte policy exceeded".into());
    }
    if !text.is_ascii() || text.contains('\0') {
        return Err("game-cache non-ASCII keys or embedded NUL are unsupported".into());
    }
    Ok(text.to_ascii_lowercase())
}

impl Profile {
    pub fn open(&mut self, filename: &str, budget: Budget) -> Result<(), String> {
        if self.files.contains_key(filename) {
            return Ok(());
        }
        if self.files.len() >= budget.caches
            || self
                .text_bytes
                .checked_add(filename.len())
                .is_none_or(|n| n > budget.text_bytes)
        {
            return Err("game-cache profile cache/text policy exceeded".into());
        }
        self.files.insert(filename.into(), Values::new());
        self.text_bytes += filename.len();
        Ok(())
    }
    pub fn get(&self, filename: &str, mission: &str, key: &str, kind: Kind) -> Option<&Stored> {
        self.files
            .get(filename)?
            .get(&(mission.into(), key.into(), kind))
    }
    pub fn put(
        &mut self,
        filename: &str,
        mission: String,
        key: String,
        value: Stored,
        budget: Budget,
    ) -> Result<(), String> {
        let entry_key = (mission, key, value.kind());
        let previous = self
            .files
            .get(filename)
            .and_then(|values| values.get(&entry_key));
        let count = self
            .entries
            .checked_add(usize::from(previous.is_none()))
            .ok_or("game-cache entry accounting overflow")?;
        let text = self
            .text_bytes
            .checked_sub(previous.map_or(0, Stored::text_bytes))
            .and_then(|n| n.checked_add(value.text_bytes()))
            .and_then(|n| {
                n.checked_add(if previous.is_none() {
                    entry_key.0.len() + entry_key.1.len()
                } else {
                    0
                })
            })
            .and_then(|n| {
                n.checked_add(if self.files.contains_key(filename) {
                    0
                } else {
                    filename.len()
                })
            })
            .ok_or("game-cache byte accounting overflow")?;
        if count > budget.entries
            || text > budget.text_bytes
            || (!self.files.contains_key(filename) && self.files.len() >= budget.caches)
        {
            return Err("game-cache entry/text policy exceeded".into());
        }
        self.files
            .entry(filename.into())
            .or_default()
            .insert(entry_key, value);
        self.entries = count;
        self.text_bytes = text;
        Ok(())
    }
    pub fn flush(&mut self, filename: &str, mission: Option<&str>, key: Option<(&str, Kind)>) {
        let Some(values) = self.files.get_mut(filename) else {
            return;
        };
        values.retain(|(m, k, kind), value| {
            let remove = mission.is_none_or(|wanted| wanted == m)
                && key.is_none_or(|(wanted, family)| wanted == k && family == *kind);
            if remove {
                self.entries -= 1;
                self.text_bytes -= m.len() + k.len() + value.text_bytes();
            }
            !remove
        });
        if mission.is_none() {
            self.files.remove(filename);
            self.text_bytes -= filename.len();
        }
    }
    /// Canonical private PC3 format; never written over original W3V files.
    pub fn encode(&self, budget: Budget) -> Result<Vec<u8>, String> {
        let mut out = Writer {
            bytes: Vec::new(),
            limit: budget.storage_bytes,
        };
        out.bytes(b"PC3CACHE\x01")?;
        out.number(self.files.len())?;
        for (filename, values) in &self.files {
            out.text(filename)?;
            out.number(values.len())?;
            for ((mission, key, kind), value) in values {
                out.text(mission)?;
                out.text(key)?;
                out.bytes(&[*kind as u8])?;
                match value {
                    Stored::Integer(v) => out.bytes(&v.to_le_bytes())?,
                    Stored::Real(bits) => out.bytes(&bits.to_le_bytes())?,
                    Stored::Boolean(v) => out.bytes(&[u8::from(*v)])?,
                    Stored::String(v) => out.text(v)?,
                }
            }
        }
        Ok(out.bytes)
    }
    pub fn decode(bytes: &[u8], budget: Budget) -> Result<Self, String> {
        if bytes.len() > budget.storage_bytes {
            return Err("saved game-cache byte policy exceeded".into());
        }
        let mut input = Reader { bytes, cursor: 0 };
        if input.take(9)? != b"PC3CACHE\x01" {
            return Err(
                "unsupported campaign cache format/version; original W3V import is unavailable"
                    .into(),
            );
        }
        let count = input.number()?;
        if count > budget.caches {
            return Err("saved game-cache count policy exceeded".into());
        }
        let mut result = Self::default();
        for _ in 0..count {
            let filename = input.text(budget.text_bytes)?;
            if filename.is_empty()
                || key(&filename, budget.text_bytes)? != filename
                || result.files.contains_key(&filename)
            {
                return Err("noncanonical or duplicate saved cache filename".into());
            }
            result.open(&filename, budget)?;
            let entries = input.number()?;
            if result
                .entries
                .checked_add(entries)
                .is_none_or(|n| n > budget.entries)
            {
                return Err("saved game-cache entry policy exceeded".into());
            }
            for _ in 0..entries {
                let mission = input.text(budget.text_bytes)?;
                let name = input.text(budget.text_bytes)?;
                if key(&mission, budget.text_bytes)? != mission
                    || key(&name, budget.text_bytes)? != name
                {
                    return Err("noncanonical saved game-cache key".into());
                }
                let value = match input.take(1)?[0] {
                    0 => Stored::Integer(i32::from_le_bytes(
                        input.take(4)?.try_into().expect("four bytes"),
                    )),
                    1 => Stored::Real(u32::from_le_bytes(
                        input.take(4)?.try_into().expect("four bytes"),
                    )),
                    2 => match input.take(1)?[0] {
                        0 => Stored::Boolean(false),
                        1 => Stored::Boolean(true),
                        _ => return Err("invalid saved game-cache boolean".into()),
                    },
                    3 => {
                        let text = input.text(budget.text_bytes)?;
                        if text.contains('\0') {
                            return Err("saved game-cache string contains unsupported NUL".into());
                        }
                        Stored::String(text)
                    }
                    _ => return Err("unsupported saved game-cache value kind".into()),
                };
                if result
                    .get(&filename, &mission, &name, value.kind())
                    .is_some()
                {
                    return Err("duplicate saved game-cache key".into());
                }
                result.put(&filename, mission, name, value, budget)?;
            }
        }
        if input.cursor != bytes.len() {
            return Err("trailing saved game-cache bytes".into());
        }
        Ok(result)
    }
}

struct Writer {
    bytes: Vec<u8>,
    limit: usize,
}
impl Writer {
    fn bytes(&mut self, bytes: &[u8]) -> Result<(), String> {
        if self
            .bytes
            .len()
            .checked_add(bytes.len())
            .is_none_or(|n| n > self.limit)
        {
            return Err("encoded game-cache byte policy exceeded".into());
        }
        self.bytes
            .try_reserve(bytes.len())
            .map_err(|_| "game-cache allocation failed")?;
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    fn number(&mut self, n: usize) -> Result<(), String> {
        self.bytes(
            &u32::try_from(n)
                .map_err(|_| "game-cache length overflow")?
                .to_le_bytes(),
        )
    }
    fn text(&mut self, text: &str) -> Result<(), String> {
        self.number(text.len())?;
        self.bytes(text.as_bytes())
    }
}
struct Reader<'a> {
    bytes: &'a [u8],
    cursor: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self
            .cursor
            .checked_add(n)
            .ok_or("saved game-cache offset overflow")?;
        let result = self
            .bytes
            .get(self.cursor..end)
            .ok_or("truncated saved game cache")?;
        self.cursor = end;
        Ok(result)
    }
    fn number(&mut self) -> Result<usize, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().expect("four bytes")) as usize)
    }
    fn text(&mut self, limit: usize) -> Result<String, String> {
        let len = self.number()?;
        if len > limit {
            return Err("saved game-cache text policy exceeded".into());
        }
        String::from_utf8(self.take(len)?.to_vec())
            .map_err(|_| "saved game-cache text is not UTF-8".into())
    }
}

#[cfg(test)]
#[path = "game_cache_tests.rs"]
pub(crate) mod tests;
