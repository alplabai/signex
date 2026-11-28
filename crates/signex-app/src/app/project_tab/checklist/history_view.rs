//! The Item pane's History (mockup row 206): one line per thing done to the
//! item, oldest first, each with who did it and on which day. An event a
//! newer build wrote shows its name as written.

use iced::widget::{Column, column};
use iced::{Element, Widget as _};
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::project::{
    ChecklistItem, ChecklistLink, ChecklistStage, HistoryEntry, HistoryEvent, TemplateRef,
};
use signex_widgets::text;

use super::super::look::field_label;
use super::super::table::{SP, TS, note};
use super::view::Ctx;
use super::words::{link_words, stage_words, template_words};
use crate::app::Message;
use crate::styles::ti;

/// What a history line says was done.
pub(super) fn event_words(line: &HistoryEntry, i18n: &I18n) -> Localized {
    let value = line.value.as_deref().unwrap_or_default();
    let Some(event) = line.event() else {
        return t!(i18n, ids::CHECKLIST_HISTORY_OTHER, "event" => line.event.as_str());
    };
    match (event, line.value.is_some()) {
        (HistoryEvent::Created, true) => {
            let template = match TemplateRef::parse(value) {
                Some(TemplateRef::Builtin(template)) => {
                    t!(i18n, template_words(template)).to_string()
                }
                Some(TemplateRef::Saved(name)) => name,
                None => value.to_string(),
            };
            t!(i18n, ids::CHECKLIST_HISTORY_CREATED_FROM, "template" => template.as_str())
        }
        (HistoryEvent::Created, false) => t!(i18n, ids::CHECKLIST_HISTORY_CREATED),
        (HistoryEvent::Imported, _) => {
            t!(i18n, ids::CHECKLIST_HISTORY_IMPORTED, "project" => value)
        }
        (HistoryEvent::Title, _) => t!(i18n, ids::CHECKLIST_HISTORY_TITLE, "title" => value),
        (HistoryEvent::Stage, _) => {
            let stage = ChecklistStage::parse(value)
                .map(|stage| t!(i18n, stage_words(stage)).to_string())
                .unwrap_or_else(|| value.to_string());
            t!(i18n, ids::CHECKLIST_HISTORY_STAGE, "stage" => stage.as_str())
        }
        (HistoryEvent::Owner, true) => t!(i18n, ids::CHECKLIST_HISTORY_OWNER, "owner" => value),
        (HistoryEvent::Owner, false) => t!(i18n, ids::CHECKLIST_HISTORY_OWNER_CLEARED),
        (HistoryEvent::Link, true) => {
            let link = ChecklistLink::parse(value).ok().flatten();
            let words = link_words(link.as_ref(), i18n).to_string();
            t!(i18n, ids::CHECKLIST_HISTORY_LINK, "link" => words.as_str())
        }
        (HistoryEvent::Link, false) => t!(i18n, ids::CHECKLIST_HISTORY_LINK_CLEARED),
        (HistoryEvent::Notes, _) => t!(i18n, ids::CHECKLIST_HISTORY_NOTES),
        (HistoryEvent::Opened, _) => t!(i18n, ids::CHECKLIST_HISTORY_OPENED),
        (HistoryEvent::Done, _) => t!(i18n, ids::CHECKLIST_HISTORY_DONE),
        (HistoryEvent::Waived, _) => t!(i18n, ids::CHECKLIST_HISTORY_WAIVED, "reason" => value),
    }
}

/// Who did it and when, as the line under it says; `None` when neither is
/// known.
pub(super) fn stamp_words(line: &HistoryEntry, i18n: &I18n) -> Option<Localized> {
    match (line.by.as_deref(), line.on.as_deref()) {
        (Some(by), Some(on)) => {
            Some(t!(i18n, ids::CHECKLIST_HISTORY_BY_ON, "by" => by, "on" => on))
        }
        (Some(by), None) => Some(t!(i18n, ids::CHECKLIST_HISTORY_BY, "by" => by)),
        (None, Some(on)) => Some(t!(i18n, ids::CHECKLIST_HISTORY_ON, "on" => on)),
        (None, None) => None,
    }
}

/// The pane's History; `None` for an item with none.
pub(super) fn history<'a>(ctx: &Ctx, item: &ChecklistItem) -> Option<Element<'a, Message>> {
    if item.history.is_empty() {
        return None;
    }
    let (i18n, tokens) = (ctx.i18n, ctx.tokens);
    let mut lines: Column<Element<'a, Message>> = Column::new()
        .spacing(SP.s)
        .push(field_label(t!(i18n, ids::CHECKLIST_HISTORY), tokens));
    for line in &item.history {
        let mut entry: Column<Element<'a, Message>> = column![
            text(event_words(line, i18n))
                .size(TS.body)
                .color(ti(tokens.text_secondary))
        ];
        if let Some(stamp) = stamp_words(line, i18n) {
            entry = entry.push(note(stamp, tokens));
        }
        lines = lines.push_boxed(entry.spacing(SP.xs));
    }
    Some(lines.boxed())
}
