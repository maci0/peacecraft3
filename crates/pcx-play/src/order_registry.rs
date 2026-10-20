//! Order names and identifiers from one caller-reviewed client initializer.
use crate::{
    camera_margins::SourceWindow,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    scripts::AssetIdentity,
};

#[derive(Clone, Debug)]
pub struct Binding {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    pub initializer: SourceWindow,
    pub bootstrap: SourceWindow,
    pub direct_register: u32,
    pub register: u32,
}

#[derive(Debug)]
pub struct Registry {
    binding: Binding,
    entries: Vec<(String, i32)>,
}

impl Registry {
    /// Decode the reviewed x86 initializer: saved ESI, one runtime pointer,
    /// repeated name/id pushes to one registration function, then restored ESI.
    /// Its caller first registers one name with a file-backed identifier operand.
    /// Strings must be bounded ASCII in initialized sections, never BSS.
    pub fn read(
        image: &[u8],
        binding: Binding,
        budget: RuleInputBudget,
        max_entries: usize,
        name_bytes: usize,
    ) -> Result<Self, String> {
        let fail = |reason: &str| format!("order registry {}: {reason}", binding.origin.source);
        if image.len() > budget.image_bytes
            || (binding.initializer.len as usize)
                .checked_add(binding.bootstrap.len as usize)
                .is_none_or(|n| n > budget.profile_bytes)
            || max_entries == 0
            || name_bytes == 0
            || name_bytes > budget.profile_bytes
        {
            return Err(fail("input budget"));
        }
        if [
            &binding.origin.source,
            &binding.origin.edition,
            &binding.origin.version,
        ]
        .iter()
        .any(|s| s.trim().is_empty())
            || sha256(image) != binding.image_sha256
        {
            return Err(fail("missing identity or image fingerprint mismatch"));
        }
        let pe = Pe::parse(image, budget).map_err(|e| fail(&e.to_string()))?;
        pe.read_from(binding.register, true)
            .map_err(|e| fail(&e.to_string()))?;
        pe.read_from(binding.direct_register, true)
            .map_err(|e| fail(&e.to_string()))?;
        let bootstrap = pe
            .window(&binding.bootstrap, true)
            .map_err(|e| fail(&e.to_string()))?;
        if bootstrap.len() != 21
            || bootstrap[0] != 0x68
            || bootstrap[5..7] != [0xff, 0x35]
            || bootstrap[11] != 0xe8
            || bootstrap[16] != 0xe8
        {
            return Err(fail("unsupported bootstrap registration"));
        }
        for (at, target) in [(11, binding.direct_register), (16, binding.initializer.va)] {
            let next = i64::from(binding.bootstrap.va) + at as i64 + 5;
            if next
                + i64::from(i32::from_le_bytes(
                    bootstrap[at + 1..at + 5].try_into().unwrap(),
                ))
                != i64::from(target)
            {
                return Err(fail("bootstrap call target differs"));
            }
        }
        let code = pe
            .window(&binding.initializer, true)
            .map_err(|e| fail(&e.to_string()))?;
        if code.len() < 26 || code[..3] != [0x56, 0x8b, 0x35] || code[7..9] != [0x8b, 0xce] {
            return Err(fail("unsupported initializer header"));
        }
        let mut entries: Vec<(String, i32)> = Vec::new();
        let mut records = 0;
        let mut text_bytes = 0usize;
        let mut insert = |address: u32, id: i32| -> Result<(), String> {
            if records >= max_entries {
                return Err(fail("registration count budget"));
            }
            let bytes = pe
                .read_from(address, false)
                .map_err(|e| fail(&e.to_string()))?;
            let text = &bytes[..bytes.len().min(name_bytes)];
            let end = text
                .iter()
                .position(|b| *b == 0)
                .ok_or_else(|| fail("unterminated order name"))?;
            if end == 0 || !text[..end].is_ascii() {
                return Err(fail("empty or unsupported order name"));
            }
            text_bytes = text_bytes
                .checked_add(end + 1)
                .ok_or_else(|| fail("name storage budget"))?;
            if text_bytes > budget.profile_bytes {
                return Err(fail("name storage budget"));
            }
            let name = std::str::from_utf8(&text[..end]).unwrap();
            if let Some((old_name, old_id)) = entries
                .iter()
                .find(|(s, n)| s.eq_ignore_ascii_case(name) || *n == id)
            {
                if old_name != name || *old_id != id {
                    return Err(fail("conflicting order name or identifier"));
                }
            } else {
                entries
                    .try_reserve(1)
                    .map_err(|_| fail("registration storage"))?;
                let mut stored = String::new();
                stored
                    .try_reserve_exact(end)
                    .map_err(|_| fail("name storage"))?;
                stored.push_str(name);
                entries.push((stored, id));
            }
            records += 1;
            Ok(())
        };
        let address = u32::from_le_bytes(bootstrap[1..5].try_into().unwrap());
        let id_address = u32::from_le_bytes(bootstrap[7..11].try_into().unwrap());
        let id = pe
            .read_from(id_address, false)
            .map_err(|e| fail(&e.to_string()))?
            .get(..4)
            .ok_or_else(|| fail("bootstrap identifier range"))?;
        insert(address, i32::from_le_bytes(id.try_into().unwrap()))?;
        let mut at = 9usize;
        while code.get(at..) != Some(&[0x5e, 0xc3]) {
            let len = if at == 9 { 15 } else { 17 };
            let b = code
                .get(at..at.checked_add(len).ok_or_else(|| fail("record range"))?)
                .ok_or_else(|| fail("truncated registration or missing return"))?;
            if b[0] != 0x68
                || b[5] != 0x68
                || (at != 9 && b[10..12] != [0x8b, 0xce])
                || b[len - 5] != 0xe8
            {
                return Err(fail("unsupported registration shape"));
            }
            let next = i64::from(binding.initializer.va) + at as i64 + len as i64;
            if next + i64::from(i32::from_le_bytes(b[len - 4..].try_into().unwrap()))
                != i64::from(binding.register)
            {
                return Err(fail("registration call target differs"));
            }
            insert(
                u32::from_le_bytes(b[1..5].try_into().unwrap()),
                i32::from_le_bytes(b[6..10].try_into().unwrap()),
            )?;
            at += len;
        }
        Ok(Self { binding, entries })
    }

    pub fn binding(&self) -> &Binding {
        &self.binding
    }

    /// Native ASCII comparison ignores case and stops at the first NUL.
    pub fn id(&self, name: &str) -> Option<i32> {
        let name = name.split('\0').next().unwrap_or("");
        self.entries
            .iter()
            .find(|(s, _)| s.eq_ignore_ascii_case(name))
            .map(|(_, n)| *n)
    }

    pub fn name(&self, id: i32) -> Option<&str> {
        self.entries
            .iter()
            .find(|(_, n)| *n == id)
            .map(|(s, _)| s.as_str())
    }
}

#[cfg(test)]
#[path = "order_registry_tests.rs"]
pub(crate) mod tests;
