//! The optional `CustomKeys.txt` hotkey overrides. The file is read once per session when
//! the `customkeys` gameplay option is on. Evidence for the file locations and the option:
//! `docs/re-notes/input-command-keys.md`.
use bevy::prelude::*;
use pcx_play::{command_hotkey::CustomKeys, options_defaults::Defaults};
use std::path::{Path, PathBuf};

use crate::{
    campaign_command_card::CommandCard, campaign_input::CampaignInput, menu_screens::Navigation,
};

/// Row of the client's option table, and the options-panel checkbox that stores it.
const OPTION: &str = "customkeys";
const CHECKBOX: &str = "CustomKeysCheckBox";
const FILE: &str = "CustomKeys.txt";
/// Path of the user-data copy below the user's home folder, from `CustomKeyInfo.txt` and the
/// client's `CustomKeyBindings` folder name.
const USER_FOLDERS: [&str; 3] = ["Documents", "Warcraft III", "CustomKeyBindings"];

/// The option: the saved checkbox when the options panel has one, else the client's fallback
/// row (`0` or `1`).
fn enabled(fallback: Option<&[u8]>, saved: Option<bool>) -> Result<bool, String> {
    if let Some(saved) = saved {
        return Ok(saved);
    }
    match fallback {
        Some(b"0") => Ok(false),
        Some(b"1") => Ok(true),
        Some(other) => Err(format!(
            "{OPTION}: fallback {:?} is not 0 or 1",
            String::from_utf8_lossy(other)
        )),
        None => Err(format!("{OPTION}: the client has no fallback row")),
    }
}

/// Where the file is looked for, first match wins: the user-data folder, then the install
/// folder.
fn candidates(home: Option<&Path>, install: Option<&Path>) -> Vec<PathBuf> {
    let user = home.map(|home| {
        USER_FOLDERS
            .iter()
            .fold(home.to_path_buf(), |path, folder| path.join(folder))
            .join(FILE)
    });
    user.into_iter()
        .chain(install.map(|install| install.join(FILE)))
        .collect()
}

/// The first candidate that exists. A missing file is not an error; an unreadable or
/// malformed one is.
fn load_from(
    candidates: &[PathBuf],
    read: impl Fn(&Path) -> std::io::Result<Vec<u8>>,
) -> Result<Option<CustomKeys>, String> {
    for path in candidates {
        let bytes = match read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(format!("{}: {error}", path.display())),
        };
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| format!("{}: not UTF-8 text", path.display()))?;
        return CustomKeys::parse(text)
            .map(Some)
            .map_err(|error| format!("{}: {error}", path.display()));
    }
    Ok(None)
}

#[cfg(not(target_arch = "wasm32"))]
fn read_file(path: &Path) -> std::io::Result<Vec<u8>> {
    /// Application ceiling on the file, not a client value.
    const MAX_BYTES: u64 = 1 << 20;
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(std::io::Error::other("file exceeds the size ceiling"));
    }
    Ok(bytes)
}

/// A browser has no user folder to read.
#[cfg(target_arch = "wasm32")]
fn read_file(_: &Path) -> std::io::Result<Vec<u8>> {
    Err(std::io::ErrorKind::NotFound.into())
}

fn home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

/// Gives the card the file's overrides once, when the option is on. A refusal is shown
/// through `CampaignInput::last_error` and leaves the stock hotkeys.
pub(crate) fn load(
    launcher: Option<Res<crate::launch_gui::Launcher>>,
    navigation: Option<Res<Navigation>>,
    mut card: ResMut<CommandCard>,
    mut input: ResMut<CampaignInput>,
) {
    if !card.wants_custom_keys() {
        return;
    }
    let install = launcher
        .as_deref()
        .and_then(crate::launch_gui::selected_install);
    let saved = navigation
        .as_deref()
        .and_then(|nav| nav.applied.toggles.get(CHECKBOX).copied());
    let result = crate::menu_lighting::with_client_image(install.as_deref(), |label, image| {
        let origin = pcx_play::campaign_inputs::client_identity(image, label);
        let defaults = Defaults::discover(image, origin, crate::campaign_game_input::DISCOVERY)
            .map_err(|e| e.to_string())?;
        enabled(defaults.get(OPTION).map(|entry| entry.fallback), saved)
    })
    .and_then(|on| {
        if !on {
            return Ok(None);
        }
        load_from(
            &candidates(home().as_deref(), install.as_deref()),
            read_file,
        )
    });
    match result {
        Ok(keys) => card.set_custom_keys(keys.unwrap_or_default()),
        Err(error) => {
            card.set_custom_keys(CustomKeys::default());
            input.last_error = Some(error);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn the_saved_checkbox_wins_over_the_client_fallback() {
        assert_eq!(enabled(Some(b"0"), None), Ok(false));
        assert_eq!(enabled(Some(b"1"), None), Ok(true));
        assert_eq!(enabled(Some(b"0"), Some(true)), Ok(true));
        assert_eq!(enabled(Some(b"1"), Some(false)), Ok(false));
        assert_eq!(enabled(None, Some(true)), Ok(true));
        assert!(enabled(None, None).is_err());
        assert!(enabled(Some(b"2"), None).is_err());
        assert!(enabled(Some(&[0xff]), None).is_err());
    }

    #[test]
    #[ignore = "requires PCX_WAR3_EXE, the selected Legacy client"]
    fn the_selected_client_ships_the_option_off() {
        let path = PathBuf::from(std::env::var_os("PCX_WAR3_EXE").expect("set PCX_WAR3_EXE"));
        let image = std::fs::read(&path).unwrap();
        let origin =
            pcx_play::campaign_inputs::client_identity(&image, &path.display().to_string());
        let defaults =
            Defaults::discover(&image, origin, crate::campaign_game_input::DISCOVERY).unwrap();
        let row = defaults.get(OPTION).map(|entry| entry.fallback);
        assert_eq!(enabled(row, None), Ok(false));
    }

    #[test]
    fn the_user_data_copy_is_tried_before_the_install_copy() {
        let paths = candidates(Some(Path::new("home")), Some(Path::new("install")));
        assert_eq!(
            paths,
            [
                Path::new("home/Documents/Warcraft III/CustomKeyBindings/CustomKeys.txt"),
                Path::new("install/CustomKeys.txt"),
            ]
        );
        assert_eq!(
            candidates(None, Some(Path::new("install"))),
            [Path::new("install/CustomKeys.txt")]
        );
        assert!(candidates(None, None).is_empty());
    }

    fn files(
        present: &[(&'static str, &'static [u8])],
    ) -> impl Fn(&Path) -> std::io::Result<Vec<u8>> {
        let present: BTreeMap<PathBuf, &'static [u8]> = present
            .iter()
            .map(|&(path, bytes)| (PathBuf::from(path), bytes))
            .collect();
        move |path| {
            present
                .get(path)
                .map(|bytes| bytes.to_vec())
                .ok_or_else(|| std::io::ErrorKind::NotFound.into())
        }
    }

    #[test]
    fn the_first_existing_file_is_the_only_one_read() {
        let paths = [PathBuf::from("user"), PathBuf::from("install")];
        let both = files(&[
            ("user", b"[hfoo]\nHotkey=T\n"),
            ("install", b"[hfoo]\nHotkey=Y\n"),
        ]);
        let keys = load_from(&paths, both).unwrap().unwrap();
        assert_eq!(
            keys,
            CustomKeys::parse("[hfoo]\nHotkey=T\n").unwrap(),
            "the user copy wins"
        );
        let only_install = files(&[("install", b"[hfoo]\nHotkey=Y\n")]);
        assert_eq!(
            load_from(&paths, only_install).unwrap(),
            Some(CustomKeys::parse("[hfoo]\nHotkey=Y\n").unwrap())
        );
        assert_eq!(load_from(&paths, files(&[])).unwrap(), None);
    }

    #[test]
    fn an_unreadable_or_malformed_file_refuses_with_its_path() {
        let paths = [PathBuf::from("user"), PathBuf::from("install")];
        for bytes in [&b"[hfoo]\nHotkey=t\n"[..], &[0xff, 0xfe][..]] {
            let error = load_from(&paths, files(&[("user", bytes)])).unwrap_err();
            assert!(error.starts_with("user: "), "{error}");
        }
        let denied = |_: &Path| Err(std::io::ErrorKind::PermissionDenied.into());
        assert!(load_from(&paths, denied).unwrap_err().contains("user"));
    }
}
