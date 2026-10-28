//! FNV-1a 64 over decoded results; a mismatch means behaviour changed.

#[derive(Clone, Copy)]
pub struct Fnv(u64);

const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const PRIME: u64 = 0x0000_0100_0000_01b3;

impl Default for Fnv {
    fn default() -> Self {
        Self(OFFSET)
    }
}

impl Fnv {
    pub fn bytes(&mut self, data: &[u8]) {
        for &b in data {
            self.0 = (self.0 ^ u64::from(b)).wrapping_mul(PRIME);
        }
    }
    pub fn text(&mut self, text: &str) {
        self.u64(text.len() as u64);
        self.bytes(text.as_bytes());
    }
    pub fn u32(&mut self, v: u32) {
        self.bytes(&v.to_le_bytes());
    }
    pub fn u64(&mut self, v: u64) {
        self.bytes(&v.to_le_bytes());
    }
    pub fn f32(&mut self, v: f32) {
        self.u32(v.to_bits());
    }
    pub fn floats(&mut self, values: &[f32]) {
        for &v in values {
            self.f32(v);
        }
    }
    pub fn finish(&self) -> u64 {
        self.0
    }
}

impl std::fmt::Write for Fnv {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        self.bytes(text.as_bytes());
        Ok(())
    }
}

impl Fnv {
    /// Folds the `Debug` rendering of a decoded value: exhaustive over its fields
    /// without a hand-written field list that could drift from the type.
    pub fn debug(&mut self, value: &impl std::fmt::Debug) {
        use std::fmt::Write;
        // Fnv::write_str never fails.
        let _ = write!(self, "{value:?}");
    }
}
