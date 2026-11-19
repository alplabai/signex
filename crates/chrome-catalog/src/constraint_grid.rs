//! Constraint grid and class matrix samples, to check against the Rules tab
//! mockups. The samples are fixed: interactions map to the no-op message.
//! The sample rules stand in for the caller's rules, which the grid and
//! the matrix only read.

use std::collections::{BTreeMap, HashMap};
use std::sync::LazyLock;

use iced::widget::{column, container};
use iced::{Element, Widget};
use signex_types::coord::Unit;
use signex_types::theme::ThemeTokens;
use signex_widgets::constraint_grid::{
    CellRef, CellValue, CellValues, ClassMatrix, ClassPair, ColumnGroup, ColumnId, ColumnKind,
    ConstraintGrid, GridColumn, GridMessage, GridRow, GridStyle, GridText, GroupId, MatrixClass,
    MatrixMessage, MatrixText, PairClearance, PairValues, Resolved, RowId, RowKind, ValueSource,
    matrix_view, view,
};
use signex_widgets::theme_ext::TypeScale;
use signex_widgets::{Label, LengthFormat, raw};

use crate::catalog::Message;

/// UI text size of the samples: the body step of the type scale.
const TEXT_SIZE: f32 = TypeScale::DEFAULT.body;
/// Visible grid height in rows.
const GRID_ROWS: f32 = 13.0;

/// Sample text: fixture content, not catalog copy. The one `raw` site of
/// the page.
fn label(text: &'static str) -> Label<'static> {
    raw(text)
}

fn length(id: u32, title: &'static str) -> GridColumn {
    GridColumn {
        id: ColumnId(id),
        title: label(title),
        kind: ColumnKind::Length,
    }
}

fn groups() -> Vec<ColumnGroup> {
    vec![
        ColumnGroup {
            id: GroupId(1),
            title: label("Track width"),
            columns: vec![
                length(1, "Min (mm)"),
                length(2, "Preferred (mm)"),
                length(3, "Max (mm)"),
            ],
        },
        ColumnGroup {
            id: GroupId(2),
            title: label("Via"),
            columns: vec![
                length(4, "Diameter (mm)"),
                length(5, "Drill (mm)"),
                GridColumn {
                    id: ColumnId(6),
                    title: label("Style"),
                    kind: ColumnKind::Choice(vec![
                        label("Through"),
                        label("Blind"),
                        label("Micro"),
                    ]),
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
                },
                length(8, "Max length (mm)"),
            ],
        },
        ColumnGroup {
            id: GroupId(4),
            title: label("Creepage"),
            columns: vec![length(9, "Min (mm)")],
        },
    ]
}

/// The sample hierarchy: (id, parent, kind, name).
const ROWS: [(u32, Option<u32>, RowKind, &str); 10] = [
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

/// Sample rules: values inherit down the rows, else the column default.
struct SampleRules {
    parents: HashMap<RowId, Option<RowId>>,
    defaults: HashMap<ColumnId, CellValue>,
    own: BTreeMap<CellRef, CellValue>,
}

impl CellValues for SampleRules {
    fn resolve(&self, cell: CellRef) -> Option<Resolved> {
        let default = *self.defaults.get(&cell.column)?;
        let mut row = Some(cell.row).filter(|r| self.parents.contains_key(r))?;
        for _ in 0..=self.parents.len() {
            if let Some(value) = self.own.get(&CellRef::new(row, cell.column)) {
                let source = if row == cell.row {
                    ValueSource::Own
                } else {
                    ValueSource::Inherited(row)
                };
                return Some(Resolved {
                    value: *value,
                    source,
                });
            }
            match self.parents.get(&row).copied().flatten() {
                Some(parent) => row = parent,
                None => break,
            }
        }
        Some(Resolved {
            value: default,
            source: ValueSource::Default,
        })
    }

    fn inherited(&self, cell: CellRef) -> Option<Resolved> {
        match *self.parents.get(&cell.row)? {
            None => Some(Resolved {
                value: *self.defaults.get(&cell.column)?,
                source: ValueSource::Default,
            }),
            Some(parent) => {
                let resolved = self.resolve(CellRef::new(parent, cell.column))?;
                let source = match resolved.source {
                    ValueSource::Own => ValueSource::Inherited(parent),
                    other => other,
                };
                Some(Resolved { source, ..resolved })
            }
        }
    }
}

static RULES: LazyLock<SampleRules> = LazyLock::new(|| {
    let length = CellValue::Length;
    let defaults = [
        (1, length(0.1)),
        (2, length(0.2)),
        (3, length(2.0)),
        (4, length(0.45)),
        (5, length(0.2)),
        (6, CellValue::Choice(0)),
        (7, CellValue::Flag(true)),
        (8, length(1.0)),
        (9, length(0.5)),
    ];
    let own = [
        (0, 2, length(0.15)),
        (1, 2, length(0.5)),
        (1, 4, length(0.6)),
        (2, 2, length(1.2)),
        (4, 9, length(6.4)),
        (6, 6, CellValue::Choice(2)),
        (7, 7, CellValue::Flag(false)),
    ];
    SampleRules {
        parents: ROWS
            .iter()
            .map(|(id, parent, _, _)| (RowId(*id), parent.map(RowId)))
            .collect(),
        defaults: defaults
            .into_iter()
            .map(|(column, value)| (ColumnId(column), value))
            .collect(),
        own: own
            .into_iter()
            .map(|(row, column, value)| (CellRef::new(RowId(row), ColumnId(column)), value))
            .collect(),
    }
});

/// The samples show millimetres in the en-US locale.
fn format() -> LengthFormat<'static> {
    LengthFormat::new(crate::locale::en_us(), Unit::Mm)
}

static GRID: LazyLock<ConstraintGrid> = LazyLock::new(|| {
    let mut grid = ConstraintGrid::new(groups());
    for (id, parent, kind, name) in ROWS {
        let _ = grid.push_row(GridRow {
            id: RowId(id),
            parent: parent.map(RowId),
            kind,
            name: name.to_owned(),
        });
    }
    for message in [
        GridMessage::RowToggled(RowId(4)),
        GridMessage::GroupToggled(GroupId(3)),
        GridMessage::CellPressed(CellRef::new(RowId(1), ColumnId(4))),
    ] {
        let _ = grid.update(message, &*RULES, format());
    }
    grid
});

/// Sample clearances: one per class, plus the written pairs.
struct SampleClearances {
    classes: Vec<f64>,
    cells: BTreeMap<ClassPair, f64>,
}

/// The sample rule: an empty cell falls back to the larger of the two
/// classes' own clearances.
impl PairValues for SampleClearances {
    fn resolve(&self, pair: ClassPair) -> Option<PairClearance> {
        let fallback = self.fallback(pair)?;
        Some(match self.cells.get(&pair) {
            Some(mm) => PairClearance::Written(*mm),
            None => PairClearance::Fallback(fallback),
        })
    }

    fn fallback(&self, pair: ClassPair) -> Option<f64> {
        let a = self.classes.get(pair.row)?;
        let b = self.classes.get(pair.col)?;
        Some(f64::max(*a, *b))
    }
}

/// The sample classes: (name, own clearance in mm).
const CLASSES: [(&str, f64); 5] = [
    ("PWR", 0.3),
    ("HV", 0.8),
    ("USB", 0.15),
    ("DIFF", 0.15),
    ("Default", 0.2),
];

static CLEARANCES: LazyLock<SampleClearances> = LazyLock::new(|| SampleClearances {
    classes: CLASSES.iter().map(|(_, mm)| *mm).collect(),
    cells: [(ClassPair::new(1, 0), 0.8), (ClassPair::new(4, 1), 1.5)]
        .into_iter()
        .collect(),
});

static MATRIX: LazyLock<ClassMatrix> = LazyLock::new(|| {
    let mut matrix = ClassMatrix::new(
        CLASSES
            .iter()
            .map(|(name, _)| MatrixClass {
                name: (*name).to_owned(),
            })
            .collect(),
    );
    for message in [
        MatrixMessage::CellPressed(ClassPair::new(1, 0)),
        MatrixMessage::EditCancelled,
    ] {
        let _ = matrix.update(message, &*CLEARANCES, format());
    }
    matrix
});

pub(crate) fn view_samples<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    let style = GridStyle::from_tokens(tokens, TEXT_SIZE);
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
    let grid =
        container(view(&GRID, &*RULES, format(), &words, style).map(|_| Message::TreeInteracted))
            .height(style.metrics().row_height * GRID_ROWS);
    let matrix = matrix_view(&MATRIX, &*CLEARANCES, format(), &matrix_words, style)
        .map(|_| Message::TreeInteracted);
    column![grid, matrix]
        .spacing(style.metrics().row_height)
        .boxed()
}
