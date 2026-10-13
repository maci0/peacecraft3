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

#[cfg(test)]
mod tests {
    use super::*;

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
