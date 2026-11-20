//! Mockup row 117 on screen: the catalog names, cells in the user's unit, numbers
//! right-aligned in their own columns, one growing column a report, the
//! rows in the shown order, and the count and caption.

use std::path::PathBuf;

use iced::widget::column;
use iced::{Length, Size, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_output::{BoardReport, BoardReportKind, ReportCell, ReportColumn, ReportSide};
use signex_types::coord::Unit;
use signex_types::theme::{ThemeId, theme_tokens};

use super::reports::{cell_text, kind_caption, kind_label, parts};
use super::reports_table::{column_label, column_width, is_numeric};
use crate::app::BoardReportsState;
use crate::app::board_output::ReportSort;
use crate::layout_probe::lay_out_texts;

/// Every column has a heading and every kind a name in the catalog.
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
    assert!(
        !mm.as_str().contains("mm"),
        "the caption names the unit, the cells hold the number"
    );
    let caption = kind_caption(BoardReportKind::NetLength, Unit::Mil, &i18n).to_string();
    assert!(caption.contains("mil"), "{caption}");
}

#[test]
fn numbers_sit_right_in_their_own_columns() {
    use ReportColumn as C;
    for column in [
        C::Quantity,
        C::NodeCount,
        C::Length,
        C::Tracks,
        C::Vias,
        C::X,
        C::Y,
        C::Rotation,
    ] {
        assert!(is_numeric(column), "{column:?} is right-aligned");
    }
    for column in [
        C::Designators,
        C::Net,
        C::Nodes,
        C::Value,
        C::Footprint,
        C::Side,
    ] {
        assert!(!is_numeric(column), "{column:?} reads from the left");
    }
}

#[test]
fn each_report_lets_one_column_grow() {
    for kind in BoardReportKind::ALL {
        let growing = kind
            .columns()
            .iter()
            .filter(|column| column_width(kind, **column) == Length::Fill)
            .count();
        assert_eq!(growing, 1, "{kind:?}");
    }
}

fn net_lengths() -> BoardReportsState {
    let row = |net: &str, mm: f64| {
        vec![
            ReportCell::Text(net.into()),
            ReportCell::Millimetres(mm),
            ReportCell::Count(1),
            ReportCell::Count(0),
        ]
    };
    BoardReportsState::new(
        PathBuf::from("dcdc.snxpcb"),
        "dcdc.snxpcb".into(),
        BoardReport {
            kind: BoardReportKind::NetLength,
            columns: BoardReportKind::NetLength.columns().to_vec(),
            rows: vec![row("FB", 9.4), row("SW", 24.6), row("VIN", 21.9)],
        },
    )
}

fn shown_texts(state: &BoardReportsState, i18n: &I18n) -> Vec<String> {
    let tokens = theme_tokens(ThemeId::BUILTINS[0]);
    let (header, body, footer) = parts(state, Unit::Mm, i18n, &tokens);
    lay_out_texts(
        column![header, body, footer].boxed(),
        Size::new(900.0, 620.0),
    )
    .into_iter()
    .map(|t| t.text)
    .collect()
}

#[test]
fn the_rows_show_in_the_sorted_order_with_the_count_and_the_unit() {
    let i18n = I18n::en_us();
    let mut state = net_lengths();
    assert_eq!(state.sort, ReportSort::opening(BoardReportKind::NetLength));
    let texts = shown_texts(&state, &i18n);
    let at = |net: &str| texts.iter().position(|t| t == net).expect(net);
    assert!(at("SW") < at("VIN") && at("VIN") < at("FB"), "{texts:?}");
    let rows = t!(i18n, ids::BOARD_REPORTS_ROWS, "count" => 3usize).into_string();
    assert!(texts.contains(&rows), "{texts:?}");
    let caption = kind_caption(BoardReportKind::NetLength, Unit::Mm, &i18n).to_string();
    assert!(texts.contains(&caption), "{texts:?}");

    state.filter = "v".into();
    state.refresh_shown();
    let texts = shown_texts(&state, &i18n);
    assert!(!texts.iter().any(|t| t == "FB"), "filtered out: {texts:?}");
    let some = t!(
        i18n,
        ids::BOARD_REPORTS_ROWS_FILTERED,
        "shown" => 1usize,
        "count" => 3usize
    )
    .into_string();
    assert!(texts.contains(&some), "{texts:?}");

    state.filter = "nothing".into();
    state.refresh_shown();
    let texts = shown_texts(&state, &i18n);
    let none = t!(i18n, ids::BOARD_REPORTS_NO_MATCH).into_string();
    assert!(texts.contains(&none), "{texts:?}");
}
