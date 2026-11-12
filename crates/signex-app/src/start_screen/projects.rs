//! The Start screen's project lists: Pinned Projects (#2305), shown only
//! when something is pinned, then Recent Projects. Each row opens its
//! project, opens a menu on right-click, and shows Pin / Unpin on hover; a
//! pinned project missing on disk says so and shows Remove.

use std::path::Path;

use iced::widget::{Space, column, container, hover, mouse_area, row, stack};
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
    let tile = container(Space::new())
        .width(52.0)
        .height(40.0)
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
        text(UserText::new(path.display().to_string()))
            .size(TS.caption)
            .font(mono())
            .color(ink3),
    ]
    .spacing(2);
    let missing = kind == RowKind::Pinned { missing: true };
    if missing {
        details = details.push(
            text(t!(i18n, ids::START_PROJECT_MISSING))
                .size(TS.caption)
                .color(error)
                .boxed(),
        );
    }
    let entry = row![tile, details].spacing(12).align_y(Alignment::Center);
    let open = match kind {
        RowKind::Recent => recent_message(path),
        RowKind::Pinned { .. } => message(StartScreenMsg::PinnedProjectClicked(path.to_path_buf())),
    };
    let base = button(entry)
        .width(Length::Fill)
        .padding([10.0, 14.0])
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
    let (label, action) = row_action(kind, path);
    let action = container(
        button(text(t!(i18n, label)).size(TS.dense))
            .padding([SP.xs, SP.m])
            .on_press(message(action))
            .style(outline_button(tokens)),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding([0.0, SP.l])
    .align_x(iced::alignment::Horizontal::Right)
    .align_y(iced::alignment::Vertical::Center);
    // A missing project keeps its Remove in view; the others show their
    // pin action on hover only, as the mock does.
    if missing {
        stack![base, action].boxed()
    } else {
        hover(base, action).boxed()
    }
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
