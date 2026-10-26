//! Source-qualified hero counters and authored progression limits.
use crate::camera_margins::SourceWindow;
use crate::neutral_player::{RuleInputBudget, Sha256Digest, sha256};
use crate::neutral_rule_input::Pe;
use crate::scripts::AssetIdentity;
use pcx_objects::editor::hero::Progression;

#[derive(Clone, Debug)]
pub struct Binding {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    pub level_initializer: SourceWindow,
    pub level_query: SourceWindow,
    pub points_initializer: SourceWindow,
    pub slot_limit: SourceWindow,
    pub first_rank: SourceWindow,
    pub hero_comparison: SourceWindow,
    pub hero_handler: SourceWindow,
    pub other_implicit: Vec<SourceWindow>,
}

#[derive(Clone, Debug)]
pub struct Rules {
    pub mirror_image: Result<std::sync::Arc<crate::mirror_image_rules::Rules>, String>,
    pub(crate) experience: Option<std::sync::Arc<crate::hero_experience::Rules>>,
    pub(crate) binding: Binding,
    pub(crate) progression: Progression,
    pub(crate) initial_level: i32,
    pub(crate) initial_points: i32,
    pub(crate) slots: usize,
    pub(crate) first_rank: i32,
    pub(crate) hero_alias: [u8; 4],
    pub(crate) other_implicit: Vec<[u8; 4]>,
}

impl Rules {
    /// Optional ability capability; unrelated hero skills retain their admission.
    pub fn with_mirror_image(
        mut self,
        rules: Result<crate::mirror_image_rules::Rules, String>,
    ) -> Self {
        self.mirror_image = rules.and_then(|rules| {
            if rules.image_sha256 != self.binding.image_sha256 {
                Err("Mirror Image source differs from hero admission".into())
            } else {
                Ok(std::sync::Arc::new(rules))
            }
        });
        self
    }
    pub fn with_experience(
        mut self,
        image: &[u8],
        profiles: &[(&str, &[u8])],
        max_bytes: usize,
        max_levels: usize,
    ) -> Result<Self, String> {
        if self.experience.is_some() || sha256(image) != self.binding.image_sha256 {
            return Err("XP source differs or already admitted".into());
        }
        let thresholds = pcx_objects::editor::hero_experience::Experience::read(
            profiles,
            self.initial_level,
            self.progression.max_level,
            max_bytes,
            max_levels,
        )?;
        let mut rules =
            crate::hero_experience::Rules::read(image, self.binding.origin.clone(), thresholds)?;
        rules.admit_awards(
            image,
            profiles,
            self.progression.max_level,
            max_bytes,
            max_levels,
        )?;
        self.experience = Some(std::sync::Arc::new(rules));
        Ok(self)
    }
    pub fn read(
        image: &[u8],
        binding: Binding,
        progression: Progression,
        budget: RuleInputBudget,
    ) -> Result<Self, String> {
        let fail = |reason: &str| format!("{}: hero skills: {reason}", binding.origin.source);
        let windows = [
            &binding.level_initializer,
            &binding.level_query,
            &binding.points_initializer,
            &binding.slot_limit,
            &binding.first_rank,
            &binding.hero_comparison,
            &binding.hero_handler,
        ];
        if image.len() > budget.image_bytes
            || binding.other_implicit.len() > budget.profile_bytes / 6
            || windows
                .iter()
                .copied()
                .chain(binding.other_implicit.iter())
                .map(|w| u64::from(w.len))
                .sum::<u64>()
                > budget.profile_bytes as u64
        {
            return Err(fail("input budget exceeded"));
        }
        if [
            &binding.origin.source,
            &binding.origin.edition,
            &binding.origin.version,
        ]
        .iter()
        .any(|s| s.trim().is_empty())
            || sha256(image) != binding.image_sha256
            || progression.sources.is_empty()
            || progression.max_level < 1
            || progression.ability_level_skip < 0
        {
            return Err(fail("incomplete or mismatched source identity/rules"));
        }
        let pe = Pe::parse(image, budget).map_err(|e| fail(&e.to_string()))?;
        let code = |window| pe.window(window, true).map_err(|e| fail(&e.to_string()));
        let initial = code(&binding.level_initializer)?;
        let query = code(&binding.level_query)?;
        let points = code(&binding.points_initializer)?;
        let slots = code(&binding.slot_limit)?;
        let first_rank = code(&binding.first_rank)?;
        if initial.len() != 7
            || initial[..3] != [0xc7, 0x46, 0x74]
            || query.len() != 20
            || query[..13]
                != [
                    0x8b, 0x81, 0xfc, 3, 0, 0, 0x85, 0xc0, 0x74, 0x0a, 0x8d, 0x48, 0x70,
                ]
            || query[13] != 0xe8
            || query[18..] != [0x40, 0xc3]
            || points.len() != 21
            || points[..6] != [0xc7, 0x86, 0x94, 0, 0, 0]
            || points[10..12] != [0x83, 0xfb]
            || points[13..] != [0x7d, 6, 0x89, 0x9e, 0x94, 0, 0, 0]
            || slots.len() != 5
            || slots[..2] != [0x83, 0xf9]
            || slots[3] != 0x7d
            || first_rank.len() != 16
            || first_rank[0] != 0x6a
            || first_rank[2..12] != [0x8b, 0xb5, 0xdc, 0xfd, 0xff, 0xff, 0x8b, 0xce, 0x57, 0xe8]
        {
            return Err(fail("unsupported counter/slot instruction shapes"));
        }
        let initial_level = i32::from_le_bytes(initial[3..7].try_into().expect("shape"))
            .checked_add(1)
            .ok_or_else(|| fail("initial level overflow"))?;
        let initial_points = i32::from_le_bytes(points[6..10].try_into().expect("shape"));
        let slots = usize::from(slots[2]);
        let first_rank = i32::from(first_rank[1] as i8);
        if initial_level < 1
            || initial_level > progression.max_level
            || initial_points < 0
            || initial_points != i32::from(points[12] as i8)
            || slots == 0
            || slots > i8::MAX as usize
            || first_rank <= 0
        {
            return Err(fail("invalid initial counters or slot limit"));
        }
        let comparison = code(&binding.hero_comparison)?;
        let handler = code(&binding.hero_handler)?;
        if comparison.len() != 8
            || comparison[..2] != [0x81, 0xfe]
            || comparison[6] != 0x74
            || binding
                .hero_comparison
                .va
                .wrapping_add(8)
                .wrapping_add_signed(i32::from(comparison[7] as i8))
                != binding.hero_handler.va
            || handler.len() != 11
            || handler[..2] != [0x8b, 0x81]
            || handler[2..6] != query[2..6]
            || handler[6..] != [0x5e, 0x5d, 0xc2, 0x14, 0]
        {
            return Err(fail("hero ability dispatch relationship"));
        }
        let alias = |bytes: &[u8]| -> Result<[u8; 4], String> {
            if bytes.len() < 6 || bytes[..2] != [0x81, 0xfe] {
                return Err(fail("implicit ability comparison"));
            }
            let id = u32::from_le_bytes(bytes[2..6].try_into().expect("shape")).to_be_bytes();
            if !id.iter().all(u8::is_ascii_graphic) {
                return Err(fail("implicit ability alias"));
            }
            Ok(id)
        };
        let hero_alias = alias(comparison)?;
        let mut other_implicit = Vec::new();
        other_implicit
            .try_reserve_exact(binding.other_implicit.len())
            .map_err(|_| fail("implicit alias allocation"))?;
        for window in &binding.other_implicit {
            if window.len != 6 {
                return Err(fail("implicit comparison length"));
            }
            let id = alias(code(window)?)?;
            if id == hero_alias || other_implicit.contains(&id) {
                return Err(fail("duplicate implicit ability alias"));
            }
            other_implicit.push(id);
        }
        Ok(Self {
            mirror_image: Err("Mirror Image source not admitted".into()),
            experience: None,
            binding,
            progression,
            initial_level,
            initial_points,
            slots,
            first_rank,
            hero_alias,
            other_implicit,
        })
    }

    pub fn origin(&self) -> &AssetIdentity {
        &self.binding.origin
    }
    pub fn image_sha256(&self) -> &Sha256Digest {
        &self.binding.image_sha256
    }
}

#[cfg(test)]
#[path = "hero_skills_tests.rs"]
pub(crate) mod tests;
