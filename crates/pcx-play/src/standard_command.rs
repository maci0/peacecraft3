//! Standard command presentation from the selected CommandFunc/CommandStrings profiles.
use crate::ability_profile::Tooltip;
use pcx_objects::editor::Editor;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Move,
    Stop,
    Hold,
    Patrol,
    Attack,
}

impl Kind {
    pub const ALL: [Self; 5] = [
        Self::Move,
        Self::Stop,
        Self::Hold,
        Self::Patrol,
        Self::Attack,
    ];

    pub fn profile(self) -> &'static str {
        match self {
            Self::Move => "CmdMove",
            Self::Stop => "CmdStop",
            Self::Hold => "CmdHoldPos",
            Self::Patrol => "CmdPatrol",
            Self::Attack => "CmdAttack",
        }
    }

    pub fn immediate(self) -> bool {
        matches!(self, Self::Stop | Self::Hold)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    pub kind: Kind,
    /// A skin key, not a texture path. Resolve it in the selected race's skin chain.
    pub art_key: String,
    pub position: [usize; 2],
    pub hotkey: char,
    pub tooltip: Tooltip,
}

impl Command {
    pub fn read(editor: &Editor, kind: Kind) -> Result<Self, String> {
        let name = kind.profile();
        let row = editor
            .command(name)
            .ok_or_else(|| format!("missing command profile {name}"))?;
        let value = |key| -> Result<&str, String> {
            match row.values(key) {
                [value] if !value.trim().is_empty() && value.len() <= 64 << 10 => Ok(value.trim()),
                _ => Err(format!(
                    "{name}.{key}: missing, ambiguous or oversized value"
                )),
            }
        };
        let position = value("Buttonpos")?
            .split(',')
            .map(|v| v.trim().parse::<usize>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("{name}.Buttonpos: {e}"))?
            .try_into()
            .map_err(|_| format!("{name}.Buttonpos requires two coordinates"))?;
        let mut hotkey = value("Hotkey")?.chars();
        let key = hotkey.next().expect("nonempty");
        if hotkey.next().is_some() || !key.is_ascii_alphabetic() {
            return Err(format!("{name}.Hotkey: unsupported non-letter binding"));
        }
        let title = value("Tip")?;
        let description = value("Ubertip")?;
        if [title, description]
            .iter()
            .any(|v| v.starts_with("TRIGSTR_") || v.contains(['<', '>']))
        {
            return Err(format!("{name}: unresolved tooltip reference"));
        }
        Ok(Self {
            kind,
            art_key: value("Art")?.into(),
            position,
            hotkey: key,
            tooltip: Tooltip {
                title: title.into(),
                description: description.into(),
            },
        })
    }
}

/// A command-grid button that opens or closes a submenu. Hotkey text is taken
/// from the profile: one letter, or a decimal code such as the cancel binding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuButton {
    pub art_key: String,
    pub position: [usize; 2],
    pub hotkey: MenuHotkey,
    pub tooltip: Tooltip,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuHotkey {
    Letter(char),
    Code(i32),
}

impl MenuButton {
    pub fn read(editor: &Editor, name: &str) -> Result<Self, String> {
        let row = editor
            .command(name)
            .ok_or_else(|| format!("missing command profile {name}"))?;
        let value = |key| -> Result<&str, String> {
            match row.values(key) {
                [value] if !value.trim().is_empty() && value.len() <= 64 << 10 => Ok(value.trim()),
                _ => Err(format!(
                    "{name}.{key}: missing, ambiguous or oversized value"
                )),
            }
        };
        let position = value("Buttonpos")?
            .split(',')
            .map(|part| part.trim().parse::<usize>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("{name}.Buttonpos: {error}"))?
            .try_into()
            .map_err(|_| format!("{name}.Buttonpos requires two coordinates"))?;
        let raw_hotkey = value("Hotkey")?;
        let hotkey = if raw_hotkey.chars().count() == 1
            && raw_hotkey
                .chars()
                .next()
                .is_some_and(|key| key.is_ascii_alphabetic())
        {
            MenuHotkey::Letter(raw_hotkey.chars().next().expect("one char"))
        } else if let Ok(code) = raw_hotkey.parse::<i32>() {
            if code < 0 {
                return Err(format!("{name}.Hotkey: negative code"));
            }
            MenuHotkey::Code(code)
        } else {
            return Err(format!("{name}.Hotkey: unsupported binding"));
        };
        let title = value("Tip")?;
        let description = value("Ubertip")?;
        if [title, description]
            .iter()
            .any(|text| text.starts_with("TRIGSTR_") || text.contains(['<', '>']))
        {
            return Err(format!("{name}: unresolved tooltip reference"));
        }
        Ok(Self {
            art_key: value("Art")?.into(),
            position,
            hotkey,
            tooltip: Tooltip {
                title: title.into(),
                description: description.into(),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pcx_objects::editor::EditorReadLimits;

    fn tables(profile: &str) -> Editor {
        Editor::from_reader(
            std::path::Path::new("synthetic command fixture"),
            &["Units\\CommandFunc.txt".into()],
            EditorReadLimits {
                max_listed_paths: 4,
                max_asset_bytes: 4096,
                max_aggregate_input_bytes: 8192,
            },
            |path| {
                Ok::<_, std::io::Error>(
                    (path == "Units\\CommandFunc.txt").then(|| profile.as_bytes().to_vec()),
                )
            },
        )
        .unwrap()
    }

    #[test]
    fn reads_authored_fields_and_refuses_missing_ambiguous_or_unhandled_values() {
        let profile = "[CmdMove]\nArt=FixtureMove\nButtonpos=2,1\nHotkey=z\nTip=Walk\nUbertip=Go to the indicated point.\n";
        let source = tables(profile);
        let command = Command::read(&source, Kind::Move).unwrap();
        assert_eq!(command.art_key, "FixtureMove");
        assert_eq!(command.position, [2, 1]);
        assert_eq!(command.hotkey, 'z');
        assert_eq!(command.tooltip.title, "Walk");
        for (old, replacement) in [
            ("Art=FixtureMove", "Art="),
            ("Buttonpos=2,1", "Buttonpos=2"),
            ("Buttonpos=2,1", "Buttonpos=-1,0"),
            ("Hotkey=z", "Hotkey=27"),
            ("Tip=Walk", "Tip=TRIGSTR_2"),
            ("Art=FixtureMove", "Art=FixtureMove\nArt=Other"),
        ] {
            assert!(
                Command::read(&tables(&profile.replace(old, replacement)), Kind::Move).is_err()
            );
        }
        assert!(Command::read(&source, Kind::Stop).is_err());
    }
}
