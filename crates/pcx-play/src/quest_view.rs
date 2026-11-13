//! Read-only projection of live quest and quest-item state for a future HUD.
//!
//! Title, description, icon path, the five quest flags and item ownership come
//! from the admitted [`QuestSession`].
//! Absent text stays `None`. An assigned empty string stays `Some("")`.
//! Destroyed quests and the items removed with them are not live identities.
//! This projection does not decide victory, group quests, or place any layout.

use crate::quest_session::{QuestFailure, QuestFlag, QuestSession};
use pcx_script::HandleId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemProjection<'a> {
    pub id: HandleId,
    pub description: Option<&'a str>,
    pub completed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuestProjection<'a> {
    pub id: HandleId,
    pub title: Option<&'a str>,
    pub description: Option<&'a str>,
    pub icon_path: Option<&'a str>,
    pub required: bool,
    pub discovered: bool,
    pub completed: bool,
    pub failed: bool,
    pub enabled: bool,
    pub items: Vec<ItemProjection<'a>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuestBoard<'a> {
    pub quests: Vec<QuestProjection<'a>>,
}

pub fn project(session: &QuestSession) -> Result<QuestBoard<'_>, QuestFailure> {
    let mut quests = Vec::new();
    quests
        .try_reserve(session.live_quests())
        .map_err(|_| QuestFailure::Capacity)?;
    for id in session.quest_ids() {
        quests.push(project_quest(session, id)?);
    }
    Ok(QuestBoard { quests })
}

pub fn project_quest(
    session: &QuestSession,
    id: HandleId,
) -> Result<QuestProjection<'_>, QuestFailure> {
    let item_ids = session.quest_items(id)?;
    let mut items = Vec::new();
    items
        .try_reserve(item_ids.len())
        .map_err(|_| QuestFailure::Capacity)?;
    for &item in item_ids {
        let owner = session.item_owner(item)?;
        if owner != id {
            return Err(QuestFailure::State("quest item parent mismatch"));
        }
        items.push(ItemProjection {
            id: item,
            description: checked_text(session.item_description(item)?, "description")?,
            completed: session.item_completed(item)?,
        });
    }
    Ok(QuestProjection {
        id,
        title: checked_text(session.quest_title(id)?, "title")?,
        description: checked_text(session.quest_description(id)?, "description")?,
        icon_path: checked_text(session.quest_icon_path(id)?, "icon_path")?,
        required: session.quest_flag(id, QuestFlag::Required)?,
        discovered: session.quest_flag(id, QuestFlag::Discovered)?,
        completed: session.quest_flag(id, QuestFlag::Completed)?,
        failed: session.quest_flag(id, QuestFlag::Failed)?,
        enabled: session.quest_flag(id, QuestFlag::Enabled)?,
        items,
    })
}

fn checked_text<'a>(
    text: Option<&'a pcx_script::ScriptString>,
    field: &str,
) -> Result<Option<&'a str>, QuestFailure> {
    text.map(|text| {
        text.as_utf8().map_err(|why| {
            QuestFailure::TextSource(format!("quest projection {field}: unsupported UTF8: {why}"))
        })
    })
    .transpose()
}

#[cfg(test)]
#[path = "quest_view_tests.rs"]
mod tests;
