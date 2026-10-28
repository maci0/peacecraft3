//! Ground splats from the selected `Splats\UberSplatData.slk`, without stock defaults.
//!
//! A unit names its splat through UnitUI `uberSplat`. The row supplies the
//! texture, blend mode, `Scale`, the `BirthTime` fade from the `Start` colour to
//! the `Middle` colour, the `Decay` fade from `Middle` to the `End` colour, and
//! those three colours. `PauseTime` is not read: its meaning for a splat bound to
//! a living unit is not established (see `docs/re-notes/campaign-ubersplats.md`).
//!
//! A row that fails validation refuses only the splat it names; the others stay
//! usable. Colour channels are kept as authored (shipped `HFS1` writes 256), so
//! clamping to a displayable range belongs to the renderer.
use crate::scripts::AssetIdentity;
use std::collections::BTreeMap;

/// RGBA channels in authored units (0 to 255 for a byte, not clamped).
pub type Shade = [f32; 4];

/// The colour keys and fade lengths of one authored splat row.
#[derive(Debug, Clone, PartialEq)]
pub struct Splat {
    /// `Dir\file.blp`.
    pub texture: String,
    pub blend_mode: i32,
    /// Authored `Scale`, read as half the square's world width.
    pub scale: f32,
    /// Seconds to fade from `start` to `middle` after the splat appears.
    pub birth: f32,
    /// Seconds to fade from `middle` to `end` after the unit stops existing.
    pub decay: f32,
    pub start: Shade,
    pub middle: Shade,
    pub end: Shade,
}

/// When a splat's unit appeared and ended, in game seconds.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Life {
    /// `None`: present before play began, so the birth fade is already over.
    pub appeared: Option<f64>,
    /// `None`: still alive.
    pub ended: Option<f64>,
}

fn lerp(from: Shade, to: Shade, amount: f32) -> Shade {
    std::array::from_fn(|i| from[i] + (to[i] - from[i]) * amount)
}

/// Linear fade over `length` seconds; a zero length is an immediate cut.
fn fade(from: Shade, to: Shade, length: f32, elapsed: f64) -> Shade {
    if elapsed >= f64::from(length) {
        to
    } else if elapsed <= 0.0 {
        from
    } else {
        lerp(from, to, (elapsed / f64::from(length)) as f32)
    }
}

impl Splat {
    /// The colour at game time `now`, or `None` once the decay fade has finished.
    /// A unit that ends mid birth-fade decays from the colour it had then.
    pub fn shade_at(&self, life: Life, now: f64) -> Option<Shade> {
        let alive_at = |time: f64| match life.appeared {
            None => self.middle,
            Some(appeared) => fade(self.start, self.middle, self.birth, time - appeared),
        };
        let Some(ended) = life.ended else {
            return Some(alive_at(now));
        };
        let since = now - ended;
        (since < f64::from(self.decay))
            .then(|| fade(alive_at(ended), self.end, self.decay, since.max(0.0)))
    }
}

#[derive(Debug)]
pub struct Catalog {
    pub origin: AssetIdentity,
    splats: BTreeMap<[u8; 4], Result<Splat, String>>,
}

fn fourcc_label(id: [u8; 4]) -> String {
    String::from_utf8_lossy(&id).into_owned()
}

/// A finite, non-negative real; no upper bound is invented here.
fn real(row: &pcx_slq::Row<'_>, name: &str) -> Result<f32, String> {
    row.by_name(name)
        .ok_or_else(|| format!("row {}: missing {name}", row.index()))?
        .trim()
        .parse::<f32>()
        .ok()
        .filter(|v| v.is_finite() && *v >= 0.0)
        .ok_or_else(|| format!("row {}: invalid {name}", row.index()))
}

fn shade(row: &pcx_slq::Row<'_>, phase: &str) -> Result<Shade, String> {
    let mut out = [0.0; 4];
    for (channel, letter) in out.iter_mut().zip(["R", "G", "B", "A"]) {
        *channel = real(row, &format!("{phase}{letter}"))?;
    }
    Ok(out)
}

fn read_splat(row: &pcx_slq::Row<'_>) -> Result<Splat, String> {
    let text = |name: &str| {
        row.by_name(name)
            .ok_or_else(|| format!("row {}: missing {name}", row.index()))
    };
    let texture = format!(
        "{}\\{}.blp",
        text("Dir")?.trim().trim_end_matches(['/', '\\']),
        text("file")?.trim()
    );
    pcx_assets::AssetPath::new(&texture).map_err(|e| format!("row {}: {e}", row.index()))?;
    let scale = real(row, "Scale")?;
    if scale == 0.0 {
        return Err(format!("row {}: invalid Scale", row.index()));
    }
    Ok(Splat {
        texture,
        blend_mode: text("BlendMode")?
            .trim()
            .parse()
            .map_err(|_| format!("row {}: invalid BlendMode", row.index()))?,
        scale,
        birth: real(row, "BirthTime")?,
        decay: real(row, "Decay")?,
        start: shade(row, "Start")?,
        middle: shade(row, "Middle")?,
        end: shade(row, "End")?,
    })
}

impl Catalog {
    pub fn read(
        bytes: &[u8],
        origin: AssetIdentity,
        max_bytes: usize,
        max_rows: u32,
    ) -> Result<Self, String> {
        if bytes.len() > max_bytes || max_rows == 0 || !crate::label_sounds::identity(&origin) {
            return Err("ubersplat input budget or identity".into());
        }
        let table = crate::label_sounds::checked_table(
            crate::scripts::ScriptAsset {
                label: &origin.source,
                bytes,
            },
            pcx_slq::CoordinateLimits {
                max_row: max_rows,
                max_column: 256,
            },
            &[
                "Name",
                "Dir",
                "file",
                "BlendMode",
                "Scale",
                "BirthTime",
                "Decay",
                "StartR",
                "StartG",
                "StartB",
                "StartA",
                "MiddleR",
                "MiddleG",
                "MiddleB",
                "MiddleA",
                "EndR",
                "EndG",
                "EndB",
                "EndA",
            ],
        )
        .map_err(|e| e.to_string())?;
        let mut splats = BTreeMap::new();
        for row in table.data_rows() {
            // A row whose name is unreadable cannot be attributed to any unit, so it
            // refuses the table; every other row problem refuses only that splat.
            let id: [u8; 4] = row
                .by_name("Name")
                .ok_or_else(|| format!("row {}: missing Name", row.index()))?
                .as_bytes()
                .try_into()
                .map_err(|_| format!("row {}: ubersplat Name must be a FOURCC", row.index()))?;
            if !id.iter().all(u8::is_ascii_graphic) {
                return Err(format!("row {}: invalid ubersplat Name", row.index()));
            }
            let parsed =
                read_splat(&row).map_err(|e| format!("ubersplat {}: {e}", fourcc_label(id)));
            if splats.insert(id, parsed).is_some() {
                splats.insert(
                    id,
                    Err(format!(
                        "ubersplat {}: row {}: duplicate Name",
                        fourcc_label(id),
                        row.index()
                    )),
                );
            }
        }
        Ok(Self { origin, splats })
    }

    /// The splat row, or why this one cannot be drawn.
    pub fn get(&self, id: [u8; 4]) -> Result<&Splat, String> {
        match self.splats.get(&id) {
            Some(splat) => splat.as_ref().map_err(Clone::clone),
            None => Err(format!("missing ubersplat {}", fourcc_label(id))),
        }
    }
}

/// The unit's UnitUI `uberSplat`, or `None` for an authored empty marker.
pub fn splat_id(definition: &pcx_sim::world::Definition) -> Result<Option<[u8; 4]>, String> {
    let Some(name) = crate::unit_model::unit_ui_text(definition, "uberSplat")? else {
        return Ok(None);
    };
    let id: [u8; 4] = name
        .as_bytes()
        .try_into()
        .map_err(|_| format!("uberSplat {name:?} is not a FOURCC"))?;
    Ok(Some(id))
}

/// What one frame saw of a unit that names a splat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence {
    Alive,
    /// Shown off by script: not drawn, lifecycle unchanged.
    Hidden,
    Dead,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sighting {
    pub id: [u8; 4],
    pub position: [f32; 2],
    pub presence: Presence,
}

/// A splat to draw this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placed<'a> {
    pub id: [u8; 4],
    pub splat: &'a Splat,
    pub position: [f32; 2],
    pub shade: Shade,
}

#[derive(Debug)]
struct Entry {
    id: [u8; 4],
    position: [f32; 2],
    visible: bool,
    life: Life,
}

/// Birth and decay clocks of the splats under units, keyed by unit handle.
///
/// The clock of the first observation is the start of play: a unit first seen
/// alive then counts as present before play began (at its `Middle` colour), and
/// one first seen alive later fades in from `Start`. A unit seen dead, or gone
/// from the sightings, starts its `Decay` fade at the first such observation, so
/// times are accurate to one observation interval.
#[derive(Debug)]
pub struct Tracker<K: Ord> {
    play_start: Option<f64>,
    entries: BTreeMap<K, Entry>,
}

impl<K: Ord> Default for Tracker<K> {
    fn default() -> Self {
        Self {
            play_start: None,
            entries: BTreeMap::new(),
        }
    }
}

impl<K: Ord + Copy> Tracker<K> {
    pub fn observe(&mut self, now: f64, sightings: &BTreeMap<K, Sighting>) {
        let play_start = *self.play_start.get_or_insert(now);
        for (key, entry) in &mut self.entries {
            if !sightings.contains_key(key) {
                entry.life.ended.get_or_insert(now);
            }
        }
        for (key, sighting) in sightings {
            match self.entries.get_mut(key) {
                Some(entry) => {
                    entry.id = sighting.id;
                    entry.position = sighting.position;
                    entry.visible = sighting.presence != Presence::Hidden;
                    match sighting.presence {
                        Presence::Dead => {
                            entry.life.ended.get_or_insert(now);
                        }
                        Presence::Alive => entry.life.ended = None,
                        Presence::Hidden => {}
                    }
                }
                None if sighting.presence == Presence::Alive => {
                    self.entries.insert(
                        *key,
                        Entry {
                            id: sighting.id,
                            position: sighting.position,
                            visible: true,
                            life: Life {
                                appeared: (now > play_start).then_some(now),
                                ended: None,
                            },
                        },
                    );
                }
                None => {}
            }
        }
    }

    /// Every visible splat at `now`. A fully decayed splat is dropped for good;
    /// a refused or missing row reports its error each call.
    pub fn placed<'a>(
        &mut self,
        catalog: &'a Catalog,
        now: f64,
    ) -> BTreeMap<K, Result<Placed<'a>, String>> {
        let mut out = BTreeMap::new();
        self.entries.retain(|key, entry| {
            let splat = match catalog.get(entry.id) {
                Ok(splat) => splat,
                Err(error) => {
                    if entry.visible {
                        out.insert(*key, Err(error));
                    }
                    return true;
                }
            };
            let Some(shade) = splat.shade_at(entry.life, now) else {
                return false;
            };
            if entry.visible {
                out.insert(
                    *key,
                    Ok(Placed {
                        id: entry.id,
                        splat,
                        position: entry.position,
                        shade,
                    }),
                );
            }
            true
        });
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn origin() -> AssetIdentity {
        AssetIdentity {
            source: "synthetic".into(),
            edition: "nonstock".into(),
            version: "test".into(),
        }
    }

    const HEADER: [&str; 19] = [
        "Name",
        "Dir",
        "file",
        "BlendMode",
        "Scale",
        "BirthTime",
        "Decay",
        "StartR",
        "StartG",
        "StartB",
        "StartA",
        "MiddleR",
        "MiddleG",
        "MiddleB",
        "MiddleA",
        "EndR",
        "EndG",
        "EndB",
        "EndA",
    ];

    /// A valid nonstock row; `edits` replaces cells by column name.
    fn row(name: &'static str, edits: &[(&str, &'static str)]) -> [&'static str; 19] {
        let mut cells = [
            name, "D", "f", "3", "37.5", "2", "4", "1", "2", "3", "0", "10", "20", "30", "40", "5",
            "6", "7", "8",
        ];
        for (column, value) in edits {
            let at = HEADER.iter().position(|h| h == column).unwrap();
            cells[at] = value;
        }
        cells
    }

    fn slk(rows: &[[&str; 19]]) -> Vec<u8> {
        let mut out = String::from("ID;PWXL;N;E\r\n");
        for (y, cells) in std::iter::once(HEADER)
            .chain(rows.iter().copied())
            .enumerate()
        {
            for (x, cell) in cells.iter().enumerate() {
                out.push_str(&format!("C;X{};Y{};K\"{cell}\"\r\n", x + 1, y + 1));
            }
        }
        out.push_str("E\r\n");
        out.into_bytes()
    }

    fn read(rows: &[[&str; 19]]) -> Result<Catalog, String> {
        Catalog::read(&slk(rows), origin(), 8192, 16)
    }

    #[test]
    fn rows_keep_authored_texture_scale_blend_fades_and_colours() {
        let catalog = read(&[row(
            "Zq9x",
            &[("Dir", "Nonstock\\Splats\\"), ("file", "Ring")],
        )])
        .unwrap();
        assert_eq!(
            catalog.get(*b"Zq9x"),
            Ok(&Splat {
                texture: "Nonstock\\Splats\\Ring.blp".into(),
                blend_mode: 3,
                scale: 37.5,
                birth: 2.0,
                decay: 4.0,
                start: [1.0, 2.0, 3.0, 0.0],
                middle: [10.0, 20.0, 30.0, 40.0],
                end: [5.0, 6.0, 7.0, 8.0],
            })
        );
        assert_eq!(
            catalog.get(*b"Zq9y"),
            Err("missing ubersplat Zq9y".to_string())
        );
    }

    #[test]
    fn channels_above_a_byte_are_kept_as_authored() {
        let catalog = read(&[row(
            "Zq9x",
            &[("MiddleA", "256"), ("EndR", "300.5"), ("StartG", "0")],
        )])
        .unwrap();
        let splat = catalog.get(*b"Zq9x").unwrap();
        assert_eq!(splat.middle[3], 256.0);
        assert_eq!(splat.end[0], 300.5);
    }

    #[test]
    fn a_malformed_row_refuses_only_its_splat() {
        for (column, bad) in [
            ("Scale", "0"),
            ("Scale", "x"),
            ("MiddleA", "-1"),
            ("MiddleA", "inf"),
            ("MiddleA", "nan"),
            ("EndB", "blue"),
            ("BirthTime", "-0.5"),
            ("Decay", ""),
            ("BlendMode", "0.5"),
            ("Dir", ".."),
        ] {
            let catalog = read(&[row("Zq9x", &[(column, bad)]), row("Zq9y", &[])]).unwrap();
            let error = catalog.get(*b"Zq9x").unwrap_err();
            assert!(error.contains("Zq9x"), "{column}={bad}: {error}");
            assert!(catalog.get(*b"Zq9y").is_ok(), "{column}={bad}");
        }
        let error = read(&[row("Zq9x", &[("MiddleA", "x")])])
            .unwrap()
            .get(*b"Zq9x")
            .unwrap_err();
        assert!(error.contains("MiddleA"), "{error}");
    }

    #[test]
    fn unnameable_rows_budgets_and_duplicates_are_refused() {
        for name in ["abc", "abcde", "ab c"] {
            let error = read(&[row(name, &[])]).unwrap_err();
            assert!(error.contains("Name"), "{name}: {error}");
        }
        let catalog = read(&[row("Zq9x", &[]), row("Zq9x", &[])]).unwrap();
        let error = catalog.get(*b"Zq9x").unwrap_err();
        assert!(error.contains("duplicate"), "{error}");
        let error = Catalog::read(&slk(&[row("Zq9x", &[])]), origin(), 8, 16).unwrap_err();
        assert!(error.contains("budget"), "{error}");
    }

    fn splat() -> Splat {
        Splat {
            texture: "D\\f.blp".into(),
            blend_mode: 0,
            scale: 1.0,
            birth: 2.0,
            decay: 4.0,
            start: [0.0, 100.0, 0.0, 0.0],
            middle: [100.0, 100.0, 100.0, 200.0],
            end: [100.0, 0.0, 300.0, 0.0],
        }
    }

    #[test]
    fn birth_fades_start_to_middle_and_present_at_start_is_already_middle() {
        let splat = splat();
        let born = |appeared| Life {
            appeared,
            ended: None,
        };
        assert_eq!(splat.shade_at(born(Some(10.0)), 9.0), Some(splat.start));
        assert_eq!(
            splat.shade_at(born(Some(10.0)), 11.0),
            Some([50.0, 100.0, 50.0, 100.0])
        );
        assert_eq!(splat.shade_at(born(Some(10.0)), 12.0), Some(splat.middle));
        assert_eq!(splat.shade_at(born(None), 0.0), Some(splat.middle));
        let instant = Splat {
            birth: 0.0,
            ..splat
        };
        assert_eq!(
            instant.shade_at(born(Some(10.0)), 10.0),
            Some(instant.middle)
        );
    }

    #[test]
    fn decay_fades_middle_to_end_then_expires() {
        let splat = splat();
        let ended = |at| Life {
            appeared: None,
            ended: Some(at),
        };
        assert_eq!(splat.shade_at(ended(10.0), 10.0), Some(splat.middle));
        assert_eq!(
            splat.shade_at(ended(10.0), 12.0),
            Some([100.0, 50.0, 200.0, 100.0])
        );
        assert_eq!(splat.shade_at(ended(10.0), 14.0), None);
        let cut = Splat {
            decay: 0.0,
            ..splat
        };
        assert_eq!(cut.shade_at(ended(10.0), 10.0), None);
    }

    #[test]
    fn a_unit_ending_mid_birth_decays_from_its_current_colour() {
        let splat = splat();
        let life = Life {
            appeared: Some(10.0),
            ended: Some(11.0),
        };
        assert_eq!(splat.shade_at(life, 11.0), Some([50.0, 100.0, 50.0, 100.0]));
        assert_eq!(splat.shade_at(life, 13.0), Some([75.0, 50.0, 175.0, 50.0]));
    }

    fn sight(presence: Presence) -> Sighting {
        Sighting {
            id: *b"Zq9x",
            position: [1.0, 2.0],
            presence,
        }
    }

    fn only(unit: u32, sighting: Sighting) -> BTreeMap<u32, Sighting> {
        BTreeMap::from([(unit, sighting)])
    }

    fn catalog() -> Catalog {
        read(&[row("Zq9x", &[("MiddleA", "256")])]).unwrap()
    }

    #[test]
    fn tracker_starts_units_seen_at_play_start_at_middle_and_later_ones_at_start() {
        let catalog = catalog();
        let middle = catalog.get(*b"Zq9x").unwrap().middle;
        let start = catalog.get(*b"Zq9x").unwrap().start;
        let mut tracker = Tracker::default();
        let first = BTreeMap::from([(1, sight(Presence::Alive))]);
        tracker.observe(5.0, &first);
        assert_eq!(
            tracker.placed(&catalog, 5.0)[&1].as_ref().unwrap().shade,
            middle
        );
        let both = BTreeMap::from([(1, sight(Presence::Alive)), (2, sight(Presence::Alive))]);
        tracker.observe(6.0, &both);
        let placed = tracker.placed(&catalog, 6.0);
        assert_eq!(placed[&1].as_ref().unwrap().shade, middle);
        assert_eq!(placed[&2].as_ref().unwrap().shade, start);
        assert_eq!(placed[&2].as_ref().unwrap().position, [1.0, 2.0]);
    }

    #[test]
    fn tracker_decays_dead_and_removed_units_then_drops_them() {
        let catalog = catalog();
        let mut tracker = Tracker::default();
        let both = BTreeMap::from([(1, sight(Presence::Alive)), (2, sight(Presence::Alive))]);
        tracker.observe(0.0, &both);
        let one_dead = BTreeMap::from([(1, sight(Presence::Dead)), (2, sight(Presence::Alive))]);
        tracker.observe(10.0, &one_dead);
        // Unit 2 leaves the sightings (removed) at 11.
        tracker.observe(11.0, &only(1, sight(Presence::Dead)));
        // The dead corpse keeps reporting Dead while it decays.
        let placed = tracker.placed(&catalog, 12.0);
        assert!(placed[&1].is_ok() && placed[&2].is_ok());
        let faded = |unit| placed[&unit].as_ref().unwrap().shade;
        assert_eq!(faded(1)[3], 132.0);
        assert_eq!(faded(2)[3], 194.0);
        tracker.observe(12.0, &only(1, sight(Presence::Dead)));
        assert!(tracker.placed(&catalog, 14.0).contains_key(&2));
        assert!(tracker.placed(&catalog, 16.0).is_empty());
    }

    #[test]
    fn tracker_keeps_hidden_units_undrawn_and_unaged_and_ignores_unseen_dead() {
        let catalog = catalog();
        let mut tracker = Tracker::default();
        tracker.observe(0.0, &only(1, sight(Presence::Alive)));
        tracker.observe(1.0, &only(1, sight(Presence::Hidden)));
        assert!(tracker.placed(&catalog, 100.0).is_empty());
        tracker.observe(101.0, &only(1, sight(Presence::Alive)));
        assert_eq!(tracker.placed(&catalog, 101.0).len(), 1);
        tracker.observe(102.0, &only(9, sight(Presence::Dead)));
        assert!(!tracker.placed(&catalog, 102.0).contains_key(&9));
    }

    #[test]
    fn tracker_reports_a_refused_or_missing_row_for_visible_units() {
        let catalog = read(&[row("Zq9x", &[("Scale", "x")])]).unwrap();
        let mut tracker = Tracker::default();
        let unknown = Sighting {
            id: *b"Zq9y",
            ..sight(Presence::Alive)
        };
        let both = BTreeMap::from([(1, sight(Presence::Alive)), (2, unknown)]);
        tracker.observe(0.0, &both);
        let placed = tracker.placed(&catalog, 0.0);
        assert!(placed[&1].as_ref().unwrap_err().contains("Scale"));
        assert!(placed[&2].as_ref().unwrap_err().contains("missing"));
    }
}
