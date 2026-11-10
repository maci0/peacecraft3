//! `war3mapImported\<Name>.wai`: the World Editor's compiled AI script data.
//!
//! The editor writes one `.wai` per AI script next to its generated `.ai`
//! source, under the `war3mapImported\` prefix. No shipped map carries a member
//! named `war3map.wai`. The module takes the member bytes and the member name is
//! the caller's concern.
//!
//! # Evidence
//!
//! Ten members were read and parsed to their tail:
//!
//! * eight in `FrozenThrone/Scenario/(4)Monolith.w3x` (`Beast`, `Furbolg`,
//!   `Lizard`, `Murloc`, `Satyr`, `Spider`, `Troll`, `Wolf`), byte-identical in
//!   the 1.28c and Legacy installs;
//! * `Snipers beta.wai` in `Diverse/(12) Elite Snipers II 1.0 Final.w3x`;
//! * `TinyLittleMind.wai` in `Diverse/The Seige Of Tzosh.w3x`.
//!
//! All ten have version 2. The eight Monolith files hold four conditions each;
//! the two others hold none.
//!
//! # Layout
//!
//! ```text
//! offset  size  field
//! 0x00    4     version                    2 in all ten files
//!         var   name, NUL-terminated       "Wolf AI", "Snipers AI", "TinyLittleMind"
//!         4     word at name end           raw: 0 (Monolith), 1 (Snipers), 0 (TinyLittleMind)
//!         4     word at name end + 4       raw: 0x19ff (Monolith), 0x1a84b, 0x1394e
//!         4     F    fourcc count          4 in all ten
//!         4F    fourcc list                raw unit codes
//!         4     C    condition count       4 (Monolith), 0 (the two others)
//!         4     word after the count       raw: 7 in all ten
//!         var   C    conditions
//!         var   tail to end of file       raw, not decoded
//! ```
//!
//! A condition is `u32 index` (must equal its position), a NUL-terminated
//! name, and one call record. A call record is:
//!
//! ```text
//! u32   kind      1 wrapped condition, 2 wrapped action, 3 nested call
//! cstr  name      argument count from the editor tables (see ArityTable)
//! u32   flag      must be 1
//! var   params    one per argument
//! u32   trailer   must be 0
//! ```
//!
//! A parameter is `u32 kind`, a NUL-terminated value, `u32 has_sub`, an
//! optional call record, and `u32 trailer` (must be 0):
//!
//! * kind 0 preset (for example `OperatorEqual`, `AICommandLeave`),
//! * kind 1 variable (not observed in these ten files),
//! * kind 2 function call; `has_sub` is 1 and the call record follows,
//! * kind 3 text (for example `1`, `40`, `2000`).
//!
//! `has_sub` must be 1 for kind 2 and 0 otherwise. Any other combination is
//! refused. The parameter and call discriminants match the ones
//! `pcx_trig::wtg_v7` reads from `war3map.wtg`. That agreement is evidence for
//! the numbering only; the record layout is measured here, not inherited.
//!
//! Every discriminant and trailer value in the ten files is in the table above.
//! The parser frames and validates each condition tree. It does not interpret
//! what the trees compute.
//!
//! # Argument counts
//!
//! The arity of each name comes from the install's editor tables, not from
//! this file: `UI\AIEditorData.txt` `[AIFunctions]` (three leading fields:
//! version, flag, return type) and `UI\TriggerData.txt` `[TriggerCalls]`
//! (three leading fields) and `[TriggerConditions]` (one leading field). Every
//! name used by the ten files resolves in those tables.
//!
//! # Not decoded
//!
//! The tail holds the rest of each script, 1.9 to 2.3 KB per Monolith file. The
//! `.ai` source names attack groups, build priorities, race and option settings
//! in that script. None of those are located in the tail yet, so they are not
//! decoded. The header words and fourcc contents are kept raw.

use std::collections::BTreeMap;

use thiserror::Error;

/// The only `.wai` version the ten measured members write.
pub const AI_DATA_VERSION: u32 = 2;

/// Bytes in the smallest condition record: an index, an empty name, and a
/// call record with no parameters.
const MIN_CONDITION_BYTES: usize = 4 + 1 + 4 + 1 + 4 + 4;

/// Bytes in one fourcc entry.
const FOURCC_BYTES: usize = 4;

/// The flag word that every call record carries. Its meaning is unattributed.
const CALL_FLAG: u32 = 1;

/// Deepest call nesting accepted. A bound against stack exhaustion, not an
/// asset value.
pub const MAX_CALL_DEPTH: usize = 64;

/// Editor sections read for argument counts, with the number of leading fields
/// in each row that are not arguments.
const ARITY_SECTIONS: [(&str, usize); 3] = [
    ("AIFunctions", 3),
    ("TriggerCalls", 3),
    ("TriggerConditions", 1),
];

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AiDataError {
    #[error("ai data needs {need} bytes at {at}, has {have}")]
    Truncated { at: usize, need: usize, have: usize },
    #[error("unsupported ai data version {0}; the measured members write 2")]
    UnsupportedVersion(u32),
    #[error("ai data string at {at} has no NUL terminator")]
    UnterminatedString { at: usize },
    #[error("ai data at {at} declares {count} {what}, which need more than the {have} bytes left")]
    CountTooLarge {
        what: &'static str,
        at: usize,
        count: u32,
        have: usize,
    },
    #[error("ai data condition at {at} has index {got}, expected {expected}")]
    ConditionIndex { at: usize, expected: u32, got: u32 },
    #[error("ai data call at {at} has kind {got}, expected 1, 2 or 3")]
    CallKind { at: usize, got: u32 },
    #[error(
        "ai data call at {at} calls {name:?}, which has no argument count in the editor tables"
    )]
    UnknownCall { at: usize, name: String },
    #[error("ai data call flag at {at} is {got}, expected 1")]
    CallFlag { at: usize, got: u32 },
    #[error("ai data call trailer at {at} is {got}, expected 0")]
    CallTrailer { at: usize, got: u32 },
    #[error("ai data parameter at {at} has kind {got}, expected 0 to 3")]
    ParamKind { at: usize, got: u32 },
    #[error("ai data parameter at {at} has has_sub {got}, expected 0 or 1")]
    HasSub { at: usize, got: u32 },
    #[error("ai data function parameter at {at} has no call record")]
    FunctionWithoutCall { at: usize },
    #[error("ai data parameter at {at} has a call record but is not a function parameter")]
    UnexpectedCall { at: usize },
    #[error("ai data parameter trailer at {at} is {got}, expected 0")]
    ParamTrailer { at: usize, got: u32 },
    #[error("ai data calls nest deeper than {MAX_CALL_DEPTH} at {at}")]
    NestingTooDeep { at: usize },
    #[error(
        "editor table line {line} is not a section header or a key=value row with the section's leading fields"
    )]
    EditorRow { line: usize },
    #[error("editor tables give {name:?} both {first} and {second} arguments")]
    ConflictingArity {
        name: String,
        first: usize,
        second: usize,
    },
}

/// Argument counts keyed by call name, read from the editor tables.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ArityTable {
    arities: BTreeMap<Vec<u8>, usize>,
}

impl ArityTable {
    /// Read the argument counts from `UI\AIEditorData.txt` and
    /// `UI\TriggerData.txt`, the two editor files a `.wai` depends on.
    pub fn from_editor_files(ai_editor: &str, trigger_data: &str) -> Result<Self, AiDataError> {
        let mut table = Self::default();
        table.read_sections(ai_editor)?;
        table.read_sections(trigger_data)?;
        Ok(table)
    }

    /// The argument count of `name`, if the editor tables declare it.
    pub fn get(&self, name: &[u8]) -> Option<usize> {
        self.arities.get(name).copied()
    }

    /// Number of declared calls.
    pub fn len(&self) -> usize {
        self.arities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.arities.is_empty()
    }

    fn read_sections(&mut self, text: &str) -> Result<(), AiDataError> {
        let mut leading: Option<usize> = None;
        for (index, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if let Some(section) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                leading = ARITY_SECTIONS
                    .iter()
                    .find(|(name, _)| *name == section)
                    .map(|&(_, fields)| fields);
                continue;
            }
            let Some(fields) = leading else { continue };
            if line.is_empty() || line.starts_with("//") || line.starts_with('_') {
                continue;
            }
            let (key, value) = line
                .split_once('=')
                .ok_or(AiDataError::EditorRow { line: index + 1 })?;
            let total = value.split(',').count();
            let args = total
                .checked_sub(fields)
                .ok_or(AiDataError::EditorRow { line: index + 1 })?;
            self.insert(key.trim().as_bytes(), args)?;
        }
        Ok(())
    }

    fn insert(&mut self, name: &[u8], args: usize) -> Result<(), AiDataError> {
        match self.arities.get(name) {
            Some(&first) if first != args => Err(AiDataError::ConflictingArity {
                name: String::from_utf8_lossy(name).into_owned(),
                first,
                second: args,
            }),
            Some(_) => Ok(()),
            None => {
                self.arities.insert(name.to_vec(), args);
                Ok(())
            }
        }
    }
}

/// A parsed `.wai`. `tail` holds the bytes after the last condition, raw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiData {
    pub version: u32,
    /// The script name, NUL excluded, for example `Wolf AI`.
    pub name: Vec<u8>,
    /// Raw word at the NUL of the name plus one.
    pub word_at_name_end: u32,
    /// Raw word four bytes after `word_at_name_end`.
    pub word_at_name_end_plus_4: u32,
    /// Unit codes from the count-prefixed list, unattributed.
    pub fourcc: Vec<[u8; 4]>,
    /// Number of condition records, each index-checked as it is read.
    pub condition_count: u32,
    /// Raw word after `condition_count`: 7 in all ten files.
    pub word_after_condition_count: u32,
    pub conditions: Vec<AiCondition>,
    /// File offset where `tail` begins; the end of the last condition.
    pub tail_offset: usize,
    /// Undecoded bytes from `tail_offset` to end of file.
    pub tail: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiCondition {
    /// Equals the condition's position in the list.
    pub index: u32,
    pub name: Vec<u8>,
    pub root: AiCall,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiCall {
    pub kind: AiCallKind,
    pub name: Vec<u8>,
    /// One per argument count declared for `name`.
    pub params: Vec<AiParam>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiParam {
    pub kind: AiParamKind,
    pub value: Vec<u8>,
    /// Present exactly when `kind` is [`AiParamKind::Function`].
    pub call: Option<Box<AiCall>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiCallKind {
    /// Discriminant 1. Every measured condition root is this kind.
    WrappedCondition,
    /// Discriminant 2. Not observed in the ten files.
    WrappedAction,
    /// Discriminant 3. A function call nested inside a parameter.
    NestedCall,
}

impl AiCallKind {
    fn from_word(word: u32) -> Option<Self> {
        match word {
            1 => Some(Self::WrappedCondition),
            2 => Some(Self::WrappedAction),
            3 => Some(Self::NestedCall),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiParamKind {
    /// Discriminant 0: an editor constant such as `OperatorEqual`.
    Preset,
    /// Discriminant 1. Not observed in the ten files.
    Variable,
    /// Discriminant 2: a function call; the value is the function name.
    Function,
    /// Discriminant 3: a literal written as text, such as `1` or `2000`.
    Text,
}

impl AiParamKind {
    fn from_word(word: u32) -> Option<Self> {
        match word {
            0 => Some(Self::Preset),
            1 => Some(Self::Variable),
            2 => Some(Self::Function),
            3 => Some(Self::Text),
            _ => None,
        }
    }
}

impl AiData {
    /// Parse one `.wai` member. `arity` must come from the same install's editor
    /// tables. Every call name must resolve in it.
    pub fn parse(data: &[u8], arity: &ArityTable) -> Result<Self, AiDataError> {
        let mut r = Reader { data, at: 0 };
        let version = r.u32()?;
        if version != AI_DATA_VERSION {
            return Err(AiDataError::UnsupportedVersion(version));
        }
        let name = r.cstr()?.to_vec();
        let word_at_name_end = r.u32()?;
        let word_at_name_end_plus_4 = r.u32()?;
        let fourcc_count = r.count(FOURCC_BYTES, "fourcc entries")?;
        let fourcc = (0..fourcc_count)
            .map(|_| r.take(FOURCC_BYTES).map(|b| b.try_into().unwrap()))
            .collect::<Result<Vec<[u8; 4]>, _>>()?;
        let condition_count = r.count(MIN_CONDITION_BYTES, "conditions")?;
        let word_after_condition_count = r.u32()?;
        let mut conditions = Vec::with_capacity(condition_count as usize);
        for expected in 0..condition_count {
            let at = r.at;
            let index = r.u32()?;
            if index != expected {
                return Err(AiDataError::ConditionIndex {
                    at,
                    expected,
                    got: index,
                });
            }
            let name = r.cstr()?.to_vec();
            let root = parse_call(&mut r, arity, 0)?;
            conditions.push(AiCondition { index, name, root });
        }
        let tail_offset = r.at;
        Ok(Self {
            version,
            name,
            word_at_name_end,
            word_at_name_end_plus_4,
            fourcc,
            condition_count,
            word_after_condition_count,
            conditions,
            tail_offset,
            tail: data[tail_offset..].to_vec(),
        })
    }
}

struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8], AiDataError> {
        let have = self.data.len() - self.at;
        if len > have {
            return Err(AiDataError::Truncated {
                at: self.at,
                need: len,
                have,
            });
        }
        let bytes = &self.data[self.at..self.at + len];
        self.at += len;
        Ok(bytes)
    }

    fn u32(&mut self) -> Result<u32, AiDataError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    /// A NUL-terminated string, NUL excluded.
    fn cstr(&mut self) -> Result<&'a [u8], AiDataError> {
        let tail = &self.data[self.at..];
        let nul = tail
            .iter()
            .position(|&b| b == 0)
            .ok_or(AiDataError::UnterminatedString { at: self.at })?;
        self.at += nul + 1;
        Ok(&tail[..nul])
    }

    /// A count word, refused unless `count * min_item_bytes` fits in the bytes
    /// that follow it. The check runs before any allocation sized by the count.
    fn count(&mut self, min_item_bytes: usize, what: &'static str) -> Result<u32, AiDataError> {
        let at = self.at;
        let count = self.u32()?;
        let have = self.data.len() - self.at;
        let fits = (count as usize)
            .checked_mul(min_item_bytes)
            .is_some_and(|need| need <= have);
        if !fits {
            return Err(AiDataError::CountTooLarge {
                what,
                at,
                count,
                have,
            });
        }
        Ok(count)
    }
}

fn parse_call(r: &mut Reader<'_>, arity: &ArityTable, depth: usize) -> Result<AiCall, AiDataError> {
    if depth > MAX_CALL_DEPTH {
        return Err(AiDataError::NestingTooDeep { at: r.at });
    }
    let at = r.at;
    let kind_word = r.u32()?;
    let kind =
        AiCallKind::from_word(kind_word).ok_or(AiDataError::CallKind { at, got: kind_word })?;
    let name = r.cstr()?;
    let flag_at = r.at;
    let flag = r.u32()?;
    if flag != CALL_FLAG {
        return Err(AiDataError::CallFlag {
            at: flag_at,
            got: flag,
        });
    }
    let count = arity.get(name).ok_or_else(|| AiDataError::UnknownCall {
        at,
        name: String::from_utf8_lossy(name).into_owned(),
    })?;
    let mut params = Vec::with_capacity(count);
    for _ in 0..count {
        params.push(parse_param(r, arity, depth)?);
    }
    let trailer_at = r.at;
    let trailer = r.u32()?;
    if trailer != 0 {
        return Err(AiDataError::CallTrailer {
            at: trailer_at,
            got: trailer,
        });
    }
    Ok(AiCall {
        kind,
        name: name.to_vec(),
        params,
    })
}

fn parse_param(
    r: &mut Reader<'_>,
    arity: &ArityTable,
    depth: usize,
) -> Result<AiParam, AiDataError> {
    let at = r.at;
    let kind_word = r.u32()?;
    let kind =
        AiParamKind::from_word(kind_word).ok_or(AiDataError::ParamKind { at, got: kind_word })?;
    let value = r.cstr()?.to_vec();
    let has_sub_at = r.at;
    let has_sub = r.u32()?;
    let call = match has_sub {
        0 => None,
        1 => Some(Box::new(parse_call(r, arity, depth + 1)?)),
        got => {
            return Err(AiDataError::HasSub {
                at: has_sub_at,
                got,
            });
        }
    };
    match (kind, call.is_some()) {
        (AiParamKind::Function, false) => return Err(AiDataError::FunctionWithoutCall { at }),
        (AiParamKind::Function, true) => {}
        (_, true) => return Err(AiDataError::UnexpectedCall { at }),
        (_, false) => {}
    }
    let trailer_at = r.at;
    let trailer = r.u32()?;
    if trailer != 0 {
        return Err(AiDataError::ParamTrailer {
            at: trailer_at,
            got: trailer,
        });
    }
    Ok(AiParam { kind, value, call })
}

#[cfg(test)]
mod tests {
    use super::*;

    const AI_EDITOR: &str = "\
//***************************************************************************
[AIFunctions]
// Key: Function name
CheckLastCommand=1,0,integer,aicommandpop
_CheckLastCommand_Category=TC_AI_COMMAND
FoodUsed=1,0,integer
";

    const TRIGGER_DATA: &str = "\
[TriggerCalls]
Ignored=1,0,integer,integer

[TriggerConditions]
OperatorCompareInteger=0,integer,ComparisonOperator,integer
";

    fn arity() -> ArityTable {
        ArityTable::from_editor_files(AI_EDITOR, TRIGGER_DATA).unwrap()
    }

    fn word(out: &mut Vec<u8>, w: u32) {
        out.extend_from_slice(&w.to_le_bytes());
    }

    fn text(out: &mut Vec<u8>, s: &str) {
        out.extend_from_slice(s.as_bytes());
        out.push(0);
    }

    /// Header with one fourcc and `conditions` condition records, then `body`.
    fn header(conditions: u32, body: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        word(&mut out, AI_DATA_VERSION);
        text(&mut out, "Wolf AI");
        word(&mut out, 0);
        word(&mut out, 0x19ff);
        word(&mut out, 1);
        out.extend_from_slice(b"n009");
        word(&mut out, conditions);
        word(&mut out, 7);
        out.extend_from_slice(body);
        out
    }

    /// `body` with room for one minimal condition, so the count check passes
    /// and the test reaches the field it is about.
    fn padded(mut body: Vec<u8>) -> Vec<u8> {
        body.extend_from_slice(&[0; MIN_CONDITION_BYTES]);
        body
    }

    /// One call record: `kind`, `name`, the flag, the parameters, and the trailer.
    fn call(out: &mut Vec<u8>, kind: u32, name: &str, params: &[u8]) {
        word(out, kind);
        text(out, name);
        word(out, CALL_FLAG);
        out.extend_from_slice(params);
        word(out, 0);
    }

    /// A preset parameter with no call record and trailer 0.
    fn preset(out: &mut Vec<u8>, value: &str) {
        word(out, 0);
        text(out, value);
        word(out, 0);
        word(out, 0);
    }

    /// The Wolf shape: `Attack Enemy` as `OperatorCompareInteger` of a nested
    /// `CheckLastCommand(AICommandLeave)`, `OperatorEqual`, and the literal `1`.
    fn attack_enemy() -> Vec<u8> {
        let mut nested = Vec::new();
        preset(&mut nested, "AICommandLeave");
        let mut root_params = Vec::new();
        // Function parameter: kind 2, the name, has_sub 1, the nested call, trailer.
        word(&mut root_params, 2);
        text(&mut root_params, "CheckLastCommand");
        word(&mut root_params, 1);
        call(&mut root_params, 3, "CheckLastCommand", &nested);
        word(&mut root_params, 0);
        preset(&mut root_params, "OperatorEqual");
        word(&mut root_params, 3);
        text(&mut root_params, "1");
        word(&mut root_params, 0);
        word(&mut root_params, 0);
        let mut record = Vec::new();
        word(&mut record, 0);
        text(&mut record, "Attack Enemy");
        call(&mut record, 1, "OperatorCompareInteger", &root_params);
        record
    }

    fn sample() -> Vec<u8> {
        let mut body = attack_enemy();
        body.extend_from_slice(b"tail");
        header(1, &body)
    }

    #[test]
    fn parses_header_condition_and_tail() {
        let data = sample();
        let ai = AiData::parse(&data, &arity()).unwrap();
        assert_eq!(ai.version, 2);
        assert_eq!(ai.name, b"Wolf AI");
        assert_eq!(ai.word_at_name_end, 0);
        assert_eq!(ai.word_at_name_end_plus_4, 0x19ff);
        assert_eq!(ai.fourcc, vec![*b"n009"]);
        assert_eq!(ai.condition_count, 1);
        assert_eq!(ai.word_after_condition_count, 7);
        assert_eq!(ai.conditions.len(), 1);
        let c = &ai.conditions[0];
        assert_eq!(
            (c.index, c.name.as_slice()),
            (0, b"Attack Enemy".as_slice())
        );
        assert_eq!(c.root.kind, AiCallKind::WrappedCondition);
        assert_eq!(c.root.name, b"OperatorCompareInteger");
        assert_eq!(c.root.params.len(), 3);
        let function = &c.root.params[0];
        assert_eq!(function.kind, AiParamKind::Function);
        assert_eq!(function.value, b"CheckLastCommand");
        let nested = function.call.as_deref().unwrap();
        assert_eq!(nested.kind, AiCallKind::NestedCall);
        assert_eq!(nested.params[0].value, b"AICommandLeave");
        assert_eq!(c.root.params[1].kind, AiParamKind::Preset);
        assert_eq!(c.root.params[2].kind, AiParamKind::Text);
        assert_eq!(c.root.params[2].value, b"1");
        assert_eq!(ai.tail, b"tail");
        assert_eq!(ai.tail_offset, data.len() - 4);
    }

    #[test]
    fn empty_input_is_truncated() {
        assert!(matches!(
            AiData::parse(&[], &arity()),
            Err(AiDataError::Truncated { at: 0, .. })
        ));
    }

    #[test]
    fn every_prefix_inside_the_decoded_part_is_refused() {
        let data = sample();
        let decoded_end = data.len() - b"tail".len();
        for n in 0..decoded_end {
            assert!(
                AiData::parse(&data[..n], &arity()).is_err(),
                "prefix of {n} bytes parsed"
            );
        }
        for n in decoded_end..=data.len() {
            assert!(
                AiData::parse(&data[..n], &arity()).is_ok(),
                "prefix of {n} bytes refused"
            );
        }
    }

    #[test]
    fn unsupported_version_is_refused() {
        let mut data = sample();
        data[..4].copy_from_slice(&3u32.to_le_bytes());
        assert_eq!(
            AiData::parse(&data, &arity()),
            Err(AiDataError::UnsupportedVersion(3))
        );
    }

    #[test]
    fn unterminated_name_is_refused() {
        let mut data = Vec::new();
        word(&mut data, AI_DATA_VERSION);
        data.extend_from_slice(b"Wolf");
        assert_eq!(
            AiData::parse(&data, &arity()),
            Err(AiDataError::UnterminatedString { at: 4 })
        );
    }

    #[test]
    fn oversized_fourcc_count_is_refused_before_allocating() {
        let mut data = Vec::new();
        word(&mut data, AI_DATA_VERSION);
        text(&mut data, "x");
        word(&mut data, 0);
        word(&mut data, 0);
        word(&mut data, u32::MAX);
        assert!(matches!(
            AiData::parse(&data, &arity()),
            Err(AiDataError::CountTooLarge {
                what: "fourcc entries",
                count: u32::MAX,
                ..
            })
        ));
    }

    #[test]
    fn oversized_condition_count_is_refused_before_allocating() {
        let data = header(u32::MAX, &[]);
        assert!(matches!(
            AiData::parse(&data, &arity()),
            Err(AiDataError::CountTooLarge {
                what: "conditions",
                ..
            })
        ));
    }

    #[test]
    fn fourcc_count_that_fits_the_file_is_accepted() {
        let mut data = Vec::new();
        word(&mut data, AI_DATA_VERSION);
        text(&mut data, "max");
        word(&mut data, 0);
        word(&mut data, 0);
        word(&mut data, 3);
        data.extend_from_slice(b"n009n009o00A");
        word(&mut data, 0);
        word(&mut data, 7);
        let ai = AiData::parse(&data, &arity()).unwrap();
        assert_eq!(ai.fourcc, vec![*b"n009", *b"n009", *b"o00A"]);
        assert!(ai.conditions.is_empty());
        assert!(ai.tail.is_empty());
    }

    #[test]
    fn wrong_condition_index_is_refused() {
        let mut body = attack_enemy();
        body[..4].copy_from_slice(&5u32.to_le_bytes());
        // The condition index sits after the 36-byte header written by `header`.
        assert_eq!(
            AiData::parse(&header(1, &padded(body)), &arity()),
            Err(AiDataError::ConditionIndex {
                at: 36,
                expected: 0,
                got: 5,
            })
        );
    }

    #[test]
    fn unknown_call_is_refused() {
        let mut body = Vec::new();
        word(&mut body, 0);
        text(&mut body, "Mystery");
        call(&mut body, 1, "NotInTheTables", &[]);
        assert!(matches!(
            AiData::parse(&header(1, &padded(body)), &arity()),
            Err(AiDataError::UnknownCall { ref name, .. }) if name == "NotInTheTables"
        ));
    }

    #[test]
    fn bad_call_kind_flag_and_trailer_are_refused() {
        let mut body = Vec::new();
        word(&mut body, 0);
        text(&mut body, "E");
        word(&mut body, 9);
        assert!(matches!(
            AiData::parse(&header(1, &padded(body)), &arity()),
            Err(AiDataError::CallKind { got: 9, .. })
        ));

        let mut body = Vec::new();
        word(&mut body, 0);
        text(&mut body, "E");
        word(&mut body, 1);
        text(&mut body, "FoodUsed");
        word(&mut body, 0);
        assert!(matches!(
            AiData::parse(&header(1, &padded(body)), &arity()),
            Err(AiDataError::CallFlag { got: 0, .. })
        ));

        let mut body = Vec::new();
        word(&mut body, 0);
        text(&mut body, "E");
        word(&mut body, 1);
        text(&mut body, "FoodUsed");
        word(&mut body, CALL_FLAG);
        word(&mut body, 4);
        assert!(matches!(
            AiData::parse(&header(1, &padded(body)), &arity()),
            Err(AiDataError::CallTrailer { got: 4, .. })
        ));
    }

    #[test]
    fn bad_parameter_shapes_are_refused() {
        let call_with = |param: &[u8]| {
            let mut body = Vec::new();
            word(&mut body, 0);
            text(&mut body, "E");
            call(&mut body, 1, "CheckLastCommand", param);
            header(1, &body)
        };
        let mut unknown_kind = Vec::new();
        word(&mut unknown_kind, 9);
        text(&mut unknown_kind, "x");
        word(&mut unknown_kind, 0);
        word(&mut unknown_kind, 0);
        assert!(matches!(
            AiData::parse(&call_with(&unknown_kind), &arity()),
            Err(AiDataError::ParamKind { got: 9, .. })
        ));

        let mut function_without_call = Vec::new();
        word(&mut function_without_call, 2);
        text(&mut function_without_call, "CheckLastCommand");
        word(&mut function_without_call, 0);
        word(&mut function_without_call, 0);
        assert!(matches!(
            AiData::parse(&call_with(&function_without_call), &arity()),
            Err(AiDataError::FunctionWithoutCall { .. })
        ));

        let mut preset_with_call = Vec::new();
        word(&mut preset_with_call, 0);
        text(&mut preset_with_call, "x");
        word(&mut preset_with_call, 1);
        call(&mut preset_with_call, 3, "FoodUsed", &[]);
        word(&mut preset_with_call, 0);
        assert!(matches!(
            AiData::parse(&call_with(&preset_with_call), &arity()),
            Err(AiDataError::UnexpectedCall { .. })
        ));

        let mut bad_has_sub = Vec::new();
        word(&mut bad_has_sub, 0);
        text(&mut bad_has_sub, "x");
        word(&mut bad_has_sub, 2);
        assert!(matches!(
            AiData::parse(&call_with(&bad_has_sub), &arity()),
            Err(AiDataError::HasSub { got: 2, .. })
        ));

        let mut nonzero_trailer = Vec::new();
        word(&mut nonzero_trailer, 0);
        text(&mut nonzero_trailer, "x");
        word(&mut nonzero_trailer, 0);
        word(&mut nonzero_trailer, 1);
        assert!(matches!(
            AiData::parse(&call_with(&nonzero_trailer), &arity()),
            Err(AiDataError::ParamTrailer { got: 1, .. })
        ));
    }

    #[test]
    fn deep_nesting_is_refused_without_overflowing_the_stack() {
        let depth = MAX_CALL_DEPTH + 8;
        let mut inner = Vec::new();
        preset(&mut inner, "x");
        for _ in 0..depth {
            let mut params = Vec::new();
            word(&mut params, 2);
            text(&mut params, "CheckLastCommand");
            word(&mut params, 1);
            let mut nested = Vec::new();
            call(&mut nested, 3, "CheckLastCommand", &inner);
            params.extend_from_slice(&nested);
            word(&mut params, 0);
            inner = params;
        }
        let mut body = Vec::new();
        word(&mut body, 0);
        text(&mut body, "Deep");
        call(&mut body, 1, "CheckLastCommand", &inner);
        let data = header(1, &body);
        assert!(matches!(
            AiData::parse(&data, &arity()),
            Err(AiDataError::NestingTooDeep { .. })
        ));
    }

    #[test]
    fn single_byte_corruption_never_panics() {
        let data = sample();
        for i in 0..data.len() {
            for value in [0x00, 0x7F, 0xFF] {
                let mut corrupt = data.clone();
                corrupt[i] = value;
                let _ = AiData::parse(&corrupt, &arity());
            }
        }
    }

    #[test]
    fn editor_tables_give_each_section_its_leading_fields() {
        let table = arity();
        assert_eq!(table.get(b"CheckLastCommand"), Some(1));
        assert_eq!(table.get(b"FoodUsed"), Some(0));
        assert_eq!(table.get(b"OperatorCompareInteger"), Some(3));
        assert_eq!(table.get(b"Ignored"), Some(1));
        assert_eq!(table.get(b"_CheckLastCommand_Category"), None);
        assert_eq!(table.len(), 4);
    }

    #[test]
    fn editor_row_with_too_few_fields_is_refused() {
        let short = "[AIFunctions]\nX=1,0\n";
        assert_eq!(
            ArityTable::from_editor_files(short, ""),
            Err(AiDataError::EditorRow { line: 2 })
        );
    }

    #[test]
    fn editor_row_without_key_value_is_refused() {
        let bad = "[TriggerConditions]\nnot a row\n";
        assert_eq!(
            ArityTable::from_editor_files("", bad),
            Err(AiDataError::EditorRow { line: 2 })
        );
    }

    #[test]
    fn conflicting_editor_arities_are_refused() {
        let first = "[AIFunctions]\nX=1,0,integer\n";
        let second = "[TriggerConditions]\nX=0,integer,integer\n";
        assert_eq!(
            ArityTable::from_editor_files(first, second),
            Err(AiDataError::ConflictingArity {
                name: "X".to_owned(),
                first: 0,
                second: 2,
            })
        );
    }

    #[test]
    fn empty_editor_text_declares_nothing() {
        let table = ArityTable::from_editor_files("", "").unwrap();
        assert!(table.is_empty());
        assert_eq!(table.get(b"FoodUsed"), None);
    }

    /// Reads the eight Monolith members and the two other members from a real
    /// install. Set `PEACECRAFT3_INSTALL` to a Warcraft III install root with
    /// `War3x.mpq` and `Maps/FrozenThrone/Scenario/(4)Monolith.w3x`.
    #[test]
    #[ignore = "needs PEACECRAFT3_INSTALL: a Warcraft III install root with War3x.mpq and Maps/FrozenThrone/Scenario/(4)Monolith.w3x"]
    fn shipped_members_parse_to_their_tail() {
        use crate::Map;
        let dir = std::path::PathBuf::from(
            std::env::var_os("PEACECRAFT3_INSTALL").expect("PEACECRAFT3_INSTALL is not set"),
        );
        let mut set = pcx_mpq::MpqSet::open_wc3(&dir).unwrap();
        let ai_editor = String::from_utf8(set.extract(r"UI\AIEditorData.txt").unwrap()).unwrap();
        let trigger_data = String::from_utf8(set.extract(r"UI\TriggerData.txt").unwrap()).unwrap();
        let table = ArityTable::from_editor_files(&ai_editor, &trigger_data).unwrap();

        // Offset where the condition list ends, measured from the eight members.
        let members = [
            ("Beast", 821),
            ("Furbolg", 823),
            ("Lizard", 822),
            ("Murloc", 822),
            ("Satyr", 821),
            ("Spider", 822),
            ("Troll", 821),
            ("Wolf", 820),
        ];
        let bytes = std::fs::read(dir.join("Maps/FrozenThrone/Scenario/(4)Monolith.w3x")).unwrap();
        let map = Map::parse(&bytes).unwrap();
        for (name, tail_offset) in members {
            let member = format!(r"war3mapImported\{name}_AI.wai");
            let wai = map.extract(&member).unwrap();
            let ai = AiData::parse(&wai, &table).unwrap();
            assert_eq!(ai.tail_offset, tail_offset, "{member}");
            assert_eq!(ai.version, 2, "{member}");
            assert_eq!(ai.condition_count, 4, "{member}");
            assert_eq!(ai.conditions.len(), 4, "{member}");
            let names: Vec<&[u8]> = ai.conditions.iter().map(|c| c.name.as_slice()).collect();
            assert_eq!(
                names,
                vec![
                    b"Attack Enemy".as_slice(),
                    b"Attack Strong Creeps".as_slice(),
                    b"Have Lumber".as_slice(),
                    b"Need Expansion".as_slice(),
                ],
                "{member}"
            );
            assert_eq!(ai.tail_offset + ai.tail.len(), wai.len(), "{member}");
        }
    }
}
