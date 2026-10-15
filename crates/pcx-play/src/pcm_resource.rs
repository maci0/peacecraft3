use super::{PcmWave, WaveError};
use crate::neutral_player::{Sha256Digest, sha256};
use crate::scripts::{AssetIdentity, ScriptAsset};

/// Complete qualified PCM resource shared by label and direct-file bindings.
#[derive(Debug)]
pub struct PcmResource {
    member: String,
    source_label: String,
    origin: AssetIdentity,
    sha256: Sha256Digest,
    bytes: Vec<u8>,
    pcm: PcmWave,
}
pub(crate) enum ResourceFailure {
    Allocation,
    Decode(WaveError),
}
impl PcmResource {
    /// Callers preflight the complete input set and its metadata before copying.
    pub(crate) fn read(
        member: &str,
        asset: ScriptAsset<'_>,
        origin: &AssetIdentity,
    ) -> Result<Self, ResourceFailure> {
        let pcm = PcmWave::inspect(asset.bytes).map_err(ResourceFailure::Decode)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(asset.bytes.len())
            .map_err(|_| ResourceFailure::Allocation)?;
        bytes.extend_from_slice(asset.bytes);
        Ok(Self {
            member: member.into(),
            source_label: asset.label.into(),
            origin: origin.clone(),
            sha256: sha256(&bytes),
            bytes,
            pcm,
        })
    }
    pub fn member(&self) -> &str {
        &self.member
    }
    pub fn source_label(&self) -> &str {
        &self.source_label
    }
    pub fn origin(&self) -> &AssetIdentity {
        &self.origin
    }
    pub fn sha256(&self) -> Sha256Digest {
        self.sha256
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn pcm(&self) -> &PcmWave {
        &self.pcm
    }
}
