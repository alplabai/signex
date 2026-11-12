//! The Start screen's project lists: Pinned Projects (#2305), shown only
//! when something is pinned, then Recent Projects. Each row opens its
//! project, opens a menu on right-click, and shows Pin / Unpin on hover; a
//! pinned project missing on disk says so and shows Remove. Recent Projects
//! is hidden when every recent project is pinned.

use std::path::Path;

use iced::widget::{Space, column, container, hover, mouse_area, row};
use iced::{Alignment, Background, Border, Element, Length, Widget as _};
use signex_i18n::{I18n, MessageId, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text};

use super::graphite::{mono, outline_button, section_header};
use super::{ProjectSections, StartScreenMsg, message, recent_label, recent_message};
use crate::app::Message;
use crate::pinned_projects::PinnedProjects;
use crate::styles::{
    Radii, Spacing, TypeScale, ti,
    tokens::{hover_overlay, resolve},
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;
/// The mock's empty thumbnail tile (52 x 40 at the default size).
const TILE_W: f32 = SP.xl * 3.0 + SP.s;
const TILE_H: f32 = SP.xl * 2.0 + SP.m;
/// Row padding (10 x 14 at the default size).
const ROW_PAD: [f32; 2] = [SP.m + SP.xs, SP.l + SP.xs];

/// Which list a row is in, and for a pinned row whether it is missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowKind {
    Recent,
    Pinned { missing: bool },
}

/// The section headings and lists, in screen order.
pub(super) fn sections<'a>(
    tokens: &ThemeTokens,
    i18n: &I18n,
    sections: &ProjectSections<'a>,
    pinned: &PinnedProjects,
) -> Vec<Element<'a, Message>> {
    let mut out = Vec::with_capacity(5);
    if !sections.pinned.is_empty() {
        let rows = sections
            .pinned
            .iter()
            .map(|path| {
                let missing = pinned.is_missing(path);
                (path.as_path(), RowKind::Pinned { missing })
            })
            .collect();
        out.push(section_header(tokens, t!(i18n, ids::START_PINNED_PROJECTS)));
        out.push(list(tokens, i18n, rows));
    }
    // Every recent project is pinned: an empty Recent section would claim
    // there are none.
    if !sections.show_recent {
        return out;
    }
    if !sections.pinned.is_empty() {
        out.push(Space::new().height(SP.m).boxed());
    }
    out.push(section_header(tokens, t!(i18n, ids::START_RECENT_PROJECTS)));
    if sections.recent.is_empty() {
        out.push(crate::panels::empty_state_card(
            tokens,
            t!(i18n, ids::START_NO_RECENT_TITLE),
            t!(i18n, ids::START_NO_RECENT_BODY),
            None,
        ));
    } else {
        let rows = sections
            .recent
            .iter()
            .map(|path| (path.as_path(), RowKind::Recent))
            .collect();
        out.push(list(tokens, i18n, rows));
    }
    out
}

/// One card holding `rows`, with a hairline between rows.
fn list<'a>(
    tokens: &ThemeTokens,
    i18n: &I18n,
    rows: Vec<(&Path, RowKind)>,
) -> Element<'a, Message> {
    let line = resolve(tokens).line_soft;
    let count = rows.len();
    let mut list = column![].width(Length::Fill);
    for (at, (path, kind)) in rows.into_iter().enumerate() {
        list = list.push(project_row(tokens, i18n, path, kind));
        if at + 1 < count {
            list = list.push(
                container(Space::new().height(1.0))
                    .width(Length::Fill)
                    .style(move |_: &iced::Theme| container::Style {
                        background: Some(Background::Color(line)),
                        ..container::Style::default()
                    })
                    .boxed(),
            );
        }
    }
    container(list)
        .width(Length::Fill)
        .padding(1)
        .style(crate::styles::card(tokens))
        .boxed()
}

fn project_row<'a>(
    tokens: &ThemeTokens,
    i18n: &I18n,
    path: &Path,
    kind: RowKind,
) -> Element<'a, Message> {
    let (ink, ink3, error) = (ti(tokens.text), resolve(tokens).ink3, ti(tokens.error));
    let (hover_fill, tile_fill) = (hover_overlay(tokens), resolve(tokens).sunken);
    // No cheap real thumbnail exists, so the mock's tile is drawn empty.
    let tile =
        container(Space::new())
            .width(TILE_W)
            .height(TILE_H)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(Background::Color(tile_fill)),
                border: Border {
                    radius: Radii::GRAPHITE.small.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            });
    let mut details = column![
        text(UserText::new(recent_label(path)))
            .size(TS.label)
            .color(ink),
        // A long path ends in an ellipsis rather than running under the
        // row's action button.
        text(UserText::new(path.display().to_string()))
            .size(TS.caption)
            .font(mono())
            .color(ink3)
            .wrapping(iced::widget::text::Wrapping::None)
            .ellipsis(iced::widget::text::Ellipsis::End)
            .width(Length::Fill),
    ]
    .spacing(SP.xs)
    .width(Length::Fill);
    let missing = kind == RowKind::Pinned { missing: true };
    if missing {
        details = details.push(
            text(t!(i18n, ids::START_PROJECT_MISSING))
                .size(TS.caption)
                .color(error)
                .boxed(),
        );
    }
    let (label, action) = row_action(kind, path);
    let action = button(text(t!(i18n, label)).size(TS.dense))
        .padding([SP.xs, SP.m])
        .on_press(message(action))
        .style(outline_button(tokens))
        .boxed();
    let mut entry = row![tile, details].spacing(SP.l).align_y(Alignment::Center);
    // A missing project keeps Remove in view, in its own column so the text
    // never runs under it; the inner button takes the press before the row.
    let action = if missing {
        entry = entry.push(action);
        None
    } else {
        Some(action)
    };
    let open = match kind {
        RowKind::Recent => recent_message(path),
        RowKind::Pinned { .. } => message(StartScreenMsg::PinnedProjectClicked(path.to_path_buf())),
    };
    let base = button(entry)
        .width(Length::Fill)
        .padding(ROW_PAD)
        .on_press(open)
        .style(move |_: &iced::Theme, status| button::Style {
            background: matches!(status, button::Status::Hovered | button::Status::Pressed)
                .then_some(Background::Color(hover_fill)),
            text_color: ink,
            border: Border {
                radius: Radii::GRAPHITE.small.into(),
                ..Border::default()
            },
            ..button::Style::default()
        });
    let base = mouse_area(base)
        .on_right_press(message(StartScreenMsg::RowRightClicked(path.to_path_buf())))
        .boxed();
    let Some(action) = action else {
        return base;
    };
    // The pin action shows on hover only, as the mock does.
    let action = container(action)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding([0.0, SP.l])
        .align_x(iced::alignment::Horizontal::Right)
        .align_y(iced::alignment::Vertical::Center);
    hover(base, action).boxed()
}

/// The row's own action: its label and the event it sends.
fn row_action(kind: RowKind, path: &Path) -> (MessageId, StartScreenMsg) {
    let path = path.to_path_buf();
    match kind {
        RowKind::Recent => (ids::START_PIN, StartScreenMsg::ProjectPinned(path)),
        RowKind::Pinned { missing: false } => {
            (ids::START_UNPIN, StartScreenMsg::ProjectUnpinned(path))
        }
        RowKind::Pinned { missing: true } => {
            (ids::START_REMOVE, StartScreenMsg::ProjectUnpinned(path))
        }
    }
}
