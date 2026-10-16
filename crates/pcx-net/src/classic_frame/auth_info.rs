//! `SERVER_AUTHREQ_109` challenge body.
//!
//! Pinned PvPGN `9cd173f4e02ba3d9f8f15a67ca308b5eb78723e4`:
//! `t_server_authreq_109` in `src/common/bnet_protocol.h` and the reply built
//! by `_client_auth_info` in `src/bnetd/handle_bnet.cpp`. `bn_int` is 4 bytes
//! and `bn_long` is 8, both little-endian (`src/common/bn_type.h`).
//!
//! The body is the version challenge: logon type, session numbers, file
//! timestamp, a terminated check-revision filename, a terminated equation, and
//! a signature whose length depends on the client variant. It is not a login
//! result, an account secret, or a CD key. The equation is not evaluated.
//!
//! Filename and equation bytes are returned without a character encoding.
//! A missing terminator is an error. Bytes are not dropped to fit a cap.

#![deny(missing_docs)]

use std::fmt;

use super::BNCS_FAMILY;

/// High byte of pinned `SERVER_AUTHREQ_109` (`0x50ff`).
pub const SERVER_AUTHREQ_109_ID: u8 = 0x50;

/// `SERVER_AUTHREQ_109_LOGONTYPE`.
pub const LOGON_TYPE_CLASSIC: u32 = 0;

/// `SERVER_AUTHREQ_109_LOGONTYPE_W3` and `SERVER_AUTHREQ_109_LOGONTYPE_W3XP`.
pub const LOGON_TYPE_WARCRAFT3: u32 = 2;

/// Signature slot appended for Warcraft III tags.
///
/// `_client_auth_info` uses `char padding[128]` and `packet_append_data` of
/// that length for `CLIENTTAG_WARCRAFT3_UINT` and `CLIENTTAG_WAR3XP_UINT` only.
/// This is not the size of every auth-info body.
pub const WARCRAFT3_SERVER_SIGNATURE_LEN: usize = 128;

/// `CLIENTTAG_WARCRAFT3_UINT`, the `tag_str_to_uint("WAR3")` value.
pub const CLIENTTAG_WARCRAFT3: u32 = 0x5741_5233;

/// `CLIENTTAG_WAR3XP_UINT`, the `tag_str_to_uint("W3XP")` value.
pub const CLIENTTAG_WAR3XP: u32 = 0x5733_5850;

const BN_INT: usize = 4;
const BN_LONG: usize = 8;
const FIXED_LEN: usize = BN_INT * 3 + BN_LONG;

/// Which `SERVER_AUTHREQ_109` tail this reply uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthReplyVariant {
    /// Logon type [`LOGON_TYPE_CLASSIC`] and no signature bytes.
    ///
    /// The pinned handler uses this shape for a client tag that is neither
    /// Warcraft III nor The Frozen Throne. Selecting it is explicit: an
    /// unlisted tag does not imply this shape.
    Classic,
    /// Logon type [`LOGON_TYPE_WARCRAFT3`] and [`WARCRAFT3_SERVER_SIGNATURE_LEN`]
    /// signature bytes.
    Warcraft3,
}

impl AuthReplyVariant {
    /// Logon type this variant requires in the body.
    pub const fn logon_type(self) -> u32 {
        match self {
            Self::Classic => LOGON_TYPE_CLASSIC,
            Self::Warcraft3 => LOGON_TYPE_WARCRAFT3,
        }
    }

    /// Signature bytes that must follow the equation terminator.
    pub const fn signature_len(self) -> usize {
        match self {
            Self::Classic => 0,
            Self::Warcraft3 => WARCRAFT3_SERVER_SIGNATURE_LEN,
        }
    }

    /// Maps a Warcraft III client tag onto [`Self::Warcraft3`].
    ///
    /// `tag` is a `tag_str_to_uint` value, so `"WAR3"` is
    /// [`CLIENTTAG_WARCRAFT3`]. Any other tag, including other clients this
    /// server can answer, is [`AuthInfoError::UnknownClient`]. Pass
    /// [`Self::Classic`] only when that product's reply layout is the one
    /// the caller has already selected.
    pub const fn from_client_tag(tag: u32) -> Result<Self, AuthInfoError> {
        match tag {
            CLIENTTAG_WARCRAFT3 | CLIENTTAG_WAR3XP => Ok(Self::Warcraft3),
            _ => Err(AuthInfoError::UnknownClient { tag }),
        }
    }
}

/// Caller caps for the two terminated fields. These are not protocol defaults.
///
/// Lengths exclude the terminating NUL. A field that would exceed its cap is
/// refused before any copy of that field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthInfoLimits {
    /// Maximum check-revision filename length.
    pub max_file_bytes: usize,
    /// Maximum equation length. The text is not executed.
    pub max_equation_bytes: usize,
}

/// Check-revision filename and equation the caller expects.
///
/// The bytes come from the caller's selected versioncheck profile. This type
/// does not load `versioncheck.json` and does not embed a profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckRevisionProfile<'a> {
    /// Filename without a NUL.
    pub file: &'a [u8],
    /// Equation without a NUL.
    pub equation: &'a [u8],
}

/// One accepted challenge. String slices exclude their terminating NUL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServerAuthInfo<'a> {
    /// `t_server_authreq_109.logontype`.
    pub logon_type: u32,
    /// `t_server_authreq_109.sessionkey`. Not an account key.
    pub session_key: u32,
    /// `t_server_authreq_109.sessionnum`. Not an account key.
    pub session_num: u32,
    /// Little-endian `bn_long` timestamp written by `file_to_mod_time`.
    pub timestamp: u64,
    /// Check-revision filename, without its NUL.
    pub check_revision_file: &'a [u8],
    /// Check-revision equation, without its NUL. Not evaluated.
    pub equation: &'a [u8],
    /// Signature bytes for this variant. Empty when the variant has none.
    pub signature: &'a [u8],
}

/// Where a structural failure was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyField {
    /// `logontype`, `sessionkey`, `sessionnum`, and `timestamp`.
    Fixed,
    /// Check-revision filename.
    File,
    /// Check-revision equation.
    Equation,
    /// Variant-specific signature tail.
    Signature,
}

impl fmt::Display for BodyField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Fixed => "fixed prefix",
            Self::File => "check-revision filename",
            Self::Equation => "check-revision equation",
            Self::Signature => "server signature",
        })
    }
}

/// Why a challenge body was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthInfoError {
    /// Family or id is not `SERVER_AUTHREQ_109`.
    UnexpectedPacket {
        /// Header family byte.
        family: u8,
        /// Header id byte.
        id: u8,
    },
    /// The body ended before this field was complete.
    Truncated {
        /// Field that ran out of bytes.
        field: BodyField,
        /// Bytes available for that field.
        have: usize,
    },
    /// A string field ended without a NUL inside the body.
    Unterminated {
        /// Filename or equation.
        field: BodyField,
    },
    /// A NUL was not found before the caller cap. The prefix is not returned.
    StringTooLong {
        /// Filename or equation.
        field: BodyField,
        /// Cap that was in force, excluding the NUL.
        limit: usize,
    },
    /// Bytes remain after the variant's signature.
    Extra {
        /// Bytes after the expected tail.
        remaining: usize,
    },
    /// `logontype` is not the value required by the selected variant.
    UnexpectedLogonType {
        /// Value in the body.
        found: u32,
        /// Value the variant requires.
        expected: u32,
    },
    /// [`AuthReplyVariant::from_client_tag`] does not assign this tag a layout.
    UnknownClient {
        /// `tag_str_to_uint` value that was refused.
        tag: u32,
    },
    /// The expected profile itself contains a NUL or exceeds its cap.
    InvalidProfile {
        /// Profile field that cannot be compared.
        field: BodyField,
    },
    /// A terminated field differs from the selected profile.
    ProfileMismatch {
        /// Filename or equation.
        field: BodyField,
    },
}

impl fmt::Display for AuthInfoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedPacket { family, id } => write!(
                f,
                "packet {family:#04x}/{id:#04x} is not a SERVER_AUTHREQ_109 challenge"
            ),
            Self::Truncated { field, have } => {
                write!(f, "{field} is truncated after {have} bytes")
            }
            Self::Unterminated { field } => write!(f, "{field} has no terminating NUL"),
            Self::StringTooLong { field, limit } => {
                write!(f, "{field} exceeds the {limit}-byte cap before its NUL")
            }
            Self::Extra { remaining } => {
                write!(f, "{remaining} trailing bytes after the challenge")
            }
            Self::UnexpectedLogonType { found, expected } => write!(
                f,
                "logon type {found:#x} does not match the variant's {expected:#x}"
            ),
            Self::UnknownClient { tag } => {
                write!(f, "client tag {tag:#010x} has no auth-info reply variant")
            }
            Self::InvalidProfile { field } => {
                write!(f, "{field} profile contains a NUL or exceeds its cap")
            }
            Self::ProfileMismatch { field } => write!(f, "{field} does not match the profile"),
        }
    }
}

impl std::error::Error for AuthInfoError {}

/// Parse a header-checked `SERVER_AUTHREQ_109` body.
///
/// [`super::Frame::server_auth_info`] and [`super::OwnedFrame::server_auth_info`]
/// are the callers. Header length was already accepted.
pub(super) fn parse<'a>(
    family: u8,
    id: u8,
    body: &'a [u8],
    variant: AuthReplyVariant,
    profile: CheckRevisionProfile<'_>,
    limits: AuthInfoLimits,
) -> Result<ServerAuthInfo<'a>, AuthInfoError> {
    if family != BNCS_FAMILY || id != SERVER_AUTHREQ_109_ID {
        return Err(AuthInfoError::UnexpectedPacket { family, id });
    }
    expect_profile(profile.file, limits.max_file_bytes, BodyField::File)?;
    expect_profile(
        profile.equation,
        limits.max_equation_bytes,
        BodyField::Equation,
    )?;

    let Some((fixed, mut rest)) = body.split_at_checked(FIXED_LEN) else {
        return Err(AuthInfoError::Truncated {
            field: BodyField::Fixed,
            have: body.len(),
        });
    };
    let logon_type = u32::from_le_bytes([fixed[0], fixed[1], fixed[2], fixed[3]]);
    let session_key = u32::from_le_bytes([fixed[4], fixed[5], fixed[6], fixed[7]]);
    let session_num = u32::from_le_bytes([fixed[8], fixed[9], fixed[10], fixed[11]]);
    let timestamp = u64::from_le_bytes([
        fixed[12], fixed[13], fixed[14], fixed[15], fixed[16], fixed[17], fixed[18], fixed[19],
    ]);
    let expected = variant.logon_type();
    if logon_type != expected {
        return Err(AuthInfoError::UnexpectedLogonType {
            found: logon_type,
            expected,
        });
    }

    let check_revision_file = take_cstr(&mut rest, limits.max_file_bytes, BodyField::File)?;
    let equation = take_cstr(&mut rest, limits.max_equation_bytes, BodyField::Equation)?;
    let signature_len = variant.signature_len();
    if rest.len() < signature_len {
        return Err(AuthInfoError::Truncated {
            field: BodyField::Signature,
            have: rest.len(),
        });
    }
    if rest.len() > signature_len {
        return Err(AuthInfoError::Extra {
            remaining: rest.len() - signature_len,
        });
    }
    if check_revision_file != profile.file {
        return Err(AuthInfoError::ProfileMismatch {
            field: BodyField::File,
        });
    }
    if equation != profile.equation {
        return Err(AuthInfoError::ProfileMismatch {
            field: BodyField::Equation,
        });
    }
    Ok(ServerAuthInfo {
        logon_type,
        session_key,
        session_num,
        timestamp,
        check_revision_file,
        equation,
        signature: rest,
    })
}

fn expect_profile(bytes: &[u8], limit: usize, field: BodyField) -> Result<(), AuthInfoError> {
    if bytes.len() > limit || bytes.contains(&0) {
        Err(AuthInfoError::InvalidProfile { field })
    } else {
        Ok(())
    }
}

fn take_cstr<'a>(
    input: &mut &'a [u8],
    max_content: usize,
    field: BodyField,
) -> Result<&'a [u8], AuthInfoError> {
    let mut index = 0;
    while index < input.len() {
        if input[index] == 0 {
            let text = &input[..index];
            *input = &input[index + 1..];
            return Ok(text);
        }
        if index == max_content {
            return Err(AuthInfoError::StringTooLong {
                field,
                limit: max_content,
            });
        }
        index += 1;
    }
    Err(AuthInfoError::Unterminated { field })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SESSION_KEY: u32 = 0x1111_2222;
    const SESSION_NUM: u32 = 0x3333_4444;
    const TIMESTAMP: u64 = 0x0102_0304_0506_0708;

    fn limits(file: usize, equation: usize) -> AuthInfoLimits {
        AuthInfoLimits {
            max_file_bytes: file,
            max_equation_bytes: equation,
        }
    }

    fn challenge(logon: u32, file: &[u8], equation: &[u8], signature: &[u8]) -> Vec<u8> {
        let mut body =
            Vec::with_capacity(FIXED_LEN + file.len() + equation.len() + signature.len() + 2);
        body.extend_from_slice(&logon.to_le_bytes());
        body.extend_from_slice(&SESSION_KEY.to_le_bytes());
        body.extend_from_slice(&SESSION_NUM.to_le_bytes());
        body.extend_from_slice(&TIMESTAMP.to_le_bytes());
        body.extend_from_slice(file);
        body.push(0);
        body.extend_from_slice(equation);
        body.push(0);
        body.extend_from_slice(signature);
        body
    }

    fn parse_body<'a>(
        stored: &'a [u8],
        variant: AuthReplyVariant,
        file: &'a [u8],
        equation: &'a [u8],
        caps: AuthInfoLimits,
    ) -> Result<ServerAuthInfo<'a>, AuthInfoError> {
        let frame = super::super::decode_one(stored, 4_096).unwrap();
        frame.server_auth_info(variant, CheckRevisionProfile { file, equation }, caps)
    }

    fn encode_body(body: &[u8]) -> Vec<u8> {
        super::super::encode(BNCS_FAMILY, SERVER_AUTHREQ_109_ID, body).unwrap()
    }

    #[test]
    fn classic_bytes_round_trip_without_a_signature() {
        let file = b"a\xff.mpq";
        let equation = b"A=1 \x80";
        let body = challenge(LOGON_TYPE_CLASSIC, file, equation, &[]);
        let stored = encode_body(&body);
        let parsed = parse_body(
            &stored,
            AuthReplyVariant::Classic,
            file,
            equation,
            limits(16, 16),
        )
        .unwrap();
        assert_eq!(parsed.logon_type, LOGON_TYPE_CLASSIC);
        assert_eq!(parsed.session_key, SESSION_KEY);
        assert_eq!(parsed.session_num, SESSION_NUM);
        assert_eq!(parsed.timestamp, TIMESTAMP);
        assert_eq!(parsed.check_revision_file, file);
        assert_eq!(parsed.equation, equation);
        assert!(parsed.signature.is_empty());

        let mut decoder = super::super::Decoder::new(super::super::FrameLimits {
            max_frame_bytes: 4_096,
            max_frames: 1,
        })
        .unwrap();
        let owned = decoder.push(&stored).unwrap();
        let from_owned = owned[0]
            .server_auth_info(
                AuthReplyVariant::Classic,
                CheckRevisionProfile { file, equation },
                limits(16, 16),
            )
            .unwrap();
        assert_eq!(from_owned, parsed);
    }

    #[test]
    fn warcraft3_keeps_a_non_zero_signature_of_the_variant_length() {
        let variant = AuthReplyVariant::from_client_tag(CLIENTTAG_WAR3XP).unwrap();
        assert_eq!(variant.signature_len(), WARCRAFT3_SERVER_SIGNATURE_LEN);
        assert_eq!(
            AuthReplyVariant::from_client_tag(CLIENTTAG_WARCRAFT3).unwrap(),
            variant
        );
        let mut signature = vec![0u8; variant.signature_len()];
        signature[0] = 0x5a;
        signature[127] = 0x11;
        let file = b"check.mpq";
        let equation = b"A=1";
        let stored = encode_body(&challenge(variant.logon_type(), file, equation, &signature));
        let parsed = parse_body(&stored, variant, file, equation, limits(16, 16)).unwrap();
        assert_eq!(parsed.signature, signature.as_slice());
        assert!(parsed.signature.iter().any(|byte| *byte != 0));
    }

    #[test]
    fn truncated_prefix_and_short_signature_are_refused() {
        let stored = encode_body(&[0u8; FIXED_LEN - 1]);
        let err =
            parse_body(&stored, AuthReplyVariant::Classic, b"", b"", limits(8, 8)).unwrap_err();
        assert_eq!(
            err,
            AuthInfoError::Truncated {
                field: BodyField::Fixed,
                have: FIXED_LEN - 1,
            }
        );

        let short = vec![0u8; WARCRAFT3_SERVER_SIGNATURE_LEN - 1];
        let stored = encode_body(&challenge(LOGON_TYPE_WARCRAFT3, b"f", b"e", &short));
        let err = parse_body(
            &stored,
            AuthReplyVariant::Warcraft3,
            b"f",
            b"e",
            limits(8, 8),
        )
        .unwrap_err();
        assert_eq!(
            err,
            AuthInfoError::Truncated {
                field: BodyField::Signature,
                have: WARCRAFT3_SERVER_SIGNATURE_LEN - 1,
            }
        );
    }

    #[test]
    fn a_missing_terminator_does_not_return_a_partial_string() {
        let mut body = challenge(LOGON_TYPE_CLASSIC, b"file", b"eq", &[]);
        body.pop();
        let stored = encode_body(&body);
        let err = parse_body(
            &stored,
            AuthReplyVariant::Classic,
            b"file",
            b"eq",
            limits(8, 8),
        )
        .unwrap_err();
        assert_eq!(
            err,
            AuthInfoError::Unterminated {
                field: BodyField::Equation,
            }
        );

        let mut body = Vec::new();
        body.extend_from_slice(&LOGON_TYPE_CLASSIC.to_le_bytes());
        body.extend_from_slice(&SESSION_KEY.to_le_bytes());
        body.extend_from_slice(&SESSION_NUM.to_le_bytes());
        body.extend_from_slice(&TIMESTAMP.to_le_bytes());
        body.extend_from_slice(b"ab");
        let stored = encode_body(&body);
        let err =
            parse_body(&stored, AuthReplyVariant::Classic, b"ab", b"", limits(4, 4)).unwrap_err();
        assert_eq!(
            err,
            AuthInfoError::Unterminated {
                field: BodyField::File,
            }
        );
    }

    #[test]
    fn a_string_past_the_cap_is_not_truncated_to_the_cap() {
        let stored = encode_body(&challenge(LOGON_TYPE_CLASSIC, b"abcd", b"eq", &[]));
        let err = parse_body(
            &stored,
            AuthReplyVariant::Classic,
            b"ab",
            b"eq",
            limits(2, 8),
        )
        .unwrap_err();
        assert_eq!(
            err,
            AuthInfoError::StringTooLong {
                field: BodyField::File,
                limit: 2,
            }
        );
    }

    #[test]
    fn unknown_tags_and_logon_types_do_not_pick_a_signature_size() {
        let star = u32::from_be_bytes(*b"STAR");
        assert_eq!(
            AuthReplyVariant::from_client_tag(star),
            Err(AuthInfoError::UnknownClient { tag: star })
        );

        let stored = encode_body(&challenge(1, b"f", b"e", &[]));
        let err =
            parse_body(&stored, AuthReplyVariant::Classic, b"f", b"e", limits(8, 8)).unwrap_err();
        assert_eq!(
            err,
            AuthInfoError::UnexpectedLogonType {
                found: 1,
                expected: LOGON_TYPE_CLASSIC,
            }
        );

        let stored = encode_body(&challenge(LOGON_TYPE_CLASSIC, b"f", b"e", &[]));
        let err = parse_body(
            &stored,
            AuthReplyVariant::Warcraft3,
            b"f",
            b"e",
            limits(8, 8),
        )
        .unwrap_err();
        assert_eq!(
            err,
            AuthInfoError::UnexpectedLogonType {
                found: LOGON_TYPE_CLASSIC,
                expected: LOGON_TYPE_WARCRAFT3,
            }
        );
    }

    #[test]
    fn extra_bytes_are_refused_for_both_variants() {
        let mut classic = challenge(LOGON_TYPE_CLASSIC, b"f", b"e", &[]);
        classic.push(0x7f);
        let stored = encode_body(&classic);
        let err =
            parse_body(&stored, AuthReplyVariant::Classic, b"f", b"e", limits(8, 8)).unwrap_err();
        assert_eq!(err, AuthInfoError::Extra { remaining: 1 });

        let mut signature = vec![0u8; WARCRAFT3_SERVER_SIGNATURE_LEN + 1];
        signature[0] = 1;
        let stored = encode_body(&challenge(LOGON_TYPE_WARCRAFT3, b"f", b"e", &signature));
        let err = parse_body(
            &stored,
            AuthReplyVariant::Warcraft3,
            b"f",
            b"e",
            limits(8, 8),
        )
        .unwrap_err();
        assert_eq!(err, AuthInfoError::Extra { remaining: 1 });
    }

    #[test]
    fn profile_mismatch_and_a_nul_in_the_profile_are_refused() {
        let stored = encode_body(&challenge(LOGON_TYPE_CLASSIC, b"file.mpq", b"A=1", &[]));
        let err = parse_body(
            &stored,
            AuthReplyVariant::Classic,
            b"file.mpq",
            b"A=2",
            limits(16, 16),
        )
        .unwrap_err();
        assert_eq!(
            err,
            AuthInfoError::ProfileMismatch {
                field: BodyField::Equation,
            }
        );
        let err = parse_body(
            &stored,
            AuthReplyVariant::Classic,
            b"other.mpq",
            b"A=1",
            limits(16, 16),
        )
        .unwrap_err();
        assert_eq!(
            err,
            AuthInfoError::ProfileMismatch {
                field: BodyField::File,
            }
        );

        let frame = super::super::decode_one(&stored, 4_096).unwrap();
        let err = frame
            .server_auth_info(
                AuthReplyVariant::Classic,
                CheckRevisionProfile {
                    file: b"fi\0le",
                    equation: b"A=1",
                },
                limits(16, 16),
            )
            .unwrap_err();
        assert_eq!(
            err,
            AuthInfoError::InvalidProfile {
                field: BodyField::File,
            }
        );
    }

    #[test]
    fn other_packet_ids_are_not_parsed_as_this_challenge() {
        let body = challenge(LOGON_TYPE_CLASSIC, b"f", b"e", &[]);
        let stored = super::super::encode(BNCS_FAMILY, 0x06, &body).unwrap();
        let err =
            parse_body(&stored, AuthReplyVariant::Classic, b"f", b"e", limits(8, 8)).unwrap_err();
        assert_eq!(
            err,
            AuthInfoError::UnexpectedPacket {
                family: BNCS_FAMILY,
                id: 0x06,
            }
        );
        let stored =
            super::super::encode(super::super::W3GS_FAMILY, SERVER_AUTHREQ_109_ID, &body).unwrap();
        let err =
            parse_body(&stored, AuthReplyVariant::Classic, b"f", b"e", limits(8, 8)).unwrap_err();
        assert_eq!(
            err,
            AuthInfoError::UnexpectedPacket {
                family: super::super::W3GS_FAMILY,
                id: SERVER_AUTHREQ_109_ID,
            }
        );
    }
}
