//! Bounded RIFF/WAVE framing for integer PCM; no playback or resampling.
use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PcmWave {
    pub channels: u16,
    pub sample_rate: u32,
    pub bits_per_sample: u16,
    pub block_align: u16,
    data: Range<usize>,
}
#[derive(Debug, thiserror::Error)]
#[error("PCM WAVE at byte {at}: {reason}")]
pub struct WaveError {
    pub at: usize,
    pub reason: &'static str,
}
impl PcmWave {
    #[cfg_attr(
        feature = "profile-cpu",
        tracing::instrument(skip_all, target = "pcx_cpu", name = "audio.wav_inspect")
    )]
    pub fn inspect(bytes: &[u8]) -> Result<Self, WaveError> {
        let bad = |at, reason| WaveError { at, reason };
        if bytes.get(..4) != Some(b"RIFF") || bytes.get(8..12) != Some(b"WAVE") {
            return Err(bad(0, "expected RIFF WAVE"));
        }
        let size = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        if size.checked_add(8) != Some(bytes.len()) {
            return Err(bad(4, "declared size mismatch"));
        }
        let (mut format, mut data) = (None, None);
        let mut at = 12usize;
        while at < bytes.len() {
            let start = at
                .checked_add(8)
                .filter(|&n| n <= bytes.len())
                .ok_or_else(|| bad(at, "truncated chunk header"))?;
            let len = u32::from_le_bytes(bytes[at + 4..start].try_into().unwrap()) as usize;
            let end = start
                .checked_add(len)
                .filter(|&n| n <= bytes.len())
                .ok_or_else(|| bad(at, "chunk overruns RIFF"))?;
            match &bytes[at..at + 4] {
                b"fmt " if format.is_some() => return Err(bad(at, "duplicate format")),
                b"fmt " => format = Some(start..end),
                b"data" if data.is_some() => return Err(bad(at, "duplicate data")),
                b"data" => data = Some(start..end),
                _ => {}
            }
            at = end
                .checked_add(len % 2)
                .filter(|&n| n <= bytes.len())
                .ok_or_else(|| bad(at, "missing chunk padding"))?;
        }
        let format = format.ok_or_else(|| bad(12, "missing format"))?;
        let data = data.ok_or_else(|| bad(12, "missing data"))?;
        let f = &bytes[format.clone()];
        if f.len() < 16 || f.len() == 17 {
            return Err(bad(format.start, "short PCM format"));
        }
        let word = |at| u16::from_le_bytes(f[at..at + 2].try_into().unwrap());
        let dword = |at| u32::from_le_bytes(f[at..at + 4].try_into().unwrap());
        let (tag, channels, sample_rate, byte_rate, block_align, bits_per_sample) =
            (word(0), word(2), dword(4), dword(8), word(12), word(14));
        if tag != 1 || !matches!(channels, 1 | 2) || !matches!(bits_per_sample, 8 | 16) {
            return Err(bad(
                format.start,
                "unsupported PCM profile (mono/stereo 8/16-bit only)",
            ));
        }
        let align = channels * (bits_per_sample / 8);
        if sample_rate == 0
            || block_align != align
            || sample_rate.checked_mul(u32::from(align)) != Some(byte_rate)
            || !data.len().is_multiple_of(usize::from(align))
        {
            return Err(bad(
                format.start,
                "inconsistent rate, alignment or sample data",
            ));
        }
        Ok(Self {
            channels,
            sample_rate,
            bits_per_sample,
            block_align,
            data,
        })
    }
    pub fn data_range(&self) -> Range<usize> {
        self.data.clone()
    }
    pub fn frames(&self) -> usize {
        self.data.len() / usize::from(self.block_align)
    }
}
