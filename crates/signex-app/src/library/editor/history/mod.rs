//! History tab — list every revision of the component.
//!
//! WS-E (refactor): the visual diff renderer (`diff_view`,
//! `symbol_canvas`, `footprint_canvas`) operated against the old
//! `SchematicSide` / `PcbSide` blobs. WS-F will rewire it against the
//! resolved `Symbol` / `Footprint` primitives. Until then this tab
//! shows the revision list + a textual placeholder for the diff card.

use iced::widget::{Space, button, column, container, scrollable, text};
use iced::{Border, Element, Length, Theme};
use signex_library::Version;
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::super::messages::{EditorMsg, LibraryMessage};
use super::super::state::ComponentEditorState;

pub fn view<'a>(
    editor: &'a ComponentEditorState,
    tokens: &'a ThemeTokens,
    window_id: iced::window::Id,
) -> Element<'a, LibraryMessage> {
    let revs = revision_list(editor, tokens, window_id);
    let diff = diff_placeholder(editor, tokens);

    container(
        column![
            scrollable(revs)
                .height(Length::FillPortion(2))
                .width(Length::Fill),
            Space::new().height(10),
            container(diff)
                .height(Length::FillPortion(3))
                .width(Length::Fill),
        ]
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .style(crate::styles::modal_card(tokens))
    .padding(14)
    .into()
}

fn revision_list<'a>(
    editor: &'a ComponentEditorState,
    tokens: &'a ThemeTokens,
    window_id: iced::window::Id,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let hover = crate::styles::ti(tokens.hover);

    let mut rows = column![].spacing(0);
    for rev in &editor.component.revisions {
        let is_selected = editor.history_selected == Some(rev.version);
        let row_bg = if is_selected { Some(hover) } else { None };
        rows = rows.push(revision_row(
            rev.version,
            rev_label(rev),
            row_bg,
            text_c,
            hover,
            window_id,
        ));
    }
    if editor.component.revisions.is_empty() {
        rows = rows
            .push(container(text("No revisions yet.").size(11).color(muted)).padding([10, 8]));
    }
    rows.into()
}

fn rev_label(rev: &signex_library::Revision) -> String {
    format!(
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
    )
}

fn revision_row<'a>(
    version: Version,
    label: String,
    row_bg: Option<iced::Color>,
    text_c: iced::Color,
    hover: iced::Color,
    window_id: iced::window::Id,
) -> Element<'a, LibraryMessage> {
    button(container(text(label).size(11).color(text_c)).padding([4, 8]))
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
        })
        .into()
}

/// WS-E placeholder for the visual diff card. WS-F will replace this
/// with the side-by-side symbol + footprint canvases driven by
/// `signex_library::diff::diff_revisions`.
fn diff_placeholder<'a>(
    editor: &'a ComponentEditorState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let summary: String = match editor.history_selected {
        Some(version) => match editor
            .component
            .revisions
            .iter()
            .find(|r| r.version == version)
        {
            Some(rev) => format!(
                "Selected v{} — {} (visual diff lands in WS-F).",
                version, rev.message
            ),
            None => format!("v{} not found in history.", version),
        },
        None => "Pick a revision to see its diff against the previous revision.".to_string(),
    };
    container(
        column![
            text("Diff Preview — coming in WS-F")
                .size(13)
                .color(text_c),
            Space::new().height(6),
            text(summary).size(11).color(muted),
        ]
        .spacing(0),
    )
    .padding(14)
    .style(crate::styles::modal_card(tokens))
    .into()
}
