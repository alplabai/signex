//! Params tab — WS-E PENDING placeholder.
//!
//! The pre-refactor Params tab edited `SharedSide.parameters`, which
//! WS-B replaced with `Revision::parameters` validated against
//! `TemplateRegistry`. WS-E owns rebuilding this against the new
//! per-class template flow. Until then this stub renders only the
//! current parameter list as read-only key=value lines.
//!
//! TODO(merge-with-WS-E): replace with the template-validated grid
//! editor (required vs optional rows, kind pick-list, unit suffix).

use iced::widget::{Space, column, container, scrollable, text};
use iced::{Element, Length};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::super::messages::LibraryMessage;
use super::super::state::ComponentEditorState;

pub fn view<'a>(
    editor: &'a ComponentEditorState,
    library_state: &'a LibraryState,
    tokens: &'a ThemeTokens,
    _window_id: iced::window::Id,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let text_c = theme_ext::text_primary(tokens);

    let mut col = column![
        text("Parameters").size(13).color(text_c),
        Space::new().height(6),
        text("Template-validated parameter editor lives in WS-E.")
            .size(11)
            .color(muted),
        Space::new().height(10),
    ]
    .spacing(2);
    if editor.draft.parameters.is_empty() {
        col = col.push(text("No parameters yet.").size(11).color(muted));
    } else {
        for (k, v) in editor.draft.parameters.iter() {
            col = col.push(
                text(format!("  {} = {:?}", k, v))
                    .size(11)
                    .color(text_c),
            );
        }
    }
    container(scrollable(col).width(Length::Fill).height(Length::Fill))
        .padding(14)
        .style(crate::styles::modal_card(tokens))
        .into()
}
