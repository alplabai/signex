//! Delete Component confirm (mockup row 137): launched from the Library Browser
//! action row's Delete Selected button. The header's close runs the same
//! Cancel as the footer.

use iced::widget::{Space, column, container};
use iced::{Element, Length, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_types::theme::ThemeTokens;

use super::super::messages::LibraryMessage;
use super::super::modal_chrome::{cancel_btn, danger_btn, footer_strip, header};
use super::super::state::DeleteConfirmState;
use crate::styles::{DialogIcon, dialog_intro};

const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

const CONFIRM_W: f32 = 480.0;

/// The delete confirm's intro title and secondary text; the window height
/// follows their wrapped line counts (`app/handlers/modal_sizing.rs`).
pub(crate) fn delete_intro_texts(i18n: &I18n, internal_pn: &str, table: &str) -> (String, String) {
    (
        t!(i18n, ids::LIBRARY_DELETE_ROW_INTRO_TITLE, "pn" => internal_pn).into_string(),
        t!(
            i18n,
            ids::LIBRARY_DELETE_ROW_INTRO,
            "pn" => internal_pn,
            "table" => table
        )
        .into_string(),
    )
}

/// Delete-row confirm modal.
pub fn view_delete_confirm<'a>(
    library_path: &'a std::path::Path,
    confirm: &'a DeleteConfirmState,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
    windowed: bool,
) -> Element<'a, LibraryMessage> {
    let cancel = LibraryMessage::BrowserDeleteRowCancel {
        library_path: library_path.to_path_buf(),
    };
    let title = t!(i18n, ids::LIBRARY_DELETE_ROW_TITLE).into_string();
    let header = header(&title, cancel.clone(), windowed, tokens);

    let (intro_title, msg_label) = delete_intro_texts(i18n, &confirm.internal_pn, &confirm.table);

    // As tall as its text: the footer follows it, with no empty band.
    let body = container(
        column![dialog_intro(
            DialogIcon::Trash,
            intro_title,
            Some(msg_label),
            tokens
        )]
        .padding([14.0, SP.xl])
        .width(Length::Fill),
    )
    .width(Length::Fill);

    let footer = footer_strip(
        Vec::new(),
        vec![
            cancel_btn(t!(i18n, ids::DIALOG_BUTTON_CANCEL), cancel, tokens),
            danger_btn(
                t!(i18n, ids::DIALOG_BUTTON_DELETE),
                LibraryMessage::BrowserDeleteRowConfirm {
                    library_path: library_path.to_path_buf(),
                    table: confirm.table.clone(),
                    row_id: confirm.row_id,
                },
                tokens,
            ),
        ],
        tokens,
    );

    let mut card = column![header, body];
    if windowed {
        card = card.push(Space::new().height(Length::Fill).boxed());
    }
    container(
        card.push(footer)
            .width(super::super::card_len(windowed, CONFIRM_W))
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
