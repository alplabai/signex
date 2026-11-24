//! The empty state (mockup row 193f): the board area as a drop target with
//! the three ways in, in the order people use them (Open folder… first,
//! a fab package is usually a folder), the accepted files in one line and
//! the last three packages. A package whose files are gone stays listed,
//! greyed, with "Files not found".

use iced::widget::{Column, column, container, row};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{UserText, ids, t};
use signex_widgets::{button, text};

use super::{Look, SP, TS};
use crate::app::gerber_viewer::recent::RecentPackage;
use crate::app::gerber_viewer::state::GerberViewerState;
use crate::app::gerber_viewer::{GerberViewerMsg, OpenKind};
use crate::panels::graphite::hint_tooltip;
use crate::styles::ti;
use crate::styles::tokens::resolve;

/// The width of the empty state's column, in body-text ems.
const COLUMN_EMS: f32 = 34.0;

pub(super) fn view<'a>(
    look: Look<'a>,
    viewer: &'a GerberViewerState,
) -> Element<'a, GerberViewerMsg> {
    let mut lines: Column<Element<'a, GerberViewerMsg>> = column![
        text(look.t(ids::GERBER_VIEWER_EMPTY_TITLE))
            .size(TS.title)
            .color(ti(look.tokens.text))
            .boxed(),
        text(look.t(ids::GERBER_VIEWER_EMPTY_BODY))
            .size(TS.body)
            .color(ti(look.tokens.text_secondary))
            .boxed(),
        actions(look),
        text(look.t(ids::GERBER_VIEWER_ACCEPTED))
            .size(TS.caption)
            .color(resolve(look.tokens).ink3)
            .boxed(),
    ]
    .spacing(SP.m)
    .align_x(Alignment::Center);
    let recent = recent_rows(look, viewer);
    if !recent.is_empty() {
        lines = lines.push(
            text(look.t(ids::GERBER_VIEWER_RECENT))
                .size(TS.section)
                .color(resolve(look.tokens).ink3)
                .boxed(),
        );
        let list = Column::with_children(recent).spacing(SP.xs);
        lines = lines.push(list.boxed());
    }
    container(container(lines).width(Length::Fixed(TS.body * COLUMN_EMS)))
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .boxed()
}

/// Open folder… (primary), Open zip package…, Open files….
fn actions<'a>(look: Look<'a>) -> Element<'a, GerberViewerMsg> {
    let folder = button(text(look.t(ids::GERBER_VIEWER_OPEN_FOLDER)).size(TS.body))
        .padding([SP.xs, SP.m])
        .style(crate::styles::primary_button(look.tokens))
        .on_press(GerberViewerMsg::OpenPressed(OpenKind::Folder));
    let zip = button(text(look.t(ids::GERBER_VIEWER_OPEN_ZIP)).size(TS.body))
        .padding([SP.xs, SP.m])
        .style(crate::styles::ghost_button(look.tokens))
        .on_press(GerberViewerMsg::OpenPressed(OpenKind::Zip));
    let zip = hint_tooltip(
        zip.boxed(),
        look.t(ids::GERBER_VIEWER_OPEN_ZIP_TIP),
        look.tokens,
        iced::widget::tooltip::Position::Bottom,
    );
    let files = button(text(look.t(ids::GERBER_VIEWER_OPEN_FILES)).size(TS.body))
        .padding([SP.xs, SP.m])
        .style(crate::styles::ghost_button(look.tokens))
        .on_press(GerberViewerMsg::OpenPressed(OpenKind::Files));
    row![folder.boxed(), zip, files.boxed()]
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .boxed()
}

/// The recent packages the empty state and the Open menu list.
pub(super) fn recent_rows<'a>(
    look: Look<'a>,
    viewer: &'a GerberViewerState,
) -> Vec<Element<'a, GerberViewerMsg>> {
    viewer
        .recent
        .shown()
        .iter()
        .enumerate()
        .map(|(index, entry)| recent_row(look, viewer, index, entry))
        .collect()
}

/// One package: its path, its layer count and when it was opened, or
/// "Files not found" greyed and deaf.
fn recent_row<'a>(
    look: Look<'a>,
    viewer: &GerberViewerState,
    index: usize,
    entry: &RecentPackage,
) -> Element<'a, GerberViewerMsg> {
    let r = resolve(look.tokens);
    let ink = if entry.missing {
        r.ink4
    } else {
        ti(look.tokens.text)
    };
    let detail = if entry.missing {
        look.t(ids::GERBER_VIEWER_RECENT_MISSING)
    } else {
        t!(look.i18n, ids::GERBER_VIEWER_RECENT_LAYERS, "count" => entry.layers)
    };
    let when = viewer
        .now
        .and_then(|now| chrono::DateTime::<chrono::Utc>::from_timestamp(now, 0))
        .map(|now| crate::start_screen::opened_label(look.i18n, Some(entry.opened_at), now));
    let mut line = row![
        text(package_label(look, entry))
            .size(TS.dense)
            .color(ink)
            .width(Length::Fill)
            .boxed(),
        text(detail).size(TS.caption).color(r.ink3).boxed(),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);
    if let Some(when) = when {
        line = line.push(text(when).size(TS.caption).color(r.ink3).boxed());
    }
    button(line)
        .padding([SP.xs, SP.s])
        .width(Length::Fill)
        .style(crate::styles::ghost_button(look.tokens))
        .on_press_maybe((!entry.missing).then_some(GerberViewerMsg::RecentPressed(index)))
        .boxed()
}

/// The package's path as the row reads it: the one path, or the first and
/// how many more.
fn package_label(look: Look<'_>, entry: &RecentPackage) -> signex_widgets::Label<'static> {
    let first = entry
        .paths
        .first()
        .map(|path| path.display().to_string())
        .unwrap_or_default();
    match entry.paths.len() {
        0 | 1 => UserText::new(first).into(),
        n => {
            t!(look.i18n, ids::GERBER_VIEWER_RECENT_MORE, "path" => first, "count" => n - 1).into()
        }
    }
}
