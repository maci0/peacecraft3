//! String records from selected GlobalStrings FDF assets.

/// Reads the value of `key` from a `GlobalStrings.fdf` text.
///
/// The records are `KEY  "value",` lines. A key that is absent returns `None`;
/// the caller reports it rather than substituting the key name.
#[must_use]
pub fn global_string(fdf: &str, key: &str) -> Option<String> {
    for line in fdf.lines() {
        let line = line.trim();
        let Some((name, rest)) = line.split_once(char::is_whitespace) else {
            continue;
        };
        if name != key {
            continue;
        }
        let rest = rest.trim();
        let rest = rest.strip_prefix('"')?;
        let end = rest.find('"')?;
        return Some(rest[..end].to_string());
    }
    None
}
