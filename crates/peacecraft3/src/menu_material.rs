//! Source-selected menu geometry blending; PRE2 and UI images use other paths.
use bevy::{
    mesh::MeshVertexBufferLayoutRef,
    pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline},
    prelude::*,
    render::render_resource::{
        AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, CompareFunction,
        RenderPipelineDescriptor, SpecializedMeshPipelineError,
    },
};
use std::fmt;

pub(crate) type MenuMaterial = ExtendedMaterial<StandardMaterial, MenuBlend>;

/// Shading bits read by menu specialization.
///
/// `0x40` is the pinned viewer's disabled depth test. Khronos `glDepthMask`
/// leaves the depth buffer unchanged in that state even when the mask is true,
/// so the bit forces the write off as well as `Always`. `0x80` forces the write
/// off and leaves the compare. Original Warcraft III execution of the bits
/// remains unobserved. Every other shading bit stays with the caller.
pub(crate) const NO_DEPTH_TEST: u32 = 0x40;
pub(crate) const NO_DEPTH_SET: u32 = 0x80;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
#[bind_group_data(MenuBlendKey)]
pub(crate) struct MenuBlend {
    one_one: bool,
    no_depth_test: bool,
    no_depth_set: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct MenuBlendKey {
    one_one: bool,
    no_depth_test: bool,
    no_depth_set: bool,
}

impl From<&MenuBlend> for MenuBlendKey {
    fn from(value: &MenuBlend) -> Self {
        Self {
            one_one: value.one_one,
            no_depth_test: value.no_depth_test,
            no_depth_set: value.no_depth_set,
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
/// The caller supplies the checked source filter word and that layer's shading
/// word. Bits other than [`NO_DEPTH_TEST`] and [`NO_DEPTH_SET`] stay with the
/// caller. Mode 1 retains the caller's cutoff. Mode 6 retains its opaque alpha
/// mode; a set depth bit still forces that depth write off.
pub(crate) fn checked_source_mode(
    mut base: StandardMaterial,
    filter_mode: u32,
    shading_flags: u32,
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
        extension: MenuBlend {
            one_one,
            no_depth_test: shading_flags & NO_DEPTH_TEST != 0,
            no_depth_set: shading_flags & NO_DEPTH_SET != 0,
        },
    })
}

impl MenuBlend {
    fn specialize_descriptor(descriptor: &mut RenderPipelineDescriptor, key: MenuBlendKey) {
        // Either depth bit forces the write off. `0x40` also selects Always.
        // Clear bits leave the stored compare and the blend-coupled write,
        // including mode 6. wgpu `Always` still updates depth when
        // `depth_write_enabled` is true, unlike a disabled OpenGL depth test.
        if key.no_depth_test || key.no_depth_set {
            let depth = descriptor
                .depth_stencil
                .as_mut()
                .expect("StandardMaterial forward pipeline has a depth state");
            if key.no_depth_test {
                depth.depth_compare = Some(CompareFunction::Always);
            }
            depth.depth_write_enabled = Some(false);
        }
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
        render::render_resource::{
            ColorTargetState, ColorWrites, DepthBiasState, DepthStencilState, Face, FragmentState,
            StencilFaceState, StencilState, TextureFormat,
        },
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

    fn depth_state(write: bool) -> DepthStencilState {
        DepthStencilState {
            format: TextureFormat::Depth32Float,
            depth_write_enabled: Some(write),
            depth_compare: Some(CompareFunction::GreaterEqual),
            stencil: StencilState {
                front: StencilFaceState::IGNORE,
                back: StencilFaceState::IGNORE,
                read_mask: 0x11,
                write_mask: 0x22,
            },
            bias: DepthBiasState {
                constant: 7,
                slope_scale: 0.5,
                clamp: 2.0,
            },
        }
    }

    fn forward(blend: Option<BlendState>, write: bool) -> RenderPipelineDescriptor {
        let mut descriptor = descriptor();
        descriptor.fragment.as_mut().unwrap().targets[0]
            .as_mut()
            .unwrap()
            .blend = blend;
        descriptor.depth_stencil = Some(depth_state(write));
        descriptor.primitive.cull_mode = Some(Face::Back);
        descriptor.multisample.alpha_to_coverage_enabled = true;
        descriptor
    }

    fn one_one_blend() -> BlendState {
        BlendState {
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
        }
    }

    #[test]
    fn source_mode_atomically_selects_key_without_changing_source_material_values() {
        let original = base(AlphaMode::Add);
        let source_color = original.base_color;
        let source_uv = original.uv_transform;
        let mode3 = checked_source_mode(original, 3, 0).unwrap();
        assert_eq!(mode3.base.alpha_mode, AlphaMode::Premultiplied);
        let mode3_key = MenuBlendKey::from(&mode3.extension);
        assert!(mode3_key.one_one);
        assert!(!mode3_key.no_depth_test);
        assert!(!mode3_key.no_depth_set);
        assert_eq!(mode3.base.base_color, source_color);
        assert_eq!(mode3.base.uv_transform, source_uv);
        assert!(mode3.base.unlit);
        assert!(!mode3.base.fog_enabled);
        assert_eq!(mode3.base.cull_mode, None);
        let mode4 = checked_source_mode(base(AlphaMode::Add), 4, 0).unwrap();
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
            let material = checked_source_mode(base(alpha), mode, 0).unwrap();
            assert_eq!(material.base.alpha_mode, alpha);
            let key = MenuBlendKey::from(&material.extension);
            assert!(!key.one_one);
            assert!(!key.no_depth_test);
            assert!(!key.no_depth_set);
        }
        for mode in [7, u32::MAX] {
            assert_eq!(
                checked_source_mode(base(AlphaMode::Opaque), mode, 0).unwrap_err(),
                SourceModeError::UnknownFilterMode(mode)
            );
        }
        for mode in 0..=6 {
            assert_eq!(
                checked_source_mode(base(AlphaMode::Premultiplied), mode, 0xC1).unwrap_err(),
                SourceModeError::InconsistentAlphaMode(mode)
            );
        }
        for cutoff in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
            assert_eq!(
                checked_source_mode(base(AlphaMode::Mask(cutoff)), 1, 0x40).unwrap_err(),
                SourceModeError::InconsistentAlphaMode(1)
            );
        }
    }

    #[test]
    fn hook_payload_and_actual_key_change_only_mode3_blend() {
        let material = checked_source_mode(base(AlphaMode::Add), 3, 0).unwrap();
        let key = MaterialExtensionKey::<MenuBlend> {
            mesh_key: MeshPipelineKey::BLEND_PREMULTIPLIED_ALPHA,
            bind_group_data: MenuBlendKey::from(&material.extension),
        };
        let mut actual = descriptor();
        let mut expected = actual.clone();
        expected.fragment.as_mut().unwrap().targets[0]
            .as_mut()
            .unwrap()
            .blend = Some(one_one_blend());
        MenuBlend::specialize_descriptor(&mut actual, key.bind_group_data);
        assert_eq!(actual, expected);
        let mode4 = checked_source_mode(base(AlphaMode::Add), 4, 0).unwrap();
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
        let material = checked_source_mode(base(AlphaMode::Add), 3, 0).unwrap();
        let mut missing = descriptor();
        missing.fragment.as_mut().unwrap().targets.clear();
        MenuBlend::specialize_descriptor(&mut missing, MenuBlendKey::from(&material.extension));
    }

    #[test]
    fn depth_flags_specialize_compare_and_write_without_other_pipeline_state() {
        let cases = [
            (0, AlphaMode::Opaque, true, None),
            (1, AlphaMode::Mask(0.75), true, None),
            (2, AlphaMode::Blend, false, Some(BlendState::ALPHA_BLENDING)),
            (
                3,
                AlphaMode::Add,
                false,
                Some(BlendState::PREMULTIPLIED_ALPHA_BLENDING),
            ),
            (4, AlphaMode::Add, false, Some(BlendState::ADDITIVE)),
            (6, AlphaMode::Opaque, true, None),
        ];
        for (mode, alpha, write, blend) in cases {
            for flags in [0, 0x40, 0x80, 0xC0, 0xC1] {
                let source = base(alpha);
                let material = checked_source_mode(source.clone(), mode, flags).unwrap();
                let key = MenuBlendKey::from(&material.extension);
                assert_eq!(key.one_one, mode == 3);
                assert_eq!(key.no_depth_test, flags & NO_DEPTH_TEST != 0);
                assert_eq!(key.no_depth_set, flags & NO_DEPTH_SET != 0);
                assert_eq!(material.base.base_color, source.base_color);
                assert_eq!(material.base.uv_transform, source.uv_transform);
                assert_eq!(material.base.unlit, source.unlit);
                assert_eq!(material.base.fog_enabled, source.fog_enabled);
                assert_eq!(material.base.cull_mode, source.cull_mode);
                let expected_alpha = if mode == 3 {
                    AlphaMode::Premultiplied
                } else {
                    alpha
                };
                assert_eq!(material.base.alpha_mode, expected_alpha);

                let mut actual = forward(blend, write);
                let depth_before = actual.depth_stencil.clone().unwrap();
                let blend_before = actual.fragment.as_ref().unwrap().targets[0]
                    .as_ref()
                    .unwrap()
                    .blend;
                MenuBlend::specialize_descriptor(&mut actual, key);
                let depth = actual.depth_stencil.clone().unwrap();
                let expected_compare = if flags & NO_DEPTH_TEST != 0 {
                    CompareFunction::Always
                } else {
                    CompareFunction::GreaterEqual
                };
                // Either depth bit forces the write off, for an opaque starting
                // write and for a blend layer whose write is already off.
                let expected_write = flags & (NO_DEPTH_TEST | NO_DEPTH_SET) == 0 && write;
                assert_eq!(
                    depth.depth_compare,
                    Some(expected_compare),
                    "mode={mode} flags={flags:#x}"
                );
                assert_eq!(
                    depth.depth_write_enabled,
                    Some(expected_write),
                    "mode={mode} flags={flags:#x}"
                );
                assert_eq!(depth.format, depth_before.format);
                assert_eq!(depth.stencil, depth_before.stencil);
                assert_eq!(depth.bias, depth_before.bias);
                let color = actual.fragment.as_ref().unwrap().targets[0]
                    .as_ref()
                    .unwrap();
                let expected_blend = if mode == 3 {
                    Some(one_one_blend())
                } else {
                    blend_before
                };
                assert_eq!(color.blend, expected_blend);
                assert_eq!(color.write_mask, ColorWrites::RED | ColorWrites::ALPHA);
                assert_eq!(actual.primitive.cull_mode, Some(Face::Back));
                assert!(actual.multisample.alpha_to_coverage_enabled);
                assert_eq!(
                    actual.label.as_ref().unwrap().as_ref(),
                    "synthetic menu pipeline"
                );
            }
        }

        let mut plain = base(AlphaMode::Opaque);
        plain.unlit = false;
        plain.fog_enabled = true;
        let kept = checked_source_mode(plain.clone(), 0, 0xC1).unwrap();
        assert!(!kept.base.unlit);
        assert!(kept.base.fog_enabled);
        assert_eq!(kept.base.base_color, plain.base_color);
        assert_eq!(kept.base.alpha_mode, AlphaMode::Opaque);
        let kept_key = MenuBlendKey::from(&kept.extension);
        let combined = checked_source_mode(base(AlphaMode::Opaque), 0, 0xC0).unwrap();
        assert_eq!(kept_key, MenuBlendKey::from(&combined.extension));
        let unrelated = checked_source_mode(base(AlphaMode::Blend), 2, 0x21).unwrap();
        let clear = checked_source_mode(base(AlphaMode::Blend), 2, 0).unwrap();
        assert_eq!(
            MenuBlendKey::from(&unrelated.extension),
            MenuBlendKey::from(&clear.extension)
        );
        let mode3 = checked_source_mode(base(AlphaMode::Add), 3, 0xC0).unwrap();
        assert_ne!(
            MenuBlendKey::from(&mode3.extension),
            MenuBlendKey::from(&combined.extension)
        );

        let clear_depth = checked_source_mode(base(AlphaMode::Opaque), 0, 0).unwrap();
        let mut untouched = forward(None, true);
        untouched.depth_stencil.as_mut().unwrap().depth_compare = Some(CompareFunction::Less);
        let before = untouched.clone();
        MenuBlend::specialize_descriptor(
            &mut untouched,
            MenuBlendKey::from(&clear_depth.extension),
        );
        assert_eq!(untouched, before);

        let write_only = checked_source_mode(base(AlphaMode::Opaque), 0, NO_DEPTH_SET).unwrap();
        let mut retained = forward(None, true);
        retained.depth_stencil.as_mut().unwrap().depth_compare = Some(CompareFunction::Less);
        MenuBlend::specialize_descriptor(&mut retained, MenuBlendKey::from(&write_only.extension));
        let retained_depth = retained.depth_stencil.unwrap();
        assert_eq!(retained_depth.depth_compare, Some(CompareFunction::Less));
        assert_eq!(retained_depth.depth_write_enabled, Some(false));
        assert_eq!(retained_depth.stencil.read_mask, 0x11);
        assert_eq!(retained_depth.bias.constant, 7);
    }

    #[test]
    fn disabled_depth_test_forces_the_inherited_write_off() {
        let opaque = checked_source_mode(base(AlphaMode::Opaque), 0, NO_DEPTH_TEST).unwrap();
        let mut pipeline = forward(None, true);
        MenuBlend::specialize_descriptor(&mut pipeline, MenuBlendKey::from(&opaque.extension));
        let depth = pipeline.depth_stencil.unwrap();
        assert_eq!(depth.depth_compare, Some(CompareFunction::Always));
        assert_eq!(depth.depth_write_enabled, Some(false));

        let blend = checked_source_mode(base(AlphaMode::Blend), 2, NO_DEPTH_TEST).unwrap();
        let mut pipeline = forward(Some(BlendState::ALPHA_BLENDING), false);
        MenuBlend::specialize_descriptor(&mut pipeline, MenuBlendKey::from(&blend.extension));
        let depth = pipeline.depth_stencil.as_ref().unwrap();
        assert_eq!(depth.depth_compare, Some(CompareFunction::Always));
        assert_eq!(depth.depth_write_enabled, Some(false));
        assert_eq!(
            pipeline.fragment.as_ref().unwrap().targets[0]
                .as_ref()
                .unwrap()
                .blend,
            Some(BlendState::ALPHA_BLENDING)
        );

        let write_only = checked_source_mode(base(AlphaMode::Opaque), 0, NO_DEPTH_SET).unwrap();
        let both =
            checked_source_mode(base(AlphaMode::Opaque), 0, NO_DEPTH_TEST | NO_DEPTH_SET).unwrap();
        assert_ne!(
            MenuBlendKey::from(&opaque.extension),
            MenuBlendKey::from(&write_only.extension)
        );
        assert_ne!(
            MenuBlendKey::from(&opaque.extension),
            MenuBlendKey::from(&both.extension)
        );
        let mut pipeline = forward(None, true);
        MenuBlend::specialize_descriptor(&mut pipeline, MenuBlendKey::from(&both.extension));
        let depth = pipeline.depth_stencil.unwrap();
        assert_eq!(depth.depth_compare, Some(CompareFunction::Always));
        assert_eq!(depth.depth_write_enabled, Some(false));
    }

    #[test]
    fn depth_flags_do_not_require_a_color_target() {
        let material = checked_source_mode(base(AlphaMode::Opaque), 0, 0xC0).unwrap();
        let mut pipeline = descriptor();
        pipeline.fragment = None;
        pipeline.depth_stencil = Some(depth_state(true));
        MenuBlend::specialize_descriptor(&mut pipeline, MenuBlendKey::from(&material.extension));
        let depth = pipeline.depth_stencil.unwrap();
        assert_eq!(depth.depth_compare, Some(CompareFunction::Always));
        assert_eq!(depth.depth_write_enabled, Some(false));
        assert!(pipeline.fragment.is_none());
    }

    #[test]
    #[should_panic(expected = "color target zero")]
    fn mode3_depth_flag_still_requires_the_color_target() {
        let material = checked_source_mode(base(AlphaMode::Add), 3, 0x40).unwrap();
        let mut missing = descriptor();
        missing.fragment.as_mut().unwrap().targets.clear();
        missing.depth_stencil = Some(depth_state(false));
        MenuBlend::specialize_descriptor(&mut missing, MenuBlendKey::from(&material.extension));
    }

    #[test]
    #[should_panic(expected = "depth state")]
    fn no_depth_test_refuses_a_pipeline_without_depth() {
        let material = checked_source_mode(base(AlphaMode::Opaque), 0, NO_DEPTH_TEST).unwrap();
        MenuBlend::specialize_descriptor(
            &mut descriptor(),
            MenuBlendKey::from(&material.extension),
        );
    }

    #[test]
    #[should_panic(expected = "depth state")]
    fn no_depth_set_refuses_a_pipeline_without_depth() {
        let material = checked_source_mode(base(AlphaMode::Blend), 2, NO_DEPTH_SET).unwrap();
        MenuBlend::specialize_descriptor(
            &mut descriptor(),
            MenuBlendKey::from(&material.extension),
        );
    }
}
