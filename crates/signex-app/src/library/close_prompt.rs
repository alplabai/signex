//! "Close Library — Unsaved Drafts" confirmation modal.
//!
//! Mirrors the dock subsystem's `ProjectCloseConfirm` flow (see
//! `app/view/dialogs.rs::view_project_close_confirm_body`) — when
//! the user closes a library that still has at least one editor
//! window with `dirty = true`, the dispatcher diverts to
//! [`crate::library::messages::LibraryMessage::ConfirmCloseLibrary`]
//! and shows this modal listing every dirty draft so the user can
//! Save All / Discard All / Cancel.
//!
//! Reuses the picker modal's chrome (`modal_card` /
//! `modal_header_strip` / `modal_footer_strip`) for visual parity
//! across the Library subsystem.

use iced::widget::{Space, column, container, row, scrollable, text};
use iced::{Element, Length};
use signex_types::theme::ThemeTokens;

use crate::styles::{Spacing, TypeScale};
use signex_widgets::theme_ext;

use super::messages::{CloseLibraryChoice, LibraryMessage};
use super::modal_chrome::{close_x, primary_btn, secondary_btn};
use super::state::{CloseLibraryConfirmState, LibraryState};

/// How a confirm modal names an unsaved file: relative to its library's
/// folder when it has one (`vendor/symbols/s.snxsym`, so same-named drafts of
/// different folders differ), else its file name.
pub(crate) fn dirty_path_label(
    path: &std::path::Path,
    library_dir: Option<&std::path::Path>,
) -> String {
    let relative = library_dir.and_then(|dir| path.strip_prefix(dir).ok());
    match relative {
        Some(rest) => rest
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/"),
        None => path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("(unnamed)")
            .to_string(),
    }
}

pub fn view<'a>(
    state: &'a LibraryState,
    confirm: &'a CloseLibraryConfirmState,
    tokens: &'a ThemeTokens,
    windowed: bool,
) -> Element<'a, LibraryMessage> {
    let ts = TypeScale::default();
    let sp = Spacing::default();
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);

    let header = container(
        row![
            text("Close Library — Unsaved Drafts")
                .size(ts.heading)
                .color(text_c),
            Space::new().width(Length::Fill),
            close_x(
                LibraryMessage::CloseLibraryConfirm(CloseLibraryChoice::Cancel),
                tokens,
            ),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([sp.m, sp.l])
    .style(crate::styles::modal_header_strip(tokens));

    let summary = text(format!(
        "'{}' has {} unsaved draft(s). What would you like to do?",
        confirm.library_name,
        confirm.dirty_editors.len() + confirm.dirty_paths.len()
    ))
    .size(ts.body)
    .color(muted);

    // List of dirty drafts by display_internal_pn. We look each one
    // up in `state.editors` (keyed by `EditorAddress`) so the modal
    // stays accurate even if the user opens / closes other editors
    // while it's up.
    let mut rows: Vec<Element<'_, LibraryMessage>> = Vec::new();
    for address in &confirm.dirty_editors {
        let label = state
            .editors
            .get(address)
            .map(|st| st.row.internal_pn.as_str().to_string())
            .unwrap_or_else(|| format!("({} / {})", address.table, address.row_id));
        rows.push(
            container(text(label).size(ts.body).color(text_c))
                .padding([sp.s, sp.m])
                .into(),
        );
    }
    let library_dir = confirm.library_path.parent();
    for path in &confirm.dirty_paths {
        rows.push(
            container(
                text(dirty_path_label(path, library_dir))
                    .size(ts.body)
                    .color(text_c),
            )
            .padding([sp.s, sp.m])
            .into(),
        );
    }
    let list = scrollable(column(rows).spacing(0))
        .height(Length::Fixed(140.0))
        .width(Length::Fill);

    let body = column![
        container(summary).padding([sp.l, sp.xl]),
        container(list).padding([0.0, sp.xl]),
    ]
    .spacing(0);

    let footer = container(
        row![
            Space::new().width(Length::Fill),
            secondary_btn(
                "Cancel",
                LibraryMessage::CloseLibraryConfirm(CloseLibraryChoice::Cancel),
                tokens,
            ),
            Space::new().width(sp.m),
            secondary_btn(
                "Discard All",
                LibraryMessage::CloseLibraryConfirm(CloseLibraryChoice::DiscardAll),
                tokens,
            ),
            Space::new().width(sp.m),
            primary_btn(
                "Save All",
                LibraryMessage::CloseLibraryConfirm(CloseLibraryChoice::SaveAll),
                tokens,
            ),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([sp.m, sp.l])
    .style(crate::styles::modal_footer_strip(tokens));

    container(
        column![header, body, Space::new().height(Length::Fill), footer]
            .width(super::card_len(
                windowed,
                super::modal_chrome::CLOSE_PROMPT_W,
            ))
            .height(super::card_len(
                windowed,
                super::modal_chrome::CLOSE_PROMPT_H,
            )),
    )
    .style(crate::styles::modal_card(tokens))
    .clip(true)
    .into()
}
