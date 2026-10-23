//! Shared checked instruction shape for owned AI-context boolean setters.
#[derive(Debug, thiserror::Error)]
#[error("AI context flag: {0}")]
pub(crate) struct Error(pub(crate) &'static str);

fn at(bytes: &[u8], offset: usize, expected: &[u8]) -> Result<(), Error> {
    (bytes.get(offset..offset + expected.len()) == Some(expected))
        .then_some(())
        .ok_or(Error("reviewed instruction relationship changed"))
}

fn word(bytes: &[u8], offset: usize) -> Result<u32, Error> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .ok_or(Error("truncated instruction operand"))?
            .try_into()
            .map_err(|_| Error("truncated instruction operand"))?,
    ))
}

fn call(bytes: &[u8], va: u32, offset: usize) -> Result<u32, Error> {
    at(bytes, offset, &[0xe8])?;
    Ok(va
        .wrapping_add(offset as u32 + 5)
        .wrapping_add(word(bytes, offset + 1)?))
}

fn jmp(bytes: &[u8], va: u32, offset: usize) -> Result<u32, Error> {
    at(bytes, offset, &[0xe9])?;
    Ok(va
        .wrapping_add(offset as u32 + 5)
        .wrapping_add(word(bytes, offset + 1)?))
}

/// Decode a shared flag-setter handler (control chain, resolver, set
/// and clear operands, both returns), returning the field offset and bit mask.
pub(crate) fn decode(bytes: &[u8], va: u32) -> Result<(u32, u32), Error> {
    let prefix = bytes
        .get(..71)
        .ok_or(Error("truncated flag setter window"))?;
    let wide = match prefix[53] {
        0x83 => false,
        0x81 => true,
        _ => return Err(Error("flag setter immediate opcode changed")),
    };
    let extra = if wide { 3 } else { 0 };
    let bytes = bytes
        .get(..71 + extra * 2)
        .ok_or(Error("truncated flag setter window"))?;
    at(bytes, 0, &[0x55, 0x8b, 0xec])?;
    at(bytes, 3, &[0xa1])?;
    if word(bytes, 4)? != 0x11b4a10 {
        return Err(Error("flag setter AI context global changed"));
    }
    at(bytes, 8, &[0x85, 0xc0])?;
    at(bytes, 10, &[0x75])?;
    if 12 + i32::from(bytes[11] as i8) != 25 {
        return Err(Error(
            "flag setter null skip does not reach the resolver path",
        ));
    }
    at(bytes, 12, &[0xc7, 0x45, 0x08, 0x57, 0, 0, 0])?;
    at(bytes, 19, &[0x5d])?;
    if jmp(bytes, va, 20)? != 0x43eca0 {
        return Err(Error("flag setter error sink changed"));
    }
    at(bytes, 25, &[0x8d, 0x48, 0x2c])?;
    at(bytes, 28, &[0x8b, 0x41, 0x04])?;
    at(bytes, 31, &[0x23, 0x01])?;
    at(bytes, 33, &[0x83, 0xf8, 0xff])?;
    at(bytes, 36, &[0x75])?;
    if 38 + i32::from(bytes[37] as i8) != 42 {
        return Err(Error(
            "flag setter sentinel branch does not reach the resolver",
        ));
    }
    at(bytes, 38, &[0x33, 0xc0])?;
    at(bytes, 40, &[0xeb])?;
    if 42 + i32::from(bytes[41] as i8) != 47 {
        return Err(Error(
            "flag setter sentinel default does not reach the boolean branch",
        ));
    }
    if call(bytes, va, 42)? != 0x7fc7e0 {
        return Err(Error("flag setter context resolver changed"));
    }
    at(bytes, 47, &[0x83, 0x7d, 0x08, 0x00])?;
    at(bytes, 51, &[0x74])?;
    if 53 + i32::from(bytes[52] as i8) != (62 + extra) as i32 {
        return Err(Error(
            "flag setter boolean branch does not skip to the clear path",
        ));
    }
    let opcode = if wide { 0x81 } else { 0x83 };
    at(bytes, 53, &[opcode, 0x88])?;
    let context_offset = word(bytes, 55)?;
    let mask = if wide {
        word(bytes, 59)?
    } else {
        if bytes[59] & 0x80 != 0 {
            return Err(Error("flag setter set mask is a negative imm8"));
        }
        u32::from(bytes[59])
    };
    if mask.count_ones() != 1 {
        return Err(Error("flag setter mask is not a single bit"));
    }
    at(bytes, 60 + extra, &[0x5d, 0xc3])?;
    at(bytes, 62 + extra, &[opcode, 0xa0])?;
    if word(bytes, 64 + extra)? != context_offset {
        return Err(Error("flag setter clear field differs from set"));
    }
    let clear = if wide {
        word(bytes, 68 + extra)?
    } else {
        i32::from(bytes[68] as i8) as u32
    };
    if clear != !mask {
        return Err(Error(
            "flag setter clear mask is not the complement of the set mask",
        ));
    }
    at(bytes, 69 + extra * 2, &[0x5d, 0xc3])?;
    Ok((context_offset, mask))
}
