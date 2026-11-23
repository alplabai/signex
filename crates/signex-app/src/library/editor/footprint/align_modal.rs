//! #370 — "Align Selection" dialog for the footprint editor.
//!
//! A pure UI shell over the existing [`AlignOp`] variants: it adds no
//! geometry. The user picks at most one horizontal op and at most one
//! vertical op, each on a segmented row (Left · Center · Right · Space ·
//! None, Top · Center · Bottom · Space · None); Align applies both under a
//! SINGLE undo snapshot (the handler in `updates::active_bar` owns that).
//! "Space" is the distribute op: the outer pads stay, the gaps between
//! the centres become equal.
//!
//! Mounted by `app/view/overlays/bars.rs` (`footprint_align_overlay`),
//! which draws nothing while `align_modal` is `None`.

use std::path::Path;

use iced::widget::{column, container, row};
use iced::{Element, Length, Widget};
use signex_widgets::{Label, SegmentedStyle, segmented, text};

use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::theme::ThemeTokens;

use crate::styles::{Spacing, TypeScale};

use crate::app::FootprintDocument;
use crate::library::editor::footprint::state::AlignOp;
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use crate::library::modal_chrome;
use crate::styles::ti;

/// The horizontal-axis options, in display order. `None` leaves the X
/// axis untouched; each `Some` is the op the dropdown's Align rows send.
pub(crate) const HORIZONTAL_OPTIONS: [(MessageId, Option<AlignOp>); 5] = [
    (ids::DIALOG_ALIGN_LEFT, Some(AlignOp::Left)),
    (ids::DIALOG_ALIGN_CENTER, Some(AlignOp::CenterH)),
    (ids::DIALOG_ALIGN_RIGHT, Some(AlignOp::Right)),
    (ids::DIALOG_ALIGN_SPACE, Some(AlignOp::DistributeH)),
    (ids::DIALOG_ALIGN_NONE, None),
];

/// The vertical-axis options, in display order. `None` leaves the Y axis
/// untouched.
pub(crate) const VERTICAL_OPTIONS: [(MessageId, Option<AlignOp>); 5] = [
    (ids::DIALOG_ALIGN_TOP, Some(AlignOp::Top)),
    (ids::DIALOG_ALIGN_CENTER, Some(AlignOp::CenterV)),
    (ids::DIALOG_ALIGN_BOTTOM, Some(AlignOp::Bottom)),
    (ids::DIALOG_ALIGN_SPACE, Some(AlignOp::DistributeV)),
    (ids::DIALOG_ALIGN_NONE, None),
];

/// Card width and label column, in body-text ems: wide enough for all
/// five segments of a row.
const CARD_EM: f32 = 30.0;
const LABEL_EM: f32 = 5.8;

/// The segmented-control look from the theme tokens.
fn segment_style(tokens: &ThemeTokens, text_size: f32) -> SegmentedStyle {
    let r = crate::styles::tokens::resolve(tokens);
    r.segmented(text_size).fill_width(true)
}

/// Build the Align dialog card for the active footprint editor. Returns
/// `None` when the modal is closed (`align_modal` is `None`), so the
/// call site can no-op cleanly.
pub fn view_align_modal<'a>(
    editor: &'a FootprintDocument,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Option<Element<'a, LibraryMessage>> {
    let ts = TypeScale::default();
    let sp = Spacing::default();
    let modal = editor.state.align_modal.as_ref()?;
    let path = editor.path.clone();
    let text_muted = ti(tokens.text_secondary);
    let style = segment_style(tokens, ts.body);

    let header = modal_chrome::header(
        &i18n.message_plain(ids::DIALOG_ALIGN_TITLE, &[]),
        cancel_msg(&path),
        false,
        tokens,
    );

    // One labelled axis row: the caption, then a segmented control whose
    // pick sends `ctor(option)` to the editor at `path`.
    let axis_row = |caption: MessageId,
                    options: &[(MessageId, Option<AlignOp>); 5],
                    selected: Option<AlignOp>,
                    ctor: fn(Option<AlignOp>) -> FootprintEditorMsg|
     -> Element<'a, LibraryMessage> {
        let pick_path = path.clone();
        let segments = segmented(
            options
                .iter()
                .map(|&(label, option)| (option, Label::from(t!(i18n, label)))),
            selected,
            style,
            move |option| LibraryMessage::PrimitiveEditorEvent {
                path: pick_path.clone(),
                msg: PrimitiveEdit::Footprint(ctor(option)),
            },
        );
        row![
            container(text(t!(i18n, caption)).size(ts.body).color(text_muted))
                .width(Length::Fixed(ts.body * LABEL_EM)),
            segments,
        ]
        .spacing(sp.s)
        .align_y(iced::Alignment::Center)
        .boxed()
    };

    let body = column![
        axis_row(
            ids::DIALOG_ALIGN_HORIZONTAL,
            &HORIZONTAL_OPTIONS,
            modal.horizontal,
            FootprintEditorMsg::AlignSetHorizontal,
        ),
        axis_row(
            ids::DIALOG_ALIGN_VERTICAL,
            &VERTICAL_OPTIONS,
            modal.vertical,
            FootprintEditorMsg::AlignSetVertical,
        ),
    ]
    .spacing(sp.m);

    let footer = modal_chrome::footer_strip(
        Vec::new(),
        vec![
            modal_chrome::cancel_btn(
                t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                cancel_msg(&path),
                tokens,
            ),
            modal_chrome::primary_btn(
                t!(i18n, ids::DIALOG_BUTTON_ALIGN),
                LibraryMessage::PrimitiveEditorEvent {
                    path,
                    msg: PrimitiveEdit::Footprint(FootprintEditorMsg::AlignConfirm),
                },
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

/// The `AlignCancel` message for the editor at `path` — reused by the
/// header ✕ and the footer Cancel button.
fn cancel_msg(path: &Path) -> LibraryMessage {
    LibraryMessage::PrimitiveEditorEvent {
        path: path.to_path_buf(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::AlignCancel),
    }
}

#[cfg(test)]
mod tests {
    use super::{HORIZONTAL_OPTIONS, VERTICAL_OPTIONS};
    use crate::library::editor::footprint::state::AlignOp;

    fn labels(options: &[(signex_i18n::MessageId, Option<AlignOp>)]) -> Vec<String> {
        let en = signex_i18n::I18n::en_us_ref();
        options
            .iter()
            .map(|&(id, _)| en.message_plain(id, &[]))
            .collect()
    }

    /// Each axis offers the same five choices as the radio lists did
    /// (three aligns, distribute, leave alone), as segments in the order
    /// Left Center Right Space None / Top Center Bottom Space None.
    #[test]
    fn each_axis_offers_three_aligns_distribute_and_none() {
        assert_eq!(
            labels(&HORIZONTAL_OPTIONS),
            ["Left", "Center", "Right", "Space", "None"]
        );
        assert_eq!(
            labels(&VERTICAL_OPTIONS),
            ["Top", "Center", "Bottom", "Space", "None"]
        );
        let ops = |o: &[(signex_i18n::MessageId, Option<AlignOp>)]| {
            o.iter().map(|&(_, op)| op).collect::<Vec<_>>()
        };
        assert_eq!(
            ops(&HORIZONTAL_OPTIONS),
            [
                Some(AlignOp::Left),
                Some(AlignOp::CenterH),
                Some(AlignOp::Right),
                Some(AlignOp::DistributeH),
                None
            ]
        );
        assert_eq!(
            ops(&VERTICAL_OPTIONS),
            [
                Some(AlignOp::Top),
                Some(AlignOp::CenterV),
                Some(AlignOp::Bottom),
                Some(AlignOp::DistributeV),
                None
            ]
        );
    }
}
