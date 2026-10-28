//! Drag-box selection: pointer gesture and pick rules.
//!
//! The 1.29.2.9231 client operands (selection cap, double-click window and
//! tolerance) are read from the reviewed, locally supplied image; the rectangle
//! colour is `[SelectionCircle] ColorFriend` and is read by the caller. Evidence
//! labels for every rule are in `docs/re-notes/campaign-box-selection.md`.
use crate::{
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    scripts::AssetIdentity,
};
use pcx_script::HandleId;
use thiserror::Error;

const IMAGE_BUDGET: usize = 32 * 1024 * 1024;
/// The native UI frame is 0.8 wide and 0.6 tall at 4:3 (`glue-coordinate-runtime.md`).
/// The double-click tolerance is stored in these units.
pub const UI_FRAME_HEIGHT: f32 = 0.6;

/// `cmp dword ptr [edi+0x10], imm8` in the add-to-selection routine: refuse at the cap.
const CAP_COMPARE: u32 = 0x4ef657;
/// `cmp dword ptr [ebx+0x1f8], imm8` in the second add-to-selection routine.
const CAP_COMPARE_SECOND: u32 = 0x4efc8b;
/// `mov dword ptr [esi+0x10], imm32` in the selection manager constructor.
const DOUBLE_CLICK_STORE: u32 = 0x5f7edd;
/// `movss xmm2, dword ptr [abs32]` loading the double-click tolerance.
const TOLERANCE_LOAD: u32 = 0x5fa087;

#[derive(Clone, Debug)]
pub struct Parameters {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    /// Most units one selection may hold.
    pub selection_cap: usize,
    /// A second press within this window and tolerance is a double click.
    pub double_click_ms: u32,
    /// Per-axis double-click distance in UI frame units.
    pub double_click_tolerance: f32,
}

impl Parameters {
    pub fn read(image: &[u8], origin: AssetIdentity) -> Result<Self, String> {
        let fail = |reason: &str| format!("selection box {}: {reason}", origin.source);
        if image.len() > IMAGE_BUDGET {
            return Err(fail("client image exceeds budget"));
        }
        if [&origin.source, &origin.edition, &origin.version]
            .into_iter()
            .any(|value| value.trim().is_empty())
        {
            return Err(fail("missing source identity"));
        }
        let digest = sha256(image);
        if digest != REVIEWED_IMAGE {
            return Err(fail(
                "unsupported client image; no selection rules substituted",
            ));
        }
        let (selection_cap, double_click_ms, double_click_tolerance) =
            decode(image).map_err(|reason| fail(&reason))?;
        Ok(Self {
            origin,
            image_sha256: digest,
            selection_cap,
            double_click_ms,
            double_click_tolerance,
        })
    }
}

fn decode(image: &[u8]) -> Result<(usize, u32, f32), String> {
    let pe = Pe::parse(
        image,
        RuleInputBudget {
            image_bytes: IMAGE_BUDGET,
            profile_bytes: 128,
            sections: 96,
        },
    )
    .map_err(|e| e.to_string())?;
    let code = |va: u32, len: usize, what: &str| -> Result<Vec<u8>, String> {
        let bytes = pe.read_from(va, true).map_err(|e| e.to_string())?;
        bytes
            .get(..len)
            .map(<[u8]>::to_vec)
            .ok_or_else(|| format!("truncated {what}"))
    };
    let first = code(CAP_COMPARE, 4, "selection cap compare")?;
    let cap = match first[..] {
        [0x83, 0x7f, 0x10, cap] => cap,
        _ => return Err("missing selection cap compare".into()),
    };
    let second = code(CAP_COMPARE_SECOND, 7, "second selection cap compare")?;
    match second[..] {
        [0x83, 0xbb, 0xf8, 0x01, 0x00, 0x00, other] if other == cap => {}
        _ => return Err("second selection cap compare disagrees".into()),
    }
    if cap == 0 {
        return Err("selection cap is zero".into());
    }
    let store = code(DOUBLE_CLICK_STORE, 7, "double-click store")?;
    let window = match store[..] {
        [0xc7, 0x46, 0x10, a, b, c, d] => u32::from_le_bytes([a, b, c, d]),
        _ => return Err("missing double-click window store".into()),
    };
    let load = code(TOLERANCE_LOAD, 8, "double-click tolerance load")?;
    let address = match load[..] {
        [0xf3, 0x0f, 0x10, 0x15, a, b, c, d] => u32::from_le_bytes([a, b, c, d]),
        _ => return Err("missing double-click tolerance load".into()),
    };
    let bytes = pe.read_from(address, false).map_err(|e| e.to_string())?;
    let word = bytes.get(..4).ok_or("truncated double-click tolerance")?;
    let tolerance = f32::from_le_bytes(word.try_into().expect("four bytes"));
    if !tolerance.is_finite() || tolerance <= 0.0 {
        return Err("invalid double-click tolerance".into());
    }
    Ok((usize::from(cap), window, tolerance))
}

/// Axis-aligned rectangle in logical window pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub min: [f32; 2],
    pub max: [f32; 2],
}

impl Rect {
    pub fn from_corners(a: [f32; 2], b: [f32; 2]) -> Self {
        Self {
            min: [a[0].min(b[0]), a[1].min(b[1])],
            max: [a[0].max(b[0]), a[1].max(b[1])],
        }
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        self.min[0] <= other.max[0]
            && other.min[0] <= self.max[0]
            && self.min[1] <= other.max[1]
            && other.min[1] <= self.max[1]
    }
}

/// How a left press ended.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Finish {
    /// No area was swept: the press position selects as a point.
    Click {
        at: [f32; 2],
        additive: bool,
        double: bool,
    },
    Box {
        rect: Rect,
        additive: bool,
    },
}

#[derive(Clone, Copy, Debug)]
struct Held {
    origin: [f32; 2],
    cursor: [f32; 2],
    additive: bool,
    double: bool,
}

#[derive(Clone, Copy, Debug)]
struct Press {
    at: [f32; 2],
    time_ms: f64,
}

/// Press, drag, release. The native manager treats a rectangle with zero width
/// or height as a point pick, so there is no pixel threshold.
#[derive(Debug, Default)]
pub struct Gesture {
    held: Option<Held>,
    previous: Option<Press>,
}

impl Gesture {
    pub fn is_active(&self) -> bool {
        self.held.is_some()
    }

    /// Begins a press. `view_height` converts the tolerance from UI frame units.
    pub fn press(
        &mut self,
        at: [f32; 2],
        additive: bool,
        now_ms: f64,
        view_height: f32,
        parameters: &Parameters,
    ) {
        let tolerance = parameters.double_click_tolerance / UI_FRAME_HEIGHT * view_height;
        let double = self.previous.is_some_and(|previous| {
            now_ms - previous.time_ms < f64::from(parameters.double_click_ms)
                && (previous.at[0] - at[0]).abs() < tolerance
                && (previous.at[1] - at[1]).abs() < tolerance
        });
        self.previous = if double {
            None
        } else {
            Some(Press {
                at,
                time_ms: now_ms,
            })
        };
        self.held = Some(Held {
            origin: at,
            cursor: at,
            additive,
            double,
        });
    }

    pub fn track(&mut self, cursor: [f32; 2]) {
        if let Some(held) = &mut self.held {
            held.cursor = cursor;
        }
    }

    /// The outline to draw: present once the cursor has left the press position.
    pub fn rect(&self) -> Option<Rect> {
        self.held
            .filter(|held| held.cursor != held.origin)
            .map(|held| Rect::from_corners(held.origin, held.cursor))
    }

    pub fn release(&mut self, cursor: [f32; 2]) -> Option<Finish> {
        let mut held = self.held.take()?;
        held.cursor = cursor;
        let rect = Rect::from_corners(held.origin, cursor);
        let area = rect.max[0] > rect.min[0] && rect.max[1] > rect.min[1];
        if area {
            // A swept press never chains into a double click.
            self.previous = None;
            Some(Finish::Box {
                rect,
                additive: held.additive,
            })
        } else {
            Some(Finish::Click {
                at: held.origin,
                additive: held.additive,
                double: held.double,
            })
        }
    }

    pub fn cancel(&mut self) {
        self.held = None;
    }
}

/// A unit the box may take, already live, shown and visible to the local player.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub id: HandleId,
    /// Owned by the local player.
    pub own: bool,
    pub structure: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pick {
    pub units: Vec<Candidate>,
    /// More candidates qualified than the cap holds.
    pub truncated: bool,
}

impl Pick {
    pub fn ids(&self) -> Vec<HandleId> {
        self.units.iter().map(|c| c.id).collect()
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum Refusal {
    #[error(
        "box held {0} units of other players; the original's rule for several is not established"
    )]
    SeveralForeign(usize),
    #[error("additive box selection mixing other players' units is not established")]
    MixedAdditive,
}

fn capped(candidates: impl Iterator<Item = Candidate>, cap: usize) -> Pick {
    let mut units: Vec<Candidate> = candidates.collect();
    let truncated = units.len() > cap;
    units.truncate(cap);
    Pick { units, truncated }
}

/// Selects from the best tier present: own units, then own structures. A box with
/// only other players' units takes one; several are refused because no rule for
/// them is established. An empty box is `Ok(None)`.
pub fn pick_box(hits: &[Candidate], cap: usize) -> Result<Option<Pick>, Refusal> {
    let own_units = || hits.iter().filter(|c| c.own && !c.structure).copied();
    let own_structures = || hits.iter().filter(|c| c.own && c.structure).copied();
    if own_units().next().is_some() {
        return Ok(Some(capped(own_units(), cap)));
    }
    if own_structures().next().is_some() {
        return Ok(Some(capped(own_structures(), cap)));
    }
    match hits {
        [] => Ok(None),
        [only] => Ok(Some(Pick {
            units: vec![*only],
            truncated: false,
        })),
        many => Err(Refusal::SeveralForeign(many.len())),
    }
}

/// Shift adds to the current selection without toggling. It applies only while the
/// selection and the new pick are all owned.
pub fn add_to_selection(current: &[Candidate], pick: &Pick, cap: usize) -> Result<Pick, Refusal> {
    if current.iter().chain(&pick.units).any(|c| !c.own) {
        return Err(Refusal::MixedAdditive);
    }
    let mut units = current.to_vec();
    let mut truncated = pick.truncated;
    for &added in &pick.units {
        if units.iter().any(|c| c.id == added.id) {
            continue;
        }
        if units.len() >= cap {
            truncated = true;
            break;
        }
        units.push(added);
    }
    Ok(Pick { units, truncated })
}

/// Double click: every own candidate sharing the clicked unit's type. Another
/// player's unit is never grouped. `None` when `clicked` is not among `hits`.
pub fn same_type(hits: &[(Candidate, [u8; 4])], clicked: HandleId, cap: usize) -> Option<Pick> {
    let (target, rawcode) = hits.iter().find(|(c, _)| c.id == clicked)?;
    if !target.own {
        return Some(Pick {
            units: vec![*target],
            truncated: false,
        });
    }
    Some(capped(
        hits.iter()
            .filter(|(c, code)| c.own && code == rawcode)
            .map(|(c, _)| *c),
        cap,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAP: usize = 12;

    fn parameters() -> Parameters {
        Parameters {
            origin: AssetIdentity {
                source: "synthetic".into(),
                edition: "fixture".into(),
                version: "fixture".into(),
            },
            image_sha256: [0; 32],
            selection_cap: CAP,
            double_click_ms: 500,
            double_click_tolerance: 0.01,
        }
    }

    fn handle(n: u32) -> HandleId {
        HandleId::new(std::num::NonZeroU64::new(u64::from(n) + 1).unwrap())
    }

    fn unit(n: u32, own: bool) -> Candidate {
        Candidate {
            id: handle(n),
            own,
            structure: false,
        }
    }

    fn building(n: u32, own: bool) -> Candidate {
        Candidate {
            structure: true,
            ..unit(n, own)
        }
    }

    #[test]
    fn rectangle_appears_with_the_first_movement_and_commits_on_release() {
        let mut gesture = Gesture::default();
        gesture.press([100.0, 200.0], false, 0.0, 768.0, &parameters());
        assert!(gesture.is_active());
        assert_eq!(gesture.rect(), None);
        gesture.track([102.0, 200.0]);
        assert_eq!(
            gesture.rect(),
            Some(Rect {
                min: [100.0, 200.0],
                max: [102.0, 200.0]
            })
        );
        gesture.track([60.0, 250.0]);
        assert_eq!(
            gesture.release([60.0, 250.0]),
            Some(Finish::Box {
                rect: Rect {
                    min: [60.0, 200.0],
                    max: [100.0, 250.0]
                },
                additive: false
            })
        );
        assert!(!gesture.is_active());
        assert_eq!(gesture.rect(), None);
    }

    #[test]
    fn zero_width_or_height_is_a_point_pick_at_the_press() {
        for end in [[100.0, 200.0], [140.0, 200.0], [100.0, 260.0]] {
            let mut gesture = Gesture::default();
            gesture.press([100.0, 200.0], true, 0.0, 768.0, &parameters());
            assert_eq!(
                gesture.release(end),
                Some(Finish::Click {
                    at: [100.0, 200.0],
                    additive: true,
                    double: false
                })
            );
        }
    }

    #[test]
    fn cancel_and_stray_release_finish_nothing() {
        let mut gesture = Gesture::default();
        assert_eq!(gesture.release([1.0, 1.0]), None);
        gesture.press([1.0, 1.0], false, 0.0, 768.0, &parameters());
        gesture.cancel();
        assert_eq!(gesture.release([9.0, 9.0]), None);
    }

    #[test]
    fn double_click_needs_time_and_distance_in_the_source_window() {
        // 0.01 frame units of a 768 px tall frame is 12.8 px.
        let p = parameters();
        let click = |gesture: &mut Gesture, at: [f32; 2], now: f64| {
            gesture.press(at, false, now, 768.0, &p);
            gesture.release(at)
        };
        let mut gesture = Gesture::default();
        click(&mut gesture, [300.0, 300.0], 0.0);
        let second = click(&mut gesture, [310.0, 308.0], 499.0);
        assert!(matches!(second, Some(Finish::Click { double: true, .. })));
        // A double click does not chain into a triple click.
        let third = click(&mut gesture, [310.0, 308.0], 600.0);
        assert!(matches!(third, Some(Finish::Click { double: false, .. })));
        for (at, now) in [([300.0, 300.0], 500.0), ([314.0, 300.0], 10.0)] {
            let mut gesture = Gesture::default();
            click(&mut gesture, [300.0, 300.0], 0.0);
            let late = click(&mut gesture, at, now);
            assert!(matches!(late, Some(Finish::Click { double: false, .. })));
        }
    }

    #[test]
    fn a_swept_press_forgets_the_previous_click() {
        let p = parameters();
        let mut gesture = Gesture::default();
        gesture.press([300.0, 300.0], false, 0.0, 768.0, &p);
        gesture.release([300.0, 300.0]);
        gesture.press([300.0, 300.0], false, 100.0, 768.0, &p);
        assert!(matches!(
            gesture.release([340.0, 340.0]),
            Some(Finish::Box { .. })
        ));
        gesture.press([300.0, 300.0], false, 200.0, 768.0, &p);
        assert!(matches!(
            gesture.release([300.0, 300.0]),
            Some(Finish::Click { double: false, .. })
        ));
    }

    #[test]
    fn rectangles_intersect_inclusively() {
        let a = Rect::from_corners([0.0, 0.0], [10.0, 10.0]);
        assert!(a.intersects(&Rect::from_corners([10.0, 10.0], [20.0, 20.0])));
        assert!(!a.intersects(&Rect::from_corners([10.5, 0.0], [20.0, 5.0])));
    }

    #[test]
    fn own_units_win_over_structures_and_other_players() {
        let hits = [
            building(1, true),
            unit(2, true),
            unit(3, false),
            building(4, false),
            unit(5, true),
        ];
        let pick = pick_box(&hits, CAP).unwrap().unwrap();
        assert_eq!(pick.ids(), [handle(2), handle(5)]);
        assert!(!pick.truncated);
    }

    #[test]
    fn own_structures_win_over_other_players_when_no_own_unit_is_present() {
        let hits = [building(1, false), building(2, true), unit(3, false)];
        assert_eq!(pick_box(&hits, CAP).unwrap().unwrap().ids(), [handle(2)]);
    }

    #[test]
    fn a_single_foreign_unit_is_selected_and_several_are_refused() {
        assert_eq!(
            pick_box(&[building(1, false)], CAP).unwrap().unwrap().ids(),
            [handle(1)]
        );
        assert_eq!(
            pick_box(&[unit(1, false), unit(2, false)], CAP),
            Err(Refusal::SeveralForeign(2))
        );
        assert_eq!(pick_box(&[], CAP), Ok(None));
    }

    #[test]
    fn the_cap_keeps_the_first_candidates_and_reports_truncation() {
        let hits: Vec<Candidate> = (0..15).map(|n| unit(n, true)).collect();
        let pick = pick_box(&hits, CAP).unwrap().unwrap();
        assert_eq!(pick.units.len(), CAP);
        assert_eq!(pick.units[0].id, handle(0));
        assert_eq!(pick.units[CAP - 1].id, handle(11));
        assert!(pick.truncated);
        let exact: Vec<Candidate> = (0..12).map(|n| unit(n, true)).collect();
        assert!(!pick_box(&exact, CAP).unwrap().unwrap().truncated);
    }

    #[test]
    fn shift_adds_without_toggling_and_respects_the_cap() {
        let current = [unit(1, true), unit(2, true)];
        let pick = pick_box(&[unit(2, true), unit(3, true)], CAP)
            .unwrap()
            .unwrap();
        let merged = add_to_selection(&current, &pick, CAP).unwrap();
        assert_eq!(merged.ids(), [handle(1), handle(2), handle(3)]);
        assert!(!merged.truncated);
        let full: Vec<Candidate> = (0..11).map(|n| unit(n, true)).collect();
        let pick = pick_box(&[unit(20, true), unit(21, true)], CAP)
            .unwrap()
            .unwrap();
        let merged = add_to_selection(&full, &pick, CAP).unwrap();
        assert_eq!(merged.units.len(), CAP);
        assert!(merged.truncated);
    }

    #[test]
    fn shift_with_another_players_units_is_refused() {
        let pick = pick_box(&[unit(3, true)], CAP).unwrap().unwrap();
        assert_eq!(
            add_to_selection(&[unit(1, false)], &pick, CAP),
            Err(Refusal::MixedAdditive)
        );
        let foreign = pick_box(&[unit(3, false)], CAP).unwrap().unwrap();
        assert_eq!(
            add_to_selection(&[unit(1, true)], &foreign, CAP),
            Err(Refusal::MixedAdditive)
        );
    }

    #[test]
    fn double_click_takes_the_clicked_type_among_own_units() {
        let hits = [
            (unit(1, true), *b"opeo"),
            (unit(2, true), *b"ogru"),
            (unit(3, true), *b"opeo"),
            (unit(4, false), *b"opeo"),
            (unit(5, false), *b"opeo"),
        ];
        let pick = same_type(&hits, handle(1), CAP).unwrap();
        assert_eq!(pick.ids(), [handle(1), handle(3)]);
        assert_eq!(same_type(&hits, handle(9), CAP), None);
        // Another player's unit stays a single selection.
        assert_eq!(same_type(&hits, handle(4), CAP).unwrap().ids(), [handle(4)]);
    }

    /// Minimal PE with the four reviewed instruction sites and the tolerance word.
    fn image(cap: u8, second: u8, window: u32, tolerance: f32) -> Vec<u8> {
        const CODE_VA: u32 = 0x4ef000;
        const CODE_SIZE: u32 = 0x10c000;
        const DATA_VA: u32 = 0xe7e000;
        const RAW: usize = 0x400;
        let mut image = vec![0; RAW + CODE_SIZE as usize + 0x1000];
        image[..2].copy_from_slice(b"MZ");
        image[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        image[0x80..0x84].copy_from_slice(b"PE\0\0");
        image[0x84..0x86].copy_from_slice(&0x14cu16.to_le_bytes());
        image[0x86..0x88].copy_from_slice(&2u16.to_le_bytes());
        image[0x94..0x96].copy_from_slice(&64u16.to_le_bytes());
        image[0x98..0x9a].copy_from_slice(&0x10bu16.to_le_bytes());
        image[0xb4..0xb8].copy_from_slice(&0x400000u32.to_le_bytes());
        image[0xd0..0xd4].copy_from_slice(&0xc00000u32.to_le_bytes());
        for (index, (va, size, raw, flags)) in [
            (CODE_VA, CODE_SIZE, RAW as u32, 0x6000_0020u32),
            (DATA_VA, 0x1000, RAW as u32 + CODE_SIZE, 0x4000_0040),
        ]
        .into_iter()
        .enumerate()
        {
            let section = 0xd8 + index * 40;
            for (offset, value) in [
                (8, size),
                (12, va - 0x400000),
                (16, size),
                (20, raw),
                (36, flags),
            ] {
                image[section + offset..section + offset + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
        let at = |va: u32| RAW + (va - CODE_VA) as usize;
        image[at(CAP_COMPARE)..at(CAP_COMPARE) + 4].copy_from_slice(&[0x83, 0x7f, 0x10, cap]);
        image[at(CAP_COMPARE_SECOND)..at(CAP_COMPARE_SECOND) + 7]
            .copy_from_slice(&[0x83, 0xbb, 0xf8, 0x01, 0x00, 0x00, second]);
        let store = [&[0xc7, 0x46, 0x10][..], &window.to_le_bytes()].concat();
        image[at(DOUBLE_CLICK_STORE)..at(DOUBLE_CLICK_STORE) + 7].copy_from_slice(&store);
        let word_va = DATA_VA + 0x168;
        let load = [&[0xf3, 0x0f, 0x10, 0x15][..], &word_va.to_le_bytes()].concat();
        image[at(TOLERANCE_LOAD)..at(TOLERANCE_LOAD) + 8].copy_from_slice(&load);
        let word = RAW + CODE_SIZE as usize + 0x168;
        image[word..word + 4].copy_from_slice(&tolerance.to_le_bytes());
        image
    }

    #[test]
    fn instruction_operands_supply_the_rules_without_substitutes() {
        assert_eq!(decode(&image(7, 7, 321, 0.25)).unwrap(), (7, 321, 0.25));
    }

    #[test]
    fn malformed_images_refuse_instead_of_substituting() {
        assert!(decode(&image(12, 12, 500, 0.01)).is_ok());
        assert!(decode(&image(0, 0, 500, 0.01)).is_err());
        assert!(decode(&image(12, 11, 500, 0.01)).is_err());
        for tolerance in [f32::NAN, 0.0, -1.0, f32::INFINITY] {
            assert!(decode(&image(12, 12, 500, tolerance)).is_err());
        }
        for site in [
            CAP_COMPARE,
            CAP_COMPARE_SECOND,
            DOUBLE_CLICK_STORE,
            TOLERANCE_LOAD,
        ] {
            let mut changed = image(12, 12, 500, 0.01);
            changed[0x400 + (site - 0x4ef000) as usize] ^= 0xff;
            assert!(decode(&changed).is_err(), "{site:#x}");
        }
        assert!(decode(&image(12, 12, 500, 0.01)[..0x2000]).is_err());
        // The synthetic image is not the reviewed client.
        let origin = parameters().origin;
        assert!(Parameters::read(&image(12, 12, 500, 0.01), origin).is_err());
    }

    #[test]
    #[ignore = "requires explicitly supplied reviewed Legacy client image"]
    fn original_rules_come_from_the_selected_image() {
        let path = std::env::var("PCX_SELECTION_CLIENT_IMAGE").expect("PCX_SELECTION_CLIENT_IMAGE");
        let bytes = std::fs::read(&path).unwrap();
        let mut origin = parameters().origin;
        origin.source = path;
        origin.edition = "Legacy target".into();
        origin.version = "1.29.2.9231".into();
        let read = Parameters::read(&bytes, origin).unwrap();
        assert_eq!(read.selection_cap, 12);
        assert_eq!(read.double_click_ms, 500);
        assert_eq!(read.double_click_tolerance, 0.01);
    }
}
