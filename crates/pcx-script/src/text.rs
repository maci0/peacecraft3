//! Raw script text and the selected native map-string boundary.
//!
//! Evaluation retains trigger tokens and paired tag markers. Map WTS lookup is
//! native-specific; UI dictionaries and text formatting are separate consumers.
//! The host distinguishes a missing map source from an absent entry in an
//! admitted source, and this module performs no recursive substitutions.

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
pub fn native_trigstr_id(text: &str) -> Option<u32> {
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

/// Native consumer access to the selected map table. Missing source is an error;
/// `Ok(None)` means that an admitted table has no entry with this id.
pub type NativeStringLookup<'a> = dyn Fn(u32) -> Result<Option<String>, String> + 'a;

/// C-string byte boundary, without Unicode conversion or trigger lookup.
pub fn native_c_bytes(bytes: &[u8]) -> &[u8] {
    &bytes[..bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len())]
}

/// The selected native map-string wrapper: C text and at most one WTS lookup.
/// No UI dictionary lookup or tag substitution happens at this boundary.
pub fn native_map_string<'a>(
    text: &'a str,
    source: Option<&NativeStringLookup<'_>>,
) -> Result<std::borrow::Cow<'a, str>, String> {
    let text = text
        .split('\0')
        .next()
        .expect("split has an initial segment");
    let Some(id) = native_trigstr_id(text) else {
        return Ok(std::borrow::Cow::Borrowed(text));
    };
    let source = source.ok_or_else(|| format!("TRIGSTR_{id}: map string source unavailable"))?;
    let mut body = source(id)
        .map_err(|why| format!("TRIGSTR_{id}: {why}"))?
        .unwrap_or_default();
    if let Some(end) = body.find('\0') {
        body.truncate(end);
    }
    Ok(std::borrow::Cow::Owned(body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_map_source_distinguishes_missing_entry_from_missing_or_broken_source() {
        assert!(
            native_map_string("TRIGSTR_7", None)
                .unwrap_err()
                .contains("source unavailable")
        );
        let absent = |_| Ok(None);
        assert_eq!(native_map_string("TRIGSTR_7", Some(&absent)).unwrap(), "");
        let unavailable = |_| Err("selected map/WTS source absent".into());
        assert!(
            native_map_string("TRIGSTR_7", Some(&unavailable))
                .unwrap_err()
                .contains("selected map/WTS source absent")
        );
        let malformed = |_| Err("fixture.wts entry7 invalid UTF8".into());
        assert!(
            native_map_string("TRIGSTR_7", Some(&malformed))
                .unwrap_err()
                .contains("fixture.wts entry7 invalid UTF8")
        );
        let body = |_| Ok(Some("TRIGSTR_8\0tail".into()));
        assert_eq!(
            native_map_string("TRIGSTR_7", Some(&body)).unwrap(),
            "TRIGSTR_8"
        );
        assert_eq!(native_map_string("literal\0tail", None).unwrap(), "literal");
        assert_eq!(native_map_string("TRIGSTR_-1", None).unwrap(), "TRIGSTR_-1");
    }

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
