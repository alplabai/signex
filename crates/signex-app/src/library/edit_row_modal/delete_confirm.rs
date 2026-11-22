//! Delete Component confirm (mockup row 137): launched from the Library Browser
//! action row's Delete Selected button. The header's close runs the same
//! Cancel as the footer.

use iced::widget::{Space, column, container};
use iced::{Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::text;

use super::super::messages::LibraryMessage;
use super::super::modal_chrome::{cancel_btn, danger_btn, footer_strip, header};
use super::super::state::DeleteConfirmState;
use crate::styles::tokens::resolve;
use crate::styles::{DialogIcon, TypeScale, dialog_intro_with};

const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;
const TS: TypeScale = TypeScale::DEFAULT;

const CONFIRM_W: f32 = 480.0;

/// The gap between the intro text and the caption under it.
pub(crate) const DELETE_CAPTION_GAP: f32 = SP.xs;

fn delete_intro_message(i18n: &I18n, internal_pn: &str, table: &str) -> Localized {
    t!(
        i18n,
        ids::LIBRARY_DELETE_ROW_INTRO,
        "pn" => internal_pn,
        "table" => table
    )
}

/// The delete confirm's intro title and secondary text; the window height
/// follows their wrapped line counts (`app/handlers/modal_sizing.rs`).
pub(crate) fn delete_intro_texts(i18n: &I18n, internal_pn: &str, table: &str) -> (String, String) {
    (
        t!(i18n, ids::LIBRARY_DELETE_ROW_INTRO_TITLE, "pn" => internal_pn).into_string(),
        delete_intro_message(i18n, internal_pn, table).into_string(),
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

    let intro_title = t!(
        i18n,
        ids::LIBRARY_DELETE_ROW_INTRO_TITLE,
        "pn" => confirm.internal_pn.as_str()
    )
    .into_string();
    let r = resolve(tokens);
    let secondary = column![
        text(delete_intro_message(
            i18n,
            &confirm.internal_pn,
            &confirm.table
        ))
        .size(TS.body)
        .color(r.ink3),
        text(t!(i18n, ids::LIBRARY_DELETE_ROW_CAPTION))
            .size(TS.caption)
            .color(r.ink4),
    ]
    .spacing(DELETE_CAPTION_GAP)
    .boxed();

    // As tall as its text: the footer follows it, with no empty band.
    let body = container(
        column![dialog_intro_with(
            DialogIcon::Trash,
            intro_title,
            Some(secondary),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout_probe::{find, lay_out_texts};
    use signex_types::theme::{ThemeId, theme_tokens};

    /// Mockup row 137: the caption sits under the intro text, in both
    /// themes.
    #[test]
    fn the_caption_says_placed_parts_keep_their_copy() {
        let i18n = I18n::en_us();
        let confirm = DeleteConfirmState {
            table: "Resistors".into(),
            row_id: signex_library::RowId::new(),
            internal_pn: "R-0402".into(),
        };
        let path = std::path::Path::new("l.snxlib");
        for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let tokens = theme_tokens(id);
            let view = view_delete_confirm(path, &confirm, &tokens, &i18n, false);
            let texts = lay_out_texts(view, iced::Size::new(600.0, 400.0));
            let caption = find(&texts, "Placed parts keep their embedded copy.").bounds;
            let intro = texts
                .iter()
                .find(|t| t.text.contains("This cannot be undone"))
                .expect("intro text")
                .bounds;
            assert!(caption.y >= intro.y + intro.height, "{id:?}");
        }
    }
}
