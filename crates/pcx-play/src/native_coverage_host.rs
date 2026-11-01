//! Native/API coverage accounting against the selected program.
//! Declaration counts (1,383 natives / 1,044 library functions in the measured
//! Legacy observation) are inventory facts, not support claims. This module
//! reports, for the admitted program, which natives have host dispatch and
//! which remain explicit refusals. No stock native list is copied here.
use super::ObjectHost;

/// Coverage for one admitted program.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NativeCoverage {
    /// Natives with host dispatch in this build.
    pub supported: Vec<String>,
    /// Declared natives without host dispatch; calls refuse explicitly.
    pub unsupported: Vec<String>,
    /// Library functions in the admitted program (callable JASS, no host needed).
    pub library_functions: usize,
}

impl ObjectHost {
    /// Names with host dispatch in this build. This is the audit surface;
    /// adding a native wires it here through its existing host module.
    pub fn supported_native_names() -> Vec<&'static str> {
        vec![
            "Preloader",
            "Preload",
            "PreloadEnd",
            "PreloadStart",
            "PreloadRefresh",
            "PreloadEndEx",
            "PreloadGenClear",
            "PreloadGenStart",
            "PreloadGenEnd",
            "AddWeatherEffect",
            "EnableWeatherEffect",
            "RemoveWeatherEffect",
            "SetSkyModel",
            "SetTerrainFogEx",
            "ResetTerrainFog",
            "TriggerRegisterGameEvent",
            "ConvertGameEvent",
            "EnumItemsInRect",
            "EnumDestructablesInRect",
            "GetEnumItem",
            "GetFilterItem",
            "GetEnumDestructable",
            "GetFilterDestructable",
            "SetDoodadAnimationRect",
            "CreateCameraSetup",
            "CameraSetupSetField",
            "CameraSetupGetField",
            "CameraSetupSetDestPosition",
            "CameraSetupGetDestPositionX",
            "CameraSetupGetDestPositionY",
            "CameraSetupGetDestPositionLoc",
            "CameraSetupApplyForceDuration",
            "CreateFogModifierRadiusLoc",
            "CreateFogModifierRadius",
            "CreateFogModifierRect",
            "FogModifierStart",
            "FogModifierStop",
            "DestroyFogModifier",
            "GetWorldBounds",
            "Rect",
        ]
    }

    /// Compare the admitted program's declarations against host dispatch.
    pub fn native_coverage(&self) -> NativeCoverage {
        let supported = Self::supported_native_names()
            .into_iter()
            .map(str::to_owned)
            .collect::<std::collections::BTreeSet<_>>();
        let mut unsupported = Vec::new();
        for (name, _) in self.inner.program.natives().iter() {
            if !supported.contains(name) {
                unsupported.push(name.clone());
            }
        }
        unsupported.sort();
        unsupported.dedup();
        let supported: Vec<String> = self
            .inner
            .program
            .natives()
            .iter()
            .filter(|(name, _)| supported.contains(*name))
            .map(|(name, _)| name.clone())
            .collect();
        NativeCoverage {
            supported,
            unsupported,
            library_functions: self.inner.program.functions().len(),
        }
    }
}
