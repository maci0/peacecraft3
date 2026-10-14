pub const TEMPLATES: &str = r#"
Frame "TEXT" "SyntheticStyle" {
 FrameFont "SyntheticFace", 1.25, "",
 FontColor 0.1 0.2 0.3 0.4,
 FontHighlightColor 0.2 0.3 0.4 0.5,
 FontDisabledColor 0.3 0.4 0.5 0.6,
 FontShadowColor 0.4 0.5 0.6 0.7,
 FontShadowOffset 0.25 -0.5,
 FontJustificationH JUSTIFYLEFT,
 FontJustificationV JUSTIFYTOP,
 FontFlags "FIXEDSIZE",
}
"#;
pub const MENU: &str = r#"
Frame "FRAME" "ControlLayer" {
 Width 3,
 Height 2,
 Frame "BUTTON" "SyntheticButton" {
  Width 1,
  Height 1,
  ButtonText "SyntheticText",
 }
}
Frame "TEXT" "SyntheticText" INHERITS "SyntheticStyle" {
}
"#;
pub const SKIN: &str = "[Default]\nSyntheticFace=Fonts\\SyntheticFixture.ttf\n";
#[test]
fn valid_contract() {
    let fdf = crate::fdf::Fdf::parse(MENU).unwrap();
    let (items, _) = crate::glue_menu::control_items(&fdf, "ControlLayer", (3.0, 2.0)).unwrap();
    assert_eq!(items.len(), 1);
    let binding = crate::mainmenu::menu_font_file(SKIN, "SyntheticFace").unwrap();
    assert_eq!(binding.file, r"Fonts\SyntheticFixture.ttf");
    assert_eq!(binding.section, "Default");
    assert_eq!(
        crate::mainmenu::served_font_name(&binding.file),
        Some("menu/fonts/SyntheticFixture.ttf".into())
    );
}
#[test]
fn typed_font_errors() {
    use crate::{fdf::FdfError, menu_skin::SkinError, menu_text::TextError};
    assert!(matches!(
        crate::mainmenu::menu_font_file("broken", "SyntheticFace"),
        Err(SkinError::Malformed(_))
    ));
    assert!(
        matches!(crate::mainmenu::menu_font_file("[Default]\nOtherFace=x\n","SyntheticFace"),Err(SkinError::UnknownFace(face)) if face=="SyntheticFace")
    );
    assert!(
        matches!(crate::mainmenu::menu_font_file("[Default]\nSyntheticFace=a\nSyntheticFace=b\n","SyntheticFace"),Err(SkinError::Duplicate{values,..}) if values==["a","b"])
    );
    assert!(matches!(
        crate::mainmenu::menu_font_file("[Default]\nSyntheticFace=\n", "SyntheticFace"),
        Err(SkinError::EmptyValue { .. })
    ));
    assert!(matches!(
        crate::fdf::Fdf::parse("Frame \"TEXT\" \"broken\" {\n"),
        Err(FdfError::Unterminated(_))
    ));
    assert!(matches!(
        crate::mainmenu::menu_text_style(&[TEMPLATES], "AbsentText"),
        Err(TextError::UnknownFrame(_))
    ));
}
