//! Component Editor — multi-window tabbed surface, one window per
//! open component. Each window's state lives in
//! `LibraryState.open_editors` keyed by `iced::window::Id`.
//!
//! See LIBRARY_PLAN §10 for the spec.

pub mod footprint;
pub mod history;
pub mod overview;
pub mod params;
pub mod sim;
pub mod supply;
pub mod symbol;
pub mod tabs;
pub mod three_d;
pub mod where_used;

use iced::widget::{Space, button, column, container, row, text};
use iced::{Border, Element, Length, Theme};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::messages::{EditorMsg, LibraryMessage};
use super::state::{ComponentEditorState, EditorTab};

/// Render a Component Editor window. The caller hosts this in the
/// new OS window opened via `iced::window::open`.
pub fn view<'a>(
    editor: &'a ComponentEditorState,
    tokens: &'a ThemeTokens,
    window_id: iced::window::Id,
) -> Element<'a, LibraryMessage> {
    let header = view_header(editor, tokens, window_id);
    let tabs = tabs::view(editor.active_tab, tokens, window_id);
    let body = view_active_tab(editor, tokens, window_id);
    let footer = view_footer(editor, tokens, window_id);

    column![header, tabs, body, footer]
        .spacing(0)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn view_header<'a>(
    editor: &'a ComponentEditorState,
    tokens: &'a ThemeTokens,
    window_id: iced::window::Id,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let title = format!(
        "{}  v{}  —  {:?}",
        editor.display_internal_pn, editor.displayed_version, editor.draft.state,
    );
    container(
        row![
            text("Component Editor — ").size(13).color(muted),
            text(title).size(13).color(text_c),
            Space::new().width(Length::Fill),
            // No native lock badge in Phase 1; placeholder to mirror
            // the LIBRARY_PLAN §10 reference layout.
            text("\u{1F512}").size(13).color(muted),
            Space::new().width(8),
            close_btn(window_id, tokens),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([8, 14])
    .style(crate::styles::modal_header_strip(tokens))
    .into()
}

fn view_active_tab<'a>(
    editor: &'a ComponentEditorState,
    tokens: &'a ThemeTokens,
    window_id: iced::window::Id,
) -> Element<'a, LibraryMessage> {
    let inner = match editor.active_tab {
        EditorTab::Overview => overview::view(editor, tokens, window_id),
        EditorTab::Symbol => symbol::view(tokens),
        EditorTab::Footprint => footprint::view(editor, tokens, window_id),
        EditorTab::ThreeD => three_d::view(tokens),
        EditorTab::Params => params::view(editor, tokens, window_id),
        EditorTab::Supply => supply::view(editor, tokens, window_id),
        EditorTab::Sim => sim::view(tokens),
        EditorTab::History => history::view(editor, tokens, window_id),
        EditorTab::WhereUsed => where_used::view(editor, tokens),
    };
    container(inner)
        .padding(14)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn view_footer<'a>(
    editor: &'a ComponentEditorState,
    tokens: &'a ThemeTokens,
    window_id: iced::window::Id,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let border = theme_ext::border_color(tokens);

    let primary = |label: &'static str, msg: EditorMsg| {
        button(container(text(label).size(11).color(iced::Color::WHITE)).padding([4, 14]))
            .on_press(LibraryMessage::EditorEvent { window_id, msg })
            .style(move |_: &Theme, _| iced::widget::button::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgb(
                    0.00, 0.47, 0.84,
                ))),
                text_color: iced::Color::WHITE,
                border: Border {
                    width: 0.0,
                    radius: 3.0.into(),
                    ..Border::default()
                },
                ..iced::widget::button::Style::default()
            })
    };
    let secondary = |label: &'static str, msg: EditorMsg| {
        button(container(text(label).size(11).color(text_c)).padding([4, 14]))
            .on_press(LibraryMessage::EditorEvent { window_id, msg })
            .style(move |_: &Theme, _| iced::widget::button::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    1.0, 1.0, 1.0, 0.04,
                ))),
                text_color: text_c,
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: border,
                },
                ..iced::widget::button::Style::default()
            })
    };

    let mut footer_row = row![
        secondary("Save Draft", EditorMsg::SaveDraft),
        Space::new().width(8),
        primary("Commit", EditorMsg::Commit),
    ]
    .align_y(iced::Alignment::Center);
    if editor.review_required {
        footer_row = footer_row.push(Space::new().width(8));
        footer_row = footer_row.push(secondary("Submit for Review", EditorMsg::SubmitForReview));
    }
    footer_row = footer_row.push(Space::new().width(Length::Fill));
    footer_row = footer_row.push(secondary("Where Used", EditorMsg::OpenWhereUsedTab));

    container(footer_row)
        .padding([10, 14])
        .style(crate::styles::modal_footer_strip(tokens))
        .into()
}

fn close_btn<'a>(window_id: iced::window::Id, tokens: &ThemeTokens) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);
    button(container(text("\u{00D7}".to_string()).size(14).color(text_c)).padding([0, 6]))
        .on_press(LibraryMessage::EditorEvent {
            window_id,
            msg: EditorMsg::CloseEditor,
        })
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let bg = match status {
                iced::widget::button::Status::Hovered => Some(iced::Background::Color(
                    iced::Color::from_rgba(1.0, 1.0, 1.0, 0.10),
                )),
                _ => Some(iced::Background::Color(iced::Color::from_rgba(
                    1.0, 1.0, 1.0, 0.03,
                ))),
            };
            iced::widget::button::Style {
                background: bg,
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: border,
                },
                text_color: text_c,
                ..iced::widget::button::Style::default()
            }
        })
        .into()
}

/// Helper used by every placeholder tab — shared message and TODO list
/// pulled from LIBRARY_PLAN §10.
pub(crate) fn placeholder_card<'a>(
    title: &'a str,
    todos: &'a [&'a str],
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let mut col = column![
        text(format!("{title} — Coming in Phase 2"))
            .size(14)
            .color(text_c),
        Space::new().height(8),
    ]
    .spacing(4);
    for todo in todos {
        col = col.push(text(format!("• {todo}")).size(11).color(muted));
    }
    container(col)
        .padding(14)
        .style(crate::styles::modal_card(tokens))
        .into()
}
