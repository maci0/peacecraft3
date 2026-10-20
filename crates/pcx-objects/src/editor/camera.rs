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

/// Selected `[Camera]` game-camera values at one zoom index and `[CameraRates]`
/// scroll rates. Angles are degrees, as authored.
#[derive(Clone, Debug, PartialEq)]
pub struct GameCamera {
    pub angle_of_attack: f32,
    pub field_of_view: f32,
    pub rotation: f32,
    pub distance: f32,
    pub far_z: f32,
    pub height: f32,
    pub forward_rate: f32,
    pub strafe_rate: f32,
}

pub fn game_camera(
    bytes: &[u8],
    member: &str,
    zoom: usize,
    max_bytes: usize,
) -> Result<GameCamera, String> {
    if bytes.len() > max_bytes {
        return Err(format!("{member}: camera config exceeds {max_bytes} bytes"));
    }
    let parsed = parse_config_profile(bytes, member).map_err(|error| error.to_string())?;
    if let Some(error) = parsed.unresolved().first() {
        return Err(error.to_string());
    }
    let value = |section: &str, key: &str, index: usize| -> Result<f32, String> {
        let fields = parsed.raw_fields(section, &key.to_ascii_lowercase());
        let [(_, raw)] = fields.as_slice() else {
            return Err(format!("{member}: expected one {section}/{key} assignment"));
        };
        profile_elements(raw)
            .map_err(|error| format!("{member}: {error}"))?
            .get(index)
            .and_then(|value| value.parse::<f32>().ok())
            .filter(|value| value.is_finite())
            .ok_or_else(|| format!("{member}: {section}/{key}[{index}] is missing or invalid"))
    };
    Ok(GameCamera {
        angle_of_attack: value("camera", "AOA", zoom)?,
        field_of_view: value("camera", "FOV", zoom)?,
        rotation: value("camera", "Rotation", zoom)?,
        distance: value("camera", "Distance", zoom)?,
        far_z: value("camera", "FarZ", zoom)?,
        height: value("camera", "Height", zoom)?,
        forward_rate: value("camerarates", "Forward", 0)?,
        strafe_rate: value("camerarates", "Strafe", 0)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_camera_reads_one_zoom_and_scroll_rates() {
        let source = b"[Camera]\nAOA=10,11\nFOV=20,21\nRotation=30,31\nDistance=40,41\nFarZ=50,51\nHeight=6,7\n[CameraRates]\nForward=9\nStrafe=8\n";
        let camera = game_camera(source, "fixture", 1, source.len()).unwrap();
        assert_eq!(
            camera,
            GameCamera {
                angle_of_attack: 11.0,
                field_of_view: 21.0,
                rotation: 31.0,
                distance: 41.0,
                far_z: 51.0,
                height: 7.0,
                forward_rate: 9.0,
                strafe_rate: 8.0,
            }
        );
        assert!(game_camera(source, "fixture", 2, source.len()).is_err());
        assert!(game_camera(b"[Camera]\nAOA=1\n", "fixture", 0, 64).is_err());
    }

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
