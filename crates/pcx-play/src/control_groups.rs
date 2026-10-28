//! One player's ten control groups: define, extend, recall and the double-press timer.
//!
//! The rules (what each key does, what is refused, the cap and the window) are
//! decoded from the client by [`crate::control_group_rules`] and passed in by the
//! caller. A group holds unit handles only; whether a handle is still a live,
//! controlled unit is asked of a [`Roster`] at the moment of each operation.
use crate::control_group_rules::GROUP_COUNT;
use pcx_script::HandleId;

/// A control group number, `0..GROUP_COUNT`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Group(u8);

impl Group {
    pub fn new(number: u8) -> Option<Self> {
        (usize::from(number) < GROUP_COUNT).then_some(Self(number))
    }

    pub fn number(self) -> u8 {
        self.0
    }

    fn index(self) -> usize {
        usize::from(self.0)
    }
}

/// What a handle is to the player using the groups right now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standing {
    /// A live unit the player controls.
    Own,
    /// A live unit the player controls that is not shown, so it cannot be
    /// selected. It stays in groups.
    Hidden,
    /// A live unit the player does not control.
    Foreign,
    /// A live object that is not a unit.
    NotUnit,
    /// Dead or removed.
    Gone,
}

pub trait Roster {
    fn standing(&self, handle: HandleId) -> Standing;
}

/// Why a define or extend changed nothing. The texts are the shipped `[Errors]`
/// strings picked through [`crate::control_group_rules::ErrorKeys`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    #[error("nothing is selected")]
    EmptySelection,
    #[error("no selected unit is controlled by the player")]
    ForeignUnits,
    #[error("only objects that are not units are selected")]
    NotUnits,
}

#[derive(Clone, Copy, Debug)]
struct Press {
    group: Group,
    at: f32,
}

#[derive(Clone, Debug, Default)]
pub struct ControlGroups {
    members: [Vec<HandleId>; GROUP_COUNT],
    last_press: Option<Press>,
}

/// The selection reduced to what a define may use.
struct Candidates {
    own: Vec<HandleId>,
    foreign: bool,
}

fn candidates(selection: &[HandleId], roster: &impl Roster) -> Result<Candidates, Refusal> {
    let mut own = Vec::new();
    let (mut foreign, mut not_unit) = (false, false);
    for &handle in selection {
        match roster.standing(handle) {
            Standing::Own | Standing::Hidden if !own.contains(&handle) => own.push(handle),
            Standing::Own | Standing::Hidden | Standing::Gone => {}
            Standing::Foreign => foreign = true,
            Standing::NotUnit => not_unit = true,
        }
    }
    if own.is_empty() && !foreign {
        return Err(if not_unit {
            Refusal::NotUnits
        } else {
            Refusal::EmptySelection
        });
    }
    Ok(Candidates { own, foreign })
}

impl ControlGroups {
    pub fn members(&self, group: Group) -> &[HandleId] {
        &self.members[group.index()]
    }

    /// Replaces the group with the selected units the player controls, at most `cap`.
    pub fn define(
        &mut self,
        group: Group,
        selection: &[HandleId],
        roster: &impl Roster,
        cap: usize,
    ) -> Result<(), Refusal> {
        let Candidates { mut own, .. } = candidates(selection, roster)?;
        if own.is_empty() {
            return Err(Refusal::ForeignUnits);
        }
        own.truncate(cap);
        self.members[group.index()] = own;
        Ok(())
    }

    /// Adds the selected units the player controls to the group, keeping its
    /// members that are still controlled units, at most `cap` in all.
    pub fn extend(
        &mut self,
        group: Group,
        selection: &[HandleId],
        roster: &impl Roster,
        cap: usize,
    ) -> Result<(), Refusal> {
        let Candidates { own, mut foreign } = candidates(selection, roster)?;
        let mut merged = Vec::new();
        for &member in &self.members[group.index()] {
            match roster.standing(member) {
                Standing::Own | Standing::Hidden if !merged.contains(&member) => {
                    merged.push(member);
                }
                Standing::Foreign => foreign = true,
                _ => {}
            }
        }
        for handle in own {
            if !merged.contains(&handle) {
                merged.push(handle);
            }
        }
        if merged.is_empty() {
            return Err(if foreign {
                Refusal::ForeignUnits
            } else {
                Refusal::EmptySelection
            });
        }
        merged.truncate(cap);
        self.members[group.index()] = merged;
        Ok(())
    }

    /// The members to select, first `cap` of those the player controls. Dead or
    /// removed members leave the group; members that are alive but not controlled
    /// stay in it and are not selected.
    pub fn recall(&mut self, group: Group, roster: &impl Roster, cap: usize) -> Vec<HandleId> {
        let members = &mut self.members[group.index()];
        members.retain(|&member| roster.standing(member) != Standing::Gone);
        members
            .iter()
            .copied()
            .filter(|&member| roster.standing(member) == Standing::Own)
            .take(cap)
            .collect()
    }

    /// Records a press that selected a non-empty group at `now` seconds. True when
    /// it is the second press of the same group within `window`; the memory is
    /// then cleared. A press whose time is not a number never counts as a second.
    pub fn press(&mut self, group: Group, now: f32, window: f32) -> bool {
        let repeated = self.last_press.is_some_and(|last| {
            last.group == group && (last.at + window).partial_cmp(&now) == Some(std::cmp::Ordering::Greater)
        });
        self.last_press = (!repeated).then_some(Press { group, at: now });
        repeated
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::BTreeMap, num::NonZeroU64};

    fn handle(token: u64) -> HandleId {
        HandleId::new(NonZeroU64::new(token).unwrap())
    }

    fn group(number: u8) -> Group {
        Group::new(number).unwrap()
    }

    struct World(BTreeMap<HandleId, Standing>);

    impl Roster for World {
        fn standing(&self, handle: HandleId) -> Standing {
            self.0.get(&handle).copied().unwrap_or(Standing::Gone)
        }
    }

    /// Handles 1..=n are the player's units; the table lists the exceptions.
    fn world(own: u64, others: &[(u64, Standing)]) -> World {
        let mut table: BTreeMap<_, _> = (1..=own).map(|n| (handle(n), Standing::Own)).collect();
        table.extend(others.iter().map(|&(n, standing)| (handle(n), standing)));
        World(table)
    }

    fn handles(numbers: &[u64]) -> Vec<HandleId> {
        numbers.iter().map(|&n| handle(n)).collect()
    }

    #[test]
    fn group_numbers_stop_at_the_group_count() {
        assert!(Group::new(9).is_some());
        assert!(Group::new(10).is_none());
    }

    #[test]
    fn define_keeps_controlled_units_in_order_up_to_the_cap() {
        let world = world(20, &[(21, Standing::Foreign), (22, Standing::NotUnit)]);
        let mut groups = ControlGroups::default();
        let selection = handles(&[3, 21, 1, 22, 3, 2]);
        groups.define(group(4), &selection, &world, 12).unwrap();
        assert_eq!(groups.members(group(4)), handles(&[3, 1, 2]));
        let big = handles(&(1..=20).collect::<Vec<_>>());
        groups.define(group(4), &big, &world, 12).unwrap();
        assert_eq!(groups.members(group(4)), handles(&(1..=12).collect::<Vec<_>>()));
    }

    #[test]
    fn define_refuses_in_the_originals_three_cases_and_keeps_the_old_group() {
        let world = world(2, &[(5, Standing::Foreign), (6, Standing::NotUnit), (7, Standing::Gone)]);
        let mut groups = ControlGroups::default();
        groups.define(group(0), &handles(&[1, 2]), &world, 12).unwrap();
        for (selection, refusal) in [
            (vec![], Refusal::EmptySelection),
            (handles(&[7]), Refusal::EmptySelection),
            (handles(&[6]), Refusal::NotUnits),
            (handles(&[5]), Refusal::ForeignUnits),
            (handles(&[5, 6]), Refusal::ForeignUnits),
        ] {
            assert_eq!(
                groups.define(group(0), &selection, &world, 12),
                Err(refusal),
                "{selection:?}"
            );
            assert_eq!(groups.members(group(0)), handles(&[1, 2]));
        }
    }

    #[test]
    fn extend_appends_new_units_after_the_kept_members() {
        let world = world(8, &[(9, Standing::Foreign), (10, Standing::Gone)]);
        let mut groups = ControlGroups::default();
        groups.define(group(1), &handles(&[4, 2, 10]), &world, 12).unwrap();
        // 10 is gone: the define already skipped it. Add 2 (present), 5 and a foreign unit.
        groups.extend(group(1), &handles(&[2, 5, 9]), &world, 12).unwrap();
        assert_eq!(groups.members(group(1)), handles(&[4, 2, 5]));
    }

    #[test]
    fn extend_drops_dead_members_and_cuts_at_the_cap() {
        let mut world = world(6, &[]);
        let mut groups = ControlGroups::default();
        groups.define(group(1), &handles(&[1, 2, 3]), &world, 4).unwrap();
        world.0.insert(handle(2), Standing::Gone);
        groups.extend(group(1), &handles(&[4, 5, 6]), &world, 4).unwrap();
        assert_eq!(groups.members(group(1)), handles(&[1, 3, 4, 5]));
    }

    #[test]
    fn extend_refuses_an_empty_selection_even_when_the_group_has_members() {
        let world = world(3, &[(8, Standing::Foreign)]);
        let mut groups = ControlGroups::default();
        groups.define(group(2), &handles(&[1]), &world, 12).unwrap();
        assert_eq!(
            groups.extend(group(2), &[], &world, 12),
            Err(Refusal::EmptySelection)
        );
        // A foreign-only selection merges with the members, so it succeeds unchanged.
        groups.extend(group(2), &handles(&[8]), &world, 12).unwrap();
        assert_eq!(groups.members(group(2)), handles(&[1]));
        // With no members left the same selection is refused as foreign.
        assert_eq!(
            groups.extend(group(3), &handles(&[8]), &world, 12),
            Err(Refusal::ForeignUnits)
        );
    }

    #[test]
    fn recall_returns_controlled_members_and_forgets_the_dead() {
        let mut world = world(5, &[]);
        let mut groups = ControlGroups::default();
        groups.define(group(7), &handles(&[1, 2, 3, 4]), &world, 12).unwrap();
        world.0.insert(handle(2), Standing::Gone);
        world.0.insert(handle(3), Standing::Foreign);
        assert_eq!(groups.recall(group(7), &world, 12), handles(&[1, 4]));
        // The dead unit is gone for good; the foreign one waits in the group.
        assert_eq!(groups.members(group(7)), handles(&[1, 3, 4]));
        world.0.insert(handle(3), Standing::Own);
        assert_eq!(groups.recall(group(7), &world, 2), handles(&[1, 3]));
        world.0.insert(handle(1), Standing::Hidden);
        assert_eq!(groups.recall(group(7), &world, 12), handles(&[3, 4]));
        assert_eq!(groups.members(group(7)), handles(&[1, 3, 4]));
        assert!(groups.recall(group(8), &world, 12).is_empty());
    }

    #[test]
    fn a_second_press_within_the_window_centres_once() {
        let mut groups = ControlGroups::default();
        let (one, two) = (group(0), group(1));
        assert!(!groups.press(one, 10.0, 0.5));
        assert!(groups.press(one, 10.25, 0.5), "inside the window");
        assert!(!groups.press(one, 10.3, 0.5), "memory was reset");
        assert!(!groups.press(two, 10.4, 0.5), "another group starts afresh");
        assert!(!groups.press(one, 10.5, 0.5));
        assert!(!groups.press(one, 11.0, 0.5), "exactly one window later is outside");
        assert!(groups.press(one, 11.49, 0.5));
    }

    #[test]
    fn a_press_without_a_usable_time_never_repeats() {
        let mut groups = ControlGroups::default();
        assert!(!groups.press(group(0), f32::NAN, 0.5));
        assert!(!groups.press(group(0), f32::NAN, 0.5));
        assert!(!groups.press(group(0), 1.0, 0.5));
        assert!(!groups.press(group(0), 1.0, f32::NAN));
    }
}
