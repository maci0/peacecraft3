//! Executed volume-group scales for the audio consumer. Unset groups keep the
//! user's mixer level; no gain law is applied here.
use super::{Entry, Error, Failure, ObjectHost};
use crate::map_startup_host::sound_playback::GroupPolicy;
use pcx_script::Value;
use std::{collections::BTreeMap, sync::Arc, time::Duration};

#[derive(Default)]
pub(super) struct State {
    scales: BTreeMap<i32, f32>,
    policy: Option<Arc<GroupPolicy>>,
    transitions: Vec<Transition>,
    clock: Duration,
    sampled: Duration,
    requested: bool,
}
struct Transition {
    current: f32,
    start: f32,
    target: f32,
    began: Duration,
}
impl State {
    fn sample(&mut self) {
        let Some(policy) = &self.policy else { return };
        let steps = self.clock.as_nanos() / policy.period().as_nanos();
        let now = Duration::from_secs_f64(steps as f64 * policy.period().as_secs_f64());
        if now <= self.sampled {
            return;
        }
        self.sampled = now;
        for t in &mut self.transitions {
            let elapsed = now.saturating_sub(t.began);
            let fraction = if policy.duration().is_zero() {
                1.0
            } else {
                (elapsed.as_secs_f64() / policy.duration().as_secs_f64()).min(1.0) as f32
            };
            t.current = t.start + (t.target - t.start) * fraction;
        }
    }
}

const SET: &str = "VolumeGroupSetVolume";
const RESET: &str = "VolumeGroupReset";

impl ObjectHost {
    /// Admit the declared pair; both must share `CreateSound`'s source.
    pub fn with_volume_groups(mut self) -> Result<Self, Error> {
        let natives = self.inner.program.natives();
        let source = natives.get("CreateSound").map(|n| n.source.clone());
        for (name, args) in [(SET, &["volumegroup", "real"][..]), (RESET, &[][..])] {
            let Some(native) = self.inner.program.natives().get(name) else {
                continue;
            };
            Self::contract_in(&self.inner, name, args, "nothing", false, None, false)?;
            if Some(&native.source) != source.as_ref() {
                return Err(self.inner.error(name, Failure::Source));
            }
        }
        self.volume_groups = Some(State::default());
        Ok(self)
    }

    /// Scale per `volumegroup` enumerand, as last set since a reset.
    pub fn volume_group_scales(&self) -> Option<&BTreeMap<i32, f32>> {
        self.volume_groups.as_ref().map(|state| &state.scales)
    }

    /// Configure a reviewed source policy; no category gain is inferred when absent.
    pub fn configure_volume_group_playback(&mut self, policy: Arc<GroupPolicy>) {
        if let Some(state) = &mut self.volume_groups {
            state.transitions = policy
                .masks()
                .iter()
                .map(|_| Transition {
                    current: policy.unity(),
                    start: policy.unity(),
                    target: policy.unity(),
                    began: state.clock,
                })
                .collect();
            state.policy = Some(policy);
        }
    }
    /// Real audio time, independent of the simulation pause and logical tick budget.
    pub fn advance_volume_group_clock(&mut self, elapsed: Duration) {
        if let Some(state) = &mut self.volume_groups {
            state.clock = state.clock.saturating_add(elapsed);
            state.sample();
        }
    }
    pub fn volume_group_channel_gain(&self, channel: i32) -> Result<f32, String> {
        let Some(state) = &self.volume_groups else {
            return Ok(1.0);
        };
        let Some(policy) = &state.policy else {
            return if !state.requested {
                Ok(1.0)
            } else {
                Err("requested sound category gain has no selected-client policy".into())
            };
        };
        let bit = u32::try_from(channel)
            .ok()
            .and_then(|n| 1u32.checked_shl(n))
            .ok_or("invalid sound category channel")?;
        Ok(policy
            .masks()
            .iter()
            .position(|mask| mask & bit != 0)
            .map_or(policy.unity(), |index| state.transitions[index].current))
    }

    pub(super) fn volume_group_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        self.volume_groups.as_ref()?;
        if !matches!(name, SET | RESET) {
            return None;
        }
        self.inner.program.natives().get(name)?;
        Some((|| {
            match (name, args) {
                (RESET, []) => {
                    let state = self.volume_groups.as_mut().expect("admitted");
                    state.requested = true;
                    state.scales.clear();
                    if let Some(policy) = &state.policy {
                        for t in &mut state.transitions {
                            t.start = t.current;
                            t.target = policy.unity();
                            t.began = state.clock;
                        }
                    }
                }
                (SET, [Value::Handle(group), scale]) => {
                    let Entry::Symbolic { kind, enumerand } = self.inner.entry(*group)? else {
                        return Err(self.inner.error(name, Failure::WrongFamily));
                    };
                    if kind != "volumegroup" {
                        return Err(self.inner.error(name, Failure::WrongFamily));
                    }
                    let enumerand = *enumerand;
                    let scale = scale
                        .as_real()
                        .filter(|v| v.is_finite())
                        .ok_or_else(|| self.inner.error(name, Failure::Signature))?;
                    let state = self.volume_groups.as_mut().expect("admitted");
                    if state.policy.is_some() {
                        let index = usize::try_from(enumerand)
                            .ok()
                            .filter(|index| *index < state.transitions.len())
                            .ok_or_else(|| self.inner.error(name, Failure::Signature))?;
                        let t = &mut state.transitions[index];
                        t.start = t.current;
                        t.target = scale;
                        t.began = state.clock;
                    }
                    state.requested = true;
                    state.scales.insert(enumerand, scale);
                }
                _ => return Err(self.inner.error(name, Failure::Signature)),
            }
            Ok(Value::Null)
        })())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn policy() -> Arc<GroupPolicy> {
        let mut reset = vec![0; 34];
        reset[..3].copy_from_slice(&[0xc7, 0x46, 0x08]);
        reset[3..7].copy_from_slice(&1.0f32.to_le_bytes());
        reset[15..18].copy_from_slice(&[0xc7, 0x46, 0x10]);
        reset[18..22].copy_from_slice(&2000u32.to_le_bytes());
        reset[28..31].copy_from_slice(&[0x83, 0xff, 8]);
        let mut set = vec![0x68];
        set.extend(2000u32.to_le_bytes());
        let masks = [2u32.to_le_bytes(), 8u32.to_le_bytes()].concat();
        Arc::new(GroupPolicy::read(&reset, &set, &masks, 0.25).unwrap())
    }
    #[test]
    fn category_native_calls_route_masks_and_preserve_unrelated_dialogue() {
        use pcx_script::{Host, Prelude};
        let common=pcx_script::parse_script("type agent extends handle\ntype timer extends agent\nnative CreateTimer takes nothing returns timer\nnative DestroyTimer takes timer t returns nothing\ntype volumegroup extends handle\ntype sound extends handle\nconstant native ConvertVolumeGroup takes integer i returns volumegroup\nnative CreateSound takes string f, boolean l, boolean d, boolean s, integer a, integer b, string e returns sound\nnative VolumeGroupSetVolume takes volumegroup g, real s returns nothing\nnative VolumeGroupReset takes nothing returns nothing\nglobals\nconstant volumegroup GROUP_A = ConvertVolumeGroup(0)\nconstant volumegroup GROUP_B = ConvertVolumeGroup(1)\nendglobals\n").unwrap();
        let map = pcx_script::parse_script("").unwrap();
        let program = Arc::new(
            pcx_script::resolve_with(&map, &[Prelude::new("synthetic:category", &common)]).unwrap(),
        );
        let inner = super::super::super::EnumHost::checked_selected(
            program,
            "synthetic:category",
            &["ConvertVolumeGroup"],
            super::super::super::Limits {
                max_handles: 32,
                admission_steps: 1000,
                expression_depth: 16,
                expression_work: 1000,
            },
        )
        .unwrap();
        let mut host = ObjectHost::checked(inner)
            .unwrap()
            .with_volume_groups()
            .unwrap();
        host.configure_volume_group_playback(policy());
        let group = host.native("ConvertVolumeGroup", &[Value::Int(0)]).unwrap();
        host.native(SET, &[group, Value::Real(0.0)]).unwrap();
        host.advance_volume_group_clock(Duration::from_secs(1));
        assert_eq!(host.volume_group_channel_gain(1).unwrap(), 0.5);
        assert_eq!(host.volume_group_channel_gain(3).unwrap(), 1.0);
        assert_eq!(host.volume_group_channel_gain(0).unwrap(), 1.0);
        host.native(RESET, &[]).unwrap();
        host.advance_volume_group_clock(Duration::from_secs(1));
        assert_eq!(host.volume_group_channel_gain(1).unwrap(), 0.75);
        host.native(SET, &[Value::Null, Value::Real(0.0)])
            .unwrap_err();
    }
    #[test]
    fn category_timeline_retargets_current_scale_and_reset_is_a_transition() {
        let policy = policy();
        let mut state = State {
            policy: Some(policy),
            transitions: vec![Transition {
                current: 1.0,
                start: 1.0,
                target: 0.0,
                began: Duration::ZERO,
            }],
            ..Default::default()
        };
        state.clock = Duration::from_secs(1);
        state.sample();
        assert_eq!(state.transitions[0].current, 0.5);
        // A second set, followed by Reset at the same real instant, starts at current gain.
        let t = &mut state.transitions[0];
        t.start = t.current;
        t.target = 0.2;
        t.began = state.clock;
        t.start = t.current;
        t.target = 1.0;
        t.began = state.clock;
        state.clock += Duration::from_secs(1);
        state.sample();
        assert_eq!(state.transitions[0].current, 0.75);
        state.clock += Duration::from_secs(1);
        state.sample();
        assert_eq!(state.transitions[0].current, 1.0);
    }
    #[test]
    fn category_source_clock_samples_only_source_update_boundaries() {
        let mut state = State {
            policy: Some(policy()),
            transitions: vec![Transition {
                current: 1.0,
                start: 1.0,
                target: 0.0,
                began: Duration::ZERO,
            }],
            ..Default::default()
        };
        for _ in 0..9 {
            state.clock += Duration::from_millis(100);
            state.sample();
        }
        assert_eq!(state.transitions[0].current, 0.625);
        state.clock += Duration::from_millis(100);
        state.sample();
        assert_eq!(state.transitions[0].current, 0.5);
    }
}
