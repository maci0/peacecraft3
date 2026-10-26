//! Construction inputs derived from one explicitly selected client and ordered profiles.
use crate::{
    camera_margins::SourceWindow,
    neutral_player::{DiscoveryBudget, Sha256Digest, sha256},
    pe_image::{PeSections, RDATA, TEXT, u16_at, u32_at},
    scripts::AssetIdentity,
};

#[derive(Clone, Debug)]
pub struct Rules {
    pub origin: AssetIdentity,
    pub common_origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    pub common_sha256: Sha256Digest,
    pub profile_sources: Vec<String>,
    pub initial_life_fraction: f32,
    pub human_build_ability: [u8; 4],
    pub refund_rate: f32,
    pub damage_refund_penalty: bool,
    /// Retained source setting; the qualified Human stop observation does not apply it.
    pub life_drain_rate: f32,
}

struct Reader<'a> {
    pe: PeSections<'a>,
    remaining: usize,
    candidates: usize,
}

impl<'a> Reader<'a> {
    fn bytes(&self, va: u32, len: usize, code: bool) -> Result<&'a [u8], String> {
        let section = self.pe.section(if code { TEXT } else { RDATA })?;
        let Some(section) = section else {
            return Err("missing construction section".into());
        };
        if code {
            let start = va
                .checked_sub(
                    self.pe
                        .image_base
                        .checked_add(section.rva)
                        .ok_or("section VA overflow")?,
                )
                .ok_or("construction code outside text")? as usize;
            return section
                .bytes
                .get(start..start.checked_add(len).ok_or("code range overflow")?)
                .ok_or_else(|| "truncated construction code".into());
        }
        self.pe
            .va_bytes(va, len)
            .ok_or_else(|| "unmapped construction data".into())
    }

    fn word(&self, va: u32, code: bool) -> Result<u32, String> {
        Ok(u32::from_le_bytes(
            self.bytes(va, 4, code)?
                .try_into()
                .map_err(|_| "construction word")?,
        ))
    }

    fn find(&mut self, needle: &[u8], code: bool) -> Result<Vec<u32>, String> {
        let section = self
            .pe
            .section(if code { TEXT } else { RDATA })?
            .ok_or("missing construction scan section")?;
        self.remaining = self
            .remaining
            .checked_sub(section.bytes.len())
            .ok_or("construction scan work budget")?;
        let base = self
            .pe
            .image_base
            .checked_add(section.rva)
            .ok_or("scan VA overflow")?;
        let mut found = Vec::new();
        for (offset, window) in section.bytes.windows(needle.len()).enumerate() {
            if window == needle {
                self.candidates = self
                    .candidates
                    .checked_sub(1)
                    .ok_or("construction candidate budget")?;
                found
                    .try_reserve(1)
                    .map_err(|_| "construction candidate allocation")?;
                found.push(
                    base.checked_add(u32::try_from(offset).map_err(|_| "scan offset width")?)
                        .ok_or("scan VA overflow")?,
                );
            }
        }
        Ok(found)
    }

    fn unique(&mut self, needle: &[u8], code: bool) -> Result<u32, String> {
        let matches = self.find(needle, code)?;
        match matches.as_slice() {
            [va] => Ok(*va),
            _ => Err("missing or ambiguous construction anchor".into()),
        }
    }

    fn call(&self, va: u32) -> Result<u32, String> {
        let bytes = self.bytes(va, 5, true)?;
        if bytes[0] != 0xe8 {
            return Err("construction relative call differs".into());
        }
        let delta = i32::from_le_bytes(bytes[1..].try_into().map_err(|_| "call width")?);
        u32::try_from(i64::from(va) + 5 + i64::from(delta))
            .map_err(|_| "construction call overflow".into())
    }

    fn native(&mut self, name: &[u8], prefix: &[u8]) -> Result<u32, String> {
        let name = self.unique(name, false)?;
        let push = [vec![0x68], name.to_le_bytes().to_vec()].concat();
        let mut matched = None;
        for va in self.find(&push, true)? {
            let code = self.bytes(va, 15, true)?;
            if code[5] != 0x68 || code[10] != 0xe8 {
                continue;
            }
            let function = self.word(va + 6, true)?;
            if self.bytes(function, prefix.len(), true)? != prefix {
                continue;
            }
            if matched.replace(function).is_some() {
                return Err("ambiguous construction native".into());
            }
        }
        matched.ok_or_else(|| "missing supported construction native".into())
    }
}

impl Rules {
    pub fn read(
        image: &[u8],
        origin: AssetIdentity,
        common: &[u8],
        common_origin: AssetIdentity,
        profiles: &[(&str, &[u8])],
        multiplication: SourceWindow,
        budget: DiscoveryBudget,
    ) -> Result<Self, String> {
        let result = (|| {
            if image.len() > budget.image.image_bytes || common.len() > budget.image.profile_bytes {
                return Err("construction image/common budget".into());
            }
            if [&origin, &common_origin].iter().any(|identity| {
                identity.source.is_empty()
                    || identity.edition.is_empty()
                    || identity.version.is_empty()
            }) {
                return Err("construction source edition/version is missing".into());
            }
            let offset = u32_at(image, 0x3c).ok_or("construction PE header")? as usize;
            if u16_at(image, offset.checked_add(4).ok_or("PE header overflow")?) != Some(0x14c)
                || usize::from(u16_at(image, offset + 6).ok_or("PE section count")?)
                    > budget.image.sections
            {
                return Err("construction PE machine/section budget".into());
            }
            let pe = PeSections::parse(image)?;
            let mut read = Reader {
                pe,
                remaining: budget.work_bytes,
                candidates: budget.registration_candidates,
            };
            if sha256(read.bytes(multiplication.va, multiplication.len as usize, true)?)
                != multiplication.digest
            {
                return Err("construction numeric multiplication window differs".into());
            }
            let fraction = initial_fraction(&mut read, common, multiplication.va)?;
            let human_build_ability = human_build_class(&mut read)?;
            let ([refund, penalty, drain], profile_sources) =
                pcx_objects::editor::misc::misc_scalars(
                    profiles,
                    [
                        "ConstructionRefundRate",
                        "ConstructionDamageRefundPenalty",
                        "ConstructionLifeDrainRate",
                    ],
                    budget
                        .image
                        .profile_bytes
                        .checked_sub(common.len())
                        .ok_or("construction profile budget")?,
                )?;
            let refund_rate = refund.ok_or("missing Misc/ConstructionRefundRate")?;
            let life_drain_rate = drain.ok_or("missing Misc/ConstructionLifeDrainRate")?;
            let damage_refund_penalty = match penalty {
                Some(0.0) => false,
                Some(1.0) => true,
                _ => return Err("ConstructionDamageRefundPenalty must be boolean".into()),
            };
            if refund_rate < 0.0 || life_drain_rate < 0.0 {
                return Err("negative construction refund/drain".into());
            }
            Ok(Self {
                image_sha256: sha256(image),
                common_sha256: sha256(common),
                origin: origin.clone(),
                common_origin,
                profile_sources,
                initial_life_fraction: fraction,
                human_build_ability,
                refund_rate,
                damage_refund_penalty,
                life_drain_rate,
            })
        })();
        result.map_err(|error: String| format!("{}: construction rules: {error}", origin.source))
    }
}

fn human_build_class(read: &mut Reader<'_>) -> Result<[u8; 4], String> {
    let name = read.unique(b"CAbilityHumanBuild\0", false)?;
    let getter = [vec![0xb8], name.to_le_bytes().to_vec(), vec![0xc3]].concat();
    let getter = read.unique(&getter, true)?;
    let pointer = read.unique(&getter.to_le_bytes(), false)?;
    let table = pointer
        .checked_sub(0x58)
        .ok_or("HumanBuild vtable underflow")?;
    let code_getter = read.word(
        table
            .checked_add(0x1c)
            .ok_or("HumanBuild code slot overflow")?,
        false,
    )?;
    let code = read.bytes(code_getter, 6, true)?;
    if code[0] != 0xb8 || code[5] != 0xc3 {
        return Err("HumanBuild virtual code getter differs".into());
    }
    let code = u32::from_le_bytes(code[1..5].try_into().map_err(|_| "HumanBuild code width")?)
        .to_be_bytes();
    if !code.iter().all(u8::is_ascii_graphic) {
        return Err("HumanBuild code is not a rawcode".into());
    }
    let constructor = [vec![0xc7, 0x06], table.to_le_bytes().to_vec()].concat();
    read.unique(&constructor, true)?;
    Ok(code)
}

fn initial_fraction(
    read: &mut Reader<'_>,
    common: &[u8],
    multiplication: u32,
) -> Result<f32, String> {
    let common = std::str::from_utf8(common).map_err(|_| "construction common is not UTF-8")?;
    let mut states = common.lines().filter_map(|line| {
        let words: Vec<_> = line.split_whitespace().collect();
        if words.get(..4) == Some(&["constant", "unitstate", "UNIT_STATE_MAX_LIFE", "="][..]) {
            Some(words[4..].join(""))
        } else {
            None
        }
    });
    let state = states.next().ok_or("missing source maximum-life state")?;
    if states.next().is_some() {
        return Err("ambiguous maximum-life state".into());
    }
    let state: usize = state
        .strip_prefix("ConvertUnitState(")
        .and_then(|s| s.strip_suffix(')'))
        .ok_or("maximum-life state shape")?
        .parse()
        .map_err(|_| "maximum-life state integer")?;
    let getter = read.native(
        b"GetUnitState\0",
        &[0x55, 0x8b, 0xec, 0x56, 0x8b, 0x75, 0x0c],
    )?;
    let getter_bytes = read.bytes(getter, 52, true)?;
    if getter_bytes[7..9] != [0x83, 0xfe] {
        return Err("unit-state bounds shape".into());
    }
    if state >= usize::from(getter_bytes[9]) {
        return Err("maximum-life state outside native bounds".into());
    }
    let dispatch = read.call(getter + 47)?;
    let d = read.bytes(dispatch, 19, true)?;
    if d[..8] != [0x55, 0x8b, 0xec, 0x8b, 0x45, 0x0c, 0x56, 0x83] || d[12..15] != [0xff, 0x24, 0x85]
    {
        return Err("unit-state dispatch shape".into());
    }
    let table = u32::from_le_bytes(d[15..19].try_into().map_err(|_| "state table width")?);
    let branch = read.word(
        table
            .checked_add(
                u32::try_from(state.checked_mul(4).ok_or("state table overflow")?)
                    .map_err(|_| "state width")?,
            )
            .ok_or("state table VA overflow")?,
        true,
    )?;
    let branch = read.bytes(branch, 12, true)?;
    if branch[..8] != [0x8b, 0x11, 0x8b, 0x75, 0x08, 0x56, 0xff, 0x92] {
        return Err("maximum-life getter shape".into());
    }
    let max_slot = u32::from_le_bytes(branch[8..12].try_into().map_err(|_| "maximum-life slot")?);
    let setter = read.native(
        b"SetWidgetLife\0",
        &[0x55, 0x8b, 0xec, 0xff, 0x75, 0x08, 0xe8],
    )?;
    let s = read.bytes(setter, 31, true)?;
    if s[18..25] != [0x8b, 0x10, 0x8b, 0xc8, 0xff, 0x75, 0x0c] || s[25..27] != [0xff, 0x92] {
        return Err("widget-life setter shape".into());
    }
    let life_slot = u32::from_le_bytes(s[27..31].try_into().map_err(|_| "life setter slot")?);
    let progress = read.native(
        b"UnitSetConstructionProgress\0",
        &[0x55, 0x8b, 0xec, 0x56, 0xff, 0x75, 0x08, 0xe8],
    )?;
    let unit_method = read.call(progress + 44)?;
    let unit = read.bytes(unit_method, 192, true)?;
    let compare = one_window(unit, &[0x3d], 5, |w| {
        w[1..5].iter().all(u8::is_ascii_graphic)
    })?;
    let ability_code = &compare[1..5];
    let getters = read.find(
        &[vec![0xb8], ability_code.to_vec(), vec![0xc3]].concat(),
        true,
    )?;
    let mut pointer = None;
    for getter in getters {
        for candidate in read.find(&getter.to_le_bytes(), false)? {
            if pointer.replace(candidate).is_some() {
                return Err("ambiguous construction ability vtable".into());
            }
        }
    }
    // A static type-code helper also returns ABnP; only the virtual getter belongs to this table.
    let pointer = pointer.ok_or("missing construction ability vtable")?;
    // These virtual offsets are independently reviewed x86 class ABI roles, not game values.
    let vtable = pointer
        .checked_sub(0x1c)
        .ok_or("ability vtable underflow")?;
    let name_getter = read.word(
        vtable.checked_add(0x58).ok_or("class-name slot overflow")?,
        false,
    )?;
    let name = read.bytes(name_getter, 6, true)?;
    if name[0] != 0xb8 || name[5] != 0xc3 {
        return Err("construction class-name getter".into());
    }
    let name = u32::from_le_bytes(name[1..5].try_into().map_err(|_| "class name address")?);
    if read.bytes(name, b"CAbilityBuildInProgress\0".len(), false)? != b"CAbilityBuildInProgress\0"
    {
        return Err("construction ability class differs".into());
    }
    let constructor = [vec![0xc7, 0x06], vtable.to_le_bytes().to_vec()].concat();
    read.unique(&constructor, true)?;
    let init_va = read.word(
        vtable
            .checked_add(0x358)
            .ok_or("initialization slot overflow")?,
        false,
    )?;
    let init = read.bytes(init_va, 128, true)?;
    let max = [vec![0xff, 0x92], max_slot.to_le_bytes().to_vec()].concat();
    let life = [vec![0xff, 0x96], life_slot.to_le_bytes().to_vec()].concat();
    let at = one_position(init, &max)?;
    let end = one_position(init, &life)?;
    if at < 8 || end != at + 22 || init[at - 8] != 0x68 || init[at - 3..at] != [0x51, 0x8b, 0xc8] {
        return Err("construction initial-life data flow differs".into());
    }
    if init.get(at + 6..at + 11) != Some(&[0x50, 0x8d, 0x45, 0x08, 0x50][..])
        || init.get(at + 16..at + 22) != Some(&[0x83, 0xc4, 0x0c, 0x8b, 0xcb, 0x50][..])
        || read.call(
            init_va
                .checked_add(u32::try_from(at + 11).map_err(|_| "initial math offset")?)
                .ok_or("initial math VA")?,
        )? != multiplication
    {
        return Err("construction initial life does not use admitted multiplication".into());
    }
    let fraction_va = u32::from_le_bytes(
        init[at - 7..at - 3]
            .try_into()
            .map_err(|_| "initial fraction address")?,
    );
    let fraction = f32::from_bits(read.word(fraction_va, false)?);
    if !fraction.is_finite() || fraction <= 0.0 || fraction >= 1.0 {
        return Err("construction initial fraction range".into());
    }
    Ok(fraction)
}

fn one_position(bytes: &[u8], needle: &[u8]) -> Result<usize, String> {
    let mut found = bytes
        .windows(needle.len())
        .enumerate()
        .filter_map(|(i, w)| (w == needle).then_some(i));
    let at = found.next().ok_or("missing construction instruction")?;
    if found.next().is_some() {
        return Err("ambiguous construction instruction".into());
    }
    Ok(at)
}

fn one_window<'a>(
    bytes: &'a [u8],
    prefix: &[u8],
    len: usize,
    accept: impl Fn(&[u8]) -> bool,
) -> Result<&'a [u8], String> {
    let mut found = bytes
        .windows(len)
        .filter(|w| w.starts_with(prefix) && accept(w));
    let first = found.next().ok_or("missing construction relationship")?;
    if found.next().is_some() {
        return Err("ambiguous construction relationship".into());
    }
    Ok(first)
}

#[cfg(test)]
#[path = "construction_rules_tests.rs"]
mod tests;
