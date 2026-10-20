//! Edition-button overrides read from the selected Legacy client, not copied paths.
use crate::{
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, sha256},
    neutral_rule_input::Pe,
    scripts::AssetIdentity,
};

#[derive(Debug)]
pub struct ButtonArt {
    pub frame: String,
    /// Normal, pushed and disabled backdrop frame names and texture paths.
    pub backdrops: [String; 3],
    pub textures: [String; 3],
}

impl ButtonArt {
    pub fn read(image: &[u8], origin: &AssetIdentity, expansion: bool) -> Result<Self, String> {
        let fail = |reason| format!("edition button {}: {reason}", origin.source);
        if origin.source.trim().is_empty()
            || origin.edition.trim().is_empty()
            || origin.version.trim().is_empty()
        {
            return Err(fail("missing source identity"));
        }
        if image.len() > 32 * 1024 * 1024 || sha256(image) != REVIEWED_IMAGE {
            return Err(fail("unsupported client image; no art substituted"));
        }
        let pe = Pe::parse(
            image,
            RuleInputBudget {
                image_bytes: 32 * 1024 * 1024,
                profile_bytes: 128,
                sections: 96,
            },
        )
        .map_err(|_| fail("invalid PE image"))?;
        let text = |operand| -> Result<String, String> {
            let code = pe
                .read_from(operand, true)
                .map_err(|_| fail("missing string operand"))?;
            let pointer = u32::from_le_bytes(
                code.get(..4)
                    .ok_or_else(|| fail("truncated string operand"))?
                    .try_into()
                    .unwrap(),
            );
            let data = pe
                .read_from(pointer, false)
                .map_err(|_| fail("missing string data"))?;
            string(data).map_err(fail)
        };
        // The reviewed initializer makes the button advertise the other edition.
        let operands = if expansion {
            [0x55a052, 0x55a072, 0x55a08a]
        } else {
            [0x559ff3, 0x55a013, 0x55a02b]
        };
        Ok(Self {
            frame: text(0x559eed)?,
            backdrops: [text(0x559f91)?, text(0x559f9d)?, text(0x559fab)?],
            textures: [text(operands[0])?, text(operands[1])?, text(operands[2])?],
        })
    }
}

fn string(data: &[u8]) -> Result<String, &'static str> {
    let end = data
        .iter()
        .take(1024)
        .position(|&b| b == 0)
        .ok_or("unterminated string")?;
    let value = std::str::from_utf8(&data[..end]).map_err(|_| "invalid string encoding")?;
    if value.is_empty() || value.chars().any(char::is_control) {
        return Err("empty or control-bearing string");
    }
    Ok(value.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn origin() -> AssetIdentity {
        AssetIdentity {
            source: "synthetic".into(),
            edition: "test".into(),
            version: "test".into(),
        }
    }
    #[test]
    fn strings_are_bounded_and_checked() {
        assert_eq!(
            string(b"art\\normal.blp\0ignored").unwrap(),
            "art\\normal.blp"
        );
        for bytes in [
            b"\0".as_slice(),
            b"missing terminator",
            b"bad\npath\0",
            b"\xff\0",
            &[b'a'; 1025],
        ] {
            assert!(string(bytes).is_err());
        }
    }
    #[test]
    fn unqualified_images_do_not_invent_an_edition_path() {
        for expansion in [false, true] {
            assert!(ButtonArt::read(b"not a client", &origin(), expansion).is_err());
        }
    }
    #[test]
    #[ignore = "requires explicitly supplied reviewed Legacy client image"]
    fn original_client_supplies_both_state_families() {
        let path = std::env::var("PCX_LIGHTING_CLIENT_IMAGE").expect("PCX_LIGHTING_CLIENT_IMAGE");
        let image = std::fs::read(path).unwrap();
        let roc = ButtonArt::read(&image, &origin(), false).unwrap();
        let tft = ButtonArt::read(&image, &origin(), true).unwrap();
        assert_eq!(roc.frame, tft.frame);
        assert_eq!(roc.backdrops, tft.backdrops);
        for (other, base) in roc.textures.iter().zip(&tft.textures) {
            assert!(other.contains("-TFT-") && base.contains("-ROC-"));
            assert_ne!(other, base);
        }
        let mut changed = image;
        changed[0] ^= 1;
        assert!(ButtonArt::read(&changed, &origin(), false).is_err());
    }
}
