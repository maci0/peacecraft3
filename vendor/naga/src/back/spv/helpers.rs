use alloc::{vec, vec::Vec};

use spirv::Word;

use crate::{Handle, UniqueArena};

#[test]
fn byte_chunks_preserve_utf8() {
    for input in ["", "abcdefghi", "aé日🦀z", "🦀🦀🦀"] {
        let chunks = string_to_byte_chunks(input, 4);
        assert_eq!(chunks.concat(), input.as_bytes());
        for chunk in chunks {
            assert!(!chunk.is_empty());
            assert!(chunk.len() <= 4);
            assert!(core::str::from_utf8(chunk).is_ok());
        }
    }
}

pub(super) fn bytes_to_words(bytes: &[u8]) -> Vec<Word> {
    bytes
        .chunks(4)
        .map(|chars| chars.iter().rev().fold(0u32, |u, c| (u << 8) | *c as u32))
        .collect()
}

pub(super) fn string_to_words(input: &str) -> Vec<Word> {
    let bytes = input.as_bytes();

    str_bytes_to_words(bytes)
}

pub(super) fn str_bytes_to_words(bytes: &[u8]) -> Vec<Word> {
    let mut words = bytes_to_words(bytes);
    if bytes.len() % 4 == 0 {
        // nul-termination
        words.push(0x0u32);
    }

    words
}

/// split a string into chunks and keep utf8 valid
pub(super) fn string_to_byte_chunks(input: &str, limit: usize) -> Vec<&[u8]> {
    let mut offset: usize = 0;
    let mut start: usize = 0;
    let mut words = vec![];
    while offset < input.len() {
        offset = input.floor_char_boundary(offset + limit);
        words.push(input[start..offset].as_bytes());
        start = offset;
    }

    words
}

pub(super) const fn map_storage_class(space: crate::AddressSpace) -> spirv::StorageClass {
    match space {
        crate::AddressSpace::Handle => spirv::StorageClass::UniformConstant,
        crate::AddressSpace::Function => spirv::StorageClass::Function,
        crate::AddressSpace::Private => spirv::StorageClass::Private,
        crate::AddressSpace::Storage { .. } => spirv::StorageClass::StorageBuffer,
        crate::AddressSpace::Uniform => spirv::StorageClass::Uniform,
        crate::AddressSpace::WorkGroup => spirv::StorageClass::Workgroup,
        crate::AddressSpace::PushConstant => spirv::StorageClass::PushConstant,
    }
}

pub(super) fn contains_builtin(
    binding: Option<&crate::Binding>,
    ty: Handle<crate::Type>,
    arena: &UniqueArena<crate::Type>,
    built_in: crate::BuiltIn,
) -> bool {
    if let Some(&crate::Binding::BuiltIn(bi)) = binding {
        bi == built_in
    } else if let crate::TypeInner::Struct { ref members, .. } = arena[ty].inner {
        members
            .iter()
            .any(|member| contains_builtin(member.binding.as_ref(), member.ty, arena, built_in))
    } else {
        false // unreachable
    }
}

impl crate::AddressSpace {
    pub(super) const fn to_spirv_semantics_and_scope(
        self,
    ) -> (spirv::MemorySemantics, spirv::Scope) {
        // Relaxed atomics must not carry a storage-class bit. Vulkan's
        // VUID-StandaloneSpirv-MemorySemantics-10871 rejects
        // UniformMemory/WorkgroupMemory combined with a relaxed order, which
        // is what `OpAtomicIAdd %uint_64` was. Barriers still OR in
        // AcquireRelease plus the class bit. Same change as wgpu#8391.
        match self {
            Self::Storage { .. } => (spirv::MemorySemantics::empty(), spirv::Scope::Device),
            Self::WorkGroup => (spirv::MemorySemantics::empty(), spirv::Scope::Workgroup),
            Self::Uniform => (spirv::MemorySemantics::empty(), spirv::Scope::Device),
            Self::Handle => (spirv::MemorySemantics::empty(), spirv::Scope::Device),
            _ => (spirv::MemorySemantics::empty(), spirv::Scope::Invocation),
        }
    }
}

/// Return true if the global requires a type decorated with `Block`.
///
/// See [`back::spv::GlobalVariable`] for details.
///
/// [`back::spv::GlobalVariable`]: super::GlobalVariable
pub fn global_needs_wrapper(ir_module: &crate::Module, var: &crate::GlobalVariable) -> bool {
    match var.space {
        crate::AddressSpace::Uniform
        | crate::AddressSpace::Storage { .. }
        | crate::AddressSpace::PushConstant => {}
        _ => return false,
    };
    match ir_module.types[var.ty].inner {
        crate::TypeInner::Struct {
            ref members,
            span: _,
        } => match members.last() {
            Some(member) => match ir_module.types[member.ty].inner {
                // Structs with dynamically sized arrays can't be copied and can't be wrapped.
                crate::TypeInner::Array {
                    size: crate::ArraySize::Dynamic,
                    ..
                } => false,
                _ => true,
            },
            None => false,
        },
        crate::TypeInner::BindingArray { .. } => false,
        // if it's not a structure or a binding array, let's wrap it to be able to put "Block"
        _ => true,
    }
}
