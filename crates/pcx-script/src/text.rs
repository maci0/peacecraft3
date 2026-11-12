//! The text-resolution seam: `TRIGSTR_<n>`, `war3map.wts` and `^^` code tags.
//!
//! Three different things land here, and all three are the map's own text
//! rather than something this crate should own:
//!
//! - The editor writes player-visible trigger strings as the *string literal*
//!   `"TRIGSTR_374"` and as the *identifier* `TRIGSTR_374` once a trigger
//!   graph is desugared to JASS. Footman Frenzy ships 862 of the literal form
//!   and the corpus carries 34,634 of them, so both spellings have to resolve.
//! - The body for that id lives in the map's `war3map.wts`, which
//!   [`crate`] deliberately does not depend on: `pcx-trig` owns that file
//!   (`Wts::body(id)`). The caller supplies a table through
//!   [`crate::eval::Host::table_string`] and the VM asks it for a body.
//! - A paired `^^…^^` in a string is a code tag. What the engine substitutes
//!   for it is a frame-string question, so it goes through
//!   [`crate::eval::Host::code_tag`] and is refused when the host has no
//!   answer. Guessing here would silently corrupt every tooltip that uses one.

/// The id a `TRIGSTR_<n>` spelling refers to, or `None` if the text is not
/// that form. Both spellings resolve through this one check, so a string
/// literal and an identifier cannot disagree about what a trigger string is.
///
/// The match is exact and case-sensitive: `TRIGSTR_0` is id 0, `trigstr_0` is
/// an ordinary identifier.
pub fn trigstr_id(text: &str) -> Option<u32> {
    let digits = text.strip_prefix("TRIGSTR_")?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    // The editor numbers entries 0..9999 and the table is sparse (DotA v5.84b
    // has 5771 of the 10000 possible ids), so a wide parse is accepted here
    // and the table lookup is what decides whether the id exists.
    digits.parse::<u32>().ok()
}

/// Native trigger lookup differs from the editor's exact identifier syntax.
/// Selected 1.29.2.9231: 0x708dc0 -> 0x434390; -1 means no lookup.
pub(crate) fn native_trigstr_id(text: &str) -> Option<u32> {
    let suffix = text.strip_prefix("TRIGSTR_")?;
    let (negative, digits) = match suffix.strip_prefix('-') {
        Some(digits) => (true, digits),
        None => (false, suffix),
    };
    let magnitude = digits
        .bytes()
        .take_while(u8::is_ascii_digit)
        .fold(0_u32, |n, digit| {
            n.wrapping_mul(10).wrapping_add(u32::from(digit - b'0'))
        });
    let id = if negative {
        magnitude.wrapping_neg()
    } else {
        magnitude
    };
    (id != u32::MAX).then_some(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_trigger_ids_use_wrapping_prefix_integer_semantics() {
        for (text, expected) in [
            ("TRIGSTR_", Some(0)),
            ("TRIGSTR_-", Some(0)),
            ("TRIGSTR_0012suffix", Some(12)),
            ("TRIGSTR_-2", Some(u32::MAX - 1)),
            ("TRIGSTR_-4294967294", Some(2)),
            ("TRIGSTR_4294967296", Some(0)),
            ("TRIGSTR_+12", Some(0)),
            ("TRIGSTR_ 12", Some(0)),
            ("TRIGSTR_12\0ignored", Some(12)),
            ("TRIGSTR_-1", None),
            ("TRIGSTR_4294967295", None),
            ("trigstr_12", None),
            ("TRIGSTR", None),
        ] {
            assert_eq!(native_trigstr_id(text), expected, "{text:?}");
        }
    }

    #[test]
    fn trigstr_ids_parse_both_spellings() {
        assert_eq!(trigstr_id("TRIGSTR_000"), Some(0));
        assert_eq!(trigstr_id("TRIGSTR_902"), Some(902));
        assert_eq!(trigstr_id("TRIGSTR_"), None);
        assert_eq!(trigstr_id("TRIGSTR_12a"), None);
        assert_eq!(trigstr_id("trigstr_12"), None);
        assert_eq!(trigstr_id("TRIGSTR"), None);
    }
}
