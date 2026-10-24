//! Source-clock scheduling for admitted MainMenu, SinglePlayer and optional Options panels.
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Owner {
    Main,
    Single,
    Options,
}

impl Owner {
    /// Source selector triplet; each owner is admitted independently by callers.
    pub const fn phases(self) -> [Phase; 3] {
        match self {
            Self::Main => [Phase::MainBirth, Phase::MainStand, Phase::MainDeath],
            Self::Single => [Phase::SingleBirth, Phase::SingleStand, Phase::SingleDeath],
            Self::Options => [
                Phase::OptionsBirth,
                Phase::OptionsStand,
                Phase::OptionsDeath,
            ],
        }
    }

    fn can_transition(self, to: Self) -> bool {
        matches!(
            (self, to),
            (Self::Main, Self::Single)
                | (Self::Single, Self::Main)
                | (Self::Main, Self::Options)
                | (Self::Options, Self::Main)
        )
    }
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
    OptionsBirth,
    OptionsStand,
    OptionsDeath,
    OptionsMorph,
    OptionsStandAlternate,
    OptionsMorphAlternate,
}

impl Phase {
    pub fn controls_visible(self) -> bool {
        !matches!(
            self,
            Self::MainBirth
                | Self::MainDeath
                | Self::SingleBirth
                | Self::SingleDeath
                | Self::OptionsBirth
                | Self::OptionsDeath
        )
    }

    /// Legacy Main/Single admission set. Optional owners must not enlarge it.
    pub const ALL: [Self; 6] = [
        Self::MainBirth,
        Self::MainStand,
        Self::MainDeath,
        Self::SingleBirth,
        Self::SingleStand,
        Self::SingleDeath,
    ];

    /// Separately admitted source Options triplet, appended after legacy indices.
    pub const OPTIONS: [Self; 3] = Owner::Options.phases();

    pub const fn owner(self) -> Owner {
        match self {
            Self::MainBirth | Self::MainStand | Self::MainDeath => Owner::Main,
            Self::SingleBirth | Self::SingleStand | Self::SingleDeath => Owner::Single,
            Self::OptionsBirth
            | Self::OptionsStand
            | Self::OptionsDeath
            | Self::OptionsMorph
            | Self::OptionsStandAlternate
            | Self::OptionsMorphAlternate => Owner::Options,
        }
    }

    pub fn selector(self) -> (&'static str, &'static str) {
        match self {
            Self::MainBirth => ("MainMenu", "Birth"),
            Self::MainStand => ("MainMenu", "Stand"),
            Self::MainDeath => ("MainMenu", "Death"),
            Self::SingleBirth => ("SinglePlayer", "Birth"),
            Self::SingleStand => ("SinglePlayer", "Stand"),
            Self::SingleDeath => ("SinglePlayer", "Death"),
            Self::OptionsBirth => ("Options", "Birth"),
            Self::OptionsStand => ("Options", "Stand"),
            Self::OptionsDeath => ("Options", "Death"),
            Self::OptionsMorph => ("Options", "Morph"),
            Self::OptionsStandAlternate => ("Options", "Stand Alternate"),
            Self::OptionsMorphAlternate => ("Options", "Morph Alternate"),
        }
    }

    pub fn birth_and_stand(self) -> Option<(Self, Self)> {
        match self {
            Self::MainBirth | Self::MainStand => Some((Self::MainBirth, Self::MainStand)),
            Self::SingleBirth | Self::SingleStand => Some((Self::SingleBirth, Self::SingleStand)),
            Self::OptionsBirth | Self::OptionsStand => {
                Some((Self::OptionsBirth, Self::OptionsStand))
            }
            Self::OptionsMorph | Self::OptionsStandAlternate => {
                Some((Self::OptionsMorph, Self::OptionsStandAlternate))
            }
            _ => None,
        }
    }

    fn next(self, target: Option<Owner>) -> Result<Option<Self>, &'static str> {
        match self {
            Self::MainBirth => Ok(Some(Self::MainStand)),
            Self::SingleBirth => Ok(Some(Self::SingleStand)),
            Self::OptionsBirth => Ok(Some(Self::OptionsStand)),
            Self::OptionsMorph => Ok(Some(Self::OptionsStandAlternate)),
            Self::OptionsMorphAlternate => Ok(Some(if target.is_some() {
                Self::OptionsDeath
            } else {
                Self::OptionsStand
            })),
            Self::MainDeath | Self::SingleDeath | Self::OptionsDeath => {
                let target = target
                    .filter(|&to| self.owner().can_transition(to))
                    .ok_or("panel death has no supported target owner")?;
                Ok(Some(target.phases()[0]))
            }
            Self::MainStand
            | Self::SingleStand
            | Self::OptionsStand
            | Self::OptionsStandAlternate => Ok(None),
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
    pub exiting: bool,
    pub retired: bool,
    first: bool,
}

impl Default for Clock {
    fn default() -> Self {
        Self {
            phase: Phase::MainBirth,
            local: Duration::ZERO,
            global: Duration::ZERO,
            target: None,
            exiting: false,
            retired: false,
            first: true,
        }
    }
}

impl Clock {
    pub fn transition_active(&self) -> bool {
        self.target.is_some()
            || self.exiting
            || matches!(
                self.phase,
                Phase::MainBirth
                    | Phase::SingleBirth
                    | Phase::OptionsBirth
                    | Phase::OptionsMorph
                    | Phase::OptionsMorphAlternate
            )
    }

    pub fn incoming(owner: Owner) -> Self {
        Self {
            phase: owner.phases()[0],
            ..Self::default()
        }
    }

    pub fn begin_exit(&mut self, owner: Owner) -> Result<(), &'static str> {
        if self.target.is_some() || self.exiting || self.retired || self.phase != owner.phases()[1]
        {
            return Err("panel exit requires a settled owner");
        }
        self.phase = owner.phases()[2];
        self.local = Duration::ZERO;
        self.first = true;
        self.exiting = true;
        Ok(())
    }

    pub fn settle_main(&mut self, elapsed: Duration) {
        self.phase = Phase::MainStand;
        self.local = elapsed;
        self.target = None;
        self.exiting = false;
        self.retired = false;
        self.first = false;
    }

    pub fn begin(&mut self, from: Owner, to: Owner) -> Result<(), &'static str> {
        if self.target.is_some() || self.exiting || self.retired || !from.can_transition(to) {
            return Err("panel transition is active or owner pair is unsupported");
        }
        if self.phase == Phase::OptionsStandAlternate && from == Owner::Options && to == Owner::Main
        {
            self.phase = Phase::OptionsMorphAlternate;
            self.local = Duration::ZERO;
            self.first = true;
            self.target = Some(to);
            return Ok(());
        }
        let [_, expected, death] = from.phases();
        if self.phase != expected {
            return Err("panel entrance has not completed or displayed owner differs");
        }
        self.phase = death;
        self.local = Duration::ZERO;
        self.first = true;
        self.target = Some(to);
        Ok(())
    }

    /// Source `Options Morph` from a settled Options Stand, with no owner target.
    pub fn begin_category(&mut self) -> Result<(), &'static str> {
        if self.target.is_some()
            || self.exiting
            || self.retired
            || self.phase != Phase::OptionsStand
        {
            return Err("panel category morph requires a settled options stand");
        }
        self.phase = Phase::OptionsMorph;
        self.local = Duration::ZERO;
        self.first = true;
        self.target = None;
        Ok(())
    }

    /// At most Morph Alternate -> Death -> Birth -> Stand, with layer durations supplied
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
        for _ in 0..4 {
            let end = duration(self.phase);
            let successor = if self.exiting || self.retired {
                None
            } else {
                self.phase.next(self.target)?
            };
            let bounded = successor.is_some() || self.exiting;
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
            if self.exiting {
                self.exiting = false;
                self.retired = true;
                self.global = global;
                return Ok((segments, None));
            }
            let previous = self.phase;
            self.phase = successor.expect("bounded phase has successor");
            self.local = Duration::ZERO;
            self.first = true;
            if matches!(
                previous,
                Phase::MainDeath | Phase::SingleDeath | Phase::OptionsDeath
            ) {
                entered = self.target;
            } else if previous != Phase::OptionsMorphAlternate && self.target.is_some() {
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
    fn external_scene_waits_for_the_outgoing_clip_and_holds_its_endpoint() {
        let mut clock = Clock::incoming(Owner::Single);
        assert!(clock.transition_active());
        assert!(clock.begin_exit(Owner::Single).is_err());
        clock.advance(ms(3), |_| ms(3)).unwrap();
        assert!(!clock.transition_active());
        assert!(clock.begin_exit(Owner::Main).is_err());
        clock.begin_exit(Owner::Single).unwrap();
        assert!(clock.exiting);
        assert!(clock.transition_active());
        assert!(!clock.phase.controls_visible());
        assert!(clock.begin(Owner::Single, Owner::Main).is_err());
        clock.advance(ms(2), |_| ms(3)).unwrap();
        assert!(!clock.retired);
        let (segments, entered) = clock.advance(ms(1), |_| ms(3)).unwrap();
        assert_eq!(entered, None);
        assert_eq!(segments.last().unwrap().phase, Phase::SingleDeath);
        assert_eq!(clock.local, ms(3));
        assert!(clock.retired);
        assert!(!clock.exiting);
        assert!(!clock.transition_active());
        assert!(clock.begin_exit(Owner::Single).is_err());
        clock.advance(ms(10), |_| ms(3)).unwrap();
        assert_eq!(clock.phase, Phase::SingleDeath);
        clock.settle_main(Duration::ZERO);
        assert!(!clock.retired);
        assert_eq!(clock.phase, Phase::MainStand);
    }

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
    #[test]
    fn optional_options_selectors_leave_legacy_indices_and_admission_unchanged() {
        assert_eq!(Phase::ALL.len(), 6);
        for (index, phase) in Phase::ALL.into_iter().enumerate() {
            assert_eq!(phase as usize, index);
            assert_ne!(phase.owner(), Owner::Options);
        }
        assert_eq!(Phase::OPTIONS, Owner::Options.phases());
        for (index, (phase, animation)) in Phase::OPTIONS
            .into_iter()
            .zip(["Birth", "Stand", "Death"])
            .enumerate()
        {
            assert_eq!(phase as usize, Phase::ALL.len() + index);
            assert_eq!(phase.owner(), Owner::Options);
            assert_eq!(phase.selector(), ("Options", animation));
        }
        assert_eq!(
            [
                Phase::OptionsMorph,
                Phase::OptionsStandAlternate,
                Phase::OptionsMorphAlternate,
            ]
            .map(|phase| phase as usize),
            [9, 10, 11]
        );
        assert_eq!(Phase::OptionsMorph.selector(), ("Options", "Morph"));
        assert_eq!(
            Phase::OptionsStandAlternate.selector(),
            ("Options", "Stand Alternate")
        );
        assert_eq!(
            Phase::OptionsMorphAlternate.selector(),
            ("Options", "Morph Alternate")
        );
        // No Options durations/plans exist in this source profile.
        let legacy = [ms(3), ms(5), ms(7), ms(13), ms(17), ms(19)];
        let duration = |p: Phase| legacy[p as usize];
        let mut c = Clock::default();
        c.advance(ms(3), duration).unwrap();
        c.begin(Owner::Main, Owner::Single).unwrap();
        c.advance(ms(20), duration).unwrap();
        assert_eq!(c.phase, Phase::SingleStand);
        c.begin(Owner::Single, Owner::Main).unwrap();
        c.advance(ms(22), duration).unwrap();
        assert_eq!(c.phase, Phase::MainStand);
    }

    #[test]
    fn directed_options_transition_uses_requested_owner_and_preserves_clock_remainder() {
        let duration = |p| match p {
            Phase::MainBirth => ms(3),
            Phase::MainDeath => ms(7),
            Phase::OptionsBirth => ms(11),
            Phase::OptionsDeath => ms(13),
            _ => ms(17),
        };
        let mut c = Clock::default();
        c.advance(ms(5), duration).unwrap();
        c.begin(Owner::Main, Owner::Options).unwrap();
        let (segments, entered) = c.advance(ms(23), duration).unwrap();
        assert_eq!(
            segments.iter().map(|s| s.phase).collect::<Vec<_>>(),
            [Phase::MainDeath, Phase::OptionsBirth, Phase::OptionsStand]
        );
        assert_eq!(entered, Some(Owner::Options));
        assert_eq!(
            (c.phase, c.local, c.global, c.target),
            (Phase::OptionsStand, ms(5), ms(28), None)
        );
        c.begin(Owner::Options, Owner::Main).unwrap();
        let (_, entered) = c.advance(ms(20), duration).unwrap();
        assert_eq!(entered, Some(Owner::Main));
        assert_eq!(
            (c.phase, c.local, c.global, c.target),
            (Phase::MainStand, ms(4), ms(48), None)
        );
    }

    #[test]
    fn controls_wait_for_source_entrance_and_hide_on_exit_but_not_category_morph() {
        for owner in [Owner::Main, Owner::Single, Owner::Options] {
            let [birth, stand, death] = owner.phases();
            assert!(!birth.controls_visible());
            assert!(stand.controls_visible());
            assert!(!death.controls_visible());
        }
        for phase in [
            Phase::OptionsMorph,
            Phase::OptionsStandAlternate,
            Phase::OptionsMorphAlternate,
        ] {
            assert!(phase.controls_visible());
        }
        let mut clock = Clock::default();
        assert!(!clock.phase.controls_visible());
        clock.advance(ms(3), |_| ms(3)).unwrap();
        assert!(clock.phase.controls_visible());
        clock.begin(Owner::Main, Owner::Options).unwrap();
        assert!(!clock.phase.controls_visible());
        clock.advance(ms(3), |_| ms(3)).unwrap();
        assert_eq!(clock.phase, Phase::OptionsBirth);
        assert!(!clock.phase.controls_visible());
        clock.advance(ms(3), |_| ms(3)).unwrap();
        assert!(clock.phase.controls_visible());
    }

    #[test]
    fn options_boundary_hold_events_and_unequal_layers_keep_existing_rules() {
        let mut c = Clock::default();
        c.advance(ms(3), |_| ms(3)).unwrap();
        c.begin(Owner::Main, Owner::Options).unwrap();
        let (_, entered) = c.advance(ms(7), |_| ms(7)).unwrap();
        assert_eq!(entered, Some(Owner::Options));
        assert_eq!(
            (c.phase, c.local, c.target),
            (Phase::OptionsBirth, Duration::ZERO, Some(Owner::Options))
        );
        assert!(c.begin(Owner::Options, Owner::Main).is_err());
        let (segments, _) = c.advance(ms(9), |_| ms(9)).unwrap();
        let windows = layer_windows(&segments[0], ms(4), ms(9));
        assert_eq!(
            (windows[1].phase, windows[1].from, windows[1].to),
            (Phase::OptionsStand, ms(0), ms(5))
        );
        assert_eq!(
            layer_sample(Phase::OptionsStand, ms(2), ms(4), ms(9)),
            (Phase::OptionsStand, ms(7))
        );
        let clip = Clip::checked([31, 35], false).unwrap();
        assert_eq!(
            clip.event_offsets(35, windows[0].from, windows[0].to, windows[0].first, 1)
                .unwrap(),
            [ms(4)]
        );
        assert_eq!(c.target, None);
        c.begin(Owner::Options, Owner::Main).unwrap();
        // Both zero-length transition clips drain within the existing step bound.
        let (segments, entered) = c.advance(ms(5), |_| Duration::ZERO).unwrap();
        assert_eq!(entered, Some(Owner::Main));
        assert_eq!(segments.len(), 3);
        assert_eq!(
            (c.phase, c.local, c.target),
            (Phase::MainStand, ms(5), None)
        );
    }

    #[test]
    fn unsupported_owner_edges_and_invalid_targets_refuse_without_clock_changes() {
        for (from, to) in [
            (Owner::Single, Owner::Options),
            (Owner::Options, Owner::Single),
            (Owner::Options, Owner::Options),
        ] {
            let mut c = Clock {
                phase: from.phases()[1],
                ..Clock::default()
            };
            let before = (c.phase, c.local, c.global, c.target);
            assert!(c.begin(from, to).is_err());
            assert_eq!((c.phase, c.local, c.global, c.target), before);
        }
        for target in [None, Some(Owner::Single), Some(Owner::Options)] {
            let mut c = Clock {
                phase: Phase::OptionsDeath,
                target,
                ..Clock::default()
            };
            let before = (c.phase, c.local, c.global, c.target);
            assert!(c.advance(ms(5), |_| ms(3)).is_err());
            assert_eq!((c.phase, c.local, c.global, c.target), before);
        }
    }

    #[test]
    fn category_plate_morphs_in_and_leaves_through_the_authored_sequences() {
        let duration = |phase: Phase| match phase {
            Phase::OptionsMorph => ms(5),
            Phase::OptionsMorphAlternate => ms(4),
            Phase::OptionsDeath => ms(6),
            Phase::MainBirth => ms(3),
            _ => ms(17),
        };
        let mut clock = Clock {
            phase: Phase::OptionsStand,
            ..Clock::default()
        };
        clock.begin_category().unwrap();
        let (segments, entered) = clock.advance(ms(5), duration).unwrap();
        assert_eq!(entered, None);
        assert_eq!(
            segments
                .iter()
                .map(|segment| segment.phase)
                .collect::<Vec<_>>(),
            [Phase::OptionsMorph]
        );
        assert_eq!(
            (clock.phase, clock.local, clock.target),
            (Phase::OptionsStandAlternate, Duration::ZERO, None)
        );
        let held = (clock.phase, clock.local, clock.global, clock.target);
        assert!(clock.begin_category().is_err());
        assert_eq!((clock.phase, clock.local, clock.global, clock.target), held);
        let segment = Segment {
            phase: Phase::OptionsMorph,
            from: Duration::ZERO,
            to: ms(5),
            first: true,
        };
        assert_eq!(
            layer_windows(&segment, ms(4), ms(5))
                .iter()
                .map(|window| window.phase)
                .collect::<Vec<_>>(),
            [Phase::OptionsMorph, Phase::OptionsStandAlternate]
        );
        clock.begin(Owner::Options, Owner::Main).unwrap();
        let (segments, entered) = clock.advance(ms(13), duration).unwrap();
        assert_eq!(
            segments
                .iter()
                .map(|segment| segment.phase)
                .collect::<Vec<_>>(),
            [
                Phase::OptionsMorphAlternate,
                Phase::OptionsDeath,
                Phase::MainBirth
            ]
        );
        assert_eq!(entered, Some(Owner::Main));
        assert_eq!(
            (clock.phase, clock.local, clock.target),
            (Phase::MainStand, Duration::ZERO, None)
        );
        let mut blank = Clock {
            phase: Phase::OptionsStand,
            ..Clock::default()
        };
        blank.begin(Owner::Options, Owner::Main).unwrap();
        assert_eq!(
            (blank.phase, blank.target),
            (Phase::OptionsDeath, Some(Owner::Main))
        );
        let mut overrun = Clock {
            phase: Phase::OptionsStandAlternate,
            ..Clock::default()
        };
        overrun.begin(Owner::Options, Owner::Main).unwrap();
        let (segments, entered) = overrun.advance(ms(14), duration).unwrap();
        assert_eq!(segments.len(), 4);
        assert_eq!(entered, Some(Owner::Main));
        assert_eq!(
            (overrun.phase, overrun.local, overrun.global, overrun.target),
            (Phase::MainStand, ms(1), ms(14), None)
        );
    }
}
