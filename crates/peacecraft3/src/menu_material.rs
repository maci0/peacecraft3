//! Source-selected menu geometry blending; PRE2 and UI images use other paths.
use bevy::{
    mesh::MeshVertexBufferLayoutRef,
    pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline},
    prelude::*,
    render::render_resource::{
        AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState,
        RenderPipelineDescriptor, SpecializedMeshPipelineError,
    },
};
use std::fmt;

pub(crate) type MenuMaterial = ExtendedMaterial<StandardMaterial, MenuBlend>;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
#[bind_group_data(MenuBlendKey)]
pub(crate) struct MenuBlend {
    one_one: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct MenuBlendKey {
    one_one: bool,
}

impl From<&MenuBlend> for MenuBlendKey {
    fn from(value: &MenuBlend) -> Self {
        Self {
            one_one: value.one_one,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SourceModeError {
    UnknownFilterMode(u32),
    InconsistentAlphaMode(u32),
}

impl fmt::Display for SourceModeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownFilterMode(mode) => write!(f, "unsupported menu filter mode {mode}"),
            Self::InconsistentAlphaMode(mode) => {
                write!(f, "menu filter mode {mode} disagrees with base alpha mode")
            }
        }
    }
}

impl std::error::Error for SourceModeError {}

/// Validate the existing layer mapping before any scene/asset writes.
///
/// The caller supplies the checked source filter word, never a mode inferred
/// from Bevy's approximation. Mode 1 retains the caller's cutoff convention;
/// mode 6 retains its explicitly reported opaque fallback.
pub(crate) fn checked_source_mode(
    mut base: StandardMaterial,
    filter_mode: u32,
) -> Result<MenuMaterial, SourceModeError> {
    let consistent = match filter_mode {
        0 | 6 => base.alpha_mode == AlphaMode::Opaque,
        1 => matches!(base.alpha_mode, AlphaMode::Mask(cutoff)
            if cutoff.is_finite() && (0.0..=1.0).contains(&cutoff)),
        2 => base.alpha_mode == AlphaMode::Blend,
        3 | 4 => base.alpha_mode == AlphaMode::Add,
        5 => base.alpha_mode == AlphaMode::Multiply,
        _ => return Err(SourceModeError::UnknownFilterMode(filter_mode)),
    };
    if !consistent {
        return Err(SourceModeError::InconsistentAlphaMode(filter_mode));
    }
    let one_one = filter_mode == 3;
    if one_one {
        // The standard Premultiplied shader branch leaves RGB unchanged.
        base.alpha_mode = AlphaMode::Premultiplied;
    }
    Ok(MenuMaterial {
        base,
        extension: MenuBlend { one_one },
    })
}

impl MenuBlend {
    fn specialize_descriptor(descriptor: &mut RenderPipelineDescriptor, key: MenuBlendKey) {
        if !key.one_one {
            return;
        }
        let target = descriptor
            .fragment
            .as_mut()
            .expect("StandardMaterial forward pipeline has a fragment stage")
            .targets
            .first_mut()
            .and_then(Option::as_mut)
            .expect("StandardMaterial forward pipeline has color target zero");
        target.blend = Some(BlendState {
            color: BlendState::ADDITIVE.color,
            // Compatibility convention: preserve existing Add's destination
            // alpha; original separate-alpha parity remains unverified.
            alpha: BlendComponent {
                src_factor: BlendFactor::Zero,
                dst_factor: BlendFactor::One,
                operation: BlendOperation::Add,
            },
        });
    }
}

impl MaterialExtension for MenuBlend {
    fn enable_oit() -> bool {
        // The menu's source-order submission must retain its explicit blend.
        false
    }

    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        Self::specialize_descriptor(descriptor, key.bind_group_data);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        pbr::{Material, MeshPipelineKey},
        render::render_resource::{ColorTargetState, ColorWrites, FragmentState, TextureFormat},
        shader::ShaderRef,
    };

    fn base(alpha_mode: AlphaMode) -> StandardMaterial {
        StandardMaterial {
            alpha_mode,
            base_color: Color::srgba(0.13, 0.29, 0.47, 0.61),
            unlit: true,
            fog_enabled: false,
            cull_mode: None,
            uv_transform: bevy::math::Affine2::from_translation(Vec2::new(0.17, 0.43)),
            ..default()
        }
    }

    fn descriptor() -> RenderPipelineDescriptor {
        RenderPipelineDescriptor {
            label: Some("synthetic menu pipeline".into()),
            fragment: Some(FragmentState {
                targets: vec![Some(ColorTargetState {
                    format: TextureFormat::Rgba16Float,
                    blend: Some(BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: ColorWrites::RED | ColorWrites::ALPHA,
                })],
                ..default()
            }),
            ..default()
        }
    }

    #[test]
    fn source_mode_atomically_selects_key_without_changing_source_material_values() {
        let original = base(AlphaMode::Add);
        let source_color = original.base_color;
        let source_uv = original.uv_transform;
        let mode3 = checked_source_mode(original, 3).unwrap();
        assert_eq!(mode3.base.alpha_mode, AlphaMode::Premultiplied);
        assert!(MenuBlendKey::from(&mode3.extension).one_one);
        assert_eq!(mode3.base.base_color, source_color);
        assert_eq!(mode3.base.uv_transform, source_uv);
        assert!(mode3.base.unlit);
        assert!(!mode3.base.fog_enabled);
        assert_eq!(mode3.base.cull_mode, None);
        let mode4 = checked_source_mode(base(AlphaMode::Add), 4).unwrap();
        assert_eq!(mode4.base.alpha_mode, AlphaMode::Add);
        assert_ne!(
            MenuBlendKey::from(&mode3.extension),
            MenuBlendKey::from(&mode4.extension)
        );
        assert_eq!(Material::alpha_mode(&mode3), AlphaMode::Premultiplied);
        assert_eq!(Material::alpha_mode(&mode4), AlphaMode::Add);
        assert!(!MenuBlend::enable_oit());
        assert!(matches!(MenuBlend::fragment_shader(), ShaderRef::Default));
        assert!(matches!(MenuBlend::vertex_shader(), ShaderRef::Default));
    }

    #[test]
    fn known_modes_preserve_the_existing_mapping_and_unknown_or_inconsistent_modes_refuse() {
        for (mode, alpha) in [
            (0, AlphaMode::Opaque),
            (1, AlphaMode::Mask(0.37)),
            (2, AlphaMode::Blend),
            (4, AlphaMode::Add),
            (5, AlphaMode::Multiply),
            (6, AlphaMode::Opaque),
        ] {
            let material = checked_source_mode(base(alpha), mode).unwrap();
            assert_eq!(material.base.alpha_mode, alpha);
            assert!(!MenuBlendKey::from(&material.extension).one_one);
        }
        for mode in [7, u32::MAX] {
            assert_eq!(
                checked_source_mode(base(AlphaMode::Opaque), mode).unwrap_err(),
                SourceModeError::UnknownFilterMode(mode)
            );
        }
        for mode in 0..=6 {
            assert_eq!(
                checked_source_mode(base(AlphaMode::Premultiplied), mode).unwrap_err(),
                SourceModeError::InconsistentAlphaMode(mode)
            );
        }
        for cutoff in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
            assert_eq!(
                checked_source_mode(base(AlphaMode::Mask(cutoff)), 1).unwrap_err(),
                SourceModeError::InconsistentAlphaMode(1)
            );
        }
    }

    #[test]
    fn hook_payload_and_actual_key_change_only_mode3_blend() {
        let material = checked_source_mode(base(AlphaMode::Add), 3).unwrap();
        let key = MaterialExtensionKey::<MenuBlend> {
            mesh_key: MeshPipelineKey::BLEND_PREMULTIPLIED_ALPHA,
            bind_group_data: MenuBlendKey::from(&material.extension),
        };
        let mut actual = descriptor();
        let mut expected = actual.clone();
        expected.fragment.as_mut().unwrap().targets[0]
            .as_mut()
            .unwrap()
            .blend = Some(BlendState {
            color: BlendComponent {
                src_factor: BlendFactor::One,
                dst_factor: BlendFactor::One,
                operation: BlendOperation::Add,
            },
            alpha: BlendComponent {
                src_factor: BlendFactor::Zero,
                dst_factor: BlendFactor::One,
                operation: BlendOperation::Add,
            },
        });
        MenuBlend::specialize_descriptor(&mut actual, key.bind_group_data);
        assert_eq!(actual, expected);
        let mode4 = checked_source_mode(base(AlphaMode::Add), 4).unwrap();
        let mut unchanged = descriptor();
        let before = unchanged.clone();
        MenuBlend::specialize_descriptor(&mut unchanged, MenuBlendKey::from(&mode4.extension));
        assert_eq!(unchanged, before);
        // The real trait entry point delegates exactly this descriptor/key
        // operation. Executing it also needs Bevy's device-owned MeshPipeline.
        let _hook = <MenuBlend as MaterialExtension>::specialize;
    }

    #[test]
    #[should_panic(expected = "color target zero")]
    fn mode3_does_not_silently_succeed_without_the_standard_forward_target() {
        let material = checked_source_mode(base(AlphaMode::Add), 3).unwrap();
        let mut missing = descriptor();
        missing.fragment.as_mut().unwrap().targets.clear();
        MenuBlend::specialize_descriptor(&mut missing, MenuBlendKey::from(&material.extension));
    }
}
