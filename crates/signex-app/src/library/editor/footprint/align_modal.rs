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
//! Move-By modal. `footprint_align_overlay` returns nothing
//! unless `ed.state.align_modal.is_some()`, so the modal's builder owns
//! its open check and no other flag needs updating.

use std::path::Path;

use iced::widget::{button, column, container, row, text};
use iced::{Background, Border, Color, Element, Length};

use signex_types::theme::ThemeTokens;

use crate::styles::{Spacing, TypeScale};

use crate::app::FootprintDocument;
use crate::library::editor::footprint::state::AlignOp;
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use crate::styles::ti;
use crate::styles::tokens;

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
    editor: &'a FootprintDocument,
    tokens: &'a ThemeTokens,
) -> Option<Element<'a, LibraryMessage>> {
    let ts = TypeScale::default();
    let sp = Spacing::default();
    let modal = editor.state.align_modal.as_ref()?;
    let path = editor.path.clone();

    let text_c = ti(tokens.text);
    let text_muted = ti(tokens.text_secondary);
    let border_c = ti(tokens.border);
    let toolbar_bg = ti(tokens.toolbar_bg);

    // Card width derives from the font size rather than magic pixels.
    let card_w = Length::Fixed(ts.label * 26.0);

    let header = container(
        row![
            text("Align Selection").size(ts.title).color(text_c),
            iced::widget::Space::new().width(Length::Fill),
            close_x(cancel_msg(&path), text_muted, tokens),
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

    // One labelled axis group: a caption over a wrapping row of
    // mutually-exclusive option chips. `selected` drives the accent
    // highlight; each chip dispatches `ctor(option)` for the editor at
    // `path`.
    let axis_group = |caption: &'static str,
                      options: &'static [(&'static str, Option<AlignOp>)],
                      selected: Option<AlignOp>,
                      ctor: fn(Option<AlignOp>) -> FootprintEditorMsg|
     -> Element<'a, LibraryMessage> {
        let mut chips = row![].spacing(sp.s);
        for &(label, option) in options {
            chips = chips.push(option_chip(
                label,
                option == selected,
                LibraryMessage::PrimitiveEditorEvent {
                    path: path.clone(),
                    msg: PrimitiveEdit::Footprint(ctor(option)),
                },
                tokens,
            ));
        }
        column![text(caption).size(ts.body).color(text_muted), chips,]
            .spacing(sp.s)
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
    .spacing(sp.l);

    let footer = row![
        iced::widget::Space::new().width(Length::Fill),
        secondary_button("Cancel", cancel_msg(&path), tokens),
        iced::widget::Space::new().width(sp.m),
        primary_button(
            "OK",
            LibraryMessage::PrimitiveEditorEvent {
                path,
                msg: PrimitiveEdit::Footprint(FootprintEditorMsg::AlignConfirm),
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

/// The `AlignCancel` message for the editor at `path` — reused by the
/// header ✕ and the footer Cancel button.
fn cancel_msg(path: &Path) -> LibraryMessage {
    LibraryMessage::PrimitiveEditorEvent {
        path: path.to_path_buf(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::AlignCancel),
    }
}

/// One mutually-exclusive option chip. `selected` fills it with the
/// accent colour; otherwise it is a bordered outline chip.
fn option_chip<'a>(
    label: &'a str,
    selected: bool,
    message: LibraryMessage,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let ts = TypeScale::default();
    let sp = Spacing::default();
    let fg = if selected {
        tokens::on_accent(tokens)
    } else {
        tokens::muted_text(tokens)
    };
    let chip =
        button(container(text(label.to_string()).size(ts.body).color(fg)).padding([sp.s, sp.m]))
            .on_press(message);
    if selected {
        chip.style(crate::styles::accent_button(tokens)).into()
    } else {
        chip.style(crate::styles::outline_button(tokens)).into()
    }
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
