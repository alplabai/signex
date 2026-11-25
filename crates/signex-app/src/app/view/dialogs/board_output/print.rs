//! The Board Print / PDF dialog (mockup row 116): the options on the left (the layer
//! sets, the Layout group of segmented controls, what each page shows), the
//! page plan on the right, then Export Assembly SVG… on the left of the
//! footer and Cancel and Export PDF… on the right.

use iced::widget::{Column, Row, container, row, scrollable};
use iced::{Background, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, MessageId, ids, t};
use signex_output::{BoardLayerSet, BoardPrintOptions, BoardPrintPages, BoardPrintScale};
use signex_types::pcb::PcbBoard;
use signex_types::theme::ThemeTokens;
use signex_widgets::segmented::SegmentedMetrics;
use signex_widgets::{Label, MeasuredWidth, SegmentedStyle, segmented, text, tooltip};

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

/// The options column's least width (340 px, mockup row 116). It is wider
/// when the Layout lines need more to keep each control on one line.
const OPTIONS_MIN: f32 = chrome_px(340.0 * 11.0 / 12.0);

/// The options column's side padding.
const OPTIONS_SIDE: f32 = SP.xl + SP.xs;

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
            right: OPTIONS_SIDE,
            bottom: SP.l,
            left: OPTIONS_SIDE,
        },
    ))
    .direction(crate::styles::thin_vertical())
    .height(Length::Fill);
    // The column is as wide as the Layout lines laid out on their own, so
    // the Pages segments keep one line in whatever the UI font measures.
    let style = layout_style(tokens);
    let measure = container(layout_grid(
        layout_lines(options, style, i18n),
        style,
        tokens,
    ))
    .padding([0.0, OPTIONS_SIDE]);
    let left: Element<'a, Message> = MeasuredWidth::new(left, measure)
        .min_width(OPTIONS_MIN)
        .into();
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
    let body = row![left, vertical_rule(tokens), right]
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
    let style = layout_style(tokens);
    Column::new()
        .spacing(SP.s + SP.xs)
        .push(kit::section_head(
            t!(i18n, ids::BOARD_PRINT_LAYOUT),
            None,
            i18n,
            tokens,
        ))
        .push(layout_grid(
            layout_lines(options, style, i18n),
            style,
            tokens,
        ))
        .boxed()
}

/// The look of the Layout group's segmented controls.
fn layout_style(tokens: &ThemeTokens) -> SegmentedStyle {
    kit::segmented_style(tokens).fill_width(true).compact(true)
}

/// The Layout group's labelled lines: Pages, Scale and Ink.
fn layout_lines<'a>(
    options: &BoardPrintOptions,
    style: SegmentedStyle,
    i18n: &I18n,
) -> [(Localized, Element<'a, Message>); 3] {
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
    [
        (t!(i18n, ids::BOARD_PRINT_PAGES), pages),
        (t!(i18n, ids::BOARD_PRINT_SCALE), scale),
        (t!(i18n, ids::BOARD_PRINT_INK), ink),
    ]
}

/// The Layout group's labelled lines: the labels in a column as wide as
/// the widest of them, the controls taking the rest. The options column is
/// measured from this grid, so the two-segment Pages control keeps one
/// line (mockup row 116).
fn layout_grid<'a>(
    lines: [(Localized, Element<'a, Message>); 3],
    style: SegmentedStyle,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let ink = tokens::resolve(tokens).ink3;
    let line_h = SegmentedMetrics::from_text_size(style.text_size).track_height;
    let gap = SP.s + SP.xs;
    let mut labels: Column<Element<'a, Message>> = Column::new().spacing(gap);
    let mut controls: Column<Element<'a, Message>> = Column::new().spacing(gap).width(Length::Fill);
    for (label, control) in lines {
        labels = labels.push(
            container(text(label).size(TS.body).color(ink))
                .height(Length::Fixed(line_h))
                .align_y(iced::alignment::Vertical::Center)
                .boxed(),
        );
        controls = controls.push(control);
    }
    let grid: Row<Element<'a, Message>> = Row::new()
        .spacing(SP.l - SP.xs)
        .push(labels.boxed())
        .push(controls.boxed());
    grid.boxed()
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
                    .padding([SP.xs, SP.s])
                    .style(crate::styles::context_menu(tokens));
            tooltip(
                kit::dependent(false, tick, tokens),
                iced::widget::container(note).style(crate::styles::context_menu(tokens)),
                tooltip::Position::Bottom,
            )
            .boxed()
        };
        col = col.push(control);
    }
    col.boxed()
}
