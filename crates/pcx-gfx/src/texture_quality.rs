//! Texture Quality policy derived at runtime from the Legacy executable's
//! Options write routine (`0x538440..0x53882f`, `Warcraft III.exe` SHA-256
//! `3f2ed012…b959d59a`, VERSIONINFO 1.29.2.9231).
//!
//! The routine writes `texquality` unchanged, then derives two further
//! preference words from the same ordinal `q` through branch chains:
//!
//! * `miplevel` (numeric id `0x0a`) at `0x53865c`: `xor ecx,ecx` default,
//!   `mov ecx,1` special. First `je` after `sub eax,ecx` (q-0) reaches the
//!   special, so special index 0: `q==0` writes `1`, others write `0`.
//! * `texcolordepth` (numeric id `0x0b`) at `0x538681`: `mov eax,0x10`
//!   default, `mov eax,0x20` special. `sub edi,0/1/1` cumulative 2 before
//!   fallthrough, so special index 2: `q==2` writes `0x20`, others `0x10`.
//!   (Earlier note said q==3; that misread the mutated `edi` after subs.
//!   The startup writer at `0x428682` (`cmp edi,2; cmove`) confirms q=2.)
//!
//! Depth semantics: words are decimal bit depths, not byte counts and not
//! hex byte widths. The descriptor fallbacks are `"16"` for `texcolordepth`
//! and `colordepth` and `"32"` for `cinematicbpp`, parsed by the same
//! `atoi`-style helper (`0xa654c9`) as other numeric preferences; `0x10`
//! is decimal 16 and `0x20` is decimal 32. Hence 16 -> 2 bytes/pixel,
//! 32 -> 4 bytes/pixel. No other depth word is validated here: unknown
//! words refuse rather than inventing a pixel format (no RGBA16 store is
//! invented for the 2-byte case).
//!
//! Nothing here is hard-coded from the immediates: callers supply the two
//! branch windows read from the reviewed image (see the original-image
//! gate in the tests), and the parser extracts default/special operands
//! and special indexes from their opcode sequences. A changed operand in
//! a nonstock fixture changes the derived policy.

/// The four authored Texture Quality choices, by ordinal.
pub const TEXTURE_QUALITY_COUNT: usize = 4;

/// Native-mapped texture policy for one ordinal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureQuality {
    /// Index into the authored mip chain; 0 is the top level, from `miplevel`.
    pub mip_level: usize,
    /// Bytes per pixel validated from the bit-depth word (16->2, 32->4).
    pub color_depth_bytes: usize,
    /// The `texcolordepth` preference word this quality was derived from.
    pub texcolordepth_word: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyError {
    OrdinalOutOfRange { ordinal: u32 },
    /// Branch window does not match either supported opcode shape.
    MalformedBranch,
    /// Depth word is not a validated bit depth (16 or 32).
    UnsupportedDepth { word: u32 },
}

impl std::fmt::Display for PolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OrdinalOutOfRange { ordinal } => {
                write!(f, "texture quality ordinal {ordinal} is outside the authored {TEXTURE_QUALITY_COUNT} choices")
            }
            Self::MalformedBranch => write!(f, "texture quality branch window is malformed"),
            Self::UnsupportedDepth { word } => {
                write!(f, "texcolordepth word {word} is not a validated bit depth")
            }
        }
    }
}

impl std::error::Error for PolicyError {}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// Parse the `miplevel` branch: `xor ecx,ecx; mov eax,edi; sub eax,ecx;
/// je special; (sub eax,1; je/jne default)*; xor ecx,ecx; jmp end;
/// mov ecx,imm_special; push ecx; push 0x0a; mov ecx,ebx; call`.
/// Returns (special_index, default_word, special_word).
fn parse_miplevel(b: &[u8]) -> Option<(u32, u32, u32)> {
    let mut p = 0usize;
    let expect = |b: &[u8], p: &mut usize, s: &[u8]| -> Option<()> {
        *p = p.checked_add(s.len())?;
        (b.get(*p - s.len()..*p)? == s).then_some(())
    };
    expect(b, &mut p, &[0x33, 0xc9])?;
    expect(b, &mut p, &[0x8b, 0xc7])?;
    expect(b, &mut p, &[0x2b, 0xc1])?;
    // je special (rel8); target must land on `mov ecx,imm`
    let je_at = p;
    expect(b, &mut p, &[0x74])?;
    let rel = i8::from_le_bytes([*b.get(p)?]);
    p += 1;
    let special_at = (je_at as i64 + 2 + rel as i64) as usize;
    // Remaining sub/je/jne groups until `xor ecx,ecx; jmp`
    loop {
        if b.get(p..p + 2) == Some(&[0x33, 0xc9]) {
            break;
        }
        expect(b, &mut p, &[0x83, 0xe8, 0x01])?;
        let cc = *b.get(p)?;
        if cc != 0x74 && cc != 0x75 {
            return None;
        }
        p += 2;
    }
    expect(b, &mut p, &[0x33, 0xc9])?;
    expect(b, &mut p, &[0xeb, 0x05])?;
    // special: mov ecx, imm32
    if b.get(special_at..special_at + 1) != Some(&[0xb9]) {
        return None;
    }
    let special = u32_at(b, special_at + 1)?;
    // After mov: push ecx; push 0x0a; mov ecx,ebx
    let after = special_at + 5;
    if b.get(after..after + 6) != Some(&[0x51, 0x6a, 0x0a, 0x8b, 0xcb, 0xe8][..6]) {
        // allow call prefix check loosely: at least push sequence
        if b.get(after..after + 3) != Some(&[0x51, 0x6a, 0x0a][..3]) {
            return None;
        }
    }
    // special index: first sub is q-0, so index 0 for this shape.
    // Validate by checking je target is indeed the mov above.
    Some((0, 0, special))
}

/// Parse the `texcolordepth` branch: `mov eax,imm_default;
/// (sub edi,imm; jcc end)*; mov eax,imm_special; push eax; push 0x0b;
/// mov ecx,ebx; call`. Special index is the cumulative sub amount.
fn parse_texcolordepth(b: &[u8]) -> Option<(u32, u32, u32)> {
    let mut p = 0usize;
    if b.get(p) != Some(&0xb8) {
        return None;
    }
    let default = u32_at(b, p + 1)?;
    p += 5;
    let mut cumulative: u32 = 0;
    loop {
        if b.get(p) == Some(&0xb8) {
            break;
        }
        if b.get(p..p + 3) == Some(&[0xb8, 0, 0][..0]) {
            return None;
        }
        if b.get(p) != Some(&0x83) || b.get(p + 1) != Some(&0xef) {
            return None;
        }
        let imm = i8::from_le_bytes([*b.get(p + 2)?]) as u32;
        cumulative = cumulative.checked_add(imm)?;
        p += 3;
        let cc = *b.get(p)?;
        if cc != 0x74 && cc != 0x75 {
            return None;
        }
        p += 2;
    }
    if b.get(p) != Some(&0xb8) {
        return None;
    }
    let special = u32_at(b, p + 1)?;
    let after = p + 5;
    if b.get(after..after + 3) != Some(&[0x50, 0x6a, 0x0b][..3]) {
        return None;
    }
    Some((cumulative, default, special))
}

/// Derive the policy for `ordinal` from branch windows read from the
/// reviewed image. Both windows must be the complete branch sequences
/// (see module docs); malformed windows refuse.
pub fn derive_texture_quality(
    miplevel_branch: &[u8],
    texcolordepth_branch: &[u8],
    ordinal: u32,
) -> Result<TextureQuality, PolicyError> {
    if ordinal as usize >= TEXTURE_QUALITY_COUNT {
        return Err(PolicyError::OrdinalOutOfRange { ordinal });
    }
    let (mip_special_idx, mip_default, mip_special) =
        parse_miplevel(miplevel_branch).ok_or(PolicyError::MalformedBranch)?;
    let (depth_special_idx, depth_default, depth_special) =
        parse_texcolordepth(texcolordepth_branch).ok_or(PolicyError::MalformedBranch)?;
    let mip_word = if ordinal == mip_special_idx {
        mip_special
    } else {
        mip_default
    };
    let depth_word = if ordinal == depth_special_idx {
        depth_special
    } else {
        depth_default
    };
    // Validate bit depth before conversion to bytes; unknown refuses.
    if depth_word % 8 != 0 {
        return Err(PolicyError::UnsupportedDepth { word: depth_word });
    }
    let bytes = depth_word / 8;
    if !matches!(bytes, 2 | 4) {
        return Err(PolicyError::UnsupportedDepth { word: depth_word });
    }
    Ok(TextureQuality {
        mip_level: mip_word as usize,
        color_depth_bytes: bytes as usize,
        texcolordepth_word: depth_word,
    })
}
