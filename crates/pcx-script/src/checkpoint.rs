//! Bounded portable fields shared by the complete-world checkpoint codec.
use crate::{HandleId, Value, value::FunctionId};

/// Allocation and work limits of the recreation's file format, not game rules.
#[derive(Clone, Copy, Debug)]
pub struct Budget {
    pub bytes: usize,
    pub items: usize,
    pub depth: usize,
}

struct Remaining {
    owned: usize,
    items: usize,
    depth: usize,
}
impl Remaining {
    fn new(budget: Budget) -> Result<Self, String> {
        if budget.depth > crate::eval::MAX_CALL_DEPTH as usize {
            return Err("checkpoint nesting policy exceeds the VM guard".into());
        }
        Ok(Self {
            owned: budget.bytes,
            items: budget.items,
            depth: budget.depth,
        })
    }
    fn owned(&mut self, bytes: usize) -> Result<(), String> {
        self.owned = self
            .owned
            .checked_sub(bytes)
            .ok_or("checkpoint allocation policy exceeded")?;
        Ok(())
    }
    fn items(&mut self, count: usize) -> Result<(), String> {
        self.items = self
            .items
            .checked_sub(count)
            .ok_or("checkpoint item policy exceeded")?;
        Ok(())
    }
    fn depth(&self, depth: usize) -> Result<(), String> {
        if depth > self.depth {
            Err("checkpoint nesting policy exceeded".into())
        } else {
            Ok(())
        }
    }
}

pub struct Writer {
    bytes: Vec<u8>,
    limit: usize,
    remaining: Remaining,
}
impl Writer {
    pub fn new(budget: Budget) -> Result<Self, String> {
        Ok(Self {
            bytes: Vec::new(),
            limit: budget.bytes,
            remaining: Remaining::new(budget)?,
        })
    }
    pub fn finish(self) -> Vec<u8> {
        self.bytes
    }
    pub fn raw(&mut self, bytes: &[u8]) -> Result<(), String> {
        if self
            .bytes
            .len()
            .checked_add(bytes.len())
            .is_none_or(|end| end > self.limit)
        {
            return Err("checkpoint byte policy exceeded".into());
        }
        self.bytes
            .try_reserve(bytes.len())
            .map_err(|_| "checkpoint output allocation failed")?;
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    pub fn byte(&mut self, value: u8) -> Result<(), String> {
        self.raw(&[value])
    }
    pub fn boolean(&mut self, value: bool) -> Result<(), String> {
        self.byte(u8::from(value))
    }
    pub fn u32(&mut self, value: u32) -> Result<(), String> {
        self.raw(&value.to_le_bytes())
    }
    pub fn u64(&mut self, value: u64) -> Result<(), String> {
        self.raw(&value.to_le_bytes())
    }
    pub fn i32(&mut self, value: i32) -> Result<(), String> {
        self.raw(&value.to_le_bytes())
    }
    pub fn real(&mut self, value: f32) -> Result<(), String> {
        self.u32(value.to_bits())
    }
    pub fn list(&mut self, count: usize) -> Result<(), String> {
        self.remaining.items(count)?;
        self.u32(u32::try_from(count).map_err(|_| "checkpoint list length overflow")?)
    }
    pub fn text(&mut self, text: &str) -> Result<(), String> {
        self.blob(text.as_bytes())
    }
    pub fn blob(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.owned(bytes.len())?;
        self.u32(u32::try_from(bytes.len()).map_err(|_| "checkpoint string length overflow")?)?;
        self.raw(bytes)
    }
    pub fn owned(&mut self, bytes: usize) -> Result<(), String> {
        self.remaining.owned(bytes)
    }
    pub fn items(&mut self, count: usize) -> Result<(), String> {
        self.remaining.items(count)
    }
    pub fn depth(&self, depth: usize) -> Result<(), String> {
        self.remaining.depth(depth)
    }
    pub fn value(&mut self, value: &Value) -> Result<(), String> {
        self.items(1)?;
        match value {
            Value::Null => self.byte(0),
            Value::Bool(value) => {
                self.byte(1)?;
                self.boolean(*value)
            }
            Value::Int(value) => {
                self.byte(2)?;
                self.i32(*value)
            }
            Value::Real(value) => {
                self.byte(3)?;
                self.real(*value)
            }
            Value::Str(value) => {
                self.byte(4)?;
                self.blob(value.as_bytes())
            }
            Value::Code(value) => {
                self.byte(5)?;
                self.u32(value.0)
            }
            Value::Handle(value) => {
                self.byte(6)?;
                self.u64(value.token())
            }
        }
    }
}

pub struct Reader<'a> {
    bytes: &'a [u8],
    cursor: usize,
    remaining: Remaining,
}
impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8], budget: Budget) -> Result<Self, String> {
        if bytes.len() > budget.bytes {
            return Err("checkpoint byte policy exceeded".into());
        }
        Ok(Self {
            bytes,
            cursor: 0,
            remaining: Remaining::new(budget)?,
        })
    }
    pub fn finish(self) -> Result<(), String> {
        if self.cursor == self.bytes.len() {
            Ok(())
        } else {
            Err("trailing checkpoint bytes".into())
        }
    }
    pub fn raw(&mut self, count: usize) -> Result<&'a [u8], String> {
        let end = self
            .cursor
            .checked_add(count)
            .ok_or("checkpoint offset overflow")?;
        let result = self
            .bytes
            .get(self.cursor..end)
            .ok_or("truncated checkpoint")?;
        self.cursor = end;
        Ok(result)
    }
    pub fn byte(&mut self) -> Result<u8, String> {
        Ok(self.raw(1)?[0])
    }
    pub fn boolean(&mut self) -> Result<bool, String> {
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err("invalid checkpoint boolean".into()),
        }
    }
    pub fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(
            self.raw(4)?.try_into().expect("four bytes"),
        ))
    }
    pub fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(
            self.raw(8)?.try_into().expect("eight bytes"),
        ))
    }
    pub fn i32(&mut self) -> Result<i32, String> {
        Ok(i32::from_le_bytes(
            self.raw(4)?.try_into().expect("four bytes"),
        ))
    }
    pub fn real(&mut self) -> Result<f32, String> {
        self.u32().map(f32::from_bits)
    }
    pub fn list(&mut self) -> Result<usize, String> {
        let count = usize::try_from(self.u32()?).map_err(|_| "checkpoint list length overflow")?;
        self.items(count)?;
        Ok(count)
    }
    pub fn text(&mut self) -> Result<String, String> {
        String::from_utf8(self.blob()?)
            .map_err(|_| "checkpoint metadata string is not UTF-8".into())
    }
    pub fn blob(&mut self) -> Result<Vec<u8>, String> {
        let count =
            usize::try_from(self.u32()?).map_err(|_| "checkpoint string length overflow")?;
        self.owned(count)?;
        let raw = self.raw(count)?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(count)
            .map_err(|_| "checkpoint string allocation failed")?;
        result.extend_from_slice(raw);
        Ok(result)
    }
    pub fn owned(&mut self, bytes: usize) -> Result<(), String> {
        self.remaining.owned(bytes)
    }
    pub fn items(&mut self, count: usize) -> Result<(), String> {
        self.remaining.items(count)
    }
    pub fn depth(&self, depth: usize) -> Result<(), String> {
        self.remaining.depth(depth)
    }
    /// The arena owner validates even retired identities; liveness remains a native read point.
    pub fn value(
        &mut self,
        functions: usize,
        mut handle: impl FnMut(HandleId) -> Result<(), String>,
    ) -> Result<Value, String> {
        self.items(1)?;
        match self.byte()? {
            0 => Ok(Value::Null),
            1 => self.boolean().map(Value::Bool),
            2 => self.i32().map(Value::Int),
            3 => self.real().map(Value::Real),
            4 => String::from_utf8(self.blob()?)
                .map(Value::Str)
                .map_err(|_| "checkpoint script string is not UTF-8 in value format 1".into()),
            5 => {
                let id = FunctionId(self.u32()?);
                if id.index() >= functions {
                    return Err("checkpoint code belongs to another program".into());
                }
                Ok(Value::Code(id))
            }
            6 => {
                let token =
                    std::num::NonZeroU64::new(self.u64()?).ok_or("null checkpoint handle token")?;
                let id = HandleId::new(token);
                handle(id)?;
                Ok(Value::Handle(id))
            }
            _ => Err("unknown checkpoint value tag".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const BUDGET: Budget = Budget {
        bytes: 4096,
        items: 64,
        depth: 8,
    };
    #[test]
    fn portable_values_keep_all_string_bytes_and_exact_real_payloads() {
        let values = [
            Value::Null,
            Value::Str(String::new()),
            Value::Str("é\0TRIGSTR_7^^tag^^".into()),
            Value::Bool(true),
            Value::Int(i32::MIN),
            Value::Real(f32::from_bits(0x80000000)),
            Value::Real(f32::from_bits(0x7fc01234)),
            Value::Code(FunctionId(1)),
            Value::Handle(HandleId::new(std::num::NonZeroU64::new(7).unwrap())),
        ];
        let mut writer = Writer::new(BUDGET).unwrap();
        writer.list(values.len()).unwrap();
        for value in &values {
            writer.value(value).unwrap();
        }
        let bytes = writer.finish();
        let mut reader = Reader::new(&bytes, BUDGET).unwrap();
        let mut rebuilt = Writer::new(BUDGET).unwrap();
        let count = reader.list().unwrap();
        rebuilt.list(count).unwrap();
        let mut handles = Vec::new();
        for _ in 0..count {
            let value = reader
                .value(2, |id| {
                    handles.push(id.token());
                    Ok(())
                })
                .unwrap();
            rebuilt.value(&value).unwrap();
        }
        reader.finish().unwrap();
        assert_eq!(rebuilt.finish(), bytes);
        assert_eq!(handles, [7]);
    }
    #[test]
    fn malformed_fields_and_foreign_identities_refuse_before_becoming_values() {
        for bytes in [
            vec![255],
            vec![1, 2],
            vec![4, 1, 0, 0, 0, 255],
            vec![5, 2, 0, 0, 0],
            vec![6, 0, 0, 0, 0, 0, 0, 0, 0],
        ] {
            assert!(
                Reader::new(&bytes, BUDGET)
                    .unwrap()
                    .value(2, |_| Ok(()))
                    .is_err()
            );
        }
        let mut writer = Writer::new(BUDGET).unwrap();
        writer
            .value(&Value::Handle(HandleId::new(
                std::num::NonZeroU64::new(9).unwrap(),
            )))
            .unwrap();
        let bytes = writer.finish();
        assert!(
            Reader::new(&bytes, BUDGET)
                .unwrap()
                .value(2, |_| Err("foreign arena".into()))
                .unwrap_err()
                .contains("foreign arena")
        );
        assert!(Reader::new(&[0, 0], BUDGET).unwrap().finish().is_err());
    }
    #[test]
    fn counts_offsets_owned_expansion_and_nesting_have_independent_bounds() {
        let bytes = u32::MAX.to_le_bytes();
        assert!(Reader::new(&bytes, BUDGET).unwrap().list().is_err());
        assert!(Reader::new(&[1; 9], Budget { bytes: 8, ..BUDGET }).is_err());
        assert!(
            Reader::new(&[4, 255, 255, 255, 255], BUDGET)
                .unwrap()
                .value(0, |_| Ok(()))
                .is_err()
        );
        let mut reader = Reader::new(&[0], BUDGET).unwrap();
        assert!(reader.raw(usize::MAX).is_err());
        assert_eq!(reader.byte().unwrap(), 0);
        assert!(reader.owned(BUDGET.bytes + 1).is_err());
        assert!(reader.depth(BUDGET.depth + 1).is_err());
        assert!(
            Writer::new(Budget {
                depth: usize::MAX,
                ..BUDGET
            })
            .is_err()
        );
        let mut writer = Writer::new(Budget { bytes: 2, ..BUDGET }).unwrap();
        assert!(writer.text("abc").is_err());
        assert!(writer.raw(&[1, 2, 3]).is_err());
        assert!(writer.finish().is_empty());
    }
}
