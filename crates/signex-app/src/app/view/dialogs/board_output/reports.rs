//! The Board Reports dialog (mockup row 117): the kind switch, the filter field and
//! the row count over the table (`reports_table.rs`), then the unit caption
//! with Cancel and Export CSV…. The filter and the sort are view only;
//! Export CSV writes the report as the board makes it.

use iced::widget::{column, container, row};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{I18n, LocalNumber, Localized, MessageId, UserText, ids, t};
use signex_output::{BoardReportKind, ReportCell, ReportSide};
use signex_types::coord::Unit;
use signex_types::measure::length_field_text;
use signex_types::theme::ThemeTokens;
use signex_widgets::{Label, segmented, text, text_input};

use super::{SP, TS, kit, out};
use crate::app::{BoardOutputMsg, BoardReportsState, Message};
use crate::styles::{DialogButtonKind, chrome_px, dialog_button, dialog_footer_split, tokens};

type Parts<'a> = (
    Element<'a, Message>,
    Element<'a, Message>,
    Element<'a, Message>,
);

/// Width of the kind switch (440 px).
const SWITCH_W: f32 = chrome_px(440.0 * 11.0 / 12.0);

pub(super) fn kind_label(kind: BoardReportKind) -> MessageId {
    match kind {
        BoardReportKind::Bom => ids::BOARD_REPORT_KIND_BOM,
        BoardReportKind::Netlist => ids::BOARD_REPORT_KIND_NETLIST,
        BoardReportKind::NetLength => ids::BOARD_REPORT_KIND_NET_LENGTH,
        BoardReportKind::Components => ids::BOARD_REPORT_KIND_COMPONENTS,
    }
}

/// The unit id a caption selects on: `mil`, else `mm`.
fn unit_key(unit: Unit) -> &'static str {
    match unit {
        Unit::Mil => "mil",
        _ => "mm",
    }
}

/// The footer caption of report `kind`: what its rows are and the unit.
pub(super) fn kind_caption(kind: BoardReportKind, unit: Unit, i18n: &I18n) -> Localized {
    let unit = unit_key(unit);
    match kind {
        BoardReportKind::Bom => t!(i18n, ids::BOARD_REPORTS_CAPTION_BOM),
        BoardReportKind::Netlist => t!(i18n, ids::BOARD_REPORTS_CAPTION_NETLIST),
        BoardReportKind::NetLength => {
            t!(i18n, ids::BOARD_REPORTS_CAPTION_NET_LENGTH, "unit" => unit)
        }
        BoardReportKind::Components => {
            t!(i18n, ids::BOARD_REPORTS_CAPTION_COMPONENTS, "unit" => unit)
        }
    }
}

/// A cell as the dialog shows it: lengths as bare numbers in the user's
/// unit (the footer caption names it), sides and types in words.
pub(super) fn cell_text(cell: &ReportCell, unit: Unit, i18n: &I18n) -> Label<'static> {
    match cell {
        ReportCell::Text(text) => UserText::new(text.clone()).into(),
        ReportCell::Count(n) => t!(i18n, ids::BOARD_REPORT_VALUE, "value" => *n).into(),
        ReportCell::Millimetres(mm) => {
            t!(i18n, ids::BOARD_REPORT_VALUE, "value" => length_field_text(*mm, unit)).into()
        }
        ReportCell::Degrees(deg) => {
            let rounded = (deg * 100.0).round() / 100.0;
            t!(i18n, ids::BOARD_REPORT_DEGREES, "value" => LocalNumber(rounded)).into()
        }
        ReportCell::Side(ReportSide::Top) => t!(i18n, ids::BOARD_REPORT_SIDE_TOP).into(),
        ReportCell::Side(ReportSide::Bottom) => t!(i18n, ids::BOARD_REPORT_SIDE_BOTTOM).into(),
        ReportCell::Type(kind) => t!(
            i18n,
            crate::panels::pcb_component::component_type_label(*kind)
        )
        .into(),
        ReportCell::Missing => t!(i18n, ids::BOARD_REPORT_MISSING).into(),
    }
}

/// Header, the toolbar over the table, and the footer.
pub(super) fn parts<'a>(
    state: &'a BoardReportsState,
    unit: Unit,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Parts<'a> {
    let close = Some(out(BoardOutputMsg::Closed));
    let header = super::super::header::graphite_header_closing(
        t!(i18n, ids::BOARD_REPORTS_TITLE).into_string(),
        Some(
            t!(i18n, ids::BOARD_REPORTS_SUBTITLE, "board" => state.board_name.as_str())
                .into_string(),
        ),
        None,
        close.clone(),
        super::super::header::DIALOG_HEADER_H,
        tokens,
    );
    let shown = &state.shown;
    let body = column![
        toolbar(state, shown.len(), i18n, tokens),
        kit::rule(tokens::resolve(tokens).line_soft),
        super::reports_table::table(state, shown, unit, i18n, tokens),
    ]
    .height(Length::Fill)
    .boxed();
    let caption = text(kind_caption(state.report.kind, unit, i18n))
        .size(TS.body)
        .color(tokens::resolve(tokens).ink3)
        .boxed();
    let footer = dialog_footer_split(
        vec![caption],
        vec![
            dialog_button(
                t!(i18n, ids::BOARD_REPORTS_CANCEL),
                DialogButtonKind::Cancel,
                close,
                tokens,
            ),
            dialog_button(
                t!(i18n, ids::BOARD_REPORTS_EXPORT_CSV),
                super::super::graphite::call_to_action(tokens),
                Some(out(BoardOutputMsg::CsvExportRequested)),
                tokens,
            ),
        ],
    );
    (header, body, footer)
}

/// The kind switch, the filter field and the row count.
fn toolbar<'a>(
    state: &'a BoardReportsState,
    shown: usize,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let kinds = segmented(
        BoardReportKind::ALL.map(|kind| (kind, Label::from(t!(i18n, kind_label(kind))))),
        state.report.kind,
        kit::segmented_style(tokens).fill_width(true),
        |kind| out(BoardOutputMsg::ReportPicked(kind)),
    );
    let filter = filter_field(state, i18n, tokens);
    let total = state.report.rows.len();
    let count = if shown == total {
        t!(i18n, ids::BOARD_REPORTS_ROWS, "count" => total)
    } else {
        t!(i18n, ids::BOARD_REPORTS_ROWS_FILTERED, "shown" => shown, "count" => total)
    };
    let count = text(count)
        .size(TS.caption)
        .font(super::super::graphite::mono())
        .color(tokens::resolve(tokens).ink4);
    container(
        row![
            container(kinds).width(Length::Fixed(SWITCH_W)),
            filter,
            count
        ]
        .spacing(SP.l - SP.xs)
        .align_y(Alignment::Center),
    )
    .padding([SP.l - SP.xs, SP.l + SP.xs])
    .width(Length::Fill)
    .boxed()
}

/// The Filter rows field, led by a magnifier inside it (mockup row 117).
pub(super) fn filter_field<'a>(
    state: &'a BoardReportsState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let input = text_input(t!(i18n, ids::BOARD_REPORTS_FILTER), &state.filter)
        .on_input(|v| out(BoardOutputMsg::ReportFilterEdited(v)))
        .size(TS.body)
        .padding(SP.s + SP.xs)
        .width(Length::Fill);
    kit::glyph_field(kit::FieldGlyph::Search, input, tokens)
}
