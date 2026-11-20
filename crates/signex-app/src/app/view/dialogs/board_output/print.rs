//! The Board Print / PDF dialog (mockup row 116): the options on the left (the layer
//! sets, the Layout group of segmented controls, what each page shows), the
//! page plan on the right, then Export Assembly SVG… on the left of the
//! footer and Cancel and Export PDF… on the right.

use iced::widget::{Column, Row, container, row, scrollable};
use iced::{Alignment, Background, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, MessageId, ids, t};
use signex_output::{BoardLayerSet, BoardPrintOptions, BoardPrintPages, BoardPrintScale};
use signex_types::pcb::PcbBoard;
use signex_types::theme::ThemeTokens;
use signex_widgets::{Label, segmented, text, tooltip};

use super::{SP, TS, kit, out};
use crate::app::board_output::set_label;
use crate::app::{BoardOutputMsg, BoardPrintFlag, BoardPrintState, Message};
use crate::styles::{
    DialogButtonKind, HAIRLINE, chrome_px, dialog_button, dialog_footer_split, tokens,
};

type Parts<'a> = (
    Element<'a, Message>,
    Element<'a, Message>,
    Element<'a, Message>,
);

/// Width of the options column (340 px).
const OPTIONS_W: f32 = chrome_px(340.0 * 11.0 / 12.0);
/// Width of the Layout group's labels (52 px).
const LAYOUT_LABEL_W: f32 = chrome_px(52.0 * 11.0 / 12.0);

fn flag_label(flag: BoardPrintFlag) -> MessageId {
    match flag {
        BoardPrintFlag::MirrorBottom => ids::BOARD_PRINT_MIRROR_BOTTOM,
        BoardPrintFlag::Outline => ids::BOARD_PRINT_OUTLINE,
        BoardPrintFlag::DrillMarks => ids::BOARD_PRINT_DRILL_MARKS,
        BoardPrintFlag::TitleBlock => ids::BOARD_PRINT_TITLE_BLOCK,
    }
}

/// Header, the options beside the page plan, and the footer.
pub(super) fn parts<'a>(
    state: &'a BoardPrintState,
    board: Option<&PcbBoard>,
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
    let left = scrollable(container(options_column(options, i18n, tokens)).padding(
        iced::Padding {
            top: SP.xs * 2.0,
            right: SP.xl + SP.xs,
            bottom: SP.l,
            left: SP.xl + SP.xs,
        },
    ))
    .direction(crate::styles::thin_vertical())
    .height(Length::Fill);
    let sunken = tokens::resolve(tokens).sunken;
    let right = container(
        scrollable(
            container(super::print_plan::plan(options, board, i18n, tokens)).padding(SP.l + SP.xs),
        )
        .direction(crate::styles::thin_vertical())
        .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(sunken)),
        ..container::Style::default()
    });
    let body = row![
        container(left).width(Length::Fixed(OPTIONS_W)),
        vertical_rule(tokens),
        right
    ]
    .height(Length::Fill)
    .boxed();
    let can_export = !options.page_plan().is_empty();
    let footer = dialog_footer_split(
        vec![dialog_button(
            t!(i18n, ids::BOARD_PRINT_EXPORT_SVG),
            DialogButtonKind::Secondary,
            Some(out(BoardOutputMsg::SvgExportRequested)),
            tokens,
        )],
        vec![
            dialog_button(
                t!(i18n, ids::BOARD_PRINT_CANCEL),
                DialogButtonKind::Cancel,
                close,
                tokens,
            ),
            dialog_button(
                t!(i18n, ids::BOARD_PRINT_EXPORT),
                super::super::graphite::call_to_action(tokens),
                can_export.then(|| out(BoardOutputMsg::PdfExportRequested)),
                tokens,
            ),
        ],
    );
    (header, body, footer)
}

/// The soft line between the options and the page plan.
fn vertical_rule<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    let ink = tokens::resolve(tokens).line_soft;
    container(iced::widget::Space::new())
        .width(Length::Fixed(HAIRLINE))
        .height(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(ink)),
            ..container::Style::default()
        })
        .boxed()
}

/// The three option groups: Layer Sets, Layout and Show.
fn options_column<'a>(
    options: &BoardPrintOptions,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let ticked = BoardLayerSet::ALL
        .iter()
        .filter(|set| options.sets.contains(set))
        .count();
    let count = t!(
        i18n,
        ids::BOARD_PRINT_SETS_COUNT,
        "ticked" => ticked,
        "total" => BoardLayerSet::ALL.len()
    );
    let mut sets: Column<Element<'a, Message>> = Column::new().spacing(SP.s).push(
        kit::section_head(t!(i18n, ids::BOARD_PRINT_SETS), Some(count), i18n, tokens),
    );
    for set in BoardLayerSet::ALL {
        sets = sets.push(kit::tick(
            options.sets.contains(&set),
            t!(i18n, set_label(set)),
            Some(out(BoardOutputMsg::PrintSetToggled(set))),
        ));
    }
    Column::new()
        .spacing(SP.xl)
        .push(sets.boxed())
        .push(layout_group(options, i18n, tokens))
        .push(show_group(options, i18n, tokens))
        .boxed()
}

/// Pages, Scale and Ink as aligned segmented controls.
fn layout_group<'a>(
    options: &BoardPrintOptions,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let style = kit::segmented_style(tokens).fill_width(true);
    let pages = segmented(
        [
            (
                BoardPrintPages::PerSet,
                Label::from(t!(i18n, ids::BOARD_PRINT_PAGES_PER_SET)),
            ),
            (
                BoardPrintPages::Composite,
                Label::from(t!(i18n, ids::BOARD_PRINT_PAGES_COMPOSITE)),
            ),
        ],
        options.pages,
        style,
        |pages| out(BoardOutputMsg::PrintPagesPicked(pages)),
    );
    let scale = segmented(
        [
            (
                BoardPrintScale::Fit,
                Label::from(t!(i18n, ids::BOARD_PRINT_SCALE_FIT)),
            ),
            (
                BoardPrintScale::OneToOne,
                Label::from(t!(i18n, ids::BOARD_PRINT_SCALE_ONE)),
            ),
        ],
        options.scale,
        style,
        |scale| out(BoardOutputMsg::PrintScalePicked(scale)),
    );
    let ink = segmented(
        [
            (false, Label::from(t!(i18n, ids::BOARD_PRINT_INK_COLOUR))),
            (true, Label::from(t!(i18n, ids::BOARD_PRINT_INK_MONO))),
        ],
        options.monochrome,
        style,
        |mono| out(BoardOutputMsg::PrintMonochromePicked(mono)),
    );
    Column::new()
        .spacing(SP.s + SP.xs)
        .push(kit::section_head(
            t!(i18n, ids::BOARD_PRINT_LAYOUT),
            None,
            i18n,
            tokens,
        ))
        .push(layout_line(t!(i18n, ids::BOARD_PRINT_PAGES), pages, tokens))
        .push(layout_line(t!(i18n, ids::BOARD_PRINT_SCALE), scale, tokens))
        .push(layout_line(t!(i18n, ids::BOARD_PRINT_INK), ink, tokens))
        .boxed()
}

/// One labelled line of the Layout group.
fn layout_line<'a>(
    label: Localized,
    control: Element<'a, Message>,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let ink = tokens::resolve(tokens).ink3;
    let line: Row<Element<'a, Message>> = Row::new()
        .spacing(SP.l - SP.xs)
        .align_y(Alignment::Center)
        .push(
            text(label)
                .size(TS.body)
                .color(ink)
                .width(Length::Fixed(LAYOUT_LABEL_W))
                .boxed(),
        )
        .push(control);
    line.boxed()
}

/// What each page shows. Mirror the Bottom Side is greyed and deaf while
/// no bottom set is ticked, with a hover note that says why.
fn show_group<'a>(
    options: &BoardPrintOptions,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let mut col: Column<Element<'a, Message>> = Column::new().spacing(SP.s).push(
        kit::section_head(t!(i18n, ids::BOARD_PRINT_SHOW), None, i18n, tokens),
    );
    let has_bottom = options.has_bottom_set();
    for flag in BoardPrintFlag::ALL {
        let live = flag != BoardPrintFlag::MirrorBottom || has_bottom;
        let tick = kit::tick(
            flag.get(options),
            t!(i18n, flag_label(flag)),
            live.then(|| out(BoardOutputMsg::PrintFlagToggled(flag))),
        );
        let control = if live {
            tick
        } else {
            let note =
                container(text(t!(i18n, ids::BOARD_PRINT_MIRROR_NEEDS_BOTTOM)).size(TS.body))
                    .padding([SP.xs, SP.s]);
            tooltip(
                kit::dependent(false, tick, tokens),
                note,
                tooltip::Position::Bottom,
            )
            .style(crate::styles::context_menu(tokens))
            .boxed()
        };
        col = col.push(control);
    }
    col.boxed()
}
