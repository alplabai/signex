//! Constraint grid and class matrix samples, to check against the Rules tab
//! mockups. The samples are fixed: interactions map to the no-op message.

use std::sync::LazyLock;

use iced::widget::{column, container};
use iced::{Element, Widget};
use signex_types::theme::ThemeTokens;
use signex_widgets::constraint_grid::{
    CellRef, CellValue, ClassMatrix, ClassPair, ColumnGroup, ColumnId, ColumnKind, ConstraintGrid,
    GridColumn, GridMessage, GridRow, GridStyle, GridText, GroupId, I18n, MatrixClass,
    MatrixMessage, MatrixText, RowId, RowKind, matrix_view, view,
};
use signex_widgets::{Label, raw};

use crate::catalog::Message;

/// UI text size of the samples.
const TEXT_SIZE: f32 = 12.0;
/// Visible grid height in rows.
const GRID_ROWS: f32 = 13.0;

fn label(text: &'static str) -> Label<'static> {
    raw(text)
}

fn length(id: u32, title: &'static str, mm: f64) -> GridColumn {
    GridColumn {
        id: ColumnId(id),
        title: label(title),
        kind: ColumnKind::Length,
        default: CellValue::Length(mm),
    }
}

fn groups() -> Vec<ColumnGroup> {
    vec![
        ColumnGroup {
            id: GroupId(1),
            title: label("Track width"),
            columns: vec![
                length(1, "Min (mm)", 0.1),
                length(2, "Preferred (mm)", 0.2),
                length(3, "Max (mm)", 2.0),
            ],
        },
        ColumnGroup {
            id: GroupId(2),
            title: label("Via"),
            columns: vec![
                length(4, "Diameter (mm)", 0.45),
                length(5, "Drill (mm)", 0.2),
                GridColumn {
                    id: ColumnId(6),
                    title: label("Style"),
                    kind: ColumnKind::Choice(vec![
                        label("Through"),
                        label("Blind"),
                        label("Micro"),
                    ]),
                    default: CellValue::Choice(0),
                },
            ],
        },
        ColumnGroup {
            id: GroupId(3),
            title: label("Neck down"),
            columns: vec![
                GridColumn {
                    id: ColumnId(7),
                    title: label("Allowed"),
                    kind: ColumnKind::Flag,
                    default: CellValue::Flag(true),
                },
                length(8, "Max length (mm)", 1.0),
            ],
        },
        ColumnGroup {
            id: GroupId(4),
            title: label("Creepage"),
            columns: vec![length(9, "Min (mm)", 0.5)],
        },
    ]
}

/// No-op fold: the samples never search.
fn same(text: &str) -> String {
    text.to_owned()
}

static GRID: LazyLock<ConstraintGrid> = LazyLock::new(|| {
    let mut grid = ConstraintGrid::new(groups());
    let rows = [
        (0, None, RowKind::All, "All"),
        (1, Some(0), RowKind::Class, "PWR"),
        (2, Some(1), RowKind::Net, "VIN_12V"),
        (3, Some(1), RowKind::Net, "VOUT_3V3"),
        (4, Some(0), RowKind::Class, "HV"),
        (5, Some(4), RowKind::Net, "L_230V"),
        (6, Some(0), RowKind::Class, "USB"),
        (7, Some(6), RowKind::Pair, "USB_D"),
        (8, Some(7), RowKind::Net, "USB_D_P"),
        (9, Some(7), RowKind::Net, "USB_D_N"),
    ];
    for (id, parent, kind, name) in rows {
        let _ = grid.push_row(GridRow {
            id: RowId(id),
            parent: parent.map(RowId),
            kind,
            name: name.to_owned(),
        });
    }
    let values = [
        (0, 2, CellValue::Length(0.15)),
        (1, 2, CellValue::Length(0.5)),
        (1, 4, CellValue::Length(0.6)),
        (2, 2, CellValue::Length(1.2)),
        (4, 9, CellValue::Length(6.4)),
        (6, 6, CellValue::Choice(2)),
        (7, 7, CellValue::Flag(false)),
    ];
    for (row, column, value) in values {
        let _ = grid.set_value(CellRef::new(RowId(row), ColumnId(column)), value);
    }
    grid.update(GridMessage::RowToggled(RowId(4)), same);
    grid.update(GridMessage::GroupToggled(GroupId(3)), same);
    grid.update(
        GridMessage::CellPressed(CellRef::new(RowId(1), ColumnId(4))),
        same,
    );
    grid
});

static MATRIX: LazyLock<ClassMatrix> = LazyLock::new(|| {
    let classes = [
        ("PWR", 0.3),
        ("HV", 0.8),
        ("USB", 0.15),
        ("DIFF", 0.15),
        ("Default", 0.2),
    ];
    let mut matrix = ClassMatrix::new(
        classes
            .into_iter()
            .map(|(name, mm)| MatrixClass {
                name: name.to_owned(),
                clearance_mm: mm,
            })
            .collect(),
    );
    matrix.set(ClassPair::new(1, 0), 0.8);
    matrix.set(ClassPair::new(4, 1), 1.5);
    matrix.update(MatrixMessage::CellPressed(ClassPair::new(1, 0)));
    matrix.update(MatrixMessage::EditCancelled);
    matrix
});

pub(crate) fn view_samples<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    let style = GridStyle::from_tokens(tokens, TEXT_SIZE);
    let i18n = I18n::en_us_ref();
    let words = GridText {
        object_header: label("Object"),
        set_here: label("Set on this row"),
        inherited_from: label("Inherited from"),
        built_in: label("Built-in default"),
        reset: label("Reset to the inherited value"),
        find_placeholder: label("Find a constraint, such as creepage"),
    };
    let matrix_words = MatrixText {
        clear: label("Clear: use the fallback"),
        fallback: label("No value of its own: the larger class clearance"),
    };
    let grid = container(view(&GRID, i18n, &words, style).map(|_| Message::TreeInteracted))
        .height(style.metrics().row_height * GRID_ROWS);
    let matrix = matrix_view(&MATRIX, i18n, &matrix_words, style).map(|_| Message::TreeInteracted);
    column![grid, matrix]
        .spacing(style.metrics().row_height)
        .boxed()
}
