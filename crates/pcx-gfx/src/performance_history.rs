//! Bounded age bands: recent frames remain individual; older pairs fold into peaks.
use std::collections::VecDeque;

pub(super) const BANDS: usize = 6;
pub(super) const COLUMNS: usize = 32;
pub(super) const SAMPLES: usize = BANDS * COLUMNS;

#[derive(Default, Clone, Copy)]
struct Sample {
    peak: f64,
    frames: usize,
}

pub(super) struct History([VecDeque<Sample>; BANDS]);

impl Default for History {
    fn default() -> Self {
        Self(std::array::from_fn(|_| {
            VecDeque::with_capacity(COLUMNS + 1)
        }))
    }
}

impl History {
    pub(super) fn clear(&mut self) {
        for band in &mut self.0 {
            band.clear();
        }
    }

    pub(super) fn push(&mut self, milliseconds: f64) {
        let mut sample = Sample {
            peak: if milliseconds.is_finite() && milliseconds >= 0.0 {
                milliseconds
            } else {
                0.0
            },
            frames: 1,
        };
        for band in &mut self.0 {
            band.push_back(sample);
            if band.len() <= COLUMNS {
                break;
            }
            let first = band.pop_front().unwrap();
            let second = band.pop_front().unwrap();
            sample = Sample {
                peak: first.peak.max(second.peak),
                frames: first.frames + second.frames,
            };
            // Overflow from the oldest band expires; memory never grows with uptime.
        }
    }

    pub(super) fn graph(&self) -> ([f32; SAMPLES], f64, usize) {
        let mut values = [0.0_f64; SAMPLES];
        let mut frames = 0;
        for (value, sample) in values
            .iter_mut()
            .rev()
            .zip(self.0.iter().flat_map(|band| band.iter().rev()))
        {
            *value = sample.peak;
            frames += sample.frames;
        }
        let ceiling = values.iter().copied().fold(1.0_f64, f64::max).ceil();
        (
            values.map(|value| (value / ceiling * 100.0) as f32),
            ceiling,
            frames,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_only_oldest_pairs_preserving_peaks_and_live_resolution() {
        let mut history = History::default();
        history.push(100.0);
        for value in 1..=32 {
            history.push(f64::from(value));
        }
        assert_eq!(history.0[0].len(), 31);
        assert_eq!(history.0[1].len(), 1);
        assert_eq!(history.0[1][0].peak, 100.0);
        assert_eq!(history.0[1][0].frames, 2);
        let (heights, ceiling, frames) = history.graph();
        assert_eq!((ceiling, frames), (100.0, 33));
        assert_eq!(heights[SAMPLES - history.0[0].len() - 1], 100.0);
        assert_eq!(heights[SAMPLES - 1], 32.0);
    }

    #[test]
    fn long_stream_stays_bounded_and_newest_sample_is_never_aggregated() {
        let mut history = History::default();
        let capacities = history.0.each_ref().map(VecDeque::capacity);
        for frame in 0..100_000 {
            history.push(f64::from(frame % 40));
        }
        for (index, band) in history.0.iter().enumerate() {
            assert!(band.len() <= COLUMNS);
            assert_eq!(band.capacity(), capacities[index]);
            assert!(band.iter().all(|sample| sample.frames == 1 << index));
        }
        assert_eq!(history.0[0].back().unwrap().peak, 39.0);
        let (_, _, frames) = history.graph();
        assert!(frames <= COLUMNS * ((1 << BANDS) - 1));
        assert!(frames > COLUMNS * ((1 << (BANDS - 1)) - 1));
        history.clear();
        history.push(f64::NAN);
        history.push(f64::INFINITY);
        assert_eq!(history.graph(), ([0.0; SAMPLES], 1.0, 2));
    }
}
