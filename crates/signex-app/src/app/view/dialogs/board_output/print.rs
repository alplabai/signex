//! The Board Print / PDF dialog: the layer sets, the page layout, the scale,
//! the ink and what each page shows, then Export PDF, or Export Assembly SVG
//! for the assembly drawings (#2486).

use iced::widget::{Column, container, scrollable};
use iced::{Element, Length, Widget as _};
use signex_i18n::{I18n, MessageId, ids, t};
use signex_output::{BoardLayerSet, BoardPrintPages, BoardPrintScale};
use signex_types::theme::ThemeTokens;
use signex_widgets::{CheckboxLabel as _, checkbox, text};

use super::{SP, TS, captioned, out, pills};
use crate::app::board_output::set_label;
use crate::app::{BoardOutputMsg, BoardPrintFlag, BoardPrintState, Message};
use crate::panels::form_look::checkbox_style;
use crate::styles::{DialogButtonKind, dialog_button, dialog_footer_split, tokens};

type Parts<'a> = (
    Element<'a, Message>,
    Element<'a, Message>,
    Element<'a, Message>,
);

fn flag_label(flag: BoardPrintFlag) -> MessageId {
    match flag {
        BoardPrintFlag::MirrorBottom => ids::BOARD_PRINT_MIRROR_BOTTOM,
        BoardPrintFlag::Outline => ids::BOARD_PRINT_OUTLINE,
        BoardPrintFlag::DrillMarks => ids::BOARD_PRINT_DRILL_MARKS,
        BoardPrintFlag::TitleBlock => ids::BOARD_PRINT_TITLE_BLOCK,
    }
}

/// A labelled checkbox that sends `msg` when clicked.
fn tick<'a>(label: MessageId, on: bool, msg: BoardOutputMsg, i18n: &I18n) -> Element<'a, Message> {
    checkbox(on)
        .size(TS.title)
        .labelled(t!(i18n, label))
        .text_size(TS.body)
        .style(checkbox_style)
        .on_toggle(move |_| out(msg.clone()))
        .boxed()
}

/// Header, the options and the footer with Export PDF and Close.
pub(super) fn parts<'a>(
    state: &'a BoardPrintState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Parts<'a> {
    let close = Some(out(BoardOutputMsg::Closed));
    let header = super::super::header::graphite_header_closing(
        t!(i18n, ids::BOARD_PRINT_TITLE).into_string(),
        Some(
            t!(i18n, ids::BOARD_PRINT_SUBTITLE, "board" => state.board_name.as_str()).into_string(),
        ),
        None,
        close.clone(),
        super::super::header::DIALOG_HEADER_H,
        tokens,
    );
    let options = &state.options;
    let mut sets: Column<Element<'a, Message>> = Column::new().spacing(SP.s);
    for set in BoardLayerSet::ALL {
        sets = sets.push(tick(
            set_label(set),
            options.sets.contains(&set),
            BoardOutputMsg::PrintSetToggled(set),
            i18n,
        ));
    }
    let pages = pills(
        vec![
            (
                t!(i18n, ids::BOARD_PRINT_PAGES_PER_SET),
                options.pages == BoardPrintPages::PerSet,
                BoardOutputMsg::PrintPagesPicked(BoardPrintPages::PerSet),
            ),
            (
                t!(i18n, ids::BOARD_PRINT_PAGES_COMPOSITE),
                options.pages == BoardPrintPages::Composite,
                BoardOutputMsg::PrintPagesPicked(BoardPrintPages::Composite),
            ),
        ],
        tokens,
    );
    let scale = pills(
        vec![
            (
                t!(i18n, ids::BOARD_PRINT_SCALE_FIT),
                options.scale == BoardPrintScale::Fit,
                BoardOutputMsg::PrintScalePicked(BoardPrintScale::Fit),
            ),
            (
                t!(i18n, ids::BOARD_PRINT_SCALE_ONE),
                options.scale == BoardPrintScale::OneToOne,
                BoardOutputMsg::PrintScalePicked(BoardPrintScale::OneToOne),
            ),
        ],
        tokens,
    );
    let ink = pills(
        vec![
            (
                t!(i18n, ids::BOARD_PRINT_INK_COLOUR),
                !options.monochrome,
                BoardOutputMsg::PrintMonochromePicked(false),
            ),
            (
                t!(i18n, ids::BOARD_PRINT_INK_MONO),
                options.monochrome,
                BoardOutputMsg::PrintMonochromePicked(true),
            ),
        ],
        tokens,
    );
    let mut show: Column<Element<'a, Message>> = Column::new().spacing(SP.s);
    for flag in BoardPrintFlag::ALL {
        show = show.push(tick(
            flag_label(flag),
            flag.get(options),
            BoardOutputMsg::PrintFlagToggled(flag),
            i18n,
        ));
    }
    let plan = options.page_plan();
    let summary = if plan.is_empty() {
        text(t!(i18n, ids::BOARD_PRINT_NO_SETS))
            .size(TS.body)
            .color(tokens::danger(tokens))
    } else {
        text(t!(i18n, ids::BOARD_PRINT_PAGE_COUNT, "count" => plan.len()))
            .size(TS.body)
            .color(tokens::muted_text(tokens))
    };
    let form = Column::new()
        .spacing(SP.l)
        .push(captioned(
            t!(i18n, ids::BOARD_PRINT_SETS),
            sets.boxed(),
            tokens,
        ))
        .push(captioned(t!(i18n, ids::BOARD_PRINT_PAGES), pages, tokens))
        .push(captioned(t!(i18n, ids::BOARD_PRINT_SCALE), scale, tokens))
        .push(captioned(t!(i18n, ids::BOARD_PRINT_INK), ink, tokens))
        .push(captioned(
            t!(i18n, ids::BOARD_PRINT_SHOW),
            show.boxed(),
            tokens,
        ))
        .push(summary.boxed());
    let body = scrollable(container(form).padding([SP.l, SP.xl]))
        .height(Length::Fill)
        .boxed();
    let footer = dialog_footer_split(
        vec![dialog_button(
            t!(i18n, ids::BOARD_PRINT_EXPORT_SVG),
            DialogButtonKind::Secondary,
            Some(out(BoardOutputMsg::SvgExportRequested)),
            tokens,
        )],
        vec![
            dialog_button(
                t!(i18n, ids::BOARD_PRINT_CLOSE),
                DialogButtonKind::Cancel,
                close,
                tokens,
            ),
            dialog_button(
                t!(i18n, ids::BOARD_PRINT_EXPORT),
                super::super::graphite::call_to_action(tokens),
                (!plan.is_empty()).then(|| out(BoardOutputMsg::PdfExportRequested)),
                tokens,
            ),
        ],
    );
    (header, body, footer)
}
