//! Opt-in encoding discovery for the existing selected-W3I first-family contract.
use crate::neutral_player::{
    self as n, DiscoveryBudget, NeutralError, RuleProblem, VerifiedNeutralRule, sha256,
};
use crate::neutral_rule_input::{HEADER_FIRST_BYTE, Pe, decode_getter, decode_header};
use crate::pe_image::windows_with_byte;
use crate::scripts::AssetIdentity;

pub(crate) const NAMES: [&str; 6] = [
    n::NATIVE_NAME,
    n::AGGRESSIVE_NATIVE_NAME,
    n::BJ_MAX_PLAYERS_NATIVE_NAME,
    n::BJ_NEUTRAL_VICTIM_NATIVE_NAME,
    n::BJ_NEUTRAL_EXTRA_NATIVE_NAME,
    n::BJ_MAX_PLAYER_SLOTS_NATIVE_NAME,
];
#[derive(Clone, Copy)]
struct Getter {
    registration: u32,
    address: u32,
    values: (u32, i32, i32),
}
fn fail(problem: RuleProblem) -> NeutralError {
    NeutralError::Rule(problem)
}
fn add(work: &mut usize, bytes: usize, limit: usize) -> Result<(), NeutralError> {
    *work = work
        .checked_add(bytes)
        .filter(|&n| n <= limit)
        .ok_or_else(|| fail(RuleProblem::Budget))?;
    Ok(())
}
fn dword(bytes: &[u8]) -> u32 {
    u32::from_le_bytes(bytes.try_into().expect("checked dword width"))
}

pub(crate) fn discover(
    image: &[u8],
    origin: AssetIdentity,
    budget: DiscoveryBudget,
) -> Result<[VerifiedNeutralRule; 6], NeutralError> {
    if image.len() > budget.image.image_bytes
        || budget.registration_candidates == 0
        || budget.work_bytes == 0
    {
        return Err(fail(RuleProblem::Budget));
    }
    let mut metadata = 0;
    for value in [&origin.edition, &origin.version, &origin.source] {
        if value.trim().is_empty() {
            return Err(fail(RuleProblem::ImageIdentity));
        }
        add(&mut metadata, value.len(), budget.image.profile_bytes)?;
    }
    metadata
        .checked_mul(NAMES.len())
        .filter(|&n| n <= budget.image.profile_bytes)
        .ok_or_else(|| fail(RuleProblem::Budget))?;
    let pe = Pe::parse(image, budget.image)?;
    let mut work = 0;
    let mut candidates = 0usize;
    let mut header = None;
    let mut getters: [Option<Getter>; 6] = [None; 6];
    for (va, code) in pe.code_ranges() {
        add(&mut work, code.len(), budget.work_bytes)?;
        add(&mut work, code.len(), budget.work_bytes)?;
        for (offset, bytes) in windows_with_byte(code, 0, HEADER_FIRST_BYTE, 36) {
            if let Ok(bounds) = decode_header(bytes) {
                let address = va
                    .checked_add(u32::try_from(offset).map_err(|_| fail(RuleProblem::Budget))?)
                    .ok_or_else(|| fail(RuleProblem::Range))?;
                if header.replace((address, bounds, sha256(bytes))).is_some() {
                    return Err(fail(RuleProblem::HeaderShape));
                }
            }
        }
        for (offset, bytes) in windows_with_byte(code, 0, 0x68, 15) {
            if bytes[5] != 0x68 || bytes[10] != 0x68 {
                continue;
            }
            candidates = candidates
                .checked_add(1)
                .filter(|&n| n <= budget.registration_candidates)
                .ok_or_else(|| fail(RuleProblem::Budget))?;
            let (sig, name, getter) = (
                dword(&bytes[1..5]),
                dword(&bytes[6..10]),
                dword(&bytes[11..15]),
            );
            let Ok(signature) = pe.read_from(sig, false) else {
                continue;
            };
            if !signature.starts_with(b"()I\0") {
                continue;
            }
            let Ok(text) = pe.read_from(name, false) else {
                continue;
            };
            for (index, native) in NAMES.iter().enumerate() {
                if text.get(..native.len()) != Some(native.as_bytes())
                    || text.get(native.len()) != Some(&0)
                {
                    continue;
                }
                add(&mut work, native.len() + 1 + 39, budget.work_bytes)?;
                let Some(body) = pe.read_from(getter, true).ok().and_then(|b| b.get(..39)) else {
                    continue;
                };
                let Ok(values) = decode_getter(body) else {
                    continue;
                };
                let registration = va
                    .checked_add(u32::try_from(offset).map_err(|_| fail(RuleProblem::Budget))?)
                    .ok_or_else(|| fail(RuleProblem::Range))?;
                if getters[index]
                    .replace(Getter {
                        registration,
                        address: getter,
                        values,
                    })
                    .is_some()
                {
                    return Err(fail(RuleProblem::Binding));
                }
            }
        }
    }
    let (header_va, (min_tag, max_tag), header_hash) =
        header.ok_or_else(|| fail(RuleProblem::HeaderShape))?;
    let image_sha256 = sha256(image);
    let mut rules = Vec::with_capacity(NAMES.len());
    for (native, found) in NAMES.into_iter().zip(getters) {
        let Getter {
            registration,
            address: getter,
            values: (threshold, below, at_or_above),
        } = found.ok_or_else(|| fail(RuleProblem::Binding))?;
        // A receipt describes discovered roles; it does not approve an imported profile.
        let mut receipt = b"PCX neutral first-family discovery\0".to_vec();
        receipt.extend(image_sha256);
        receipt.extend(native.as_bytes());
        for value in [&origin.edition, &origin.version, &origin.source] {
            receipt.extend((value.len() as u64).to_le_bytes());
            receipt.extend(value.as_bytes());
        }
        for address in [registration, getter, header_va] {
            receipt.extend(address.to_le_bytes());
        }
        receipt.extend(header_hash);
        rules.push(VerifiedNeutralRule {
            native_name: native,
            origin: origin.clone(),
            image_sha256,
            profile_sha256: sha256(&receipt),
            threshold,
            below,
            at_or_above,
            min_tag,
            max_tag,
        });
    }
    rules.try_into().map_err(|_| fail(RuleProblem::Binding))
}
