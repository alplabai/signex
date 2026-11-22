//! One Properties edit of a Bezier, dimension or table (#1267, #1300), and
//! the drawing it gives. Pure: the handler sends the result through
//! `Command::UpdateSchDrawing`, so every edit is one undo step.

use signex_types::net_class::NetLineStyle;
use signex_types::schematic::{
    DIMENSION_MAX_PRECISION, DimensionArrows, DimensionDirection, DimensionUnits, LineEnds,
    PinSort, Point, SchDrawing, SchTable, StrokeColor, TableBorders, TableRefresh, TableSource,
};

/// Smallest text size and largest padding a table takes (mm).
const MIN_TABLE_TEXT_MM: f64 = 0.5;
const MAX_TABLE_PADDING_MM: f64 = 10.0;

/// Whether a table can store this text size; anything else is refused.
pub(crate) fn table_text_size_holds(size: f64) -> bool {
    size >= MIN_TABLE_TEXT_MM
}

/// Whether a table can store this cell padding; anything else is refused.
pub(crate) fn table_padding_holds(padding: f64) -> bool {
    (0.0..=MAX_TABLE_PADDING_MM).contains(&padding)
}

/// Which kind of source a table's Source pick list chooses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableSourceKind {
    ComponentPins,
    Typed,
    ProjectSheets,
}

impl TableSourceKind {
    pub const ALL: [Self; 3] = [Self::ComponentPins, Self::Typed, Self::ProjectSheets];

    pub fn of(source: &TableSource) -> Self {
        match source {
            TableSource::ComponentPins { .. } => Self::ComponentPins,
            TableSource::Typed => Self::Typed,
            TableSource::ProjectSheets { .. } => Self::ProjectSheets,
        }
    }
}

/// Which coordinate of a point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Axis {
    X,
    Y,
}

/// One Properties edit of an annotation drawing.
#[derive(Debug, Clone, PartialEq)]
pub enum AnnotationEdit {
    /// Point `index` of a Bezier (0 start, 1 C1, 2 C2, 3 end).
    BezierPoint(usize, Axis, f64),
    Width(f64),
    LineStyle(NetLineStyle),
    Ends(LineEnds),
    StrokeColor(Option<StrokeColor>),
    /// The dimension's first (`false`) or second (`true`) point.
    DimensionPoint(bool, Axis, f64),
    Direction(DimensionDirection),
    Units(DimensionUnits),
    Precision(u8),
    Arrows(DimensionArrows),
    /// Override text on (with the measured text to start from) or off.
    OverrideEnabled(bool),
    OverrideText(String),
    TableSource(TableSourceKind, Option<String>),
    TableDesignator(String),
    TablePinSort(PinSort),
    TableRefresh(TableRefresh),
    TablePerChannel(bool),
    TableColumnShown(String, bool),
    TableTextSize(f64),
    TableCellPadding(f64),
    TableBorders(TableBorders),
    TableHeaderFill(bool),
    /// A typed table's cell, from an edit in place on the sheet.
    TableCell(usize, usize, String),
    /// A typed table's columns and rows, from a CSV import.
    TableImported(Vec<String>, Vec<Vec<String>>),
}

fn set_axis(p: &mut Point, axis: Axis, value: f64) {
    match axis {
        Axis::X => p.x = value,
        Axis::Y => p.y = value,
    }
}

/// `drawing` with `edit` applied; `None` when the edit does not apply to
/// this kind or changes nothing.
pub(crate) fn apply_annotation_edit(
    drawing: &SchDrawing,
    edit: &AnnotationEdit,
) -> Option<SchDrawing> {
    let mut next = drawing.clone();
    let applied = match &mut next {
        SchDrawing::Bezier(b) => match edit {
            AnnotationEdit::BezierPoint(i, axis, v) if *i < 4 => {
                let p = match *i {
                    0 => &mut b.start,
                    1 => &mut b.c1,
                    2 => &mut b.c2,
                    _ => &mut b.end,
                };
                set_axis(p, *axis, *v);
                true
            }
            AnnotationEdit::Width(w) => {
                b.width = w.max(0.0);
                true
            }
            AnnotationEdit::LineStyle(s) => {
                b.line_style = *s;
                true
            }
            AnnotationEdit::Ends(e) => {
                b.ends = *e;
                true
            }
            AnnotationEdit::StrokeColor(c) => {
                b.stroke_color = *c;
                true
            }
            _ => false,
        },
        SchDrawing::Dimension(d) => apply_to_dimension(d, edit),
        SchDrawing::Table(t) => apply_to_table(t, edit),
        SchDrawing::Line { .. }
        | SchDrawing::Rect { .. }
        | SchDrawing::Circle { .. }
        | SchDrawing::Arc { .. }
        | SchDrawing::Polyline { .. } => false,
    };
    (applied && next != *drawing).then_some(next)
}

fn apply_to_dimension(
    d: &mut signex_types::schematic::SchDimension,
    edit: &AnnotationEdit,
) -> bool {
    match edit {
        AnnotationEdit::DimensionPoint(second, axis, v) => {
            let p = if *second { &mut d.to } else { &mut d.from };
            set_axis(p, *axis, *v);
        }
        AnnotationEdit::Direction(dir) => {
            // Keep the line where it is when the direction changes.
            let through = d.line()[0];
            d.direction = *dir;
            d.offset = d.offset_to(through);
        }
        AnnotationEdit::Units(u) => d.units = *u,
        AnnotationEdit::Precision(p) => d.precision = (*p).min(DIMENSION_MAX_PRECISION),
        AnnotationEdit::Arrows(a) => d.arrows = *a,
        AnnotationEdit::OverrideEnabled(on) => {
            d.override_text = on.then(|| d.measured_text());
        }
        AnnotationEdit::OverrideText(text) => d.override_text = Some(text.clone()),
        AnnotationEdit::Width(w) => d.width = w.max(0.0),
        AnnotationEdit::StrokeColor(c) => d.stroke_color = *c,
        _ => return false,
    }
    true
}

fn apply_to_table(t: &mut SchTable, edit: &AnnotationEdit) -> bool {
    match edit {
        AnnotationEdit::TableSource(kind, first_designator) => {
            if TableSourceKind::of(&t.source) == *kind {
                return false;
            }
            t.source = match kind {
                TableSourceKind::ComponentPins => TableSource::ComponentPins {
                    designator: first_designator.clone().unwrap_or_default(),
                    sort: PinSort::Number,
                },
                // The rows a bound source showed stay, as typed text.
                TableSourceKind::Typed => TableSource::Typed,
                TableSourceKind::ProjectSheets => TableSource::ProjectSheets { per_channel: true },
            };
            if *kind != TableSourceKind::Typed {
                t.columns = SchTable::columns_for(&t.source);
                t.rows.clear();
            }
            t.links.clear();
        }
        AnnotationEdit::TableDesignator(designator) => match &mut t.source {
            TableSource::ComponentPins { designator: d, .. } => *d = designator.clone(),
            _ => return false,
        },
        AnnotationEdit::TablePinSort(sort) => match &mut t.source {
            TableSource::ComponentPins { sort: s, .. } => *s = *sort,
            _ => return false,
        },
        AnnotationEdit::TablePerChannel(on) => match &mut t.source {
            TableSource::ProjectSheets { per_channel } => *per_channel = *on,
            _ => return false,
        },
        AnnotationEdit::TableRefresh(r) => t.refresh = *r,
        AnnotationEdit::TableColumnShown(key, shown) => return t.set_column_shown(key, *shown),
        AnnotationEdit::TableTextSize(v) => {
            if !table_text_size_holds(*v) {
                return false;
            }
            t.style.text_size = *v;
        }
        AnnotationEdit::TableCellPadding(v) => {
            if !table_padding_holds(*v) {
                return false;
            }
            t.style.cell_padding = *v;
        }
        AnnotationEdit::TableBorders(b) => t.style.borders = *b,
        AnnotationEdit::TableHeaderFill(on) => t.style.header_fill = *on,
        AnnotationEdit::TableCell(row, column, text) => {
            if t.source.is_bound() || *column >= t.columns.len() {
                return false;
            }
            t.set_cell(*row, *column, text.clone());
        }
        AnnotationEdit::TableImported(titles, rows) => {
            if t.source.is_bound() {
                return false;
            }
            t.columns = titles
                .iter()
                .enumerate()
                .map(|(i, title)| {
                    signex_types::schematic::TableColumn::new(&format!("c{}", i + 1), title, true)
                })
                .collect();
            t.rows = rows.clone();
            t.links.clear();
        }
        _ => return false,
    }
    true
}
