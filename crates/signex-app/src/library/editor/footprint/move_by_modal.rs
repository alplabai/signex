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

use iced::widget::{button, column, container, row, text, text_input};
use iced::{Background, Border, Color, Element, Length};

use signex_types::theme::ThemeTokens;

use crate::styles::{Spacing, TypeScale};

use crate::app::FootprintDocument;
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use crate::styles::ti;
use crate::styles::tokens::{self, Overlay};

/// Build the Move-By modal card for the active footprint editor.
/// Returns `None` when the modal is closed (`move_by_modal` is
/// `None`), so the call site can no-op cleanly.
pub fn view_move_by_modal<'a>(
    editor: &'a FootprintDocument,
    tokens: &'a ThemeTokens,
) -> Option<Element<'a, LibraryMessage>> {
    let ts = TypeScale::default();
    let sp = Spacing::default();
    let modal = editor.state.move_by_modal.as_ref()?;
    let path = editor.path.clone();

    let text_c = ti(tokens.text);
    let text_muted = ti(tokens.text_secondary);
    let border_c = ti(tokens.border);
    let toolbar_bg = ti(tokens.toolbar_bg);
    let input_bg = tokens::overlay(tokens, Overlay::Faint);
    let selection = tokens::selection_tint(tokens);

    // Label / input / card widths derive from the font size rather
    // than magic pixel constants.
    let label_w = Length::Fixed(ts.label * 6.0);
    let input_w = Length::Fixed(ts.label * 7.0);
    let card_w = Length::Fixed(ts.label * 22.0);

    let header = container(
        row![
            text("Move Selection By").size(ts.title).color(text_c),
            iced::widget::Space::new().width(Length::Fill),
            close_x(
                LibraryMessage::PrimitiveEditorEvent {
                    path: path.clone(),
                    msg: PrimitiveEdit::Footprint(FootprintEditorMsg::MoveByCancel),
                },
                text_muted,
                tokens,
            ),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([sp.s, sp.m])
    .style(move |_: &iced::Theme| container::Style {
        background: Some(Background::Color(toolbar_bg)),
        text_color: Some(text_c),
        border: Border {
            width: 0.0,
            radius: iced::border::Radius::default()
                .top_left(crate::styles::MODAL_CORNER_RADIUS)
                .top_right(crate::styles::MODAL_CORNER_RADIUS),
            color: border_c,
        },
        ..container::Style::default()
    });

    let mk_field = |label: &'static str,
                    value: &str,
                    field_path: PathBuf,
                    ctor: fn(String) -> FootprintEditorMsg|
     -> Element<'a, LibraryMessage> {
        let submit_path = field_path.clone();
        let input = text_input("0.0", value)
            .size(ts.label)
            .padding(sp.s)
            .width(input_w)
            .style(move |_: &iced::Theme, _| iced::widget::text_input::Style {
                background: Background::Color(input_bg),
                border: Border {
                    width: 1.0,
                    radius: 2.0.into(),
                    color: border_c,
                },
                icon: Color::TRANSPARENT,
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
            container(text(label).size(ts.body).color(text_muted)).width(label_w),
            input,
        ]
        .spacing(sp.s)
        .align_y(iced::Alignment::Center)
        .into()
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

    let footer = row![
        iced::widget::Space::new().width(Length::Fill),
        secondary_button(
            "Cancel",
            LibraryMessage::PrimitiveEditorEvent {
                path: path.clone(),
                msg: PrimitiveEdit::Footprint(FootprintEditorMsg::MoveByCancel),
            },
            tokens,
        ),
        iced::widget::Space::new().width(sp.m),
        primary_button(
            "OK",
            LibraryMessage::PrimitiveEditorEvent {
                path,
                msg: PrimitiveEdit::Footprint(FootprintEditorMsg::MoveByConfirm),
            },
            tokens,
        ),
    ]
    .align_y(iced::Alignment::Center);

    let card = container(
        column![
            header,
            container(body).padding(sp.l),
            container(footer).padding([sp.m, sp.l]),
        ]
        .width(card_w),
    )
    .style(crate::styles::modal_card(tokens))
    .clip(true);

    Some(card.into())
}

fn close_x<'a>(
    message: LibraryMessage,
    text_color: Color,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let ts = TypeScale::default();
    let sp = Spacing::default();
    button(text("\u{2715}").size(ts.body).color(text_color))
        .padding(sp.xs)
        .on_press(message)
        .style(crate::styles::subtle_button(tokens))
        .into()
}

fn secondary_button<'a>(
    label: &str,
    message: LibraryMessage,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let ts = TypeScale::default();
    let sp = Spacing::default();
    button(
        container(text(label.to_string()).size(ts.body).color(ti(tokens.text)))
            .padding([sp.s, sp.l]),
    )
    .on_press(message)
    .style(crate::styles::outline_button(tokens))
    .into()
}

fn primary_button<'a>(
    label: &str,
    message: LibraryMessage,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let ts = TypeScale::default();
    let sp = Spacing::default();
    button(
        container(
            text(label.to_string())
                .size(ts.body)
                .color(tokens::on_accent(tokens)),
        )
        .padding([sp.s, sp.l]),
    )
    .on_press(message)
    .style(crate::styles::accent_button(tokens))
    .into()
}
