//! Immutable auxiliary PE inputs. These are client sources, never resolver assets.
use crate::{
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    pe_image::u32_at,
    scripts::AssetIdentity,
};
use sha2::{Digest, Sha256};
use std::sync::Arc;

pub const CLIENT_IMAGE_BYTES: usize = 32 << 20;

#[derive(Clone, Copy, Debug)]
pub struct ImportLimits {
    pub image_bytes: usize,
    pub descriptors: usize,
    pub thunks: usize,
    pub string_bytes: usize,
    pub work_bytes: usize,
}
impl Default for ImportLimits {
    fn default() -> Self {
        Self {
            image_bytes: CLIENT_IMAGE_BYTES,
            descriptors: 1024,
            thunks: 65536,
            string_bytes: 512,
            work_bytes: 2 << 20,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportDemand {
    pub main_sha256: Sha256Digest,
    pub descriptor_rva: u32,
    pub iat_rva: u32,
    pub import_name: String,
    pub module_name: String,
}
fn word(bytes: &[u8], offset: usize) -> Result<u32, String> {
    u32_at(bytes, offset).ok_or_else(|| "client import truncated word".into())
}
impl ImportDemand {
    /// Resolve the descriptor owning this exact call's IAT slot, within its own terminated list.
    pub fn for_iat(image: &[u8], iat_va: u32, limits: ImportLimits) -> Result<Self, String> {
        if image.len() > limits.image_bytes
            || limits.descriptors == 0
            || limits.thunks == 0
            || limits.string_bytes == 0
        {
            return Err("client import input/work budget".into());
        }
        let pe = Pe::parse(
            image,
            RuleInputBudget {
                image_bytes: limits.image_bytes,
                profile_bytes: limits.work_bytes,
                sections: 96,
            },
        )
        .map_err(|e| e.to_string())?;
        let header = usize::try_from(word(image, 0x3c)?).map_err(|_| "client PE offset")?;
        let optional = header.checked_add(24).ok_or("client PE offset overflow")?;
        let end = optional
            .checked_add(112)
            .ok_or("client PE optional overflow")?;
        let opt = image
            .get(optional..end)
            .ok_or("client PE optional truncated")?;
        let optional_len =
            crate::pe_image::u16_at(image, header + 20).ok_or("client PE optional size")?;
        if optional_len < 112 || word(opt, 92)? < 2 {
            return Err("client import directory absent".into());
        }
        let base = word(opt, 28)?;
        let iat_rva = iat_va.checked_sub(base).ok_or("client IAT outside image")?;
        let directory = word(opt, 104)?;
        let length = word(opt, 108)?;
        if directory == 0 || length < 20 {
            return Err("client import directory absent".into());
        }
        directory
            .checked_add(length)
            .ok_or("client import directory overflow")?;
        let mut remaining = limits.work_bytes;
        let mut read = |rva: u32, count: usize| -> Result<Vec<u8>, String> {
            remaining = remaining
                .checked_sub(count)
                .ok_or("client import work exhausted")?;
            let va = base
                .checked_add(rva)
                .ok_or("client import address overflow")?;
            pe.read_from(va, false)
                .map_err(|e| e.to_string())?
                .get(..count)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| "client import range truncated".into())
        };
        let mut found = None;
        let mut ended = false;
        let mut thunks = 0;
        for index in 0..limits.descriptors {
            let offset = u32::try_from(index)
                .ok()
                .and_then(|n| n.checked_mul(20))
                .ok_or("client descriptor overflow")?;
            if offset > length - 20 {
                break;
            }
            let rva = directory
                .checked_add(offset)
                .ok_or("client descriptor overflow")?;
            let descriptor = read(rva, 20)?;
            if descriptor.iter().all(|b| *b == 0) {
                ended = true;
                break;
            }
            let lookup = word(&descriptor, 0)?;
            let first = word(&descriptor, 16)?;
            if first == 0 {
                return Err("client import lacks IAT".into());
            }
            let lookup = if lookup == 0 { first } else { lookup };
            let mut terminated = false;
            for slot in 0..limits.thunks {
                thunks += 1;
                if thunks > limits.thunks {
                    return Err("client import thunk budget".into());
                }
                let displacement = u32::try_from(slot)
                    .ok()
                    .and_then(|n| n.checked_mul(4))
                    .ok_or("client thunk overflow")?;
                let pointer = word(
                    &read(
                        lookup
                            .checked_add(displacement)
                            .ok_or("client thunk overflow")?,
                        4,
                    )?,
                    0,
                )?;
                if pointer == 0 {
                    terminated = true;
                    break;
                }
                let target = first
                    .checked_add(displacement)
                    .ok_or("client IAT overflow")?;
                if target != iat_rva {
                    continue;
                }
                if found.is_some() {
                    return Err("client import descriptor ambiguous".into());
                }
                if pointer & (1 << 31) != 0 {
                    return Err("client import is ordinal-only".into());
                }
                if word(&read(target, 4)?, 0)? == 0 {
                    return Err("client IAT is empty".into());
                }
                let mut string = |rva: u32| -> Result<String, String> {
                    let mut bytes = Vec::new();
                    for at in 0..limits.string_bytes {
                        let b = read(
                            rva.checked_add(at.try_into().map_err(|_| "client string overflow")?)
                                .ok_or("client string overflow")?,
                            1,
                        )?[0];
                        if b == 0 {
                            if bytes.is_empty() || !bytes.is_ascii() {
                                return Err("client import string is not bounded ASCII".into());
                            }
                            return String::from_utf8(bytes)
                                .map_err(|_| "client import string encoding".into());
                        }
                        bytes.push(b);
                    }
                    Err("client import string budget".into())
                };
                let import_name = string(
                    pointer
                        .checked_add(2)
                        .ok_or("client import name overflow")?,
                )?;
                let module_name = string(word(&descriptor, 12)?)?;
                checked_leaf(&module_name)?;
                found = Some(Self {
                    main_sha256: sha256(image),
                    descriptor_rva: rva,
                    iat_rva,
                    import_name,
                    module_name,
                });
            }
            if !terminated {
                return Err("client import thunk list unterminated".into());
            }
        }
        if !ended {
            return Err("client import descriptors unterminated/budget".into());
        }
        found.ok_or_else(|| "client IAT has no named import owner".into())
    }
}

pub fn checked_leaf(name: &str) -> Result<(), String> {
    if name.is_empty()
        || !name.is_ascii()
        || name.trim() != name
        || name.ends_with('.')
        || name == "."
        || name == ".."
        || name.bytes().any(|b| {
            b.is_ascii_control()
                || matches!(
                    b,
                    b'/' | b'\\' | b':' | b'<' | b'>' | b'"' | b'|' | b'?' | b'*'
                )
        })
    {
        return Err("client module owner must be a bounded ASCII leaf".into());
    }
    Ok(())
}
/// Normalize a supplied member path, never an OS search path or resolver key.
pub fn member_path(path: &str) -> Result<String, String> {
    let path = path.replace('\\', "/");
    if path.is_empty()
        || path.contains(':')
        || path.chars().any(char::is_control)
        || path
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == "..")
    {
        return Err("invalid client source member path".into());
    }
    Ok(path.to_ascii_lowercase())
}
#[derive(Clone, Debug)]
pub struct ModuleInput {
    pub origin: AssetIdentity,
    pub member: String,
    pub bytes: Arc<[u8]>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientModuleReceipt {
    pub demand: ImportDemand,
    pub module_sha256: Sha256Digest,
    pub byte_length: usize,
    pub origin: AssetIdentity,
    pub member: String,
}
#[derive(Clone, Debug)]
pub struct IdentifiedModule {
    pub bytes: Arc<[u8]>,
    pub origin: AssetIdentity,
    pub receipt: ClientModuleReceipt,
}
#[derive(Clone, Debug)]
pub struct ClientSources {
    main: Arc<[u8]>,
    origin: AssetIdentity,
    generation: Sha256Digest,
    demand: ImportDemand,
    module: Result<Option<IdentifiedModule>, String>,
}
impl ClientSources {
    /// Identity hashes content and logical roles, not installation/transport diagnostic labels.
    pub fn checked(
        main: Arc<[u8]>,
        origin: AssetIdentity,
        main_member: &str,
        archive_hashes: &[Sha256Digest],
        demand: ImportDemand,
        module: Result<Option<ModuleInput>, String>,
    ) -> Result<Self, String> {
        if main.is_empty() || main.len() > CLIENT_IMAGE_BYTES || sha256(&main) != demand.main_sha256
        {
            return Err("client source main identity/budget mismatch".into());
        }
        let optional = usize::try_from(word(&main, 0x3c)?)
            .map_err(|_| "client header offset")?
            .checked_add(24)
            .ok_or("client header overflow")?;
        let base = word(&main, optional.checked_add(28).ok_or("client base offset")?)?;
        if ImportDemand::for_iat(
            &main,
            base.checked_add(demand.iat_rva)
                .ok_or("client IAT address overflow")?,
            ImportLimits::default(),
        )? != demand
        {
            return Err("client demand does not match main import descriptor".into());
        }
        if origin.source.trim().is_empty() {
            return Err("client source identity absent".into());
        }
        checked_leaf(&demand.module_name)?;
        let main_member = member_path(main_member)?;
        let parent = main_member
            .rsplit_once('/')
            .map(|(p, _)| format!("{p}/"))
            .unwrap_or_default();
        let module = match module {
            Ok(Some(module)) => {
                if module.origin.source.trim().is_empty()
                    || module.bytes.is_empty()
                    || module.bytes.len() > CLIENT_IMAGE_BYTES
                    || member_path(&module.member)?
                        != format!("{parent}{}", demand.module_name.to_ascii_lowercase())
                {
                    return Err(
                        "client auxiliary is not the demanded same-parent bounded member".into(),
                    );
                }
                let receipt = ClientModuleReceipt {
                    demand: demand.clone(),
                    module_sha256: sha256(&module.bytes),
                    byte_length: module.bytes.len(),
                    origin: module.origin.clone(),
                    member: module.member,
                };
                Ok(Some(IdentifiedModule {
                    bytes: module.bytes,
                    origin: module.origin,
                    receipt,
                }))
            }
            Ok(None) => Ok(None),
            Err(error) => Err(error),
        };
        let mut hash = Sha256::new();
        hash.update(b"pcx-client-sources-v1\0");
        hash.update(demand.main_sha256);
        hash.update((archive_hashes.len() as u64).to_le_bytes());
        // Picker order is not archive priority; retain the content multiset across transports.
        let mut archive_hashes = archive_hashes.to_vec();
        archive_hashes.sort_unstable();
        for digest in archive_hashes {
            hash.update(digest);
        }
        hash.update(demand.descriptor_rva.to_le_bytes());
        hash.update(demand.iat_rva.to_le_bytes());
        for text in [&demand.import_name, &demand.module_name] {
            hash.update((text.len() as u64).to_le_bytes());
            hash.update(text.as_bytes());
        }
        match &module {
            Ok(Some(module)) => {
                hash.update([2]);
                hash.update(module.receipt.module_sha256);
            }
            Ok(None) => hash.update([0]),
            Err(_) => hash.update([1]),
        }
        Ok(Self {
            main,
            origin,
            generation: hash.finalize().into(),
            demand,
            module,
        })
    }
    pub fn main(&self) -> (&AssetIdentity, &[u8]) {
        (&self.origin, &self.main)
    }
    pub fn generation(&self) -> Sha256Digest {
        self.generation
    }
    pub fn demand(&self) -> &ImportDemand {
        &self.demand
    }
    pub fn module(
        &self,
        demand: &ImportDemand,
        max_bytes: u64,
    ) -> Result<Option<&IdentifiedModule>, String> {
        if demand != &self.demand {
            return Err("client module belongs to a foreign main/import demand".into());
        }
        let module = self.module.as_ref().map_err(Clone::clone)?;
        if module
            .as_ref()
            .is_some_and(|m| m.bytes.len() as u64 > max_bytes)
        {
            return Err("client module query budget".into());
        }
        Ok(module.as_ref())
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, thiserror::Error)]
pub enum PinError {
    #[error("invalid client source: {0}")]
    Invalid(String),
    #[error("client source unavailable: {0}")]
    Unavailable(String),
}
#[cfg(not(target_arch = "wasm32"))]
impl From<&str> for PinError {
    fn from(value: &str) -> Self {
        Self::Invalid(value.into())
    }
}
#[cfg(not(target_arch = "wasm32"))]
impl From<String> for PinError {
    fn from(value: String) -> Self {
        Self::Unavailable(value)
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn pin_sibling(
    root: &std::path::Path,
    main_source: &std::path::Path,
    demand: &ImportDemand,
) -> Result<Option<ModuleInput>, PinError> {
    use std::io::Read;
    checked_leaf(&demand.module_name).map_err(PinError::Invalid)?;
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let main = main_source.canonicalize().map_err(|e| e.to_string())?;
    if main.parent() != Some(root.as_path()) {
        return Err("main client is not in the selected canonical installation".into());
    }
    let mut main_bytes = Vec::new();
    std::fs::File::open(&main)
        .map_err(|e| e.to_string())?
        .take(CLIENT_IMAGE_BYTES as u64 + 1)
        .read_to_end(&mut main_bytes)
        .map_err(|e| e.to_string())?;
    if main_bytes.len() > CLIENT_IMAGE_BYTES || sha256(&main_bytes) != demand.main_sha256 {
        return Err("selected main file changed before module pin".into());
    }
    let mut found = None;
    for (count, entry) in std::fs::read_dir(&root)
        .map_err(|e| e.to_string())?
        .enumerate()
    {
        if count >= 4096 {
            return Err("client module directory work budget".into());
        }
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.eq_ignore_ascii_case(&demand.module_name))
        {
            continue;
        }
        if found.is_some() {
            return Err("client module filename ambiguous".into());
        }
        found = Some(entry.path());
    }
    let Some(path) = found else { return Ok(None) };
    let before = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    if !before.is_file()
        || before.file_type().is_symlink()
        || before.len() > CLIENT_IMAGE_BYTES as u64
        || before.len() == 0
    {
        return Err("client module is not a bounded regular sibling file".into());
    }
    let canonical = path.canonicalize().map_err(|e| e.to_string())?;
    if canonical.parent() != Some(root.as_path()) {
        return Err("client module escapes selected installation".into());
    }
    let file = std::fs::File::open(&canonical).map_err(|e| e.to_string())?;
    let stamp = crate::campaign_data_set::stamp_of;
    if stamp(&file.metadata().map_err(|e| e.to_string())?) != stamp(&before) {
        return Err("client module changed before pin".into());
    }
    let mut bytes = Vec::new();
    file.take(CLIENT_IMAGE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    let after = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    if bytes.len() as u64 != before.len()
        || stamp(&after) != stamp(&before)
        || after.file_type().is_symlink()
        || path.canonicalize().map_err(|e| e.to_string())? != canonical
    {
        return Err("client module changed during pin".into());
    }
    Ok(Some(ModuleInput {
        origin: AssetIdentity {
            source: canonical.display().to_string(),
            edition: "selected auxiliary client module".into(),
            version: format!("sha256 {:02x?}", sha256(&bytes)),
        },
        member: path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or("module filename encoding")?
            .into(),
        bytes: bytes.into(),
    }))
}

#[cfg(test)]
#[path = "client_module_tests.rs"]
mod tests;
