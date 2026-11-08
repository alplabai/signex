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

use iced::widget::{Space, column, container, text};
use iced::{Element, Length, Widget};
use signex_types::theme::ThemeTokens;

use crate::styles::{DialogIcon, Spacing, TypeScale, dialog_body, dialog_intro, dialog_list};
use signex_widgets::theme_ext;

use super::messages::{CloseLibraryChoice, LibraryMessage};
use super::modal_chrome::{cancel_btn, danger_btn, footer_strip, header, primary_btn};
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

/// The intro's title and secondary text; the window height follows their
/// wrapped line counts (`app/handlers/modal_sizing.rs`).
pub(crate) fn intro_texts(library_name: &str, drafts: usize) -> (&'static str, String) {
    (
        "Save the drafts before closing?",
        format!("'{library_name}' has {drafts} unsaved draft(s). What would you like to do?"),
    )
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

    let header = header(
        "Close Library — Unsaved Drafts",
        LibraryMessage::CloseLibraryConfirm(CloseLibraryChoice::Cancel),
        windowed,
        tokens,
    );

    let (intro_title, intro_text) = intro_texts(
        &confirm.library_name,
        confirm.dirty_editors.len() + confirm.dirty_paths.len(),
    );
    let summary = dialog_intro(DialogIcon::Warn, intro_title, Some(intro_text), tokens);

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
                .boxed(),
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
            .boxed(),
        );
    }
    let list = container(dialog_list(rows, tokens)).height(Length::Fixed(140.0));

    let body = dialog_body(column![summary, list].spacing(sp.m));

    let footer = footer_strip(
        Vec::new(),
        vec![
            cancel_btn(
                "Cancel",
                LibraryMessage::CloseLibraryConfirm(CloseLibraryChoice::Cancel),
                tokens,
            ),
            danger_btn(
                "Discard All",
                LibraryMessage::CloseLibraryConfirm(CloseLibraryChoice::DiscardAll),
                tokens,
            ),
            primary_btn(
                "Save All",
                LibraryMessage::CloseLibraryConfirm(CloseLibraryChoice::SaveAll),
                tokens,
            ),
        ],
        tokens,
    );

    // As tall as the content in the main window; in its own window (sized
    // to the content) the footer stays at the bottom.
    let mut card = column![header, body];
    if windowed {
        card = card.push(Space::new().height(Length::Fill).boxed());
    }
    container(
        card.push(footer)
            .width(super::card_len(
                windowed,
                super::modal_chrome::CLOSE_PROMPT_W,
            ))
            .height(if windowed {
                Length::Fill
            } else {
                Length::Shrink
            }),
    )
    .style(crate::styles::modal_card(tokens))
    .clip(true)
    .boxed()
}
