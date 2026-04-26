//! History tab — list every revision of the component.
//!
//! Phase 1 ships the row list + a placeholder diff card. The data
//! side of `signex_library::diff::diff_revisions` is already shipped;
//! Phase 2 wires the visual 2D renderer into the placeholder card.

use iced::widget::{Space, button, column, container, scrollable, text};
use iced::{Border, Element, Length, Theme};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::super::messages::{EditorMsg, LibraryMessage};
use super::super::state::ComponentEditorState;

pub fn view<'a>(
    editor: &'a ComponentEditorState,
    tokens: &'a ThemeTokens,
    window_id: iced::window::Id,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);
    let hover = crate::styles::ti(tokens.hover);

    let mut rows = column![].spacing(0);
    for rev in &editor.component.revisions {
        let is_selected = editor.history_selected == Some(rev.version);
        let row_bg = if is_selected { Some(hover) } else { None };
        let label = format!(
            "v{}   {:?}   {}   — {}",
            rev.version,
            rev.state,
            if rev.author.is_empty() {
                "(unknown)"
            } else {
                rev.author.as_str()
            },
            if rev.message.is_empty() {
                "(no message)"
            } else {
                rev.message.as_str()
            },
        );
        let version = rev.version;
        let row_btn = button(container(text(label).size(11).color(text_c)).padding([4, 8]))
            .padding(0)
            .width(Length::Fill)
            .on_press(LibraryMessage::EditorEvent {
                window_id,
                msg: EditorMsg::HistorySelectRevision(version),
            })
            .style(move |_: &Theme, status: iced::widget::button::Status| {
                let bg = match (row_bg, status) {
                    (Some(c), _) => Some(iced::Background::Color(c)),
                    (None, iced::widget::button::Status::Hovered) => {
                        Some(iced::Background::Color(hover))
                    }
                    _ => None,
                };
                iced::widget::button::Style {
                    background: bg,
                    text_color: text_c,
                    border: Border::default(),
                    ..iced::widget::button::Style::default()
                }
            });
        rows = rows.push(row_btn);
    }
    if editor.component.revisions.is_empty() {
        rows =
            rows.push(container(text("No revisions yet.").size(11).color(muted)).padding([10, 8]));
    }

    let placeholder = container(
        column![
            text("Visual diff — Coming in Phase 2")
                .size(13)
                .color(text_c),
            Space::new().height(6),
            text(
                "TODO(v0.9-phase-2): wire signex_library::diff::diff_revisions output into a 2D \
                 schematic / footprint diff renderer. The data side already exists.",
            )
            .size(11)
            .color(muted),
        ]
        .spacing(0),
    )
    .padding(14)
    .style(move |_: &Theme| iced::widget::container::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgba(
            1.0, 1.0, 1.0, 0.02,
        ))),
        border: Border {
            width: 1.0,
            radius: 4.0.into(),
            color: border,
        },
        ..Default::default()
    });

    container(
        column![
            scrollable(rows)
                .height(Length::FillPortion(3))
                .width(Length::Fill),
            Space::new().height(10),
            container(placeholder).height(Length::FillPortion(2)),
        ]
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .style(crate::styles::modal_card(tokens))
    .padding(14)
    .into()
}
