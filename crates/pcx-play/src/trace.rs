//! The trace schema and its canonical codec.
//!
//! See the crate module doc for the record grammar. This module owns the data
//! types, the encoder, the strict decoder and the trace hash.

use std::fmt;

use pcx_sim::Fx;

use crate::is_valid_name;

/// First line of every trace: `pcx-trace` followed by this version.
pub const TRACE_MAGIC: &str = "pcx-trace";

/// The format version this build reads and writes.
pub const TRACE_VERSION: u32 = 1;

/// FNV-1a 64 offset basis.
pub const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;

/// FNV-1a 64 prime.
pub const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Byte length of the canonical hex form of an RNG value: `0x` plus 16 digits.
const RNG_HEX_LEN: usize = 18;

/// FNV-1a 64 over `bytes`.
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Which asset set a run was given (plan §3.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetMode {
    Vanilla,
    Hd,
    Reforged,
}

impl AssetMode {
    pub fn as_str(self) -> &'static str {
        match self {
            AssetMode::Vanilla => "vanilla",
            AssetMode::Hd => "hd",
            AssetMode::Reforged => "reforged",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "vanilla" => Some(AssetMode::Vanilla),
            "hd" => Some(AssetMode::Hd),
            "reforged" => Some(AssetMode::Reforged),
            _ => None,
        }
    }
}

impl fmt::Display for AssetMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The verdict the fix ledger records per upstream change (plan §3.10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixDisposition {
    Adopt,
    Reject,
    Defer,
}

impl FixDisposition {
    pub fn as_str(self) -> &'static str {
        match self {
            FixDisposition::Adopt => "adopt",
            FixDisposition::Reject => "reject",
            FixDisposition::Defer => "defer",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "adopt" => Some(FixDisposition::Adopt),
            "reject" => Some(FixDisposition::Reject),
            "defer" => Some(FixDisposition::Defer),
            _ => None,
        }
    }
}

/// One active fix-ledger row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixRow {
    pub id: String,
    pub disposition: FixDisposition,
}

/// What happened to a timer during one tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerEventKind {
    Create,
    Fire,
    Destroy,
}

impl TimerEventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            TimerEventKind::Create => "create",
            TimerEventKind::Fire => "fire",
            TimerEventKind::Destroy => "destroy",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "create" => Some(TimerEventKind::Create),
            "fire" => Some(TimerEventKind::Fire),
            "destroy" => Some(TimerEventKind::Destroy),
            _ => None,
        }
    }
}

/// Everything the trace says before the first tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceHeader {
    pub format_version: u32,
    pub scenario: String,
    pub map: String,
    pub asset_mode: AssetMode,
    pub seed: u64,
    /// Active fix-ledger rows, sorted by id, without duplicates.
    pub fix_rows: Vec<FixRow>,
    pub tick_count: u64,
}

/// One unit's post-step state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitState {
    pub id: u32,
    pub owner: u8,
    pub x: Fx,
    pub y: Fx,
    /// The movement order, `None` when the unit is idle.
    pub target: Option<(Fx, Fx)>,
}

/// One draw from the shared sim RNG.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RngDraw {
    /// Draw ordinal within the whole run, counting from 0.
    pub index: u64,
    pub value: u64,
}

/// One timer event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimerEvent {
    /// Event ordinal within the whole run, counting from 0.
    pub seq: u64,
    pub kind: TimerEventKind,
    pub name: String,
}

/// Observations of one simulation step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TickRecord {
    /// Step index. Record `i` holds the state after step `i`.
    pub tick: u64,
    /// Post-step unit states, sorted by ascending id.
    pub units: Vec<UnitState>,
    /// RNG draws taken during the step, sorted by ascending index.
    pub rng_draws: Vec<RngDraw>,
    /// Timer events of the step, sorted by ascending sequence number.
    pub timers: Vec<TimerEvent>,
}

/// A complete play-test run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trace {
    pub header: TraceHeader,
    /// One record per step, labelled 0..header.tick_count.
    pub ticks: Vec<TickRecord>,
}

/// Why a byte string is not a canonical trace.
#[derive(Debug, thiserror::Error)]
pub enum TraceError {
    #[error("trace is empty")]
    Empty,
    #[error("trace is not valid UTF-8")]
    NotUtf8,
    #[error("trace does not end with a newline")]
    MissingFinalNewline,
    #[error("trace contains a carriage return; only LF line endings are canonical")]
    CarriageReturn,
    #[error("line {line}: empty line")]
    EmptyLine { line: usize },
    #[error("line {line}: unknown record tag {tag:?}")]
    UnknownTag { line: usize, tag: String },
    #[error("line {line}: record {tag:?} takes {want} field(s), got {got}")]
    BadFieldCount {
        line: usize,
        tag: String,
        want: String,
        got: usize,
    },
    #[error("line {line}: cannot read {field} from {value:?}")]
    BadValue {
        line: usize,
        field: &'static str,
        value: String,
    },
    #[error("line {line}: expected {want}, got {got:?}")]
    UnexpectedRecord {
        line: usize,
        want: &'static str,
        got: String,
    },
    #[error("trace ended after line {line}, expected {want}")]
    UnexpectedEnd { line: usize, want: &'static str },
    #[error("line {line}: unsupported trace version {got}; this build reads version {supported}")]
    UnsupportedVersion {
        line: usize,
        got: u32,
        supported: u32,
    },
    #[error("line {line}: {kind} name {value:?} is invalid")]
    BadName {
        line: usize,
        kind: &'static str,
        value: String,
    },
    #[error("line {line}: fix rows must be sorted by ascending id and unique")]
    FixOrder { line: usize },
    #[error("line {line}: expected tick record {want}, got {got}")]
    TickOrder { line: usize, want: u64, got: u64 },
    #[error("line {line}: units within a tick must be sorted by ascending id")]
    UnitOrder { line: usize },
    #[error("line {line}: RNG draw indexes must ascend across the whole run")]
    RngOrder { line: usize },
    #[error("line {line}: timer sequence numbers must ascend across the whole run")]
    TimerOrder { line: usize },
    #[error("header declares {declared} ticks but {found} tick records are present")]
    TickCountMismatch { declared: u64, found: u64 },
    #[error("line {line}: `end` must be the last record, got {got:?}")]
    TrailingRecords { line: usize, got: String },
}

impl Trace {
    /// The canonical serialisation.
    ///
    /// Record order and row order are fixed by the format, so the bytes are
    /// byte-stable for a given trace value.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = String::new();
        push_record(
            &mut out,
            TRACE_MAGIC,
            &[&self.header.format_version.to_string()],
        );
        push_record(&mut out, "scenario", &[&self.header.scenario]);
        push_record(&mut out, "map", &[&self.header.map]);
        push_record(&mut out, "asset-mode", &[self.header.asset_mode.as_str()]);
        push_record(&mut out, "seed", &[&self.header.seed.to_string()]);
        for row in &self.header.fix_rows {
            push_record(
                &mut out,
                "fix",
                &[row.disposition.as_str(), row.id.as_str()],
            );
        }
        push_record(&mut out, "ticks", &[&self.header.tick_count.to_string()]);
        for record in &self.ticks {
            push_record(&mut out, "tick", &[&record.tick.to_string()]);
            for u in &record.units {
                let mut fields = vec![
                    u.id.to_string(),
                    u.owner.to_string(),
                    u.x.to_string(),
                    u.y.to_string(),
                ];
                match u.target {
                    Some((tx, ty)) => {
                        fields.push(tx.to_string());
                        fields.push(ty.to_string());
                    }
                    None => fields.push("-".to_string()),
                }
                let refs: Vec<&str> = fields.iter().map(String::as_str).collect();
                push_record(&mut out, "unit", &refs);
            }
            for draw in &record.rng_draws {
                push_record(
                    &mut out,
                    "rng",
                    &[&draw.index.to_string(), &fmt_hex(draw.value)],
                );
            }
            for event in &record.timers {
                push_record(
                    &mut out,
                    "timer",
                    &[&event.seq.to_string(), event.kind.as_str(), &event.name],
                );
            }
        }
        out.push_str("end\n");
        out.into_bytes()
    }

    /// FNV-1a 64 over [`Trace::encode`], the identity two runs are compared by.
    pub fn canonical_hash(&self) -> u64 {
        fnv1a64(&self.encode())
    }

    /// Decodes a canonical trace, refusing anything that is not exactly the
    /// form [`Trace::encode`] writes.
    pub fn decode(bytes: &[u8]) -> Result<Trace, TraceError> {
        if bytes.is_empty() {
            return Err(TraceError::Empty);
        }
        if bytes.last() != Some(&b'\n') {
            return Err(TraceError::MissingFinalNewline);
        }
        if bytes.contains(&b'\r') {
            return Err(TraceError::CarriageReturn);
        }
        let text = std::str::from_utf8(bytes).map_err(|_| TraceError::NotUtf8)?;
        let lines: Vec<&str> = text[..text.len() - 1].split('\n').collect();
        let mut at = 0usize;

        let (line, raw) = take_line(&lines, &mut at, "pcx-trace <version>")?;
        let (tag, fields) = split_record(line, raw)?;
        if tag != TRACE_MAGIC {
            return Err(TraceError::UnexpectedRecord {
                line,
                want: "pcx-trace <version>",
                got: raw.to_string(),
            });
        }
        expect_fields(line, TRACE_MAGIC, &fields, 1)?;
        let version: u32 = parse_value(line, "version", &fields[0])?;
        if version != TRACE_VERSION {
            return Err(TraceError::UnsupportedVersion {
                line,
                got: version,
                supported: TRACE_VERSION,
            });
        }

        let (line, raw) = take_line(&lines, &mut at, "scenario <name>")?;
        let (tag, fields) = split_record(line, raw)?;
        expect_tag(line, &tag, "scenario", raw)?;
        expect_fields(line, "scenario", &fields, 1)?;
        let scenario = checked_name(line, "scenario", &fields[0])?;

        let (line, raw) = take_line(&lines, &mut at, "map <name>")?;
        let (tag, fields) = split_record(line, raw)?;
        expect_tag(line, &tag, "map", raw)?;
        expect_fields(line, "map", &fields, 1)?;
        let map = checked_name(line, "map", &fields[0])?;

        let (line, raw) = take_line(&lines, &mut at, "asset-mode <mode>")?;
        let (tag, fields) = split_record(line, raw)?;
        expect_tag(line, &tag, "asset-mode", raw)?;
        expect_fields(line, "asset-mode", &fields, 1)?;
        let asset_mode = AssetMode::parse(&fields[0]).ok_or_else(|| TraceError::BadValue {
            line,
            field: "asset-mode",
            value: fields[0].clone(),
        })?;

        let (line, raw) = take_line(&lines, &mut at, "seed <u64>")?;
        let (tag, fields) = split_record(line, raw)?;
        expect_tag(line, &tag, "seed", raw)?;
        expect_fields(line, "seed", &fields, 1)?;
        let seed: u64 = parse_value(line, "seed", &fields[0])?;

        let mut fix_rows: Vec<FixRow> = Vec::new();
        let tick_count;
        loop {
            let (line, raw) = take_line(&lines, &mut at, "fix <disposition> <id> | ticks <n>")?;
            let (tag, fields) = split_record(line, raw)?;
            if tag != "fix" {
                expect_tag(line, &tag, "ticks", raw)?;
                expect_fields(line, "ticks", &fields, 1)?;
                tick_count = parse_value(line, "ticks", &fields[0])?;
                break;
            }
            expect_fields(line, "fix", &fields, 2)?;
            let disposition =
                FixDisposition::parse(&fields[0]).ok_or_else(|| TraceError::BadValue {
                    line,
                    field: "disposition",
                    value: fields[0].clone(),
                })?;
            let id = checked_name(line, "fix row id", &fields[1])?;
            if fix_rows
                .last()
                .is_some_and(|last: &FixRow| last.id.as_str() >= id.as_str())
            {
                return Err(TraceError::FixOrder { line });
            }
            fix_rows.push(FixRow { id, disposition });
        }

        let mut ticks: Vec<TickRecord> = Vec::new();
        let mut current: Option<TickRecord> = None;
        let mut last_unit_id: Option<u32> = None;
        let mut last_rng_index: Option<u64> = None;
        let mut last_timer_seq: Option<u64> = None;
        loop {
            let (line, raw) = take_line(&lines, &mut at, "tick | unit | rng | timer | end")?;
            let (tag, fields) = split_record(line, raw)?;
            match tag.as_str() {
                "tick" => {
                    if let Some(record) = current.replace(TickRecord {
                        tick: 0,
                        units: Vec::new(),
                        rng_draws: Vec::new(),
                        timers: Vec::new(),
                    }) {
                        ticks.push(record);
                    }
                    expect_fields(line, "tick", &fields, 1)?;
                    let got: u64 = parse_value(line, "tick", &fields[0])?;
                    let want = ticks.len() as u64;
                    if got != want {
                        return Err(TraceError::TickOrder { line, want, got });
                    }
                    current.as_mut().expect("just replaced").tick = got;
                    last_unit_id = None;
                }
                "unit" => {
                    let record = current
                        .as_mut()
                        .ok_or_else(|| TraceError::UnexpectedRecord {
                            line,
                            want: "tick",
                            got: raw.to_string(),
                        })?;
                    let unit = parse_unit(line, &fields)?;
                    if last_unit_id.is_some_and(|last| unit.id <= last) {
                        return Err(TraceError::UnitOrder { line });
                    }
                    last_unit_id = Some(unit.id);
                    record.units.push(unit);
                }
                "rng" => {
                    let record = current
                        .as_mut()
                        .ok_or_else(|| TraceError::UnexpectedRecord {
                            line,
                            want: "tick",
                            got: raw.to_string(),
                        })?;
                    expect_fields(line, "rng", &fields, 2)?;
                    let index: u64 = parse_value(line, "index", &fields[0])?;
                    if last_rng_index.is_some_and(|last| index <= last) {
                        return Err(TraceError::RngOrder { line });
                    }
                    last_rng_index = Some(index);
                    let value = parse_hex(line, &fields[1])?;
                    record.rng_draws.push(RngDraw { index, value });
                }
                "timer" => {
                    let record = current
                        .as_mut()
                        .ok_or_else(|| TraceError::UnexpectedRecord {
                            line,
                            want: "tick",
                            got: raw.to_string(),
                        })?;
                    expect_fields(line, "timer", &fields, 3)?;
                    let seq: u64 = parse_value(line, "seq", &fields[0])?;
                    if last_timer_seq.is_some_and(|last| seq <= last) {
                        return Err(TraceError::TimerOrder { line });
                    }
                    last_timer_seq = Some(seq);
                    let kind =
                        TimerEventKind::parse(&fields[1]).ok_or_else(|| TraceError::BadValue {
                            line,
                            field: "timer kind",
                            value: fields[1].clone(),
                        })?;
                    let name = checked_name(line, "timer", &fields[2])?;
                    record.timers.push(TimerEvent { seq, kind, name });
                }
                "end" => {
                    if let Some(record) = current.take() {
                        ticks.push(record);
                    }
                    expect_fields(line, "end", &fields, 0)?;
                    if at < lines.len() {
                        return Err(TraceError::TrailingRecords {
                            line: line + 1,
                            got: lines[at].to_string(),
                        });
                    }
                    if ticks.len() as u64 != tick_count {
                        return Err(TraceError::TickCountMismatch {
                            declared: tick_count,
                            found: ticks.len() as u64,
                        });
                    }
                    return Ok(Trace {
                        header: TraceHeader {
                            format_version: version,
                            scenario,
                            map,
                            asset_mode,
                            seed,
                            fix_rows,
                            tick_count,
                        },
                        ticks,
                    });
                }
                _ => {
                    return Err(TraceError::UnknownTag {
                        line,
                        tag: tag.to_string(),
                    });
                }
            }
        }
    }
}

fn push_record(out: &mut String, tag: &str, fields: &[&str]) {
    out.push_str(tag);
    for field in fields {
        out.push(' ');
        out.push_str(field);
    }
    out.push('\n');
}

fn fmt_hex(value: u64) -> String {
    format!("{value:#018x}")
}

fn take_line<'a>(
    lines: &[&'a str],
    at: &mut usize,
    want: &'static str,
) -> Result<(usize, &'a str), TraceError> {
    let line = *at + 1;
    match lines.get(*at) {
        Some(raw) => {
            *at += 1;
            if raw.is_empty() {
                return Err(TraceError::EmptyLine { line });
            }
            Ok((line, raw))
        }
        None => Err(TraceError::UnexpectedEnd { line, want }),
    }
}

fn split_record(line: usize, raw: &str) -> Result<(String, Vec<String>), TraceError> {
    let mut parts = raw.split(' ');
    let tag = parts.next().unwrap_or_default().to_string();
    let fields: Vec<String> = parts.map(|s| s.to_string()).collect();
    if fields.iter().any(|f| f.is_empty()) {
        return Err(TraceError::BadValue {
            line,
            field: "field",
            value: raw.to_string(),
        });
    }
    Ok((tag, fields))
}

fn expect_tag(line: usize, got: &str, want: &'static str, raw: &str) -> Result<(), TraceError> {
    if got == want {
        Ok(())
    } else {
        Err(TraceError::UnexpectedRecord {
            line,
            want,
            got: raw.to_string(),
        })
    }
}

fn expect_fields(line: usize, tag: &str, fields: &[String], want: usize) -> Result<(), TraceError> {
    if fields.len() == want {
        Ok(())
    } else {
        Err(TraceError::BadFieldCount {
            line,
            tag: tag.to_string(),
            want: want.to_string(),
            got: fields.len(),
        })
    }
}

fn parse_value<T: std::str::FromStr>(
    line: usize,
    field: &'static str,
    raw: &str,
) -> Result<T, TraceError> {
    raw.parse().map_err(|_| TraceError::BadValue {
        line,
        field,
        value: raw.to_string(),
    })
}

fn checked_name(line: usize, kind: &'static str, raw: &str) -> Result<String, TraceError> {
    if is_valid_name(raw) {
        Ok(raw.to_string())
    } else {
        Err(TraceError::BadName {
            line,
            kind,
            value: raw.to_string(),
        })
    }
}

fn parse_hex(line: usize, raw: &str) -> Result<u64, TraceError> {
    let bad = || TraceError::BadValue {
        line,
        field: "rng value",
        value: raw.to_string(),
    };
    if raw.len() != RNG_HEX_LEN || !raw.starts_with("0x") {
        return Err(bad());
    }
    let digits = &raw[2..];
    if !digits
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(bad());
    }
    u64::from_str_radix(digits, 16).map_err(|_| bad())
}

fn parse_unit(line: usize, fields: &[String]) -> Result<UnitState, TraceError> {
    if fields.len() != 5 && fields.len() != 6 {
        return Err(TraceError::BadFieldCount {
            line,
            tag: "unit".to_string(),
            want: "5 or 6".to_string(),
            got: fields.len(),
        });
    }
    let id: u32 = parse_value(line, "id", &fields[0])?;
    let owner: u8 = parse_value(line, "owner", &fields[1])?;
    let x = parse_value(line, "x", &fields[2])?;
    let y = parse_value(line, "y", &fields[3])?;
    let target = if fields.len() == 6 {
        let tx = parse_value(line, "target x", &fields[4])?;
        let ty = parse_value(line, "target y", &fields[5])?;
        Some((tx, ty))
    } else {
        if fields[4] != "-" {
            return Err(TraceError::BadValue {
                line,
                field: "target",
                value: fields[4].clone(),
            });
        }
        None
    };
    Ok(UnitState {
        id,
        owner,
        x,
        y,
        target,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Trace {
        Trace {
            header: TraceHeader {
                format_version: TRACE_VERSION,
                scenario: "sample".into(),
                map: "synthetic.sample".into(),
                asset_mode: AssetMode::Vanilla,
                seed: 7,
                fix_rows: vec![FixRow {
                    id: "fix.a".into(),
                    disposition: FixDisposition::Adopt,
                }],
                tick_count: 1,
            },
            ticks: vec![TickRecord {
                tick: 0,
                units: vec![UnitState {
                    id: 1,
                    owner: 0,
                    x: -3,
                    y: 65536,
                    target: Some((9, -9)),
                }],
                rng_draws: vec![RngDraw {
                    index: 0,
                    value: 0xdead_beef_0000_0001,
                }],
                timers: vec![TimerEvent {
                    seq: 0,
                    kind: TimerEventKind::Fire,
                    name: "t.a".into(),
                }],
            }],
        }
    }

    #[test]
    fn a_trace_survives_a_canonical_round_trip() {
        let trace = sample();
        let bytes = trace.encode();
        let back = Trace::decode(&bytes).unwrap_or_else(|e| panic!("{}", e));
        assert_eq!(back, trace, "round trip changed the trace");
        assert_eq!(
            back.encode(),
            bytes,
            "the decoder must accept only canonical form"
        );
    }

    #[test]
    fn the_hash_is_fnv_1a_64_over_the_canonical_bytes() {
        assert_eq!(fnv1a64(b""), FNV_OFFSET_BASIS);
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a64(b"foobar"), 0x8594_4171_f739_67e8);
        let trace = sample();
        assert_eq!(trace.canonical_hash(), fnv1a64(&trace.encode()));
    }

    /// Assembles a trace text without the `end` record.
    fn text(version: u32, header: &[&str], body: &[&str]) -> String {
        let mut out = format!("pcx-trace {version}\n");
        for line in header {
            out.push_str(line);
            out.push('\n');
        }
        for line in body {
            out.push_str(line);
            out.push('\n');
        }
        out
    }

    #[test]
    fn a_malformed_trace_is_refused() {
        let header = [
            "scenario sample",
            "map synthetic.sample",
            "asset-mode vanilla",
            "seed 7",
            "ticks 1",
        ];
        let fix_header = [
            "scenario sample",
            "map synthetic.sample",
            "asset-mode vanilla",
            "seed 7",
            "fix adopt fix.b",
            "fix adopt fix.a",
            "ticks 1",
        ];
        let two_tick_header = [
            "scenario sample",
            "map synthetic.sample",
            "asset-mode vanilla",
            "seed 7",
            "ticks 2",
        ];
        let cases: Vec<(&str, String)> = vec![
            ("wrong version", text(2, &header, &[])),
            (
                "unknown tag",
                text(
                    1,
                    &[
                        "scenario sample",
                        "mapp synthetic.sample",
                        "asset-mode vanilla",
                        "seed 7",
                        "ticks 1",
                    ],
                    &["tick 0"],
                ),
            ),
            (
                "unsorted units",
                text(1, &header, &["tick 0", "unit 2 0 0 0 -", "unit 1 0 0 0 -"]),
            ),
            (
                "rng order",
                text(
                    1,
                    &header,
                    &[
                        "tick 0",
                        "rng 1 0x0000000000000002",
                        "rng 0 0x0000000000000001",
                    ],
                ),
            ),
            (
                "timer order",
                text(
                    1,
                    &header,
                    &["tick 0", "timer 1 fire t.b", "timer 0 fire t.a"],
                ),
            ),
            ("fix order", text(1, &fix_header, &["tick 0"])),
            ("tick label", text(1, &header, &["tick 5"])),
            (
                "tick count mismatch",
                text(1, &two_tick_header, &["tick 0"]),
            ),
            (
                "trailing record",
                format!("{}end\ntick 1\n", text(1, &header, &["tick 0"])),
            ),
            (
                "bad name",
                text(
                    1,
                    &[
                        "scenario sample!",
                        "map synthetic.sample",
                        "asset-mode vanilla",
                        "seed 7",
                        "ticks 1",
                    ],
                    &["tick 0"],
                ),
            ),
            (
                "bad rng hex",
                text(1, &header, &["tick 0", "rng 0 0xDEADBEEF"]),
            ),
            (
                "bad field count",
                text(1, &header, &["tick 0", "unit 1 0 0"]),
            ),
            ("missing end", text(1, &header, &["tick 0"])),
        ];
        for (label, input) in cases {
            assert!(
                Trace::decode(input.as_bytes()).is_err(),
                "{label} must be refused"
            );
        }
    }

    #[test]
    fn an_idle_unit_round_trips_its_dash_target() {
        let input = format!(
            "{}end\n",
            text(
                1,
                &[
                    "scenario sample",
                    "map synthetic.sample",
                    "asset-mode vanilla",
                    "seed 7",
                    "ticks 1",
                ],
                &["tick 0", "unit 1 0 0 0 -"],
            )
        );
        let trace = Trace::decode(input.as_bytes()).unwrap_or_else(|e| panic!("{}", e));
        assert_eq!(trace.ticks[0].units[0].target, None);
        assert_eq!(String::from_utf8(trace.encode()).unwrap(), input);
    }

    #[test]
    fn a_truncated_or_cramped_file_is_refused() {
        assert!(matches!(Trace::decode(b""), Err(TraceError::Empty)));
        assert!(matches!(
            Trace::decode(b"pcx-trace 1"),
            Err(TraceError::MissingFinalNewline)
        ));
        let cr = "pcx-trace 1\r\n";
        assert!(matches!(
            Trace::decode(cr.as_bytes()),
            Err(TraceError::CarriageReturn)
        ));
    }
}
