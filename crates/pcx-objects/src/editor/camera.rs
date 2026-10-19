//! Selected camera configuration using the shared profile lexer.
use super::defaults::{parse_config_profile, profile_elements};

/// The caller selects a zoom-list index; no engine zoom state is inferred.
pub fn near_clip(bytes: &[u8], member: &str, zoom: usize, max_bytes: usize) -> Result<f32, String> {
    if bytes.len() > max_bytes {
        return Err(format!("{member}: camera config exceeds {max_bytes} bytes"));
    }
    let parsed = parse_config_profile(bytes, member).map_err(|error| error.to_string())?;
    if let Some(error) = parsed.unresolved().first() {
        return Err(error.to_string());
    }
    let fields = parsed.raw_fields("camera", "nearz");
    let [(_, raw)] = fields.as_slice() else {
        return Err(format!("{member}: expected one Camera/NearZ assignment"));
    };
    let values = profile_elements(raw).map_err(|error| format!("{member}: {error}"))?;
    let near = values
        .get(zoom)
        .and_then(|value| value.parse::<f32>().ok())
        .filter(|value| value.is_finite() && *value > 0.0)
        .ok_or_else(|| format!("{member}: Camera/NearZ[{zoom}] is missing or invalid"))?;
    Ok(near)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_uses_profile_grammar_and_refuses_missing_or_ambiguous_values() {
        let source = b"[Other]\nPath=\"a//b\" // outside comment\n[Camera]\r\n// comment\r\nNearZ=7.5, 19 // comment with \"quotes\"\r\n";
        assert_eq!(near_clip(source, "fixture", 1, source.len()), Ok(19.0));
        assert!(near_clip(source, "fixture", 2, source.len()).is_err());
        assert!(near_clip(source, "fixture", 0, source.len() - 1).is_err());
        for source in [
            &b"[Camera]\nNearZ=0"[..],
            &b"[Camera]\nNearZ=NaN"[..],
            &b"[Camera]\nNearZ=2\nNearZ=2"[..],
            &b"[Camera]\nNearZ=\"open"[..],
            &b"[Other]\nNearZ=3"[..],
        ] {
            assert!(near_clip(source, "fixture", 0, 128).is_err());
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    #[ignore = "requires PCX_CAMERA_MISC selecting a locally supplied UI/MiscData.txt"]
    fn selected_local_camera_config() {
        use std::io::Read;
        let path = std::env::var_os("PCX_CAMERA_MISC").expect("explicit local config selection");
        let file = std::fs::File::open(&path).unwrap();
        let mut bytes = Vec::new();
        file.take(65_537).read_to_end(&mut bytes).unwrap();
        let result = near_clip(
            &bytes,
            &std::path::PathBuf::from(path).display().to_string(),
            0,
            65_536,
        );
        assert!(result.is_ok(), "{result:?}");
    }
}
