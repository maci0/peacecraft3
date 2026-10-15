//! Explicit local source bindings, not automatic native lookup/precedence discovery.
use crate::neutral_player::{NeutralError, RuleInputBudget, Sha256Digest, sha256};
use crate::neutral_rule_input::Pe;
pub use crate::neutral_rule_input::Window as SourceWindow;
use crate::scripts::AssetIdentity;

/// Caller-reviewed section/key ordering and scale references in one qualified PE.
/// Fingerprints establish byte identity; the caller must establish each reference's role.
#[derive(Clone, Debug)]
pub struct CameraMarginBinding {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    pub section: SourceWindow,
    pub keys: [SourceWindow; 4],
    pub scale: SourceWindow,
}
#[derive(Clone, Copy)]
pub struct CameraMarginBudget {
    pub image: RuleInputBudget,
    pub config_bytes: usize,
}
#[derive(Debug, thiserror::Error)]
#[error("camera input {source_context}: {problem}")]
pub struct Error {
    pub source_context: String,
    pub problem: Problem,
}
#[derive(Debug, thiserror::Error)]
pub enum Problem {
    #[error("input/selection budget exceeded")]
    Budget,
    #[error("missing source identity or incorrect image fingerprint")]
    Identity,
    #[error("invalid selected image window: {0}")]
    Image(#[source] NeutralError),
    #[error("invalid section/key/scale selection")]
    Selection,
    #[error("config line {line}: {reason}")]
    Config { line: usize, reason: &'static str },
    #[error("missing required section/key or nonfinite scaled margin")]
    MissingOrInvalid,
}
#[derive(Debug)]
pub struct CameraMargins {
    binding: CameraMarginBinding,
    config_origin: AssetIdentity,
    config_sha256: Sha256Digest,
    values: [f32; 4],
}
fn identity(origin: &AssetIdentity) -> bool {
    [&origin.edition, &origin.version, &origin.source]
        .iter()
        .all(|s| !s.trim().is_empty())
}
impl CameraMargins {
    /// Reads NUL-terminated source names and the little-endian f32 scale from
    /// initialized, readable, SHA-checked PE ranges. Missing config never defaults.
    pub fn read(
        image: &[u8],
        binding: CameraMarginBinding,
        config: &[u8],
        config_origin: AssetIdentity,
        budget: CameraMarginBudget,
    ) -> Result<Self, Error> {
        let fail = |problem| Error {
            source_context: format!("{} / {}", binding.origin.source, config_origin.source),
            problem,
        };
        if image.len() > budget.image.image_bytes || config.len() > budget.config_bytes {
            return Err(fail(Problem::Budget));
        }
        let selected = binding
            .keys
            .iter()
            .chain([&binding.section, &binding.scale]);
        let mut total = 0usize;
        for w in selected {
            total = total
                .checked_add(w.len as usize)
                .filter(|&n| n <= budget.image.profile_bytes)
                .ok_or_else(|| fail(Problem::Budget))?;
        }
        if !identity(&binding.origin)
            || !identity(&config_origin)
            || sha256(image) != binding.image_sha256
        {
            return Err(fail(Problem::Identity));
        }
        let pe = Pe::parse(image, budget.image).map_err(|e| fail(Problem::Image(e)))?;
        let name = |w: &SourceWindow| -> Result<&str, Error> {
            let bytes = pe.window(w, false).map_err(|e| fail(Problem::Image(e)))?;
            let text = bytes
                .strip_suffix(&[0])
                .and_then(|b| std::str::from_utf8(b).ok());
            text.filter(|s| {
                !s.is_empty()
                    && s.trim() == *s
                    && !s.chars().any(|c| c.is_control() || "[]=;/".contains(c))
            })
            .ok_or_else(|| fail(Problem::Selection))
        };
        let section = name(&binding.section)?;
        let keys = [
            name(&binding.keys[0])?,
            name(&binding.keys[1])?,
            name(&binding.keys[2])?,
            name(&binding.keys[3])?,
        ];
        for (i, key) in keys.iter().enumerate() {
            if keys[..i].iter().any(|old| old.eq_ignore_ascii_case(key)) {
                return Err(fail(Problem::Selection));
            }
        }
        let raw = pe
            .window(&binding.scale, false)
            .map_err(|e| fail(Problem::Image(e)))?;
        let scale = f32::from_le_bytes(raw.try_into().map_err(|_| fail(Problem::Selection))?);
        if !scale.is_finite() || scale <= 0.0 {
            return Err(fail(Problem::Selection));
        }
        let text = std::str::from_utf8(config).map_err(|_| {
            fail(Problem::Config {
                line: 0,
                reason: "config is not UTF-8",
            })
        })?;
        let raw = parse(text, section, keys).map_err(fail)?;
        let values = raw.map(|n| n as f32 * scale);
        if values.iter().any(|v| !v.is_finite()) {
            return Err(fail(Problem::MissingOrInvalid));
        }
        Ok(Self {
            binding,
            config_origin,
            config_sha256: sha256(config),
            values,
        })
    }
    pub fn binding(&self) -> &CameraMarginBinding {
        &self.binding
    }
    pub fn config_origin(&self) -> &AssetIdentity {
        &self.config_origin
    }
    pub fn config_sha256(&self) -> Sha256Digest {
        self.config_sha256
    }
    pub fn values(&self) -> &[f32; 4] {
        &self.values
    }
}
fn parse(text: &str, section: &str, keys: [&str; 4]) -> Result<[i32; 4], Problem> {
    let mut active = false;
    let mut seen = false;
    let mut values = [None; 4];
    for (line, text) in text.trim_start_matches('\u{feff}').lines().enumerate() {
        let text = text.split_once("//").map_or(text, |(head, _)| head).trim();
        if text.is_empty() || text.starts_with(';') {
            continue;
        }
        let invalid = |reason| Problem::Config {
            line: line + 1,
            reason,
        };
        if text.starts_with('[') {
            let name = text
                .strip_prefix('[')
                .and_then(|s| s.strip_suffix(']'))
                .ok_or_else(|| invalid("malformed section"))?;
            active = name.trim().eq_ignore_ascii_case(section);
            if active && seen {
                return Err(invalid("duplicate selected section"));
            }
            seen |= active;
        } else if active {
            let (key, value) = text
                .split_once('=')
                .ok_or_else(|| invalid("expected key=value"))?;
            if let Some(at) = keys.iter().position(|k| k.eq_ignore_ascii_case(key.trim())) {
                if values[at].is_some() {
                    return Err(invalid("duplicate selected key"));
                }
                values[at] = Some(
                    value
                        .trim()
                        .parse::<i32>()
                        .map_err(|_| invalid("expected signed integer"))?,
                );
            }
        }
    }
    match values {
        [Some(a), Some(b), Some(c), Some(d)] => Ok([a, b, c, d]),
        _ => Err(Problem::MissingOrInvalid),
    }
}

#[cfg(test)]
#[path = "camera_margin_tests.rs"]
mod tests;
