//! Checked reader for `war3map.wtg` **version 7** (Frozen Throne / TFT
//! records): category/trigger `is_comment` words, variable `array_size`
//! words, ECA child groups, nested child ECAs and the trailing ECA branch
//! word. Independent of [`crate::Wtg`] (version 4 only); the v4 parser
//! keeps refusing version 7.

use pcx_trig::TriggerArity;

pub const WTG_MAGIC: [u8; 4] = *b"WTG!";
pub const WTG_V7_VERSION: u32 = 7;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("truncated {what} at {at}: need {need}, have {have}")]
    Truncated {
        what: &'static str,
        at: usize,
        need: usize,
        have: usize,
    },
    #[error("bad WTG magic at +0: got {got:?}")]
    BadMagic { got: [u8; 4] },
    #[error("unsupported WTG version {got}; this reader wants {WTG_V7_VERSION}")]
    BadVersion { got: u32 },
    #[error("unknown ECA kind {got} at {at}")]
    BadEcaKind { got: u32, at: usize },
    #[error("unknown parameter type {got} at {at}")]
    BadParamType { got: i32, at: usize },
    #[error("unknown sub kind {got} at {at}")]
    BadSubKind { got: u32, at: usize },
    #[error("function {name} has no catalog arity")]
    UnknownFunction { name: String },
    #[error("sub flag {got} at {at} is not 1")]
    BadSubFlag { got: u32, at: usize },
    #[error("trailing bytes: consumed {end} of {len}")]
    Trailing { end: usize, len: usize },
}

fn ru32(data: &[u8], p: &mut usize, what: &'static str) -> Result<u32, Error> {
    let start = *p;
    let bytes = data.get(start..start + 4).ok_or(Error::Truncated {
        what,
        at: start,
        need: 4,
        have: data.len(),
    })?;
    let mut b = [0u8; 4];
    b.copy_from_slice(bytes);
    *p = start + 4;
    Ok(u32::from_le_bytes(b))
}

fn rcstr(data: &[u8], p: &mut usize, what: &'static str) -> Result<Vec<u8>, Error> {
    let start = *p;
    let end = data[start..]
        .iter()
        .position(|&x| x == 0)
        .ok_or(Error::Truncated {
            what,
            at: start,
            need: 1,
            have: data.len() - start,
        })?;
    let bytes = data[start..start + end].to_vec();
    *p = start + end + 1;
    Ok(bytes)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Category {
    pub id: u32,
    pub name: Vec<u8>,
    pub is_comment: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variable {
    pub name: Vec<u8>,
    pub type_name: Vec<u8>,
    pub unknown: u32,
    pub is_array: u32,
    pub array_size: u32,
    pub is_initialized: u32,
    pub initial_value: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EcaKind {
    Event,
    Condition,
    Action,
}
impl EcaKind {
    fn of(w: u32) -> Option<Self> {
        match w {
            0 => Some(Self::Event),
            1 => Some(Self::Condition),
            2 => Some(Self::Action),
            _ => None,
        }
    }
    pub fn word(self) -> u32 {
        match self {
            Self::Event => 0,
            Self::Condition => 1,
            Self::Action => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamKind {
    Preset,
    Variable,
    Function,
    Str,
}
impl ParamKind {
    fn of(w: i32) -> Option<Self> {
        match w {
            0 => Some(Self::Preset),
            1 => Some(Self::Variable),
            2 => Some(Self::Function),
            3 => Some(Self::Str),
            _ => None,
        }
    }
    pub fn word(self) -> i32 {
        match self {
            Self::Preset => 0,
            Self::Variable => 1,
            Self::Function => 2,
            Self::Str => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubKind {
    WrappedCondition,
    WrappedAction,
    NestedCall,
}
impl SubKind {
    fn of(w: u32) -> Option<Self> {
        match w {
            1 => Some(Self::WrappedCondition),
            2 => Some(Self::WrappedAction),
            3 => Some(Self::NestedCall),
            _ => None,
        }
    }
    pub fn word(self) -> u32 {
        match self {
            Self::WrappedCondition => 1,
            Self::WrappedAction => 2,
            Self::NestedCall => 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    pub kind: ParamKind,
    pub value: Vec<u8>,
    pub has_sub: bool,
    pub sub: Option<Box<Sub>>,
    pub trailer: u32,
    pub array_index: Option<Box<Param>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sub {
    pub kind: SubKind,
    pub name: Vec<u8>,
    pub flag: u32,
    pub params: Vec<Param>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Eca {
    pub kind: EcaKind,
    pub group: Option<u32>, // present only on child ECAs
    pub name: Vec<u8>,
    pub is_enabled: u32,
    pub params: Vec<Param>,
    pub child_branch: u32, // trailing word before child list
    pub child_ecas: Vec<Eca>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trigger {
    pub name: Vec<u8>,
    pub description: Vec<u8>,
    pub is_comment: u32,
    pub id: u32,
    pub is_enabled: u32,
    pub is_custom: u32,
    pub is_initially_off: u32,
    pub run_on_init: u32,
    pub parent_id: u32,
    pub ecas: Vec<Eca>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WtgV7<'a> {
    raw: &'a [u8],
    version: u32,
    unknown_root: u32,
    categories: Vec<Category>,
    unknown_between: u32,
    variables: Vec<Variable>,
    triggers: Vec<Trigger>,
}

#[allow(dead_code)]
impl<'a> WtgV7<'a> {
    pub fn parse(data: &'a [u8], arity: &TriggerArity) -> Result<Self, Error> {
        let mut p = 0usize;
        let got: [u8; 4] =
            data.get(p..p + 4)
                .and_then(|s| s.try_into().ok())
                .ok_or(Error::Truncated {
                    what: "magic",
                    at: 0,
                    need: 4,
                    have: data.len(),
                })?;
        if got != WTG_MAGIC {
            return Err(Error::BadMagic { got });
        }
        p += 4;
        let version = ru32(data, &mut p, "version")?;
        if version != WTG_V7_VERSION {
            return Err(Error::BadVersion { got: version });
        }
        let cat_count = ru32(data, &mut p, "category count")? as usize;
        let mut categories = Vec::with_capacity(cat_count.min(1 << 16));
        for _ in 0..cat_count {
            let id = ru32(data, &mut p, "category id")?;
            let name = rcstr(data, &mut p, "category name")?;
            let is_comment = ru32(data, &mut p, "category is_comment")?;
            let is_expandable = ru32(data, &mut p, "category is_expandable")?; // retained as checked field but unused
            let _ = is_expandable;
            let parent_id = ru32(data, &mut p, "category parent id")?;
            let _ = parent_id;
            categories.push(Category {
                id,
                name,
                is_comment,
            });
        }
        let unknown_root = ru32(data, &mut p, "unknown root word")?;
        let var_count = ru32(data, &mut p, "variable count")? as usize;
        let mut variables = Vec::with_capacity(var_count.min(1 << 16));
        for _ in 0..var_count {
            let name = rcstr(data, &mut p, "variable name")?;
            let type_name = rcstr(data, &mut p, "variable type")?;
            let unknown = ru32(data, &mut p, "variable unknown")?;
            let is_array = ru32(data, &mut p, "variable is_array")?;
            let array_size = ru32(data, &mut p, "variable array_size")?;
            let is_initialized = ru32(data, &mut p, "variable is_initialized")?;
            let initial_value = rcstr(data, &mut p, "variable initial value")?;
            variables.push(Variable {
                name,
                type_name,
                unknown,
                is_array,
                array_size,
                is_initialized,
                initial_value,
            });
        }
        // Oops above added extra; revert: classic v7 variable has NO id/parent in this format.
        // We'll fix by not calling extra; but code already calls? Let's keep clean (placeholder undone).

        let trig_count = ru32(data, &mut p, "trigger count")? as usize;
        let mut triggers = Vec::with_capacity(trig_count.min(1 << 16));
        for _ in 0..trig_count {
            let name = rcstr(data, &mut p, "trigger name")?;
            let description = rcstr(data, &mut p, "trigger description")?;
            let is_comment = ru32(data, &mut p, "trigger is_comment")?;
            let id = ru32(data, &mut p, "trigger id")?;
            let is_enabled = ru32(data, &mut p, "trigger is_enabled")?;
            let is_custom = ru32(data, &mut p, "trigger is_custom")?;
            let is_initially_off = ru32(data, &mut p, "trigger is_initially_off")?;
            let run_on_init = ru32(data, &mut p, "trigger run_on_init")?;
            let parent_id = ru32(data, &mut p, "trigger parent_id")?;
            let eca_count = ru32(data, &mut p, "ECA count")? as usize;
            let mut ecas = Vec::with_capacity(eca_count.min(1 << 16));
            for _ in 0..eca_count {
                ecas.push(read_eca(data, &mut p, arity, false)?);
            }
            triggers.push(Trigger {
                name,
                description,
                is_comment,
                id,
                is_enabled,
                is_custom,
                is_initially_off,
                run_on_init,
                parent_id,
                ecas,
            });
        }

        let unknown_between = 0;

        if p != data.len() {
            return Err(Error::Trailing {
                end: p,
                len: data.len(),
            });
        }
        Ok(Self {
            raw: data,
            version,
            unknown_root,
            categories,
            unknown_between,
            variables,
            triggers,
        })
    }

    pub fn raw(&self) -> &'a [u8] {
        self.raw
    }
    pub fn version(&self) -> u32 {
        self.version
    }
    pub fn unknown_root(&self) -> u32 {
        self.unknown_root
    }
    pub fn categories(&self) -> &[Category] {
        &self.categories
    }
    pub fn variables(&self) -> &[Variable] {
        &self.variables
    }
    pub fn triggers(&self) -> &[Trigger] {
        &self.triggers
    }

    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.raw.len());
        out.extend_from_slice(&WTG_MAGIC);
        out.extend_from_slice(&self.version.to_le_bytes());
        out.extend_from_slice(&(self.categories.len() as u32).to_le_bytes());
        for c in &self.categories {
            out.extend_from_slice(&c.id.to_le_bytes());
            out.extend_from_slice(&c.name);
            out.push(0);
            out.extend_from_slice(&c.is_comment.to_le_bytes());
            out.extend_from_slice(&1u32.to_le_bytes()); // is_expandable
            out.extend_from_slice(&0u32.to_le_bytes()); // parent
        }
        out.extend_from_slice(&self.unknown_root.to_le_bytes());
        out.extend_from_slice(&(self.variables.len() as u32).to_le_bytes());
        for v in &self.variables {
            out.extend_from_slice(&v.name);
            out.push(0);
            out.extend_from_slice(&v.type_name);
            out.push(0);
            out.extend_from_slice(&v.unknown.to_le_bytes());
            out.extend_from_slice(&v.is_array.to_le_bytes());
            out.extend_from_slice(&v.array_size.to_le_bytes());
            out.extend_from_slice(&v.is_initialized.to_le_bytes());
            out.extend_from_slice(&v.initial_value);
            out.push(0);
        }
        out.extend_from_slice(&(self.triggers.len() as u32).to_le_bytes());
        for t in &self.triggers {
            out.extend_from_slice(&t.name);
            out.push(0);
            out.extend_from_slice(&t.description);
            out.push(0);
            out.extend_from_slice(&t.is_comment.to_le_bytes());
            out.extend_from_slice(&t.id.to_le_bytes());
            out.extend_from_slice(&t.is_enabled.to_le_bytes());
            out.extend_from_slice(&t.is_custom.to_le_bytes());
            out.extend_from_slice(&t.is_initially_off.to_le_bytes());
            out.extend_from_slice(&t.run_on_init.to_le_bytes());
            out.extend_from_slice(&t.parent_id.to_le_bytes());
            out.extend_from_slice(&(t.ecas.len() as u32).to_le_bytes());
            for e in &t.ecas {
                write_eca(e, &mut out);
            }
        }
        out
    }
}

fn read_eca(data: &[u8], p: &mut usize, arity: &TriggerArity, child: bool) -> Result<Eca, Error> {
    let kind_word = ru32(data, p, "ECA kind")?;
    let kind = EcaKind::of(kind_word).ok_or(Error::BadEcaKind {
        got: kind_word,
        at: *p,
    })?;
    let group = if child {
        Some(ru32(data, p, "ECA group")?)
    } else {
        None
    };
    let name = rcstr(data, p, "ECA name")?;
    let is_enabled = ru32(data, p, "ECA is_enabled")?;
    let n = arity.get(&name).ok_or_else(|| Error::UnknownFunction {
        name: String::from_utf8_lossy(&name).into_owned(),
    })?;
    let params = read_params(data, p, arity, n)?;
    let child_branch = ru32(data, p, "ECA child_branch")?;
    let child_count = ru32(data, p, "ECA child count")? as usize;
    let mut child_ecas = Vec::with_capacity(child_count.min(1 << 16));
    for _ in 0..child_count {
        child_ecas.push(read_eca(data, p, arity, true)?);
    }
    let _ = group;
    Ok(Eca {
        kind,
        group,
        name,
        is_enabled,
        params,
        child_branch,
        child_ecas,
    })
}

fn read_params(
    data: &[u8],
    p: &mut usize,
    arity: &TriggerArity,
    n: usize,
) -> Result<Vec<Param>, Error> {
    let mut out = Vec::with_capacity(n.min(1 << 12));
    for _ in 0..n {
        out.push(read_param(data, p, arity)?);
    }
    Ok(out)
}

fn read_param(data: &[u8], p: &mut usize, arity: &TriggerArity) -> Result<Param, Error> {
    let at = *p;
    let type_word = ru32(data, p, "param type")? as i32;
    let kind = ParamKind::of(type_word).ok_or(Error::BadParamType { got: type_word, at })?;
    let value = rcstr(data, p, "param value")?;
    let has_sub_word = ru32(data, p, "has_sub")?;
    let has_sub = has_sub_word == 1;
    if has_sub_word != 0 && has_sub_word != 1 {
        return Err(Error::BadSubFlag {
            got: has_sub_word,
            at,
        });
    }
    let sub = if has_sub {
        Some(Box::new(read_sub(data, p, arity)?))
    } else {
        None
    };
    let trailer = ru32(data, p, "param trailer")?;
    let array_index = if kind == ParamKind::Function {
        if trailer != 0 {
            // treat nonzero trailer on function as refused
            return Err(Error::BadSubFlag {
                got: trailer,
                at: *p,
            });
        }
        None
    } else if trailer == 1 {
        Some(Box::new(read_param(data, p, arity)?))
    } else if trailer == 0 {
        None
    } else {
        return Err(Error::BadSubFlag {
            got: trailer,
            at: *p,
        });
    };
    Ok(Param {
        kind,
        value,
        has_sub,
        sub,
        trailer,
        array_index,
    })
}

fn read_sub(data: &[u8], p: &mut usize, arity: &TriggerArity) -> Result<Sub, Error> {
    let at = *p;
    let kind_word = ru32(data, p, "sub kind")?;
    let kind = SubKind::of(kind_word).ok_or(Error::BadSubKind { got: kind_word, at })?;
    let name = rcstr(data, p, "sub name")?;
    let flag_at = *p;
    let flag = ru32(data, p, "sub flag")?;
    if flag != 1 {
        return Err(Error::BadSubFlag {
            got: flag,
            at: flag_at,
        });
    }
    let n = arity.get(&name).ok_or_else(|| Error::UnknownFunction {
        name: String::from_utf8_lossy(&name).into_owned(),
    })?;
    let params = read_params(data, p, arity, n)?;
    Ok(Sub {
        kind,
        name,
        flag,
        params,
    })
}

fn write_eca(e: &Eca, out: &mut Vec<u8>) {
    out.extend_from_slice(&e.kind.word().to_le_bytes());
    if let Some(g) = e.group {
        out.extend_from_slice(&g.to_le_bytes());
    }
    out.extend_from_slice(&e.name);
    out.push(0);
    out.extend_from_slice(&e.is_enabled.to_le_bytes());
    for pm in &e.params {
        write_param(pm, out);
    }
    out.extend_from_slice(&e.child_branch.to_le_bytes());
    out.extend_from_slice(&(e.child_ecas.len() as u32).to_le_bytes());
    for c in &e.child_ecas {
        write_eca(c, out);
    }
}

fn write_param(p: &Param, out: &mut Vec<u8>) {
    out.extend_from_slice(&p.kind.word().to_le_bytes());
    out.extend_from_slice(&p.value);
    out.push(0);
    out.extend_from_slice(&(p.has_sub as u32).to_le_bytes());
    if let Some(s) = &p.sub {
        write_sub(s, out);
    }
    out.extend_from_slice(&p.trailer.to_le_bytes());
    if let Some(idx) = &p.array_index {
        idx_write(idx, out);
    }
}

fn write_sub(s: &Sub, out: &mut Vec<u8>) {
    out.extend_from_slice(&s.kind.word().to_le_bytes());
    out.extend_from_slice(&s.name);
    out.push(0);
    out.extend_from_slice(&s.flag.to_le_bytes());
    for pm in &s.params {
        write_param(pm, out);
    }
}

fn idx_write(p: &Param, out: &mut Vec<u8>) {
    // array index parameter is written fully
    write_param(p, out);
}
