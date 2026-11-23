//! v0.14 — "Move Selection By" typed-delta modal for the footprint editor.
//!
//! The user types an exact (dx, dy) mm offset instead of nudging by the
//! active grid step. Confirm feeds the parsed delta into the same
//! `footprint_nudge_selection` helper the one-step nudge uses (history
//! snapshot, `nudge_pads`, sketch mirror, primitive re-sync).
//!
//! A compact two-field card: X (mm) and Y (mm), X focused when the dialog
//! opens ([`focus_x_field`], returned by the footprint dispatcher), Cancel
//! and Move. Mounted by `app/view/overlays/bars.rs`
//! (`footprint_move_by_overlay`), which draws nothing while
//! `move_by_modal` is `None`.

use std::path::PathBuf;

use iced::widget::{Id, column, container, operation, row};
use iced::{Element, Font, Length, Task, Widget as _};
use signex_widgets::{text, text_input};

use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::theme::ThemeTokens;

use crate::styles::{Spacing, TypeScale};

use crate::app::FootprintDocument;
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use crate::library::modal_chrome;
use crate::styles::ti;

/// Widget id of the X field, focused when the dialog opens.
const X_FIELD: &str = "footprint.move_by.x";

/// Card width and label column, in body-text ems.
const CARD_EM: f32 = 21.0;
const LABEL_EM: f32 = 4.2;

/// Focus the X field and select its text: the task the footprint
/// dispatcher returns when the dialog opens.
pub fn focus_x_field<T: 'static>() -> Task<T> {
    Task::batch([
        operation::focus(Id::new(X_FIELD)),
        operation::text_input::select_all(Id::new(X_FIELD)),
    ])
}

fn footprint_msg(path: PathBuf, msg: FootprintEditorMsg) -> LibraryMessage {
    LibraryMessage::PrimitiveEditorEvent {
        path,
        msg: PrimitiveEdit::Footprint(msg),
    }
}

/// Build the Move-By modal card for the active footprint editor.
/// Returns `None` when the modal is closed (`move_by_modal` is
/// `None`), so the call site can no-op cleanly.
pub fn view_move_by_modal<'a>(
    editor: &'a FootprintDocument,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Option<Element<'a, LibraryMessage>> {
    let ts = TypeScale::default();
    let sp = Spacing::default();
    let modal = editor.state.move_by_modal.as_ref()?;
    let path = editor.path.clone();
    let text_muted = ti(tokens.text_secondary);

    let header = modal_chrome::header(
        &i18n.message_plain(ids::DIALOG_MOVE_BY_TITLE, &[]),
        footprint_msg(path.clone(), FootprintEditorMsg::MoveByCancel),
        false,
        tokens,
    );

    let field = |label: MessageId,
                 value: &'a str,
                 id: Option<&'static str>,
                 ctor: fn(String) -> FootprintEditorMsg|
     -> Element<'a, LibraryMessage> {
        let input_path = path.clone();
        let mut input = text_input(t!(i18n, ids::PLACEHOLDER_MOVE_OFFSET), value)
            .style(crate::panels::form_look::text_input_style)
            .font(Font::MONOSPACE)
            .size(ts.body)
            .padding([sp.xs, sp.s])
            .width(Length::Fill)
            .on_input(move |s| footprint_msg(input_path.clone(), ctor(s)))
            .on_submit(footprint_msg(
                path.clone(),
                FootprintEditorMsg::MoveByConfirm,
            ));
        if let Some(id) = id {
            input = input.id(Id::new(id));
        }
        row![
            container(text(t!(i18n, label)).size(ts.body).color(text_muted))
                .width(Length::Fixed(ts.body * LABEL_EM)),
            input,
        ]
        .spacing(sp.s)
        .align_y(iced::Alignment::Center)
        .boxed()
    };

    let body = column![
        field(
            ids::DIALOG_MOVE_BY_X,
            &modal.dx_buf,
            Some(X_FIELD),
            FootprintEditorMsg::MoveBySetX,
        ),
        field(
            ids::DIALOG_MOVE_BY_Y,
            &modal.dy_buf,
            None,
            FootprintEditorMsg::MoveBySetY,
        ),
    ]
    .spacing(sp.s);

    let footer = modal_chrome::footer_strip(
        Vec::new(),
        vec![
            modal_chrome::cancel_btn(
                t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                footprint_msg(path.clone(), FootprintEditorMsg::MoveByCancel),
                tokens,
            ),
            modal_chrome::primary_btn(
                t!(i18n, ids::DIALOG_BUTTON_MOVE),
                footprint_msg(path, FootprintEditorMsg::MoveByConfirm),
                tokens,
            ),
        ],
        tokens,
    );

    let card = container(
        column![header, container(body).padding(sp.l), footer]
            .width(Length::Fixed(ts.body * CARD_EM)),
    )
    .style(crate::styles::modal_card(tokens))
    .clip(true);

    Some(card.boxed())
}

#[cfg(test)]
mod tests {
    /// The dialog's fixed text comes from the catalog.
    #[test]
    fn move_by_text_reads_from_the_catalog() {
        let en = signex_i18n::I18n::en_us_ref();
        let text = |id| en.message_plain(id, &[]);
        assert_eq!(
            text(signex_i18n::ids::DIALOG_MOVE_BY_TITLE),
            "Move Selection By"
        );
        assert_eq!(text(signex_i18n::ids::DIALOG_MOVE_BY_X), "X (mm)");
        assert_eq!(text(signex_i18n::ids::DIALOG_MOVE_BY_Y), "Y (mm)");
        assert_eq!(text(signex_i18n::ids::DIALOG_BUTTON_MOVE), "Move");
    }
}
