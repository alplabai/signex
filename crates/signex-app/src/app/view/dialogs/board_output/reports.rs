//! The Board Reports dialog: a tab per report, the table in view and Export
//! CSV.

use iced::widget::{Column, Row, column, container, scrollable};
use iced::{Element, Length, Widget as _};
use signex_i18n::{I18n, LocalNumber, MessageId, UserText, ids, t};
use signex_output::{BoardReportKind, ReportCell, ReportColumn, ReportSide};
use signex_types::coord::Unit;
use signex_types::measure::format_length_mm;
use signex_types::theme::ThemeTokens;
use signex_widgets::{Label, text};

use super::{SP, TS, out, pills};
use crate::app::{BoardOutputMsg, BoardReportsState, Message};
use crate::styles::{DialogButtonKind, dialog_button, dialog_footer_split, ti, tokens};

type Parts<'a> = (
    Element<'a, Message>,
    Element<'a, Message>,
    Element<'a, Message>,
);

fn kind_label(kind: BoardReportKind) -> MessageId {
    match kind {
        BoardReportKind::Bom => ids::BOARD_REPORT_KIND_BOM,
        BoardReportKind::Netlist => ids::BOARD_REPORT_KIND_NETLIST,
        BoardReportKind::NetLength => ids::BOARD_REPORT_KIND_NET_LENGTH,
        BoardReportKind::Components => ids::BOARD_REPORT_KIND_COMPONENTS,
    }
}

fn column_label(column: ReportColumn) -> MessageId {
    use ReportColumn as C;
    match column {
        C::Designators => ids::BOARD_REPORT_COL_DESIGNATORS,
        C::Quantity => ids::BOARD_REPORT_COL_QUANTITY,
        C::Value => ids::BOARD_REPORT_COL_VALUE,
        C::Footprint => ids::BOARD_REPORT_COL_FOOTPRINT,
        C::Description => ids::BOARD_REPORT_COL_DESCRIPTION,
        C::Type => ids::BOARD_REPORT_COL_TYPE,
        C::Net => ids::BOARD_REPORT_COL_NET,
        C::NodeCount => ids::BOARD_REPORT_COL_NODE_COUNT,
        C::Nodes => ids::BOARD_REPORT_COL_NODES,
        C::Length => ids::BOARD_REPORT_COL_LENGTH,
        C::Tracks => ids::BOARD_REPORT_COL_TRACKS,
        C::Vias => ids::BOARD_REPORT_COL_VIAS,
        C::Designator => ids::BOARD_REPORT_COL_DESIGNATOR,
        C::X => ids::BOARD_REPORT_COL_X,
        C::Y => ids::BOARD_REPORT_COL_Y,
        C::Rotation => ids::BOARD_REPORT_COL_ROTATION,
        C::Side => ids::BOARD_REPORT_COL_SIDE,
    }
}

/// How wide a column is, in parts of the table's width: lists of
/// designators and pads take the most.
fn column_part(column: ReportColumn) -> u16 {
    match column {
        ReportColumn::Designators | ReportColumn::Nodes => 5,
        ReportColumn::Description | ReportColumn::Footprint => 3,
        ReportColumn::Value | ReportColumn::Net | ReportColumn::Designator => 2,
        _ => 1,
    }
}

/// A cell as the dialog shows it: lengths in the user's unit, sides and
/// types in words.
fn cell_text(cell: &ReportCell, unit: Unit, i18n: &I18n) -> Label<'static> {
    match cell {
        ReportCell::Text(text) => UserText::new(text.clone()).into(),
        ReportCell::Count(n) => t!(i18n, ids::BOARD_REPORT_VALUE, "value" => *n).into(),
        ReportCell::Millimetres(mm) => {
            t!(i18n, ids::BOARD_REPORT_VALUE, "value" => format_length_mm(*mm, unit)).into()
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

/// Header, tabs and table, and the footer with Export CSV and Close.
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
    let report = &state.report;
    let tabs = pills(
        BoardReportKind::ALL
            .into_iter()
            .map(|kind| {
                (
                    t!(i18n, kind_label(kind)),
                    kind == report.kind,
                    BoardOutputMsg::ReportPicked(kind),
                )
            })
            .collect(),
        tokens,
    );
    let count = text(t!(i18n, ids::BOARD_REPORTS_ROWS, "count" => report.rows.len()))
        .size(TS.body)
        .color(tokens::muted_text(tokens));
    let body = column![
        container(tabs).padding([SP.l, SP.xl]),
        headings(&report.columns, i18n, tokens),
        scrollable(table(state, unit, i18n, tokens)).height(Length::Fill),
        container(count).padding([SP.s, SP.xl]),
    ]
    .height(Length::Fill)
    .boxed();
    let footer = dialog_footer_split(
        Vec::new(),
        vec![
            dialog_button(
                t!(i18n, ids::BOARD_REPORTS_CLOSE),
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

/// The column headings, fixed above the scrolling rows.
fn headings<'a>(
    columns: &[ReportColumn],
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let muted = tokens::muted_text(tokens);
    let mut line: Row<Element<'a, Message>> = Row::new().spacing(SP.m).padding([SP.xs, SP.xl]);
    for column in columns {
        line = line.push(
            text(t!(i18n, column_label(*column)))
                .size(TS.caption)
                .color(muted)
                .width(Length::FillPortion(column_part(*column)))
                .boxed(),
        );
    }
    line.boxed()
}

/// One line a row, or a muted note when the report has none.
fn table<'a>(
    state: &'a BoardReportsState,
    unit: Unit,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let report = &state.report;
    if report.rows.is_empty() {
        return container(
            text(t!(i18n, ids::BOARD_REPORTS_EMPTY))
                .size(TS.body)
                .color(tokens::muted_text(tokens)),
        )
        .padding([SP.m, SP.xl])
        .boxed();
    }
    let ink = ti(tokens.text);
    let mut rows: Column<Element<'a, Message>> = Column::new().spacing(SP.xs);
    for cells in &report.rows {
        let mut line: Row<Element<'a, Message>> = Row::new().spacing(SP.m).padding([SP.xs, SP.xl]);
        for (column, cell) in report.columns.iter().zip(cells) {
            line = line.push(
                text(cell_text(cell, unit, i18n))
                    .size(TS.body)
                    .color(ink)
                    .width(Length::FillPortion(column_part(*column)))
                    .boxed(),
            );
        }
        rows = rows.push(line.boxed());
    }
    rows.boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every column has a heading and every kind a tab name in the catalog.
    #[test]
    fn every_column_and_kind_has_a_catalog_name() {
        let i18n = I18n::en_us();
        for kind in BoardReportKind::ALL {
            assert!(!t!(i18n, kind_label(kind)).into_string().is_empty());
            for column in kind.columns() {
                let label = t!(i18n, column_label(*column)).into_string();
                assert!(!label.is_empty(), "{column:?}");
            }
        }
    }

    #[test]
    fn lengths_show_in_the_users_unit_and_sides_in_words() {
        let i18n = I18n::en_us();
        let mm = cell_text(&ReportCell::Millimetres(2.54), Unit::Mm, &i18n);
        assert!(mm.as_str().contains("2.54"), "{}", mm.as_str());
        let mil = cell_text(&ReportCell::Millimetres(2.54), Unit::Mil, &i18n);
        assert!(mil.as_str().contains("100"), "{}", mil.as_str());
        let side = cell_text(&ReportCell::Side(ReportSide::Bottom), Unit::Mm, &i18n);
        assert_eq!(side.as_str(), "Bottom");
    }
}
