//! The detail strip under the graph for the selected commit: short SHA and
//! subject, the changed files with their status letters, and an actions row
//! the caller fills (Show differences…, Restore this version, Show on board
//! / sheet, the ⋯ menu).

use std::sync::OnceLock;

use iced::widget::text::{Ellipsis, Wrapping};
use iced::widget::{Column, Row, column, container, row, svg};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, Padding, Theme, Widget};
use signex_i18n::{I18n, MessageId, UserText, ids, t};

use super::model::{ChangedFile, FileStatus, GraphCommit};
use super::style::{
    BRANCH_GREEN, DETAIL_FILE_GAP_EM, DETAIL_GAP_EM, DETAIL_HEAD_GAP_EM, DETAIL_MARGIN_EM,
    DETAIL_PAD_EM, HistoryGraphStyle, ICON_BUTTON_EM, ICON_EM, MODIFIED_AMBER, STATUS_LETTER_EM,
};
use crate::history_pane::short_sha;
use crate::theme_ext::{HAIRLINE, Spacing};
use crate::{button, text, tooltip};

/// Three dots: the ⋯ menu glyph.
const MORE_GLYPH: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><circle cx="3.5" cy="8" r="1.3"/><circle cx="8" cy="8" r="1.3"/><circle cx="12.5" cy="8" r="1.3"/></svg>"#;

fn more_glyph() -> svg::Handle {
    static HANDLE: OnceLock<svg::Handle> = OnceLock::new();
    HANDLE
        .get_or_init(|| svg::Handle::from_memory(MORE_GLYPH))
        .clone()
}

fn weighted(weight: iced::font::Weight, family: Font) -> Font {
    Font { weight, ..family }
}

/// The ⋯ button of the actions row (New branch here, Tag, Copy SHA), with
/// its tooltip. The caller opens its menu on `on_press`.
pub fn more_button<'a, M: Clone + 'a>(
    style: &HistoryGraphStyle,
    i18n: &I18n,
    on_press: Option<M>,
) -> Element<'a, M> {
    let s = *style;
    let side = s.em(ICON_BUTTON_EM);
    let glyph = s.em(ICON_EM);
    let ink = s.ink3;
    let icon = svg(more_glyph())
        .width(glyph)
        .height(glyph)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..svg::Style::default()
        });
    let face = button(container(icon).center(side))
        .padding(Padding::ZERO)
        .on_press_maybe(on_press)
        .style(move |_: &Theme, status: button::Status| {
            let hot = matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: hot.then_some(Background::Color(s.hover)),
                text_color: ink,
                border: Border {
                    radius: s.radii.control.into(),
                    ..Border::default()
                },
                ..button::Style::default()
            }
        });
    let tip = container(
        text(t!(i18n, ids::PANELS_HISTORY_MORE))
            .size(s.caption_size)
            .color(s.ink),
    )
    .padding(Padding::from([
        s.em(DETAIL_FILE_GAP_EM),
        s.em(DETAIL_GAP_EM),
    ]))
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(s.panel)),
        border: Border {
            color: s.border,
            width: HAIRLINE,
            radius: s.radii.small.into(),
            ..Border::default()
        },
        ..container::Style::default()
    });
    tooltip(face, tip, tooltip::Position::Top).boxed()
}

/// The catalog id and colour of a status letter.
pub(super) fn status_look(s: &HistoryGraphStyle, status: FileStatus) -> (MessageId, Color) {
    match status {
        FileStatus::Modified => (ids::PANELS_HISTORY_FILE_MODIFIED, MODIFIED_AMBER),
        FileStatus::Added => (ids::PANELS_HISTORY_FILE_ADDED, BRANCH_GREEN),
        FileStatus::Deleted => (ids::PANELS_HISTORY_FILE_DELETED, s.err),
    }
}

fn file_line<'a, M: 'a>(s: &HistoryGraphStyle, i18n: &I18n, file: &ChangedFile) -> Element<'a, M> {
    let (id, ink) = status_look(s, file.status);
    let letter = container(
        text(t!(i18n, id))
            .size(s.caption_size)
            .font(weighted(iced::font::Weight::Semibold, Font::MONOSPACE))
            .color(ink),
    )
    .width(s.em(STATUS_LETTER_EM));
    row![
        letter,
        text(UserText::new(file.path.clone()))
            .size(s.caption_size)
            .font(Font::MONOSPACE)
            .color(s.ink)
            .wrapping(Wrapping::None)
            .ellipsis(Ellipsis::End)
            .width(Length::Fill),
    ]
    .align_y(Alignment::Center)
    .boxed()
}

/// The detail strip of `commit`. `actions` are the caller's buttons, laid
/// out in a wrapping row.
pub fn commit_detail<'a, M: 'a>(
    style: &HistoryGraphStyle,
    i18n: &I18n,
    commit: &GraphCommit,
    files: &[ChangedFile],
    actions: Vec<Element<'a, M>>,
) -> Element<'a, M> {
    let s = *style;
    let head = row![
        text(UserText::new(short_sha(&commit.sha)))
            .size(s.caption_size)
            .font(Font::MONOSPACE)
            .color(s.ink4)
            .wrapping(Wrapping::None),
        text(UserText::new(commit.subject.clone()))
            .size(s.text_size)
            .font(weighted(iced::font::Weight::Medium, Font::DEFAULT))
            .color(s.ink)
            .wrapping(Wrapping::None)
            .ellipsis(Ellipsis::End)
            .width(Length::Fill),
    ]
    .spacing(s.em(DETAIL_HEAD_GAP_EM))
    .align_y(Alignment::Center);

    let file_list: Column<Element<'a, M>> =
        Column::with_children(files.iter().map(|f| file_line(&s, i18n, f)))
            .spacing(s.em(DETAIL_FILE_GAP_EM));

    let action_row = Row::with_children(actions)
        .spacing(s.em(DETAIL_GAP_EM))
        .align_y(Alignment::Center)
        .wrap()
        .vertical_spacing(s.em(DETAIL_GAP_EM));

    let strip = container(column![head, file_list, action_row].spacing(s.em(DETAIL_GAP_EM)))
        .width(Length::Fill)
        .padding(Padding::from([
            s.em(DETAIL_PAD_EM.0),
            s.em(DETAIL_PAD_EM.1),
        ]))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(s.sunken)),
            text_color: Some(s.ink),
            border: Border {
                color: s.border,
                width: HAIRLINE,
                radius: s.radii.button.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    container(strip)
        .width(Length::Fill)
        .padding(Padding::from([
            s.em(DETAIL_MARGIN_EM),
            Spacing::from_base(s.text_size).m,
        ]))
        .boxed()
}
