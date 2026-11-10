//! Parser for Warcraft III `*.toc` frame-definition tables of contents.
//!
//! A `.toc` lists one `.fdf` member path per line, in load order. The shipped
//! `UI\FrameDef\FrameDef.toc` (identical in `War3.mpq` and `War3x.mpq`) is
//! CRLF-terminated, uses `\` separators, and has no blank lines, comments, or
//! trailing content. The parser accepts that shape and refuses anything else,
//! naming the line, rather than guessing at what the loader would do.
//!
//! Not verified: whether the original loader accepts comments, blank lines, or
//! other member types in a `.toc`. Those inputs are refused here.

use std::fmt;

/// Largest `.toc` accepted, in bytes. A limit of this parser, not a file format value.
pub const MAX_TOC_BYTES: usize = 64 * 1024;
/// Most lines read from a `.toc`. A limit of this parser, not a file format value.
pub const MAX_TOC_LINES: usize = 1024;
/// Longest member path accepted, in bytes (classic Windows `MAX_PATH`).
pub const MAX_MEMBER_PATH_BYTES: usize = 260;

const FDF_SUFFIX: &str = ".fdf";

/// A `.toc` that cannot be read as a list of member paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TocError {
    /// The input is longer than [`MAX_TOC_BYTES`].
    TooLarge { len: usize },
    /// The input is not UTF-8.
    NotUtf8(std::str::Utf8Error),
    /// The input has more than [`MAX_TOC_LINES`] lines.
    TooManyLines,
    /// The input lists no members.
    Empty,
    /// Line `line` (1-based) is not a member path.
    BadEntry { line: usize, reason: &'static str },
}

impl fmt::Display for TocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { len } => {
                write!(f, "toc is {len} bytes, over the {MAX_TOC_BYTES} byte limit")
            }
            Self::NotUtf8(e) => write!(f, "toc is not UTF-8: {e}"),
            Self::TooManyLines => write!(f, "toc has more than {MAX_TOC_LINES} lines"),
            Self::Empty => write!(f, "toc lists no members"),
            Self::BadEntry { line, reason } => write!(f, "toc line {line}: {reason}"),
        }
    }
}

impl std::error::Error for TocError {}

/// Returns the member paths in `bytes`, in file order.
///
/// Each line must be a `\`-separated relative path of printable ASCII ending in
/// `.fdf` (any case). Line endings may be CRLF or LF. Duplicates are returned as
/// written.
pub fn parse(bytes: &[u8]) -> Result<Vec<String>, TocError> {
    if bytes.len() > MAX_TOC_BYTES {
        return Err(TocError::TooLarge { len: bytes.len() });
    }
    let text = std::str::from_utf8(bytes).map_err(TocError::NotUtf8)?;
    let mut members = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if index == MAX_TOC_LINES {
            return Err(TocError::TooManyLines);
        }
        let member = check_member(line).map_err(|reason| TocError::BadEntry {
            line: index + 1,
            reason,
        })?;
        members.push(member.to_string());
    }
    if members.is_empty() {
        return Err(TocError::Empty);
    }
    Ok(members)
}

/// Checks one line. Length is tested before content, so the reason never
/// depends on scanning an unbounded line.
fn check_member(line: &str) -> Result<&str, &'static str> {
    if line.is_empty() {
        return Err("blank line");
    }
    if line.len() > MAX_MEMBER_PATH_BYTES {
        return Err("path is longer than MAX_MEMBER_PATH_BYTES");
    }
    if !line.bytes().all(|b| (b' '..=b'~').contains(&b)) {
        return Err("byte outside printable ASCII");
    }
    if line.starts_with('\\') || line.contains('/') {
        return Err("not a backslash-separated relative path");
    }
    if line
        .split('\\')
        .any(|segment| matches!(segment, "" | "." | ".."))
    {
        return Err("empty, \".\" or \"..\" path segment");
    }
    let tail = line.get(line.len().saturating_sub(FDF_SUFFIX.len())..);
    if !tail.is_some_and(|tail| tail.eq_ignore_ascii_case(FDF_SUFFIX)) {
        return Err("path does not end in .fdf");
    }
    Ok(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bad(line: usize, reason: &'static str) -> TocError {
        TocError::BadEntry { line, reason }
    }

    #[test]
    fn crlf_lines_keep_file_order_and_backslashes() {
        let members = parse(
            b"UI\\FrameDef\\GlobalStrings.fdf\r\nUI\\FrameDef\\Glue\\Skirmish.fdf\r\nUI\\FrameDef\\UI\\ChatDialog.fdf\r\n",
        )
        .unwrap();
        assert_eq!(
            members,
            [
                r"UI\FrameDef\GlobalStrings.fdf",
                r"UI\FrameDef\Glue\Skirmish.fdf",
                r"UI\FrameDef\UI\ChatDialog.fdf",
            ]
        );
    }

    #[test]
    fn lf_endings_and_a_missing_final_newline_are_accepted() {
        assert_eq!(
            parse(b"a.fdf\nb.fdf").unwrap(),
            ["a.fdf".to_string(), "b.fdf".to_string()]
        );
    }

    #[test]
    fn extension_matches_in_any_case_and_duplicates_are_kept() {
        assert_eq!(
            parse(b"A.FDF\r\nA.FDF\r\n").unwrap(),
            ["A.FDF".to_string(), "A.FDF".to_string()]
        );
    }

    #[test]
    fn empty_input_is_refused() {
        assert_eq!(parse(b""), Err(TocError::Empty));
    }

    #[test]
    fn blank_line_is_refused_with_its_number() {
        assert_eq!(parse(b"a.fdf\r\n\r\nb.fdf\r\n"), Err(bad(2, "blank line")));
    }

    #[test]
    fn a_trailing_blank_line_is_refused() {
        assert_eq!(parse(b"a.fdf\n\n"), Err(bad(2, "blank line")));
    }

    #[test]
    fn forward_slash_and_leading_backslash_are_refused() {
        assert_eq!(
            parse(b"UI/FrameDef/a.fdf\n"),
            Err(bad(1, "not a backslash-separated relative path"))
        );
        assert_eq!(
            parse(b"\\a.fdf\n"),
            Err(bad(1, "not a backslash-separated relative path"))
        );
    }

    #[test]
    fn dot_and_empty_segments_are_refused() {
        for line in ["a\\\\b.fdf", "a\\..\\b.fdf", "a\\.\\b.fdf"] {
            assert_eq!(
                parse(line.as_bytes()),
                Err(bad(1, "empty, \".\" or \"..\" path segment")),
                "{line}"
            );
        }
    }

    #[test]
    fn non_fdf_and_truncated_names_are_refused() {
        assert_eq!(parse(b"a.txt\n"), Err(bad(1, "path does not end in .fdf")));
        assert_eq!(
            parse(b"UI\\FrameDef\\Glue\\Sk"),
            Err(bad(1, "path does not end in .fdf"))
        );
        assert_eq!(parse(b".fd"), Err(bad(1, "path does not end in .fdf")));
    }

    #[test]
    fn non_ascii_and_control_bytes_are_refused() {
        assert_eq!(
            parse("\u{feff}a.fdf\n".as_bytes()),
            Err(bad(1, "byte outside printable ASCII"))
        );
        assert_eq!(
            parse(b"a\tb.fdf\n"),
            Err(bad(1, "byte outside printable ASCII"))
        );
        assert_eq!(
            parse(b"a.fdf\rb.fdf\n"),
            Err(bad(1, "byte outside printable ASCII"))
        );
    }

    #[test]
    fn invalid_utf8_is_refused() {
        assert!(matches!(parse(&[0xff, b'\n']), Err(TocError::NotUtf8(_))));
    }

    #[test]
    fn path_length_limit_is_inclusive() {
        let limit = format!("{}.fdf", "a".repeat(MAX_MEMBER_PATH_BYTES - 4));
        assert_eq!(limit.len(), MAX_MEMBER_PATH_BYTES);
        assert_eq!(parse(limit.as_bytes()).unwrap(), [limit.as_str()]);

        let over = format!("a{limit}");
        assert_eq!(
            parse(over.as_bytes()),
            Err(bad(1, "path is longer than MAX_MEMBER_PATH_BYTES"))
        );
    }

    #[test]
    fn line_count_limit_is_inclusive() {
        let at_limit = "a.fdf\n".repeat(MAX_TOC_LINES);
        assert_eq!(parse(at_limit.as_bytes()).unwrap().len(), MAX_TOC_LINES);

        let over = "a.fdf\n".repeat(MAX_TOC_LINES + 1);
        assert!(over.len() <= MAX_TOC_BYTES);
        assert_eq!(parse(over.as_bytes()), Err(TocError::TooManyLines));
    }

    #[test]
    fn byte_size_limit_is_checked_before_any_line_is_read() {
        let over = vec![b'\n'; MAX_TOC_BYTES + 1];
        assert_eq!(
            parse(&over),
            Err(TocError::TooLarge {
                len: MAX_TOC_BYTES + 1
            })
        );
    }

    #[test]
    fn error_display_names_the_line() {
        let text = parse(b"a.fdf\n\n").unwrap_err().to_string();
        assert_eq!(text, "toc line 2: blank line");
    }
}
