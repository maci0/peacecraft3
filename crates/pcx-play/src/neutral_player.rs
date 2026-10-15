//! Selected-successful-W3I reconstruction; no native cache/default emulation.
use crate::{
    map_admission::ParsedMapContext,
    scripts::{AssetIdentity, MapScripts, SourceDigest},
};
use pcx_script::{Host, Value, ast::TypeKind};
use sha2::{Digest, Sha256};
use std::sync::Arc;

pub const NATIVE_NAME: &str = "GetPlayerNeutralPassive";
pub const AGGRESSIVE_NATIVE_NAME: &str = "GetPlayerNeutralAggressive";
pub type Sha256Digest = [u8; 32];
pub(crate) fn sha256(bytes: &[u8]) -> Sha256Digest {
    Sha256::digest(bytes).into()
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleProblem {
    Budget,
    UnapprovedProfile,
    ProfileFormat,
    ImageIdentity,
    PeFormat,
    Range,
    RangeIdentity,
    Binding,
    GetterShape,
    HeaderShape,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextProblem {
    MissingAdmission,
    ChangedAdmission,
    MissingInfoReceipt,
    ChangedInfo,
    InfoParse(String),
    MissingThirdWord,
    UnsupportedHeader,
    Signature,
    Arguments,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NeutralError {
    Rule(RuleProblem),
    Context {
        map: String,
        problem: ContextProblem,
    },
}
impl std::fmt::Display for NeutralError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "neutral-player context: {self:?}")
    }
}
impl std::error::Error for NeutralError {}

/// A fingerprint supplied by local review/configuration, independently of imported bytes.
/// Do not construct this from an untrusted profile's self-reported digest.
pub struct LocalProfileApproval(Sha256Digest);
impl LocalProfileApproval {
    pub fn from_reviewed_fingerprint(digest: Sha256Digest) -> Self {
        Self(digest)
    }
}
/// All bounds are explicit caller policy, not native limits.
#[derive(Clone, Copy)]
pub struct RuleInputBudget {
    pub image_bytes: usize,
    pub profile_bytes: usize,
    pub sections: usize,
}
#[derive(Debug)]
pub struct VerifiedNeutralRule {
    pub(crate) native_name: &'static str,
    pub(crate) origin: AssetIdentity,
    pub(crate) image_sha256: Sha256Digest,
    pub(crate) profile_sha256: Sha256Digest,
    pub(crate) threshold: u32,
    pub(crate) below: i32,
    pub(crate) at_or_above: i32,
    pub(crate) min_tag: u32,
    pub(crate) max_tag: u32,
}
impl VerifiedNeutralRule {
    pub fn import(
        image: &[u8],
        profile: &[u8],
        approval: &LocalProfileApproval,
        budget: RuleInputBudget,
    ) -> Result<Self, NeutralError> {
        super::neutral_rule_input::import(image, profile, &approval.0, budget, NATIVE_NAME)
    }
    /// Import only the aggressive native's independently reviewed profile.
    pub fn import_aggressive(
        image: &[u8],
        profile: &[u8],
        approval: &LocalProfileApproval,
        budget: RuleInputBudget,
    ) -> Result<Self, NeutralError> {
        super::neutral_rule_input::import(
            image,
            profile,
            &approval.0,
            budget,
            AGGRESSIVE_NATIVE_NAME,
        )
    }
    pub fn origin(&self) -> &AssetIdentity {
        &self.origin
    }
    pub fn image_sha256(&self) -> Sha256Digest {
        self.image_sha256
    }
    pub fn profile_sha256(&self) -> Sha256Digest {
        self.profile_sha256
    }
}

/// Private, same-import witness. No public constructor can authenticate loose labels.
pub(crate) struct AdmissionWitness {
    map_sha256: Sha256Digest,
    info_sha256: Sha256Digest,
    map_identity: AssetIdentity,
    map_label: String,
    map_fnv: u64,
    scripts: Arc<MapScripts>,
}
impl AdmissionWitness {
    pub(crate) fn new(
        map: &[u8],
        info: &[u8],
        map_identity: AssetIdentity,
        map_digest: &SourceDigest,
        scripts: Arc<MapScripts>,
    ) -> Self {
        Self {
            map_sha256: sha256(map),
            info_sha256: sha256(info),
            map_identity,
            map_label: map_digest.label.clone(),
            map_fnv: map_digest.hash,
            scripts,
        }
    }
}
/// Owns the exact admitted context; no filename cache or global native pointer.
pub struct SelectedMapNeutralHost<H> {
    inner: H,
    context: Arc<ParsedMapContext>,
    rule: Arc<VerifiedNeutralRule>,
    word: u32,
    map_sha256: Sha256Digest,
    info_sha256: Sha256Digest,
}
impl<H> SelectedMapNeutralHost<H> {
    pub fn try_new(
        context: Arc<ParsedMapContext>,
        rule: Arc<VerifiedNeutralRule>,
        inner: H,
    ) -> Result<Self, NeutralError> {
        let fail = |problem| NeutralError::Context {
            map: context.map_digest.label.clone(),
            problem,
        };
        let witness = context
            .admission_witness
            .as_ref()
            .ok_or_else(|| fail(ContextProblem::MissingAdmission))?;
        if context.map_digest.label != witness.map_label
            || context.map_digest.hash != witness.map_fnv
            || context.sources.map != witness.map_identity
            || context.scripts.identity != witness.map_identity
            || !Arc::ptr_eq(&context.scripts, &witness.scripts)
            || context.world.source.install != witness.map_identity.source
            || context.world.source.edition != witness.map_identity.edition
            || context.world.source.map != witness.map_label
        {
            return Err(fail(ContextProblem::ChangedAdmission));
        }
        let raw = context.world.info.raw();
        if sha256(raw) != witness.info_sha256 {
            return Err(fail(ContextProblem::ChangedInfo));
        }
        let label = format!("{}:war3map.w3i", witness.map_label);
        let mut receipts = context
            .world_member_digests
            .iter()
            .filter(|r| r.label == label);
        let receipt = receipts
            .next()
            .ok_or_else(|| fail(ContextProblem::MissingInfoReceipt))?;
        // FNV is only an existing reproducibility join; SHA/admission owns identity.
        if receipts.next().is_some() || receipt.hash != pcx_script::fnv1a64(raw) {
            return Err(fail(ContextProblem::ChangedInfo));
        }
        let parsed = pcx_map::MapInfo::parse(raw)
            .map_err(|e| fail(ContextProblem::InfoParse(e.to_string())))?;
        if parsed.version() != context.world.info.version()
            || parsed.constant_b != context.world.info.constant_b
        {
            return Err(fail(ContextProblem::ChangedInfo));
        }
        if !(rule.min_tag..=rule.max_tag).contains(&parsed.version()) {
            return Err(fail(ContextProblem::UnsupportedHeader));
        }
        let word = parsed
            .constant_b
            .ok_or_else(|| fail(ContextProblem::MissingThirdWord))?;
        let signature = context
            .scripts
            .program()
            .natives()
            .get(rule.native_name)
            .ok_or_else(|| fail(ContextProblem::Signature))?;
        if !signature.params.is_empty()
            || signature.ret.array
            || signature.ret.kind != TypeKind::Integer
        {
            return Err(fail(ContextProblem::Signature));
        }
        let map_sha256 = witness.map_sha256;
        let info_sha256 = witness.info_sha256;
        Ok(Self {
            inner,
            context,
            rule,
            word,
            map_sha256,
            info_sha256,
        })
    }
    pub fn context(&self) -> &Arc<ParsedMapContext> {
        &self.context
    }
    pub fn inner(&self) -> &H {
        &self.inner
    }
    pub fn inner_mut(&mut self) -> &mut H {
        &mut self.inner
    }
    pub fn map_sha256(&self) -> Sha256Digest {
        self.map_sha256
    }
    pub fn info_sha256(&self) -> Sha256Digest {
        self.info_sha256
    }
}
impl<H: Host> Host for SelectedMapNeutralHost<H> {
    fn native(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
        if name != self.rule.native_name {
            return self.inner.native(name, args);
        }
        if !args.is_empty() {
            return Err(NeutralError::Context {
                map: self.context.map_digest.label.clone(),
                problem: ContextProblem::Arguments,
            }
            .to_string());
        }
        Ok(Value::Int(if self.word < self.rule.threshold {
            self.rule.below
        } else {
            self.rule.at_or_above
        }))
    }
    fn table_string(&self, id: u32) -> Option<String> {
        self.inner.table_string(id)
    }
    fn resolve_table_string(&self, id: u32) -> Result<Option<String>, String> {
        self.inner.resolve_table_string(id)
    }
    fn code_tag(&self, tag: &str) -> Option<String> {
        self.inner.code_tag(tag)
    }
}

#[cfg(test)]
#[path = "neutral_player_tests.rs"]
mod tests;
