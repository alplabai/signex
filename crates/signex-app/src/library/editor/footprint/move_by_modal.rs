//! v0.14 — "Move Selection By X, Y…" typed-delta modal for the
//! footprint editor.
//!
//! Replaces the plain one-grid-step nudge as the active-bar's primary
//! "Move Selection by X, Y…" action: the user types an exact (dx, dy)
//! mm offset instead of nudging by the active grid step. Confirm feeds
//! the parsed delta into the SAME proven
//! `footprint_nudge_selection` dispatcher helper (history snapshot +
//! `nudge_pads` + sketch mirror + primitive re-sync) that the one-step
//! nudge uses — see `app/dispatch/library.rs`.
//!
//! Mounting site: `app/view/mod.rs::collect_overlays`, in the same
//! block that mounts the footprint active bar + its dropdown overlay.
//! The builder (`footprint_move_by_overlay`) returns nothing unless
//! `ed.state.move_by_modal.is_some()`; that guard is the only open
//! check, so no other flag needs updating.

use std::path::PathBuf;

use iced::widget::{column, container, row};
use iced::{Background, Border, Element, Length, Widget as _};
use signex_widgets::{raw, text, text_input};

use signex_i18n::{ids, t};
use signex_types::theme::ThemeTokens;

use crate::styles::{Spacing, TypeScale};

use crate::app::FootprintDocument;
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use crate::library::modal_chrome;
use crate::styles::ti;
use crate::styles::tokens::{self, Overlay};

/// Build the Move-By modal card for the active footprint editor.
/// Returns `None` when the modal is closed (`move_by_modal` is
/// `None`), so the call site can no-op cleanly.
pub fn view_move_by_modal<'a>(
    editor: &'a FootprintDocument,
    tokens: &'a ThemeTokens,
    i18n: &signex_i18n::I18n,
) -> Option<Element<'a, LibraryMessage>> {
    let ts = TypeScale::default();
    let sp = Spacing::default();
    let modal = editor.state.move_by_modal.as_ref()?;
    let path = editor.path.clone();

    let text_c = ti(tokens.text);
    let text_muted = ti(tokens.text_secondary);
    let border_c = ti(tokens.border);
    let input_bg = tokens::overlay(tokens, Overlay::Faint);
    let selection = tokens::selection_tint(tokens);

    // Label / input / card widths derive from the font size rather
    // than magic pixel constants.
    let label_w = Length::Fixed(ts.label * 6.0);
    let input_w = Length::Fixed(ts.label * 7.0);
    let card_w = Length::Fixed(ts.label * 22.0);

    let header = modal_chrome::header(
        "Move Selection By",
        LibraryMessage::PrimitiveEditorEvent {
            path: path.clone(),
            msg: PrimitiveEdit::Footprint(FootprintEditorMsg::MoveByCancel),
        },
        false,
        tokens,
    );

    let mk_field = |label: &'static str,
                    value: &'a str,
                    field_path: PathBuf,
                    ctor: fn(String) -> FootprintEditorMsg|
     -> Element<'a, LibraryMessage> {
        let submit_path = field_path.clone();
        let input = text_input(raw("0.0"), value)
            .size(ts.label)
            .padding(sp.s)
            .width(input_w)
            .style(move |_: &iced::Theme, _| iced::widget::text_input::Style {
                background: Background::Color(input_bg),
                border: Border {
                    width: 1.0,
                    radius: 2.0.into(),
                    color: border_c,
                    ..Border::default()
                },
                placeholder: text_muted,
                value: text_c,
                selection,
            })
            .on_input(move |s| LibraryMessage::PrimitiveEditorEvent {
                path: field_path.clone(),
                msg: PrimitiveEdit::Footprint(ctor(s)),
            })
            .on_submit(LibraryMessage::PrimitiveEditorEvent {
                path: submit_path,
                msg: PrimitiveEdit::Footprint(FootprintEditorMsg::MoveByConfirm),
            });
        row![
            container(text(raw(label)).size(ts.body).color(text_muted)).width(label_w),
            input,
        ]
        .spacing(sp.s)
        .align_y(iced::Alignment::Center)
        .boxed()
    };

    let body = column![
        mk_field(
            "X (mm)",
            &modal.dx_buf,
            path.clone(),
            FootprintEditorMsg::MoveBySetX,
        ),
        mk_field(
            "Y (mm)",
            &modal.dy_buf,
            path.clone(),
            FootprintEditorMsg::MoveBySetY,
        ),
    ]
    .spacing(sp.m);

    let footer = modal_chrome::footer_strip(
        Vec::new(),
        vec![
            modal_chrome::cancel_btn(
                t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                LibraryMessage::PrimitiveEditorEvent {
                    path: path.clone(),
                    msg: PrimitiveEdit::Footprint(FootprintEditorMsg::MoveByCancel),
                },
                tokens,
            ),
            modal_chrome::primary_btn(
                t!(i18n, ids::DIALOG_BUTTON_OK),
                LibraryMessage::PrimitiveEditorEvent {
                    path,
                    msg: PrimitiveEdit::Footprint(FootprintEditorMsg::MoveByConfirm),
                },
                tokens,
            ),
        ],
        tokens,
    );

    let card = container(column![header, container(body).padding(sp.l), footer,].width(card_w))
        .style(crate::styles::modal_card(tokens))
        .clip(true);

    Some(card.boxed())
}
