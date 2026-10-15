//! Source-clock scheduling for the admitted MainMenu / SinglePlayer panel pair.
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Owner {
    Main,
    Single,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum Phase {
    MainBirth,
    MainStand,
    MainDeath,
    SingleBirth,
    SingleStand,
    SingleDeath,
}

impl Phase {
    pub const ALL: [Self; 6] = [
        Self::MainBirth,
        Self::MainStand,
        Self::MainDeath,
        Self::SingleBirth,
        Self::SingleStand,
        Self::SingleDeath,
    ];

    pub fn selector(self) -> (&'static str, &'static str) {
        match self {
            Self::MainBirth => ("MainMenu", "Birth"),
            Self::MainStand => ("MainMenu", "Stand"),
            Self::MainDeath => ("MainMenu", "Death"),
            Self::SingleBirth => ("SinglePlayer", "Birth"),
            Self::SingleStand => ("SinglePlayer", "Stand"),
            Self::SingleDeath => ("SinglePlayer", "Death"),
        }
    }

    pub fn birth_and_stand(self) -> Option<(Self, Self)> {
        match self {
            Self::MainBirth | Self::MainStand => Some((Self::MainBirth, Self::MainStand)),
            Self::SingleBirth | Self::SingleStand => Some((Self::SingleBirth, Self::SingleStand)),
            _ => None,
        }
    }

    fn next(self) -> Option<Self> {
        match self {
            Self::MainBirth => Some(Self::MainStand),
            Self::MainDeath => Some(Self::SingleBirth),
            Self::SingleBirth => Some(Self::SingleStand),
            Self::SingleDeath => Some(Self::MainBirth),
            Self::MainStand | Self::SingleStand => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Clip {
    pub interval: [u32; 2],
    pub loops: bool,
}

impl Clip {
    pub fn checked(interval: [u32; 2], loops: bool) -> Result<Self, &'static str> {
        if interval[0] > interval[1] || (loops && interval[0] == interval[1]) {
            return Err("reversed interval or zero-duration looping clip");
        }
        Ok(Self { interval, loops })
    }

    pub fn duration(self) -> Duration {
        Duration::from_millis(u64::from(self.interval[1] - self.interval[0]))
    }

    pub fn frame(self, elapsed: Duration) -> u32 {
        let length = u128::from(self.interval[1] - self.interval[0]);
        let offset = if self.loops {
            elapsed.as_millis() % length
        } else {
            elapsed.as_millis().min(length)
        };
        self.interval[0] + u32::try_from(offset).expect("bounded by source interval")
    }

    /// Half-open regular passes, with a nonlooping final endpoint owned by the
    /// retiring clip. A shorter layer's endpoint cannot fire again while held.
    pub fn event_count(self, frame: u32, from: Duration, to: Duration, first: bool) -> u128 {
        if !(self.interval[0]..=self.interval[1]).contains(&frame) || to < from {
            return 0;
        }
        let at = Duration::from_millis(u64::from(frame - self.interval[0]));
        let length = self.duration();
        if !self.loops {
            if at == length {
                return u128::from((length.is_zero() && first) || (from < length && to >= length));
            }
            return u128::from(from <= at && at < to);
        }
        // A looping endpoint is the next pass's start, so one source event at
        // each end remains two authored events, each emitted once per pass.
        let period = length.as_nanos();
        let at = at.as_nanos();
        let before = |end: u128| {
            if end <= at {
                0
            } else {
                (end - 1 - at) / period + 1
            }
        };
        before(to.as_nanos()) - before(from.as_nanos())
    }
    pub fn event_offsets(
        self,
        frame: u32,
        from: Duration,
        to: Duration,
        first: bool,
        budget: usize,
    ) -> Result<Vec<Duration>, &'static str> {
        let count = self.event_count(frame, from, to, first);
        if count > budget as u128 {
            return Err("panel sound dispatch exceeds project work bound; clock retained");
        }
        if count == 0 {
            return Ok(Vec::new());
        }
        let at = Duration::from_millis(u64::from(frame - self.interval[0])).as_nanos();
        let period = self.duration().as_nanos();
        let start = if self.loops && from.as_nanos() > at {
            (from.as_nanos() - at).div_ceil(period)
        } else {
            0
        };
        (0..count)
            .map(|i| {
                let n = if self.loops {
                    at + period * (start + i)
                } else {
                    at
                };
                let seconds =
                    u64::try_from(n / 1_000_000_000).map_err(|_| "panel event offset overflow")?;
                Ok(Duration::new(seconds, (n % 1_000_000_000) as u32))
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct Segment {
    pub phase: Phase,
    pub from: Duration,
    pub to: Duration,
    pub first: bool,
}

/// One layer may finish Birth and enter Stand before the other layer. The
/// group barrier controls input, while every layer retains its authored clock.
pub struct LayerWindow {
    pub phase: Phase,
    pub from: Duration,
    pub to: Duration,
    pub first: bool,
    pub event_bias_nanos: i128,
}

pub fn layer_windows(
    segment: &Segment,
    birth: Duration,
    group_birth: Duration,
) -> Vec<LayerWindow> {
    let Some((birth_phase, stand_phase)) = segment.phase.birth_and_stand() else {
        return vec![LayerWindow {
            phase: segment.phase,
            from: segment.from,
            to: segment.to,
            first: segment.first,
            event_bias_nanos: 0,
        }];
    };
    if segment.phase == stand_phase {
        let offset = group_birth.saturating_sub(birth);
        return vec![LayerWindow {
            phase: stand_phase,
            from: segment.from + offset,
            to: segment.to + offset,
            first: segment.first && offset.is_zero(),
            event_bias_nanos: -(offset.as_nanos() as i128),
        }];
    }
    let mut windows = vec![LayerWindow {
        phase: birth_phase,
        from: segment.from,
        to: segment.to,
        first: segment.first,
        event_bias_nanos: 0,
    }];
    if segment.to >= birth {
        windows.push(LayerWindow {
            phase: stand_phase,
            from: segment.from.saturating_sub(birth),
            to: segment.to.saturating_sub(birth),
            first: (segment.first && birth.is_zero())
                || (segment.from < birth && segment.to >= birth),
            event_bias_nanos: birth.as_nanos() as i128,
        });
    }
    windows
}

pub fn layer_sample(
    phase: Phase,
    local: Duration,
    birth: Duration,
    group_birth: Duration,
) -> (Phase, Duration) {
    let Some((birth_phase, stand_phase)) = phase.birth_and_stand() else {
        return (phase, local);
    };
    if phase == birth_phase {
        if local < birth {
            (phase, local)
        } else {
            (stand_phase, local - birth)
        }
    } else {
        (stand_phase, local + group_birth.saturating_sub(birth))
    }
}

#[derive(Debug, Clone)]
pub struct Clock {
    pub phase: Phase,
    pub local: Duration,
    pub global: Duration,
    pub target: Option<Owner>,
    first: bool,
}

impl Default for Clock {
    fn default() -> Self {
        Self {
            phase: Phase::MainBirth,
            local: Duration::ZERO,
            global: Duration::ZERO,
            target: None,
            first: true,
        }
    }
}

impl Clock {
    pub fn settle_main(&mut self, elapsed: Duration) {
        self.phase = Phase::MainStand;
        self.local = elapsed;
        self.target = None;
        self.first = false;
    }

    pub fn begin(&mut self, from: Owner, to: Owner) -> Result<(), &'static str> {
        if self.target.is_some()
            || !matches!(
                (from, to),
                (Owner::Main, Owner::Single) | (Owner::Single, Owner::Main)
            )
        {
            return Err("panel transition is active or owner pair is unsupported");
        }
        let expected = match from {
            Owner::Main => Phase::MainStand,
            Owner::Single => Phase::SingleStand,
        };
        if self.phase != expected {
            return Err("panel entrance has not completed or displayed owner differs");
        }
        self.phase = match from {
            Owner::Main => Phase::MainDeath,
            Owner::Single => Phase::SingleDeath,
        };
        self.local = Duration::ZERO;
        self.first = true;
        self.target = Some(to);
        Ok(())
    }

    /// At most Death -> Birth -> Stand, with unequal layer durations supplied
    /// by the admitted clips. Preview on a clone before publishing any effects.
    pub fn advance(
        &mut self,
        delta: Duration,
        duration: impl Fn(Phase) -> Duration,
    ) -> Result<(Vec<Segment>, Option<Owner>), &'static str> {
        let global = self
            .global
            .checked_add(delta)
            .ok_or("panel clock overflow")?;
        if global.as_millis() > u128::from(u64::MAX) {
            return Err("panel global clock exceeds supported millisecond range");
        }
        let mut remaining = delta;
        let mut segments = Vec::new();
        let mut entered = None;
        for _ in 0..3 {
            let end = duration(self.phase);
            let bounded = self.phase.next().is_some();
            let available = end.saturating_sub(self.local);
            let step = if bounded {
                remaining.min(available)
            } else {
                remaining
            };
            let to = self
                .local
                .checked_add(step)
                .ok_or("panel local clock overflow")?;
            segments.push(Segment {
                phase: self.phase,
                from: self.local,
                to,
                first: self.first,
            });
            self.local = to;
            self.first = false;
            remaining = remaining.saturating_sub(step);
            if !bounded || self.local < end {
                self.global = global;
                return Ok((segments, entered));
            }
            let previous = self.phase;
            self.phase = self.phase.next().expect("bounded phase has successor");
            self.local = Duration::ZERO;
            self.first = true;
            if matches!(previous, Phase::MainDeath | Phase::SingleDeath) {
                entered = self.target;
            } else if self.target.is_some() {
                self.target = None;
            }
            // Even an exact boundary samples the incoming phase at frame zero;
            // events there belong to its next positive clock movement.
            if remaining.is_zero() && !duration(self.phase).is_zero() {
                self.global = global;
                return Ok((segments, entered));
            }
        }
        self.global = global;
        if remaining.is_zero() {
            Ok((segments, entered))
        } else {
            Err("panel phase work bound exceeded")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_reset_does_not_replay_a_zero_length_stand_event() {
        let mut clock = Clock {
            global: Duration::from_millis(73),
            ..Clock::default()
        };
        clock.settle_main(Duration::ZERO);
        let (segments, _) = clock.advance(Duration::ZERO, |_| Duration::ZERO).unwrap();
        assert_eq!(clock.global, Duration::from_millis(73));
        let clip = Clip::checked([13, 13], false).unwrap();
        assert_eq!(segments.len(), 1);
        assert_eq!(
            clip.event_count(13, segments[0].from, segments[0].to, segments[0].first),
            0
        );
    }
    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }
    fn duration(p: Phase) -> Duration {
        match p {
            Phase::MainDeath => ms(7),
            Phase::SingleBirth => ms(13),
            Phase::MainBirth => ms(3),
            _ => ms(5),
        }
    }

    #[test]
    fn source_boundaries_carry_remainder_and_global_clock_without_replaying_startup() {
        let mut c = Clock::default();
        assert!(c.begin(Owner::Main, Owner::Single).is_err());
        c.advance(ms(4), duration).unwrap();
        assert_eq!(
            (c.phase, c.local, c.global),
            (Phase::MainStand, ms(1), ms(4))
        );
        c.begin(Owner::Main, Owner::Single).unwrap();
        let (s, owner) = c.advance(ms(23), duration).unwrap();
        assert_eq!(s.len(), 3);
        assert_eq!(owner, Some(Owner::Single));
        assert_eq!(
            (c.phase, c.local, c.global, c.target),
            (Phase::SingleStand, ms(3), ms(27), None)
        );
        c.begin(Owner::Single, Owner::Main).unwrap();
        assert!(c.begin(Owner::Single, Owner::Main).is_err());
        c.advance(ms(9), duration).unwrap();
        assert_eq!(
            (c.phase, c.local, c.global),
            (Phase::MainStand, ms(1), ms(36))
        );
    }
    #[test]
    fn unequal_birth_layers_enter_their_own_stand_without_restarting_at_group_barrier() {
        let segment = Segment {
            phase: Phase::MainBirth,
            from: ms(0),
            to: ms(7),
            first: true,
        };
        let windows = layer_windows(&segment, ms(3), ms(7));
        assert_eq!(windows.len(), 2);
        assert_eq!(
            (windows[1].phase, windows[1].from, windows[1].to),
            (Phase::MainStand, ms(0), ms(4))
        );
        assert_eq!(
            layer_sample(Phase::MainBirth, ms(6), ms(3), ms(7)),
            (Phase::MainStand, ms(3))
        );
        assert_eq!(
            layer_sample(Phase::MainStand, ms(2), ms(3), ms(7)),
            (Phase::MainStand, ms(6))
        );
        let stand = Segment {
            phase: Phase::MainStand,
            from: ms(0),
            to: ms(2),
            first: true,
        };
        let window = layer_windows(&stand, ms(3), ms(7)).remove(0);
        assert_eq!((window.from, window.to), (ms(4), ms(6)));
        assert!(!window.first);
        let clip = Clip::checked([10, 15], true).unwrap();
        assert_eq!(
            clip.event_offsets(11, ms(0), ms(17), true, 4).unwrap(),
            [ms(1), ms(6), ms(11), ms(16)]
        );
        assert!(clip.event_offsets(11, ms(0), ms(17), true, 3).is_err());
    }

    #[test]
    fn shorter_layer_final_events_and_large_delta_loop_events_are_exact() {
        let clip = Clip::checked([20, 23], false).unwrap();
        assert_eq!(clip.event_count(20, ms(0), ms(1), true), 1);
        assert_eq!(clip.event_count(23, ms(1), ms(3), false), 1);
        assert_eq!(clip.event_count(23, ms(3), ms(9), false), 0);
        assert_eq!(clip.event_count(23, ms(9), ms(20), false), 0);
        let looping = Clip::checked([40, 45], true).unwrap();
        assert_eq!(looping.event_count(40, ms(0), ms(16), true), 4);
        assert_eq!(looping.event_count(45, ms(0), ms(16), true), 3);
        assert_eq!(looping.event_count(42, ms(0), ms(2), true), 0);
        assert_eq!(looping.event_count(42, ms(2), ms(3), false), 1);
        let zero = Clip::checked([9, 9], false).unwrap();
        assert_eq!(zero.event_count(9, ms(0), ms(0), true), 1);
        assert_eq!(zero.event_count(9, ms(0), ms(0), false), 0);
        assert!(Clip::checked([9, 9], true).is_err());
        assert!(Clip::checked([9, 8], false).is_err());
    }
}
