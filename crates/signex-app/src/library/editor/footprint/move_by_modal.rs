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
//! block that mounts the footprint active bar + its dropdown overlay,
//! gated by the `needs_overlay` predicate (`ed.state.move_by_modal
//! .is_some()`). A modal flag left out of `needs_overlay` renders as a
//! silent no-op from a non-canvas tab — see
//! `reference_overlay_predicate_gotcha` in project memory.

use std::path::PathBuf;

use iced::widget::{button, column, container, row, text, text_input};
use iced::{Background, Border, Color, Element, Length};

use signex_types::theme::ThemeTokens;

use crate::app::FootprintEditorState;
use crate::library::messages::{LibraryMessage, PrimitiveEditorMsg};
use crate::styles::ti;

/// Base font size the whole card derives its spacing/widths from —
/// no hardcoded pixel constants independent of this.
const FONT_SIZE: f32 = 12.0;

/// Build the Move-By modal card for the active footprint editor.
/// Returns `None` when the modal is closed (`move_by_modal` is
/// `None`), so the call site can no-op cleanly.
pub fn view_move_by_modal<'a>(
    editor: &'a FootprintEditorState,
    tokens: &'a ThemeTokens,
) -> Option<Element<'a, LibraryMessage>> {
    let modal = editor.state.move_by_modal.as_ref()?;
    let path = editor.path.clone();

    let text_c = ti(tokens.text);
    let text_muted = ti(tokens.text_secondary);
    let border_c = ti(tokens.border);
    let toolbar_bg = ti(tokens.toolbar_bg);

    // Label / input / card widths derive from the font size rather
    // than magic pixel constants.
    let label_w = Length::Fixed(FONT_SIZE * 6.0);
    let input_w = Length::Fixed(FONT_SIZE * 7.0);
    let card_w = Length::Fixed(FONT_SIZE * 22.0);

    let header = container(
        row![
            text("Move Selection By")
                .size(FONT_SIZE + 1.0)
                .color(text_c),
            iced::widget::Space::new().width(Length::Fill),
            close_x(
                LibraryMessage::PrimitiveEditorEvent {
                    path: path.clone(),
                    msg: PrimitiveEditorMsg::FootprintMoveByCancel,
                },
                text_muted,
            ),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([FONT_SIZE * 0.5, FONT_SIZE * 0.75])
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
                    ctor: fn(String) -> PrimitiveEditorMsg|
     -> Element<'a, LibraryMessage> {
        let submit_path = field_path.clone();
        let input = text_input("0.0", value)
            .size(FONT_SIZE)
            .padding(FONT_SIZE * 0.35)
            .width(input_w)
            .style(move |_: &iced::Theme, _| iced::widget::text_input::Style {
                background: Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.04)),
                border: Border {
                    width: 1.0,
                    radius: 2.0.into(),
                    color: border_c,
                },
                icon: Color::TRANSPARENT,
                placeholder: text_muted,
                value: text_c,
                selection: Color::from_rgba(0.4, 0.6, 1.0, 0.4),
            })
            .on_input(move |s| LibraryMessage::PrimitiveEditorEvent {
                path: field_path.clone(),
                msg: ctor(s),
            })
            .on_submit(LibraryMessage::PrimitiveEditorEvent {
                path: submit_path,
                msg: PrimitiveEditorMsg::FootprintMoveByConfirm,
            });
        row![
            container(text(label).size(FONT_SIZE - 1.0).color(text_muted)).width(label_w),
            input,
        ]
        .spacing(FONT_SIZE * 0.5)
        .align_y(iced::Alignment::Center)
        .into()
    };

    let body = column![
        mk_field(
            "X (mm)",
            &modal.dx_buf,
            path.clone(),
            PrimitiveEditorMsg::FootprintMoveBySetX,
        ),
        mk_field(
            "Y (mm)",
            &modal.dy_buf,
            path.clone(),
            PrimitiveEditorMsg::FootprintMoveBySetY,
        ),
    ]
    .spacing(FONT_SIZE * 0.6);

    let footer = row![
        iced::widget::Space::new().width(Length::Fill),
        secondary_button(
            "Cancel",
            LibraryMessage::PrimitiveEditorEvent {
                path: path.clone(),
                msg: PrimitiveEditorMsg::FootprintMoveByCancel,
            },
            text_c,
            border_c,
        ),
        iced::widget::Space::new().width(FONT_SIZE * 0.6),
        primary_button(
            "OK",
            LibraryMessage::PrimitiveEditorEvent {
                path,
                msg: PrimitiveEditorMsg::FootprintMoveByConfirm,
            },
            border_c,
        ),
    ]
    .align_y(iced::Alignment::Center);

    let card = container(
        column![
            header,
            container(body).padding(FONT_SIZE),
            container(footer).padding([FONT_SIZE * 0.6, FONT_SIZE]),
        ]
        .width(card_w),
    )
    .style(crate::styles::modal_card(tokens))
    .clip(true);

    Some(card.into())
}

fn close_x(message: LibraryMessage, text_color: Color) -> Element<'static, LibraryMessage> {
    button(text("\u{2715}").size(FONT_SIZE - 1.0).color(text_color))
        .padding(FONT_SIZE * 0.25)
        .on_press(message)
        .style(move |_: &iced::Theme, _| button::Style {
            background: Some(Background::Color(Color::TRANSPARENT)),
            text_color,
            ..button::Style::default()
        })
        .into()
}

fn secondary_button<'a>(
    label: &str,
    message: LibraryMessage,
    text_color: Color,
    border: Color,
) -> Element<'a, LibraryMessage> {
    button(
        container(
            text(label.to_string())
                .size(FONT_SIZE - 1.0)
                .color(text_color),
        )
        .padding([FONT_SIZE * 0.4, FONT_SIZE * 1.1]),
    )
    .on_press(message)
    .style(move |_: &iced::Theme, _| button::Style {
        background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.04))),
        border: Border {
            width: 1.0,
            radius: 4.0.into(),
            color: border,
        },
        text_color,
        ..button::Style::default()
    })
    .into()
}

fn primary_button<'a>(
    label: &str,
    message: LibraryMessage,
    border: Color,
) -> Element<'a, LibraryMessage> {
    button(
        container(
            text(label.to_string())
                .size(FONT_SIZE - 1.0)
                .color(Color::WHITE),
        )
        .padding([FONT_SIZE * 0.4, FONT_SIZE * 1.1]),
    )
    .on_press(message)
    .style(move |_: &iced::Theme, _| button::Style {
        background: Some(Background::Color(Color::from_rgba(0.30, 0.55, 0.95, 1.0))),
        border: Border {
            width: 1.0,
            radius: 4.0.into(),
            color: border,
        },
        text_color: Color::WHITE,
        ..button::Style::default()
    })
    .into()
}
