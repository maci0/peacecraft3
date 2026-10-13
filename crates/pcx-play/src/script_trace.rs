//! Strict decoding and comparison for source-aware chapter execution traces.
use crate::campaign::{CallRecord, ChapterTrace, Gate, NativeRecord};
use crate::scripts::{AssetIdentity, SourceDigest};
use pcx_script::{HandleId, Value, value::FunctionId};

impl ChapterTrace {
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        let text =
            std::str::from_utf8(bytes).map_err(|e| format!("script trace is not UTF-8: {e}"))?;
        let mut lines = text.lines();
        if lines.next() != Some("pcx-script-trace 1") {
            return Err("expected pcx-script-trace 1".into());
        }
        let mut field = |name: &str| -> Result<String, String> {
            let line = lines.next().ok_or_else(|| format!("missing {name}"))?;
            let words: Vec<_> = line.split_whitespace().collect();
            if words.len() != 2 || words[0] != name {
                return Err(format!("expected {name}, found {line:?}"));
            }
            Ok(words[1].to_string())
        };
        let chapter = unhex(&field("chapter")?)?;
        let edition = unhex(&field("edition")?)?;
        let version = unhex(&field("version")?)?;
        let source = unhex(&field("asset-source")?)?;
        let ticks = number(&field("ticks")?)?;
        if chapter.is_empty()
            || edition.is_empty()
            || version.is_empty()
            || source.is_empty()
            || ticks == 0
        {
            return Err("script trace identity and positive tick count are required".into());
        }
        let mut trace = Self {
            chapter,
            identity: AssetIdentity {
                edition,
                version,
                source,
            },
            sources: Vec::new(),
            ticks,
            calls: Vec::new(),
            natives: Vec::new(),
            gates: Vec::new(),
        };
        let mut sequence = 0;
        let mut phase = 0;
        for (index, line) in lines.enumerate() {
            let error = |message: String| format!("script trace line {}: {message}", index + 7);
            let words: Vec<_> = line.split_whitespace().collect();
            (|| -> Result<(), String> {
                match words.as_slice() {
                    ["source", label, hash] if phase == 0 => {
                        let label = unhex(label)?;
                        if label.is_empty() || trace.sources.iter().any(|s| s.label == label) {
                            return Err("empty or duplicate source label".into());
                        }
                        trace.sources.push(SourceDigest {
                            label,
                            hash: u64::from_str_radix(hash, 16)
                                .map_err(|_| "invalid source checksum")?,
                        });
                    }
                    ["event", seq, "call", tick, call_phase, function, result] if phase <= 1 => {
                        phase = 1;
                        if number::<u64>(seq)? != sequence {
                            return Err(format!("expected event sequence {sequence}"));
                        }
                        let tick = parse_tick(tick, ticks)?;
                        trace.calls.push(CallRecord {
                            sequence,
                            tick,
                            phase: unhex(call_phase)?,
                            function: unhex(function)?,
                            result: parse_value(result)?,
                        });
                        sequence += 1;
                    }
                    ["event", seq, "native", tick, name, count, rest @ ..] if phase <= 1 => {
                        phase = 1;
                        if number::<u64>(seq)? != sequence {
                            return Err(format!("expected event sequence {sequence}"));
                        }
                        let tick = parse_tick(tick, ticks)?;
                        let count: usize = number(count)?;
                        if rest.len().checked_sub(2) != Some(count)
                            || rest.get(count) != Some(&"result")
                        {
                            return Err(
                                "native argument count or result delimiter is invalid".into()
                            );
                        }
                        let args = rest[..count]
                            .iter()
                            .map(|v| parse_value(v))
                            .collect::<Result<_, _>>()?;
                        trace.natives.push(NativeRecord {
                            sequence,
                            tick,
                            name: unhex(name)?,
                            args,
                            result: parse_value(rest[count + 1])?,
                        });
                        sequence += 1;
                    }
                    ["gate", tick, gate, predicate] => {
                        phase = 2;
                        let tick = number(tick)?;
                        if tick >= ticks {
                            return Err("gate tick outside chapter".into());
                        }
                        let gate = match *gate {
                            "Dialogue" => Gate::Dialogue,
                            "Quest" => Gate::Quest,
                            "Victory" => Gate::Victory,
                            "EndGame" => Gate::EndGame,
                            _ => return Err(format!("unknown gate {gate:?}")),
                        };
                        trace.gates.push((tick, gate, unhex(predicate)?));
                    }
                    _ => return Err(format!("unsupported or out-of-order record {line:?}")),
                }
                Ok(())
            })()
            .map_err(error)?;
        }
        if trace.encode() != bytes {
            return Err("script trace is not canonical".into());
        }
        Ok(trace)
    }
}
fn number<T: std::str::FromStr>(text: &str) -> Result<T, String> {
    text.parse()
        .map_err(|_| format!("invalid integer {text:?}"))
}
fn unhex(text: &str) -> Result<String, String> {
    if text == "-" {
        return Ok(String::new());
    }
    if !text.len().is_multiple_of(2) || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("invalid hex text {text:?}"));
    }
    let mut bytes = Vec::new();
    for chunk in text.as_bytes().chunks_exact(2) {
        let value = std::str::from_utf8(chunk).map_err(|_| "invalid hex byte")?;
        bytes.push(u8::from_str_radix(value, 16).map_err(|_| "invalid hex byte")?);
    }
    String::from_utf8(bytes).map_err(|_| "hex text is not UTF-8".into())
}
fn parse_tick(text: &str, ticks: u64) -> Result<Option<u64>, String> {
    if text == "init" {
        return Ok(None);
    }
    let tick = number(text)?;
    if tick >= ticks {
        return Err(format!("event tick {tick} outside chapter"));
    }
    Ok(Some(tick))
}
fn parse_value(text: &str) -> Result<Value, String> {
    if text == "null" {
        return Ok(Value::Null);
    }
    let (kind, value) = text
        .split_once(':')
        .ok_or_else(|| format!("invalid value {text:?}"))?;
    match kind {
        "bool" => match value {
            "true" => Ok(Value::Bool(true)),
            "false" => Ok(Value::Bool(false)),
            _ => Err("invalid boolean".into()),
        },
        "int" => Ok(Value::Int(number(value)?)),
        "real" => Ok(Value::Real(f32::from_bits(
            u32::from_str_radix(value, 16).map_err(|_| "invalid real bits")?,
        ))),
        "string" => Ok(Value::Str(unhex(value)?)),
        "code" => Ok(Value::Code(FunctionId(number(value)?))),
        "host-handle" => Ok(Value::Handle(HandleId::new(
            std::num::NonZeroU64::new(number(value)?)
                .ok_or("a null handle must be spelled null")?,
        ))),
        _ => Err(format!("unknown value class {kind:?}")),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptDivergence {
    pub line: usize,
    pub tick: Option<u64>,
    pub field: String,
    pub left: String,
    pub right: String,
}
impl std::fmt::Display for ScriptDivergence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let tick = self
            .tick
            .map(|t| t.to_string())
            .unwrap_or_else(|| "header/init".into());
        write!(
            f,
            "diverge tick={tick} field={} line={} left={:?} right={:?}",
            self.field, self.line, self.left, self.right
        )
    }
}
/// Compare canonical bytes, preserving NaN payloads and signed zero.
pub fn first_script_divergence(
    left: &ChapterTrace,
    right: &ChapterTrace,
) -> Option<ScriptDivergence> {
    let left = String::from_utf8(left.encode()).expect("encoder emits UTF-8");
    let right = String::from_utf8(right.encode()).expect("encoder emits UTF-8");
    let a: Vec<_> = left.lines().collect();
    let b: Vec<_> = right.lines().collect();
    for index in 0..a.len().max(b.len()) {
        if a.get(index) == b.get(index) {
            continue;
        }
        let source = a.get(index).or_else(|| b.get(index)).copied().unwrap_or("");
        let words: Vec<_> = source.split_whitespace().collect();
        let (tick, field) = if words.first() == Some(&"event") && words.len() > 4 {
            let tick = words[3].parse().ok();
            let label = if words[2] == "native" {
                words[4]
            } else {
                words.get(5).copied().unwrap_or("-")
            };
            (
                tick,
                format!(
                    "script.event[{}].{}:{}",
                    words[1],
                    words[2],
                    unhex(label).unwrap_or_else(|_| label.into())
                ),
            )
        } else {
            (
                None,
                format!("script.{}", words.first().copied().unwrap_or("end")),
            )
        };
        return Some(ScriptDivergence {
            line: index + 1,
            tick,
            field,
            left: a.get(index).copied().unwrap_or("<end>").into(),
            right: b.get(index).copied().unwrap_or("<end>").into(),
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> ChapterTrace {
        ChapterTrace {
            chapter: "synthetic trace".into(),
            identity: AssetIdentity {
                edition: "synthetic".into(),
                version: "test".into(),
                source: "memory".into(),
            },
            sources: vec![SourceDigest {
                label: "local map".into(),
                hash: 5,
            }],
            ticks: 1,
            calls: vec![CallRecord {
                sequence: 1,
                tick: Some(0),
                phase: "entry".into(),
                function: "Entry".into(),
                result: Value::Real(f32::from_bits(0x7fc01234)),
            }],
            natives: vec![NativeRecord {
                sequence: 0,
                tick: Some(0),
                name: "Native".into(),
                args: vec![
                    Value::Str("\nα\t".into()),
                    Value::Handle(HandleId::new(std::num::NonZeroU64::new(9).unwrap())),
                    Value::Code(FunctionId(2)),
                    Value::Bool(false),
                    Value::Real(-0.0),
                ],
                result: Value::Int(-3),
            }],
            gates: vec![(0, Gate::Quest, "Observed".into())],
        }
    }
    #[test]
    fn canonical_roundtrip_preserves_all_classes_nan_bits_and_signed_zero() {
        let trace = fixture();
        let bytes = trace.encode();
        let decoded = ChapterTrace::decode(&bytes).unwrap();
        assert_eq!(bytes, decoded.encode());
        assert!(first_script_divergence(&trace, &decoded).is_none());
        let mut different = decoded;
        different.natives[0].result = Value::Int(4);
        let divergence = first_script_divergence(&trace, &different).unwrap();
        assert_eq!(divergence.tick, Some(0));
        assert_eq!(divergence.field, "script.event[0].native:Native");
    }
    #[test]
    fn corrupt_noncanonical_and_reordered_traces_are_refused() {
        let bytes = fixture().encode();
        let text = String::from_utf8(bytes).unwrap();
        for malformed in [
            text.replace("event 0", "event 7"),
            text.replace("host-handle:9", "host-handle:0"),
            text.replace("bool:false", "bool:maybe"),
            text.replace("event 0 native 0", "event 0 native 1"),
            text.replace("\n", "\r\n"),
            text.trim_end().to_string(),
            text.replace(
                "source 6c6f63616c206d6170 0000000000000005",
                "source 6c6f63616c206d6170 5",
            ),
        ] {
            assert!(
                ChapterTrace::decode(malformed.as_bytes()).is_err(),
                "accepted {malformed:?}"
            );
        }
    }
}
