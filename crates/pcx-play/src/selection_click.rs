//! Shift+click on a unit: add it, remove it, or replace the selection.
//! Evidence labels for every rule are in `docs/re-notes/input-selection-rules.md`.
use crate::selection_box::{Candidate, Refusal};
use pcx_script::HandleId;

/// What a Shift+click does to the selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Toggle {
    /// The selection becomes exactly these units, in order.
    Select(Vec<HandleId>),
    /// The selection is full; the click adds nothing.
    Full,
}

/// A selected unit is removed. A building clicked while units are selected replaces the
/// selection with the building (observed), as does a unit clicked while a building is
/// selected (inference). Otherwise the unit is appended unless the selection holds `cap`.
/// Another player's unit mixed with a selection is refused: the original's rule is unmeasured.
pub fn toggle(current: &[Candidate], clicked: Candidate, cap: usize) -> Result<Toggle, Refusal> {
    if current.iter().any(|c| c.id == clicked.id) {
        return Ok(Toggle::Select(
            current
                .iter()
                .filter(|c| c.id != clicked.id)
                .map(|c| c.id)
                .collect(),
        ));
    }
    if current.is_empty() {
        return Ok(Toggle::Select(vec![clicked.id]));
    }
    if !clicked.own || current.iter().any(|c| !c.own) {
        return Err(Refusal::MixedAdditive);
    }
    if current.iter().any(|c| c.structure != clicked.structure) {
        return Ok(Toggle::Select(vec![clicked.id]));
    }
    if current.len() >= cap {
        return Ok(Toggle::Full);
    }
    Ok(Toggle::Select(
        current.iter().map(|c| c.id).chain([clicked.id]).collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAP: usize = 12;

    fn handle(n: u64) -> HandleId {
        HandleId::new(std::num::NonZeroU64::new(n + 1).unwrap())
    }

    fn unit(n: u64, own: bool) -> Candidate {
        Candidate {
            id: handle(n),
            own,
            structure: false,
        }
    }

    fn building(n: u64) -> Candidate {
        Candidate {
            structure: true,
            ..unit(n, true)
        }
    }

    #[test]
    fn a_second_click_on_a_selected_unit_removes_it() {
        let current = [unit(0, true), unit(1, true), unit(2, true)];
        assert_eq!(
            toggle(&current, unit(1, true), CAP),
            Ok(Toggle::Select(vec![handle(0), handle(2)]))
        );
        assert_eq!(
            toggle(&[unit(0, true)], unit(0, true), CAP),
            Ok(Toggle::Select(vec![]))
        );
    }

    #[test]
    fn an_unselected_unit_is_appended() {
        assert_eq!(
            toggle(&[unit(0, true)], unit(1, true), CAP),
            Ok(Toggle::Select(vec![handle(0), handle(1)]))
        );
    }

    #[test]
    fn a_building_replaces_a_unit_selection_and_the_reverse() {
        assert_eq!(
            toggle(&[unit(0, true), unit(1, true)], building(2), CAP),
            Ok(Toggle::Select(vec![handle(2)]))
        );
        assert_eq!(
            toggle(&[building(2)], unit(0, true), CAP),
            Ok(Toggle::Select(vec![handle(0)]))
        );
    }

    #[test]
    fn buildings_add_to_buildings() {
        assert_eq!(
            toggle(&[building(0)], building(1), CAP),
            Ok(Toggle::Select(vec![handle(0), handle(1)]))
        );
    }

    #[test]
    fn a_full_selection_takes_no_more() {
        let current: Vec<_> = (0..CAP as u64).map(|n| unit(n, true)).collect();
        assert_eq!(toggle(&current, unit(99, true), CAP), Ok(Toggle::Full));
        // Removal still works at the cap.
        assert!(matches!(
            toggle(&current, unit(0, true), CAP),
            Ok(Toggle::Select(rest)) if rest.len() == CAP - 1
        ));
    }

    #[test]
    fn another_players_unit_selects_alone_or_is_refused_next_to_a_selection() {
        assert_eq!(
            toggle(&[], unit(5, false), CAP),
            Ok(Toggle::Select(vec![handle(5)]))
        );
        assert_eq!(
            toggle(&[unit(0, true)], unit(5, false), CAP),
            Err(Refusal::MixedAdditive)
        );
        assert_eq!(
            toggle(&[unit(5, false)], unit(0, true), CAP),
            Err(Refusal::MixedAdditive)
        );
    }
}
