//! Explicit model-viewer inputs; no model or terrain asset is chosen implicitly.
#[derive(Debug, PartialEq, Eq)]
pub struct ViewerArgs {
    pub install: Option<String>,
    pub model: String,
    pub tile: Option<[u8; 4]>,
}

impl ViewerArgs {
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let mut positional = Vec::new();
        let mut tile = None;
        let mut args = args.iter();
        while let Some(arg) = args.next() {
            if arg == "--tile" {
                if tile.is_some() {
                    return Err("--tile may be supplied only once".into());
                }
                let value = args
                    .next()
                    .ok_or("--tile requires an exact four-byte tileID")?;
                tile = Some(
                    value
                        .as_bytes()
                        .try_into()
                        .map_err(|_| "--tile requires an exact four-byte tileID")?,
                );
            } else if arg.starts_with('-') {
                return Err(format!("unknown viewer option {arg:?}"));
            } else {
                positional.push(arg);
                if positional.len() > 2 {
                    return Err("view accepts [INSTALL] MODEL [--tile FOURCC]".into());
                }
            }
        }
        let model = positional.last().ok_or(
            "view requires an explicit MODEL path; use view [INSTALL] MODEL [--tile FOURCC]",
        )?;
        let lower = model.to_ascii_lowercase();
        if !(lower.ends_with(".mdx") || lower.ends_with(".mdl")) {
            return Err("MODEL must name an explicit .mdx or .mdl archive member".into());
        }
        Ok(Self {
            install: (positional.len() == 2).then(|| positional[0].clone()),
            model: (*model).clone(),
            tile,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parse(args: &[&str]) -> Result<ViewerArgs, String> {
        ViewerArgs::parse(&args.iter().map(|value| (*value).into()).collect::<Vec<_>>())
    }

    #[test]
    fn explicit_model_only_has_no_implicit_terrain() {
        assert_eq!(
            parse(&["Synthetic\\Chosen.MDX"]).unwrap(),
            ViewerArgs {
                install: None,
                model: "Synthetic\\Chosen.MDX".into(),
                tile: None
            }
        );
    }
    #[test]
    fn tile_option_is_forwarded_exactly_in_any_position() {
        for args in [
            vec!["--tile", "Zzzz", "synthetic-install", "chosen.mdl"],
            vec!["synthetic-install", "--tile", "Zzzz", "chosen.mdl"],
            vec!["synthetic-install", "chosen.mdl", "--tile", "Zzzz"],
        ] {
            assert_eq!(
                parse(&args).unwrap(),
                ViewerArgs {
                    install: Some("synthetic-install".into()),
                    model: "chosen.mdl".into(),
                    tile: Some(*b"Zzzz")
                }
            );
        }
    }
    #[test]
    fn missing_assets_and_ambiguous_arguments_are_refused() {
        for args in [
            vec![],
            vec!["synthetic-install"],
            vec!["a", "b", "chosen.mdx"],
            vec!["--unknown", "chosen.mdx"],
        ] {
            assert!(parse(&args).is_err(), "{args:?}");
        }
    }
    #[test]
    fn missing_invalid_and_repeated_tile_options_are_refused() {
        for args in [
            vec!["chosen.mdx", "--tile"],
            vec!["chosen.mdx", "--tile", "abc"],
            vec!["chosen.mdx", "--tile", "abcde"],
            vec!["chosen.mdx", "--tile", "Zzzz", "--tile", "Aaaa"],
        ] {
            assert!(parse(&args).is_err(), "{args:?}");
        }
    }
}
