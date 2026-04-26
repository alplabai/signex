//! "Submit for Review" modal.
//!
//! UI-WS7 spec:
//! - Triggered by the Component Editor footer's "Submit for Review"
//!   button (only visible when `manifest.workflow.review_required`).
//! - Free-form text input for reviewer notes.
//! - Submit / Cancel buttons.
//! - On Submit: dispatches `EditorMsg::SubmitForReviewConfirm` which
//!   the dispatcher routes to `LocalGitAdapter::save_revision` with
//!   `revision.state = LifecycleState::InReview` — the local-git
//!   adapter routes to `review/<uuid>` automatically (already wired
//!   in WS-A).
//!
//! State lives on `ComponentEditorState.review_dialog` so the modal
//! can be carried across re-renders without a global modal slot.

use iced::widget::{Space, button, column, container, row, text, text_input};
use iced::{Border, Element, Length, Theme};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::super::messages::{EditorMsg, LibraryMessage};
use super::super::state::ComponentEditorState;

/// Width / height of the modal card. Sized to fit a ~6-line notes
/// field comfortably; the text_input itself drives the actual rows
/// of input (single-line, but wide enough that long messages don't
/// look cramped).
const MODAL_W: f32 = 520.0;

/// Render the modal centred on a dim backdrop. Caller wraps the
/// returned widget in the standard overlay container — we return the
/// card itself so the view layer can compose it the same way as the
/// picker modal.
pub fn view<'a>(
    editor: &'a ComponentEditorState,
    tokens: &'a ThemeTokens,
    window_id: iced::window::Id,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    let header = container(
        row![
            text("Submit for Review").size(14).color(text_c),
            Space::new().width(Length::Fill),
            close_x(window_id, tokens),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([10, 14])
    .style(crate::styles::modal_header_strip(tokens));

    // Header context — the user wants to know what they're submitting.
    let target_line = format!(
        "Submitting {} v{}  →  review/{}",
        editor.display_internal_pn, editor.displayed_version, editor.component_id,
    );

    let body = column![
        text(target_line).size(11).color(muted),
        Space::new().height(8),
        text("Reviewer notes").size(11).color(text_c),
        Space::new().height(4),
        text_input(
            "What changed? What should the reviewer focus on?",
            &editor.review_notes_buf,
        )
        .on_input(move |s| LibraryMessage::EditorEvent {
            window_id,
            msg: EditorMsg::SubmitForReviewNotesChanged(s),
        })
        .padding(8)
        .size(12),
        Space::new().height(8),
        text(
            "Submitting routes the draft to the `review/<uuid>` branch in the library's git \
             repo. The current draft state will be set to InReview before commit.",
        )
        .size(10)
        .color(muted),
    ]
    .spacing(0)
    .padding([14, 14]);

    let footer = container(
        row![
            // Status / error line on the left.
            {
                let status = editor.review_status.as_deref().unwrap_or("");
                text(status.to_string()).size(10).color(muted)
            },
            Space::new().width(Length::Fill),
            secondary_btn(
                "Cancel",
                LibraryMessage::EditorEvent {
                    window_id,
                    msg: EditorMsg::SubmitForReviewCancel,
                },
                text_c,
                border,
            ),
            Space::new().width(8),
            primary_btn(
                "Submit",
                LibraryMessage::EditorEvent {
                    window_id,
                    msg: EditorMsg::SubmitForReviewConfirm,
                },
            ),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([10, 14])
    .style(crate::styles::modal_footer_strip(tokens));

    container(column![header, body, footer].spacing(0).width(MODAL_W))
        .width(MODAL_W)
        .style(crate::styles::modal_card(tokens))
        .into()
}

fn close_x<'a>(window_id: iced::window::Id, tokens: &ThemeTokens) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);
    button(container(text("\u{00D7}".to_string()).size(14).color(text_c)).padding([0, 6]))
        .on_press(LibraryMessage::EditorEvent {
            window_id,
            msg: EditorMsg::SubmitForReviewCancel,
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

fn primary_btn<'a>(label: &'a str, message: LibraryMessage) -> Element<'a, LibraryMessage> {
    button(container(text(label.to_string()).size(11).color(iced::Color::WHITE)).padding([4, 14]))
        .on_press(message)
        .style(|_: &Theme, _| iced::widget::button::Style {
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
        .into()
}

fn secondary_btn<'a>(
    label: &'a str,
    message: LibraryMessage,
    text_c: iced::Color,
    border: iced::Color,
) -> Element<'a, LibraryMessage> {
    button(container(text(label.to_string()).size(11).color(text_c)).padding([4, 14]))
        .on_press(message)
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
        .into()
}
