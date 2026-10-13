//! Case-insensitive MPQ path-glob matching.
//!
//! MPQ lookup folds ASCII case, and the internal separator is `\`
//! (see `pcx_mpq::hash::hash_string`, which folds ASCII case and rewrites `/`
//! to `\` before hashing). This matcher mirrors that lookup: it folds ASCII
//! case and treats `\` and `/` as the same separator, so a command-line pattern
//! agrees with what `MpqArchive::contains` will accept.
//!
//! Supported wildcards, none of which require an external dependency:
//! - `*`  — any run of characters within one path segment, never a separator;
//! - `?`  — exactly one character within a segment, never a separator;
//! - `**` — as a whole segment only, zero or more segments including the
//!   separators between them (so `Units\**\*.mdx` also matches `Units\a.mdx`).
//!
//! A segment such as `a**b` is not a globstar; it is matched as `*` within that
//! segment by [`match_segment`].

/// Fold ASCII case and unify `\`/`/`, matching the archive's own normalisation.
fn normalize(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c == '\\' {
                '/'
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect()
}

/// Preserve separator positions, including empty segments.
fn segments(s: &str) -> Vec<&str> {
    s.split('/').collect()
}

/// Match whole segments, honouring `**` as a zero-or-more-segment wildcard.
fn match_segments(pat: &[&str], path: &[&str]) -> bool {
    let mut previous = vec![false; path.len() + 1];
    previous[0] = true;
    for segment in pat {
        let mut current = vec![false; path.len() + 1];
        if *segment == "**" {
            current[0] = previous[0];
            for j in 1..=path.len() {
                current[j] = previous[j] || current[j - 1];
            }
        } else {
            for j in 1..=path.len() {
                current[j] = previous[j - 1] && match_segment(segment, path[j - 1]);
            }
        }
        previous = current;
    }
    previous[path.len()]
}

/// Match one path segment with `*` and `?` (iterative greedy backtracking).
fn match_segment(pat: &str, text: &str) -> bool {
    let p: Vec<char> = pat.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (mut i, mut j) = (0usize, 0usize);
    let mut star: Option<usize> = None;
    let mut mark = 0usize;
    while j < t.len() {
        if i < p.len() && (p[i] == '?' || p[i] == t[j]) {
            i += 1;
            j += 1;
        } else if i < p.len() && p[i] == '*' {
            star = Some(i);
            mark = j;
            i += 1;
        } else if let Some(si) = star {
            i = si + 1;
            mark += 1;
            j = mark;
        } else {
            return false;
        }
    }
    while i < p.len() && p[i] == '*' {
        i += 1;
    }
    i == p.len()
}

/// True when `path` matches `pattern` under MPQ case and separator rules.
///
/// A pattern without `*` or `?` is matched literally, so callers can use this
/// for exact paths too.
pub fn matches(pattern: &str, path: &str) -> bool {
    let pat = normalize(pattern);
    let p = normalize(path);
    match_segments(&segments(&pat), &segments(&p))
}

#[cfg(test)]
mod tests {
    use super::matches;

    #[test]
    fn exact_paths_ignore_case_and_separators() {
        assert!(matches(
            "Units\\Footman\\Footman.mdx",
            "units/footman/footman.mdx"
        ));
        assert!(matches(
            "units/footman/footman.MDX",
            "Units\\Footman\\Footman.mdx"
        ));
        assert!(!matches(
            "Units\\Footman\\Footman.mdx",
            "Units\\Footman\\Grunt.mdx"
        ));
    }

    #[test]
    fn star_within_one_segment() {
        assert!(matches("*.blp", "foo.blp"));
        assert!(matches("*.blp", "FOO.BLP"));
        assert!(matches("*.blp", ".blp"));
        assert!(!matches("*.blp", "foo.mdx"));
        assert!(
            !matches("*.blp", "dir/foo.blp"),
            "`*` must not cross a separator"
        );
    }

    #[test]
    fn star_in_the_middle_of_a_segment() {
        assert!(matches("war3*.mpq", "war3.mpq"));
        assert!(matches("war3*.mpq", "War3Patch.MPQ"));
        assert!(!matches("war3*.mpq", "war3.mpqx"));
        assert!(!matches("a*b", "a/b"));
    }

    #[test]
    fn question_matches_exactly_one_character() {
        assert!(matches("a?c", "abc"));
        assert!(matches("a?c", "aBc"));
        assert!(!matches("a?c", "ac"));
        assert!(!matches("a?c", "abbc"));
        assert!(!matches("a?c", "a/c"), "`?` must not match a separator");
    }

    #[test]
    fn trailing_star_matches_empty() {
        assert!(matches("Textures\\*", "Textures\\foo.blp"));
        assert!(matches("Textures\\*", "textures/Foo.BLP"));
        assert!(matches("Textures\\*", "Textures\\"));
        assert!(!matches("Textures\\*", "Textures\\sub\\foo.blp"));
        assert!(!matches("Textures\\*", "Terrain\\foo.blp"));
    }

    #[test]
    fn globstar_spans_segments_including_zero() {
        assert!(matches("Units\\**\\*.mdx", "Units\\Footman.mdx"));
        assert!(matches("Units\\**\\*.mdx", "Units\\Footman\\Footman.mdx"));
        assert!(matches("Units\\**\\*.mdx", "units/a/b/c/footman.MDX"));
        assert!(!matches("Units\\**\\*.mdx", "Units\\Footman\\Footman.mdl"));
        assert!(!matches("Units\\**\\*.mdx", "Buildings\\Footman.mdx"));
    }

    #[test]
    fn bare_globstar_matches_anything() {
        assert!(matches("**", "foo"));
        assert!(matches("**", "a/b/c.blp"));
        assert!(matches("**", ""));
        assert!(matches("**\\*.blp", "a/b/c.blp"));
        assert!(matches("**/*.blp", "a\\b\\c.blp"));
    }

    #[test]
    fn globstar_at_the_end() {
        assert!(matches("Units\\**", "Units\\a\\b\\c.mdx"));
        assert!(matches("Units\\**", "Units"));
        assert!(!matches("Units\\**", "Buildings\\a"));
    }

    #[test]
    fn literal_separators_are_preserved() {
        assert!(!matches("Textures//foo.blp", "Textures/foo.blp"));
        assert!(!matches("/Textures/foo.blp", "Textures/foo.blp"));
        assert!(!matches("Textures/foo.blp/", "Textures/foo.blp"));
    }

    #[test]
    fn multiple_globs_in_one_pattern() {
        assert!(matches("**\\*\\*.mdx", "Units\\Footman\\footman.mdx"));
        assert!(matches("U?its\\**\\F*.mdx", "Units\\a\\Footman.mdx"));
        assert!(!matches("U?its\\**\\F*.mdx", "Units\\a\\Grunt.mdx"));
    }

    #[test]
    fn question_and_star_combine_in_a_segment() {
        assert!(matches("Foo?.*", "Foo1.mdx"));
        assert!(matches("Foo?.*", "foot.MDL"));
        assert!(!matches("Foo?.*", "Foo.mdx"));
    }
}
