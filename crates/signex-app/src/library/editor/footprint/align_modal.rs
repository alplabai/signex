//! #370 — "Align…" dialog for the footprint editor.
//!
//! The generic Align ▸ "Align…" row used to be a `coming soon` stub;
//! this modal gives it a real per-axis dialog. It is a pure UI shell
//! over the EXISTING [`AlignOp`] variants — it introduces no new
//! geometry. The user picks at most one horizontal op and at most one
//! vertical op; Confirm applies both chosen axes under a SINGLE undo
//! snapshot (the handler in `updates::active_bar` owns that), so the
//! whole confirm is one undo step even when both axes move pads.
//!
//! Structurally a sibling of `move_by_modal.rs` (the proven template):
//! a `view_align_modal` returning `Option<Element>` that yields `None`
//! when the modal is closed, all spacing/widths derived from the
//! `FONT_SIZE` constant, and `ti(tokens.…)` for every colour.
//!
//! Mounting site: `app/view/mod.rs::collect_overlays`, alongside the
//! Move-By modal, gated by the `needs_overlay` predicate
//! (`ed.state.align_modal.is_some()`). A modal flag left out of
//! `needs_overlay` renders as a silent no-op from a non-canvas tab —
//! the same trap `move_by_modal.rs` documents.

use std::path::Path;

use iced::widget::{button, column, container, row, text};
use iced::{Background, Border, Color, Element, Length};

use signex_types::theme::ThemeTokens;

use crate::app::FootprintEditorState;
use crate::library::editor::footprint::state::AlignOp;
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use crate::styles::ti;

/// Base font size the whole card derives its spacing/widths from —
/// no hardcoded pixel constants independent of this (mirrors
/// `move_by_modal::FONT_SIZE`).
const FONT_SIZE: f32 = 12.0;

/// The horizontal-axis options offered in the dialog, in display order.
/// `None` = "leave the X axis untouched"; each `Some` maps to the same
/// concrete [`AlignOp`] the dropdown's Align rows dispatch.
const HORIZONTAL_OPTIONS: [(&str, Option<AlignOp>); 5] = [
    ("None", None),
    ("Left", Some(AlignOp::Left)),
    ("Center", Some(AlignOp::CenterH)),
    ("Right", Some(AlignOp::Right)),
    ("Distribute", Some(AlignOp::DistributeH)),
];

/// The vertical-axis options, in display order. `None` = "leave the Y
/// axis untouched".
const VERTICAL_OPTIONS: [(&str, Option<AlignOp>); 5] = [
    ("None", None),
    ("Top", Some(AlignOp::Top)),
    ("Center", Some(AlignOp::CenterV)),
    ("Bottom", Some(AlignOp::Bottom)),
    ("Distribute", Some(AlignOp::DistributeV)),
];

/// Build the Align dialog card for the active footprint editor. Returns
/// `None` when the modal is closed (`align_modal` is `None`), so the
/// call site can no-op cleanly.
pub fn view_align_modal<'a>(
    editor: &'a FootprintEditorState,
    tokens: &'a ThemeTokens,
) -> Option<Element<'a, LibraryMessage>> {
    let modal = editor.state.align_modal.as_ref()?;
    let path = editor.path.clone();

    let text_c = ti(tokens.text);
    let text_muted = ti(tokens.text_secondary);
    let border_c = ti(tokens.border);
    let accent_c = ti(tokens.accent);
    let toolbar_bg = ti(tokens.toolbar_bg);

    // Card width derives from the font size rather than magic pixels.
    let card_w = Length::Fixed(FONT_SIZE * 26.0);

    let header = container(
        row![
            text("Align Selection").size(FONT_SIZE + 1.0).color(text_c),
            iced::widget::Space::new().width(Length::Fill),
            close_x(cancel_msg(&path), text_muted),
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

    // One labelled axis group: a caption over a wrapping row of
    // mutually-exclusive option chips. `selected` drives the accent
    // highlight; each chip dispatches `ctor(option)` for the editor at
    // `path`.
    let axis_group = |caption: &'static str,
                      options: &'static [(&'static str, Option<AlignOp>)],
                      selected: Option<AlignOp>,
                      ctor: fn(Option<AlignOp>) -> FootprintEditorMsg|
     -> Element<'a, LibraryMessage> {
        let mut chips = row![].spacing(FONT_SIZE * 0.4);
        for &(label, option) in options {
            chips = chips.push(option_chip(
                label,
                option == selected,
                LibraryMessage::PrimitiveEditorEvent {
                    path: path.clone(),
                    msg: PrimitiveEdit::Footprint(ctor(option)),
                },
                accent_c,
                text_muted,
                border_c,
            ));
        }
        column![text(caption).size(FONT_SIZE - 1.0).color(text_muted), chips,]
            .spacing(FONT_SIZE * 0.4)
            .into()
    };

    let body = column![
        axis_group(
            "Horizontal",
            &HORIZONTAL_OPTIONS,
            modal.horizontal,
            FootprintEditorMsg::AlignSetHorizontal,
        ),
        axis_group(
            "Vertical",
            &VERTICAL_OPTIONS,
            modal.vertical,
            FootprintEditorMsg::AlignSetVertical,
        ),
    ]
    .spacing(FONT_SIZE);

    let footer = row![
        iced::widget::Space::new().width(Length::Fill),
        secondary_button("Cancel", cancel_msg(&path), text_c, border_c),
        iced::widget::Space::new().width(FONT_SIZE * 0.6),
        primary_button(
            "OK",
            LibraryMessage::PrimitiveEditorEvent {
                path,
                msg: PrimitiveEdit::Footprint(FootprintEditorMsg::AlignConfirm),
            },
            accent_c,
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

/// The `AlignCancel` message for the editor at `path` — reused by the
/// header ✕ and the footer Cancel button.
fn cancel_msg(path: &Path) -> LibraryMessage {
    LibraryMessage::PrimitiveEditorEvent {
        path: path.to_path_buf(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::AlignCancel),
    }
}

/// One mutually-exclusive option chip. `selected` fills it with the
/// accent colour; otherwise it is a bordered, muted, transparent chip.
fn option_chip<'a>(
    label: &'a str,
    selected: bool,
    message: LibraryMessage,
    accent: Color,
    text_muted: Color,
    border: Color,
) -> Element<'a, LibraryMessage> {
    let (bg, fg) = if selected {
        (accent, Color::WHITE)
    } else {
        (Color::TRANSPARENT, text_muted)
    };
    button(
        container(text(label.to_string()).size(FONT_SIZE - 1.0).color(fg))
            .padding([FONT_SIZE * 0.35, FONT_SIZE * 0.7]),
    )
    .on_press(message)
    .style(move |_: &iced::Theme, _| button::Style {
        background: Some(Background::Color(bg)),
        border: Border {
            width: 1.0,
            radius: 4.0.into(),
            color: border,
        },
        text_color: fg,
        ..button::Style::default()
    })
    .into()
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
        background: Some(Background::Color(Color::TRANSPARENT)),
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
    accent: Color,
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
        background: Some(Background::Color(accent)),
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
