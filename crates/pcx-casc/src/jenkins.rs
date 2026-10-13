// lookup3's two-output little-endian hash. Algorithm constants are format
// invariants; index dimensions and both rolling seeds come from the records.
pub(crate) fn hash(data: &[u8], primary: u32, secondary: u32) -> (u32, u32) {
    let seed = 0xdead_beefu32
        .wrapping_add(data.len() as u32)
        .wrapping_add(primary);
    let (mut a, mut b, mut c) = (seed, seed, seed.wrapping_add(secondary));
    let mut remaining = data;
    while remaining.len() > 12 {
        a = a.wrapping_add(u32::from_le_bytes(remaining[..4].try_into().unwrap()));
        b = b.wrapping_add(u32::from_le_bytes(remaining[4..8].try_into().unwrap()));
        c = c.wrapping_add(u32::from_le_bytes(remaining[8..12].try_into().unwrap()));
        a = a.wrapping_sub(c);
        a ^= c.rotate_left(4);
        c = c.wrapping_add(b);
        b = b.wrapping_sub(a);
        b ^= a.rotate_left(6);
        a = a.wrapping_add(c);
        c = c.wrapping_sub(b);
        c ^= b.rotate_left(8);
        b = b.wrapping_add(a);
        a = a.wrapping_sub(c);
        a ^= c.rotate_left(16);
        c = c.wrapping_add(b);
        b = b.wrapping_sub(a);
        b ^= a.rotate_left(19);
        a = a.wrapping_add(c);
        c = c.wrapping_sub(b);
        c ^= b.rotate_left(4);
        b = b.wrapping_add(a);
        remaining = &remaining[12..];
    }
    if remaining.is_empty() {
        return (c, b);
    }
    for (i, &value) in remaining.iter().enumerate() {
        let shift = (i % 4) * 8;
        match i / 4 {
            0 => a = a.wrapping_add(u32::from(value) << shift),
            1 => b = b.wrapping_add(u32::from(value) << shift),
            _ => c = c.wrapping_add(u32::from(value) << shift),
        }
    }
    c ^= b;
    c = c.wrapping_sub(b.rotate_left(14));
    a ^= c;
    a = a.wrapping_sub(c.rotate_left(11));
    b ^= a;
    b = b.wrapping_sub(a.rotate_left(25));
    c ^= b;
    c = c.wrapping_sub(b.rotate_left(16));
    a ^= c;
    a = a.wrapping_sub(c.rotate_left(4));
    b ^= a;
    b = b.wrapping_sub(a.rotate_left(14));
    c ^= b;
    c = c.wrapping_sub(b.rotate_left(24));
    (c, b)
}
