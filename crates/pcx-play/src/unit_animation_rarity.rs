//! Source-qualified explicit animation rarity selection, independent of script RNG.
use crate::{neutral_player::sha256, pe_image::PeSections, scripts::AssetIdentity};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Debug)]
pub struct Rules {
    pub origin: AssetIdentity,
    pub image_sha256: [u8; 32],
    rare: i32,
    sequence_limit: usize,
    rare_initial: f32,
    seed: u32,
    mask: u32,
    base: u32,
    one: f32,
    tags: BTreeMap<String, i32>,
    input_delimiters: String,
    model_delimiters: String,
    fallback: i32,
    token_limits: [usize; 2],
    pub(crate) props: String,
}
fn word(b: &[u8], i: usize) -> Result<u32, String> {
    b.get(i..i + 4)
        .and_then(|b| b.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| "animation rarity operand truncated".into())
}
fn shape(b: &[u8], i: usize, s: &[u8]) -> Result<(), String> {
    if b.get(i..i + s.len()) == Some(s) {
        Ok(())
    } else {
        Err(format!("animation rarity instruction shape at {i}"))
    }
}
fn call(b: &[u8], va: u32, i: usize) -> Result<u32, String> {
    shape(b, i, &[0xe8])?;
    Ok(va.wrapping_add(i as u32 + 5).wrapping_add(word(b, i + 1)?))
}
struct Reader<F> {
    read: F,
    left: usize,
}
impl<F: Fn(u32, usize) -> Result<Vec<u8>, String>> Reader<F> {
    fn bytes(&mut self, va: u32, n: usize) -> Result<Vec<u8>, String> {
        self.left = self
            .left
            .checked_sub(n)
            .ok_or("animation rarity reader budget")?;
        let b = (self.read)(va, n)?;
        if b.len() != n {
            return Err("animation rarity truncated window".into());
        }
        Ok(b)
    }
    fn text(&mut self, va: u32) -> Result<String, String> {
        let mut b = Vec::new();
        for i in 0..256 {
            let c = self.bytes(
                va.checked_add(i)
                    .ok_or("animation rarity pointer overflow")?,
                1,
            )?[0];
            if c == 0 {
                return String::from_utf8(b).map_err(|_| "animation rarity non-UTF8 text".into());
            }
            b.push(c)
        }
        Err("animation rarity text budget".into())
    }
}
impl Rules {
    pub fn read(image: &[u8], origin: AssetIdentity) -> Result<Self, String> {
        if image.len() > 32 << 20 || sha256(image) != crate::menu_lighting::REVIEWED_IMAGE {
            return Err("unsupported animation rarity client".into());
        }
        let pe = PeSections::parse(image)?;
        Self::decode(
            sha256(image),
            origin,
            |va, n| {
                pe.va_bytes(va, n)
                    .map(<[u8]>::to_vec)
                    .ok_or_else(|| format!("animation rarity window {va:#x}"))
            },
            64 << 10,
        )
    }
    fn decode(
        image_sha256: [u8; 32],
        origin: AssetIdentity,
        read: impl Fn(u32, usize) -> Result<Vec<u8>, String>,
        budget: usize,
    ) -> Result<Self, String> {
        if [&origin.source, &origin.edition, &origin.version]
            .iter()
            .any(|s| s.trim().is_empty())
        {
            return Err("animation rarity source identity".into());
        }
        let mut r = Reader { read, left: budget };
        let registration = r.bytes(0x49d632, 20)?;
        for i in [0, 5, 10] {
            shape(&registration, i, &[0x68])?;
        }
        if r.text(word(&registration, 6)?)? != "SetUnitAnimationWithRarity" {
            return Err("animation rarity public registration".into());
        }
        let register = call(&registration, 0x49d632, 15)?;
        let conversion = r.bytes(0x49b868, 20)?;
        for i in [0, 5, 10] {
            shape(&conversion, i, &[0x68])?;
        }
        if call(&conversion, 0x49b868, 15)? != register
            || r.text(word(&conversion, 6)?)? != "ConvertRarityControl"
        {
            return Err("animation rarity conversion registration".into());
        }
        let convert = r.bytes(word(&conversion, 11)?, 8)?;
        shape(&convert, 0, &[0x55, 0x8b, 0xec, 0x8b, 0x45, 8, 0x5d, 0xc3])?;
        let wrapper_va = word(&registration, 11)?;
        let wrapper = r.bytes(wrapper_va, 28)?;
        shape(&wrapper, 0, &[0x55, 0x8b, 0xec, 0x83, 0x6d, 0x10])?;
        shape(&wrapper, 7, &[0xb8])?;
        shape(&wrapper, 12, &[0x75, 5, 0xb8])?;
        shape(&wrapper, 19, &[0x89, 0x45, 0x10, 0x5d, 0xe9])?;
        let rare = i32::from(wrapper[6] as i8);
        let frequent_flag = word(&wrapper, 8)?;
        let rare_flag = word(&wrapper, 15)?;
        let common = wrapper_va
            .wrapping_add(28)
            .wrapping_add(word(&wrapper, 24)?);
        let dispatch = r.bytes(common + 0x10b, 21)?;
        shape(
            &dispatch,
            0,
            &[0xff, 0x75, 0x10, 0x57, 0x53, 0xff, 0x75, 0xe4, 0xe8],
        )?;
        if call(&dispatch, common + 0x10b, 8)? != 0x966c10 {
            return Err("animation rarity shared dispatch".into());
        }
        let filter = r.bytes(0x966da4, 20)?;
        shape(&filter, 0, &[0x8b, 0x45, 0x14, 0xa8])?;
        shape(&filter, 7, &[0x8b, 0x4b, 0x30, 0x83, 0xe0])?;
        if u32::from(filter[4]) != frequent_flag | rare_flag
            || u32::from(filter[12]) != frequent_flag
            || call(&filter, 0x966da4, 15)? != 0x982ff0
        {
            return Err("animation rarity mode/filter join".into());
        }
        let extrema = r.bytes(0x983013, 0x72)?;
        shape(
            &extrema,
            0,
            &[
                0x8b, 0x5d, 8, 0x85, 0xdb, 0x74, 5, 0x0f, 0x57, 0xc9, 0xeb, 8, 0xf3, 0x0f, 0x10,
                0x0d,
            ],
        )?;
        let rare_initial = f32::from_bits(word(&r.bytes(word(&extrema, 16)?, 4)?, 0)?);
        shape(
            &extrema,
            0x58,
            &[
                0x85, 0xdb, 0x74, 5, 0x0f, 0x2f, 0xc8, 0xeb, 3, 0x0f, 0x2f, 0xc1, 0x77, 3, 0x0f,
                0x28, 0xc8,
            ],
        )?;
        // SEQS import -> descriptor -> runtime rarity; offsets are format/ABI invariants.
        let load = r.bytes(0x8f9918, 10)?;
        shape(&load, 0, &[0xe8])?;
        shape(&load, 7, &[0xd9, 0x5e, 0x0c])?;
        let descriptor = r.bytes(0xcc2cd2, 18)?;
        shape(&descriptor, 0, &[0x8b, 0x41, 0x10, 0x89, 0x42, 0x10])?;
        let runtime = r.bytes(0x982bea, 10)?;
        shape(
            &runtime,
            0,
            &[0xf3, 0x0f, 0x10, 0x45, 0xd0, 0xf3, 0x0f, 0x11, 0x47, 0x18],
        )?;
        let initial = r.bytes(0x4140d0, 16)?;
        shape(&initial, 0, &[0x68])?;
        shape(&initial, 5, &[0xb9])?;
        shape(&initial, 15, &[0xc3])?;
        if call(&initial, 0x4140d0, 10)? != 0x9892a0 {
            return Err("animation rarity generator packing".into());
        }
        let choose = r.bytes(0x9839ea, 33)?;
        shape(&choose, 0, &[0x68])?;
        shape(&choose, 5, &[0xb9])?;
        if word(&choose, 1)? != word(&initial, 6)? || call(&choose, 0x9839ea, 28)? != 0x99ff10 {
            return Err("animation rarity independent stream join".into());
        }
        let real = r.bytes(0x99ff10, 0x50)?;
        if call(&real, 0x99ff10, 24)? != 0x480dd0 {
            return Err("animation rarity generator draw".into());
        }
        shape(&real, 0x20, &[0x25])?;
        shape(&real, 0x25, &[0x0d])?;
        shape(&real, 0x32, &[0xf3, 0x0f, 0x5c, 5])?;
        let mask = word(&real, 0x21)?;
        let base = word(&real, 0x26)?;
        let one = f32::from_bits(word(&r.bytes(word(&real, 0x36)?, 4)?, 0)?);
        if !rare_initial.is_finite()
            || !one.is_finite()
            || one <= 0.
            || f32::from_bits(base) != one
            || mask & base != 0
            || mask & !0x007f_ffff != 0
        {
            return Err("animation rarity real operands".into());
        }
        let index_gate = r.bytes(0xcc20cf, 6)?;
        shape(&index_gate, 0, &[0x81, 0xf9])?;
        let sequence_limit = usize::try_from(word(&index_gate, 2)?)
            .map_err(|_| "animation rarity sequence index limit")?;
        if sequence_limit == 0 {
            return Err("animation rarity empty sequence index limit".into());
        }
        let activation = r.bytes(0x964021, 7)?;
        shape(&activation, 0, &[0x52, 0x50])?;
        if call(&activation, 0x964021, 2)? != 0x983940 {
            return Err("animation rarity deferred chooser".into());
        }
        let equality = r.bytes(0x9830d0, 19)?;
        shape(
            &equality,
            0,
            &[0x0f, 0x2e, 0x45, 0x0c, 0x9f, 0xf6, 0xc4, 0x44, 0x7a],
        )?;
        let fallback_code = r.bytes(0x4ac682, 8)?;
        shape(&fallback_code, 0, &[0x6a])?;
        let fallback = i32::from(fallback_code[1] as i8);
        let props_code = r.bytes(0x5d5dd8, 5)?;
        shape(&props_code, 0, &[0x68])?;
        let props = r.text(word(&props_code, 1)?)?;
        let input_limit = r.bytes(0x4ac670, 2)?;
        shape(&input_limit, 0, &[0x6a])?;
        let model_limit = r.bytes(0x9774fd, 5)?;
        shape(&model_limit, 0, &[0x68])?;
        let token_limits = [
            usize::from(input_limit[1]),
            usize::try_from(word(&model_limit, 1)?).map_err(|_| "animation rarity token limit")?,
        ];
        if token_limits.contains(&0) {
            return Err("animation rarity empty token limit".into());
        }
        let input = r.bytes(0x4ac66b, 5)?;
        shape(&input, 0, &[0x68])?;
        let input_delimiters = r.text(word(&input, 1)?)?;
        let model = r.bytes(0x6dafe0, 5)?;
        shape(&model, 0, &[0x68])?;
        let model_delimiters = r.text(word(&model, 1)?)?;
        let bytes = r.bytes(0x6db0e0, 0x4b0)?;
        let mut i = 0;
        let mut tags = BTreeMap::new();
        let mut helper = None;
        let mut global = None;
        loop {
            match bytes.get(i..i + 2) {
                Some([0x83, 0xc4]) => {
                    i += 3;
                    continue;
                }
                Some([0xc3, ..]) => break,
                _ => {}
            }
            let (value, n) = match bytes.get(i) {
                Some(0x6a) => (i32::from(bytes[i + 1] as i8), 2),
                Some(0x68) => (word(&bytes, i + 1)? as i32, 5),
                _ => return Err("animation tag registration shape".into()),
            };
            i += n;
            shape(&bytes, i, &[0x68])?;
            let name = r.text(word(&bytes, i + 1)?)?;
            i += 5;
            shape(&bytes, i, &[0xff, 0x35])?;
            let slot = word(&bytes, i + 2)?;
            i += 6;
            let target = call(&bytes, 0x6db0e0, i)?;
            i += 5;
            if helper.replace(target).is_some_and(|old| old != target)
                || global.replace(slot).is_some_and(|old| old != slot)
                || name.is_empty()
                || !name.is_ascii()
                || tags.insert(name.to_ascii_lowercase(), value).is_some()
            {
                return Err("animation tag registration inconsistent".into());
            }
        }
        if tags.is_empty()
            || !tags.values().any(|v| *v == fallback)
            || props.is_empty()
            || input_delimiters.is_empty()
            || model_delimiters.is_empty()
        {
            return Err("animation rarity source catalog incomplete".into());
        }
        Ok(Self {
            origin,
            image_sha256,
            rare,
            sequence_limit,
            rare_initial,
            seed: word(&initial, 1)?,
            mask,
            base,
            one,
            tags,
            input_delimiters,
            model_delimiters,
            fallback,
            token_limits,
            props,
        })
    }
    fn tokens(&self, text: &str, model: bool) -> Result<Vec<i32>, String> {
        if !text.is_ascii() {
            return Err("animation rarity non-ASCII tag text is unsupported".into());
        }
        let delimiters = if model {
            &self.model_delimiters
        } else {
            &self.input_delimiters
        };
        let mut tags = Vec::new();
        for token in text
            .split(|c| delimiters.contains(c))
            .filter(|s| !s.is_empty())
        {
            if token.len() >= self.token_limits[usize::from(model)] {
                return Err("animation rarity source token budget".into());
            }
            let value = self.tags.get(&token.to_ascii_lowercase()).copied();
            if model && value.is_none() {
                break;
            }
            tags.push(value.unwrap_or(self.fallback));
        }
        if model {
            tags.sort_unstable_by_key(|n| *n as u32);
        }
        Ok(tags)
    }
    pub(crate) fn candidates(
        &self,
        model: &crate::unit_model::Model,
        name: &str,
        props: &[String],
        rarity: i32,
    ) -> Result<Vec<usize>, String> {
        if model.sequences.len() > self.sequence_limit {
            return Err("animation rarity source sequence index budget".into());
        }
        let mut wanted = self.tokens(name, false)?;
        for prop in props {
            wanted.extend(self.tokens(prop, false)?);
        }
        if wanted.is_empty() {
            return Ok(Vec::new());
        }
        if wanted.len() > u32::BITS as usize {
            return Err("animation rarity tag mask budget".into());
        }
        let mut selected = Vec::new();
        let mut best = (0u32, usize::MAX);
        let mut signatures = Vec::new();
        for (index, sequence) in model.sequences.iter().enumerate() {
            let tags = self.tokens(&sequence.name, true)?;
            let mut score = 0;
            let mut extra = 0;
            for tag in &tags {
                let mut matched = false;
                for (n, want) in wanted.iter().enumerate() {
                    if want == tag {
                        score |= 1 << (wanted.len() - n - 1);
                        matched = true;
                    }
                }
                if !matched && *tag >= 0 {
                    extra += 1;
                }
            }
            if score > best.0 || (score == best.0 && extra < best.1) {
                best = (score, extra);
                selected.clear();
                signatures.clear();
            }
            if (score, extra) == best {
                selected.push(index);
                signatures.push(tags.into_iter().filter(|n| *n >= 0).collect::<Vec<_>>());
            }
        }
        if best.0 & (1 << (wanted.len() - 1)) == 0 {
            return Ok(Vec::new());
        }
        let Some(first) = signatures.first().cloned() else {
            return Ok(selected);
        };
        let mut k = 1;
        while k < selected.len() {
            if signatures[k] != first {
                selected.swap_remove(k);
                signatures.swap_remove(k);
            } else {
                k += 1
            }
        }
        let mut extremum = if rarity == self.rare {
            self.rare_initial
        } else {
            0.
        };
        for index in &selected {
            let n = model.sequences[*index].rarity();
            if !n.is_finite() || n < 0. {
                return Err("animation rarity requires finite nonnegative SEQS weights".into());
            }
            extremum = if rarity == self.rare {
                extremum.min(n)
            } else {
                extremum.max(n)
            };
        }
        let mut k = 0;
        while k < selected.len() {
            if model.sequences[selected[k]].rarity() != extremum {
                selected.swap_remove(k);
            } else {
                k += 1
            }
        }
        Ok(selected)
    }
}

pub(crate) fn properties(
    definition: &pcx_sim::world::Definition,
    key: &str,
) -> Result<Vec<String>, String> {
    use pcx_objects::editor::{FieldKind, FieldSource, SlkId};
    use pcx_sim::world::{FieldProblem, FieldValue};
    let mut fields=definition.fields.values().filter(|f|f.metadata.as_ref().is_some_and(|m|matches!(&m.source,FieldSource::Profile{key:actual,..} if actual.eq_ignore_ascii_case(key))));
    let field = fields
        .next()
        .ok_or("animation properties metadata absent")?;
    let m = field.metadata.as_ref().expect("matched");
    if fields.next().is_some()
        || m.from != SlkId::UnitMetaData
        || !m.field.eq_ignore_ascii_case(key)
        || m.kind != FieldKind::Token
        || m.declared != "stringList"
        || m.index != -1
        || !matches!(
            &m.source,
            FieldSource::Profile {
                index: -1,
                slots: 1,
                ..
            }
        )
    {
        return Err("animation properties metadata mapping differs".into());
    }
    let text = match &field.value {
        Err(FieldProblem::Missing) => return Ok(Vec::new()),
        Ok(FieldValue::Cell(s)) => s.as_str(),
        Ok(FieldValue::Override(pcx_objects::Value::Str(s))) => {
            std::str::from_utf8(s).map_err(|_| "animation properties UTF-8")?
        }
        _ => return Err("animation properties value is malformed".into()),
    };
    if text.len() > 4096 {
        return Err("animation properties text budget".into());
    }
    if matches!(text.trim(), "" | "_" | "-") {
        return Ok(Vec::new());
    }
    Ok(text.split(',').map(|s| s.trim().to_owned()).collect())
}

#[derive(Clone)]
pub(crate) struct State {
    pub rules: Arc<Rules>,
    policy: Arc<crate::script_random::profile::Policy>,
    stream: crate::script_random::state::Generator,
}
impl State {
    pub fn new(
        rules: Rules,
        policy: Arc<crate::script_random::profile::Policy>,
    ) -> Result<Self, String> {
        if &rules.image_sha256 != policy.image_sha256() {
            return Err("animation rarity random/source identity differs".into());
        }
        let stream = crate::script_random::state::Generator::new(&policy, rules.seed);
        Ok(Self {
            rules: Arc::new(rules),
            policy,
            stream,
        })
    }
    pub fn choose(&mut self, weights: &[f32]) -> Result<usize, String> {
        let mut total = 0f32;
        for n in weights {
            if !n.is_finite() || *n < 0. {
                return Err("invalid animation rarity weight".into());
            }
            total += n;
        }
        if weights.is_empty() || !total.is_finite() {
            return Err("animation rarity empty/overflow weight sum".into());
        }
        let word = self.stream.next(&self.policy).map_err(|e| e.to_string())?;
        let mut remaining =
            (f32::from_bits((word & self.rules.mask) | self.rules.base) - self.rules.one) * total;
        for (i, weight) in weights.iter().enumerate() {
            if *weight > remaining {
                return Ok(i);
            }
            remaining -= weight;
        }
        Ok(weights.len() - 1)
    }
}
#[cfg(test)]
#[path = "unit_animation_rarity_tests.rs"]
pub(crate) mod tests;
