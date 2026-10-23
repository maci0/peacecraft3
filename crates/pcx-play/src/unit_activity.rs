//! Source-bound activity identifiers; execution lives on the existing unit arena.
use crate::{
    camera_margins::SourceWindow,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    order_registry::Registry,
    scripts::AssetIdentity,
};

#[derive(Clone, Debug)]
pub struct Binding {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    pub hold: SourceWindow,
    pub stop: SourceWindow,
    pub movement: SourceWindow,
    pub patrol: SourceWindow,
}

#[derive(Clone, Debug)]
pub struct Rules {
    pub(crate) binding: Binding,
    pub(crate) hold: i32,
    pub(crate) stop: i32,
    pub(crate) movement: i32,
    pub(crate) patrol: i32,
    pub(crate) follow: Option<FollowRules>,
    pub(crate) move_speed: Option<pcx_objects::editor::misc::MoveSpeedLimits>,
}

#[derive(Clone, Debug)]
pub struct FollowRules {
    pub range: f32,
    pub structure_range: f32,
    pub sources: Vec<(String, Sha256Digest)>,
}

impl Rules {
    pub fn read(
        image: &[u8],
        binding: Binding,
        registry: &Registry,
        budget: RuleInputBudget,
    ) -> Result<Self, String> {
        let fail = |reason: &str| format!("unit activity {}: {reason}", binding.origin.source);
        if image.len() > budget.image_bytes
            || [binding.hold, binding.stop, binding.movement, binding.patrol]
                .iter()
                .map(|w| u64::from(w.len))
                .sum::<u64>()
                > budget.profile_bytes as u64
        {
            return Err(fail("input budget"));
        }
        if [
            &binding.origin.source,
            &binding.origin.edition,
            &binding.origin.version,
        ]
        .iter()
        .any(|s| s.trim().is_empty())
            || sha256(image) != binding.image_sha256
            || registry.binding().image_sha256 != binding.image_sha256
        {
            return Err(fail("client identity mismatch"));
        }
        let pe = Pe::parse(image, budget).map_err(|e| fail(&e.to_string()))?;
        let compare = |window: &SourceWindow, prefix: &[u8]| -> Result<i32, String> {
            let b = pe.window(window, true).map_err(|e| fail(&e.to_string()))?;
            if b.len() != prefix.len() + 4 || !b.starts_with(prefix) {
                return Err(fail("unsupported order comparison"));
            }
            let id = i32::from_le_bytes(b[prefix.len()..].try_into().unwrap());
            if registry.name(id).is_none() {
                return Err(fail("order missing from selected registry"));
            }
            Ok(id)
        };
        let hold = compare(&binding.hold, &[0x3d])?;
        let stop = compare(&binding.stop, &[0x81, 0xfb])?;
        let movement = compare(&binding.movement, &[0x3d])?;
        let patrol = compare(&binding.patrol, &[0x3d])?;
        if hold == stop
            || stop == movement
            || hold == movement
            || [hold, stop, movement].contains(&patrol)
        {
            return Err(fail("ambiguous activity identifiers"));
        }
        Ok(Self {
            binding,
            hold,
            stop,
            movement,
            patrol,
            follow: None,
            move_speed: None,
        })
    }

    pub fn with_follow_profiles(
        mut self,
        profiles: &[(&str, &[u8])],
        max_bytes: usize,
    ) -> Result<Self, String> {
        let ([range, structure_range], _) = pcx_objects::editor::misc::misc_scalars(
            profiles,
            ["FollowRange", "StructureFollowRange"],
            max_bytes,
        )?;
        let required = |value: Option<f32>, key: &str| {
            value
                .filter(|v| *v >= 0.0)
                .ok_or_else(|| format!("missing or negative Misc/{key}"))
        };
        self.follow = Some(FollowRules {
            range: required(range, "FollowRange")?,
            structure_range: required(structure_range, "StructureFollowRange")?,
            sources: profiles
                .iter()
                .map(|(label, bytes)| ((*label).into(), sha256(bytes)))
                .collect(),
        });
        Ok(self)
    }

    pub fn follow_rules(&self) -> Option<&FollowRules> {
        self.follow.as_ref()
    }
    pub fn with_move_speed_profiles(
        mut self,
        profiles: &[(&str, &[u8])],
        max_bytes: usize,
    ) -> Result<Self, String> {
        self.move_speed = Some(pcx_objects::editor::misc::MoveSpeedLimits::read(
            profiles, max_bytes,
        )?);
        Ok(self)
    }
    pub fn move_speed_limits(&self) -> Option<pcx_objects::editor::misc::MoveSpeedLimits> {
        self.move_speed
    }
    pub fn binding(&self) -> &Binding {
        &self.binding
    }
}
