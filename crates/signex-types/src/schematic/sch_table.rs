//! A table placed on a schematic sheet (#1300): typed in, or bound to a
//! data source (a part's pins, the project's sheets) and rebuilt from it
//! after a compile. See docs/SCHEMATIC_TABLES.md.
//!
//! The layout (column widths, row height) is computed here from the text
//! size, the padding and a fixed monospace advance, so the canvas, the hit
//! test, SVG and PDF all place every cell at the same spot.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Point, SchematicSheet};

/// Em advance of one character of table text (the cells are monospace).
pub const TABLE_ADVANCE_EM: f64 = 0.6;

/// Narrowest a column draws, in characters.
pub const TABLE_MIN_COLUMN_CHARS: usize = 3;

/// Gap between a table's title line and its top edge, as a share of the
/// text size.
pub const TABLE_TITLE_GAP_EM: f64 = 0.5;

/// Share of the info tint (the sheet's net-label colour) mixed over the
/// paper for a filled header row; see docs/RENDERING_RULES.md, "Tables".
pub const TABLE_HEADER_TINT_SHARE: f32 = 0.18;

/// `tint` mixed over `paper` at [`TABLE_HEADER_TINT_SHARE`], per channel
/// (0..1 floats).
pub fn table_header_fill(paper: [f32; 3], tint: [f32; 3]) -> [f32; 3] {
    let k = TABLE_HEADER_TINT_SHARE;
    [
        paper[0] + (tint[0] - paper[0]) * k,
        paper[1] + (tint[1] - paper[1]) * k,
        paper[2] + (tint[2] - paper[2]) * k,
    ]
}

/// Typed rows and columns a new table starts with.
pub const TABLE_NEW_COLUMNS: usize = 3;
pub const TABLE_NEW_ROWS: usize = 3;

fn yes() -> bool {
    true
}

fn is_true(value: &bool) -> bool {
    *value
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// The order a pin table lists the pins in.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PinSort {
    #[default]
    Number,
    Name,
}

/// Where a table's rows come from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TableSource {
    /// The pins of the part with this designator.
    ComponentPins {
        designator: String,
        #[serde(default)]
        sort: PinSort,
    },
    /// Typed in (or imported from CSV); never rebuilt.
    Typed,
    /// Every page of the project, in page order.
    ProjectSheets {
        /// One row per channel of a multi-channel sheet, else per sheet.
        #[serde(default = "yes")]
        per_channel: bool,
    },
}

impl TableSource {
    /// Whether the rows are rebuilt from the design.
    pub fn is_bound(&self) -> bool {
        !matches!(self, Self::Typed)
    }
}

/// One column: the key a bound source fills it by, the header text and
/// whether it is drawn. Hidden columns keep their cells.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableColumn {
    pub key: String,
    pub title: String,
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub shown: bool,
}

impl TableColumn {
    pub fn new(key: &str, title: &str, shown: bool) -> Self {
        Self {
            key: key.to_string(),
            title: title.to_string(),
            shown,
        }
    }
}

/// When a bound table is rebuilt.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TableRefresh {
    /// After every successful compile.
    #[default]
    OnCompile,
    /// Never: the rows stay as they are.
    Off,
}

/// Which cell borders a table draws. The outline is always drawn except
/// for [`TableBorders::None`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TableBorders {
    /// Outline, header rule and every cell edge.
    #[default]
    All,
    /// Outline and the header rule only.
    Outer,
    /// No lines.
    None,
}

impl TableBorders {
    pub const ALL: [Self; 3] = [Self::All, Self::Outer, Self::None];
}

fn default_text_size() -> f64 {
    1.8
}

fn default_cell_padding() -> f64 {
    0.6
}

/// How a table is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TableStyle {
    /// Cell text height (mm).
    #[serde(default = "default_text_size")]
    pub text_size: f64,
    /// Space between a cell's edge and its text (mm).
    #[serde(default = "default_cell_padding")]
    pub cell_padding: f64,
    #[serde(default)]
    pub borders: TableBorders,
    /// Whether the header row is filled with the sheet's info tint.
    #[serde(default = "yes")]
    pub header_fill: bool,
}

impl Default for TableStyle {
    fn default() -> Self {
        Self {
            text_size: default_text_size(),
            cell_padding: default_cell_padding(),
            borders: TableBorders::All,
            header_fill: true,
        }
    }
}

/// A table on the sheet; `position` is its top-left corner (below the
/// title line, when there is a title).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchTable {
    pub uuid: Uuid,
    pub position: Point,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub source: TableSource,
    pub columns: Vec<TableColumn>,
    /// Cells, one inner list per row, aligned to `columns` (hidden ones
    /// included).
    #[serde(default)]
    pub rows: Vec<Vec<String>>,
    /// Per row, the sheet file the row opens; `""` for none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<String>,
    #[serde(default)]
    pub refresh: TableRefresh,
    pub style: TableStyle,
    #[serde(default, skip_serializing_if = "is_false")]
    pub locked: bool,
}

/// Where a table's cells go, in sheet mm.
#[derive(Debug, Clone, PartialEq)]
pub struct TableLayout {
    /// Top-left of the grid.
    pub origin: Point,
    /// Indices into [`SchTable::columns`] of the drawn columns, left to right.
    pub columns: Vec<usize>,
    /// Left edge of each drawn column, relative to `origin.x`.
    pub column_x: Vec<f64>,
    pub column_widths: Vec<f64>,
    /// Height of the header and of every row.
    pub row_height: f64,
    /// Rows below the header.
    pub body_rows: usize,
    pub width: f64,
    /// Grid height, header included.
    pub height: f64,
    /// Height of the title line above the grid (`0` without a title).
    pub title_height: f64,
}

/// A cell under a point: `row` is `None` for the header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableCell {
    pub row: Option<usize>,
    /// Index into [`SchTable::columns`].
    pub column: usize,
}

impl TableLayout {
    /// The top-left corner of drawn column `j`, row `row` (`None` = header).
    pub fn cell_origin(&self, j: usize, row: Option<usize>) -> Point {
        let y_index = row.map_or(0, |r| r + 1);
        Point::new(
            self.origin.x + self.column_x.get(j).copied().unwrap_or(0.0),
            self.origin.y + y_index as f64 * self.row_height,
        )
    }

    /// The whole drawing's box, title line included.
    pub fn bounds(&self) -> (Point, Point) {
        (
            Point::new(self.origin.x, self.origin.y - self.title_height),
            Point::new(self.origin.x + self.width, self.origin.y + self.height),
        )
    }
}

/// Column keys of a pin table.
pub mod pin_columns {
    pub const PIN: &str = "pin";
    pub const NAME: &str = "name";
    pub const NET: &str = "net";
    pub const TYPE: &str = "type";
    pub const UNIT: &str = "unit";
    pub const PIN_LENGTH: &str = "pin_length";
}

/// Column keys of a contents table.
pub mod sheet_columns {
    pub const PAGE: &str = "page";
    pub const SHEET: &str = "sheet";
    pub const TITLE: &str = "title";
    pub const REV: &str = "rev";
    pub const LAST_CHANGED: &str = "last_changed";
}

impl SchTable {
    /// A typed table of [`TABLE_NEW_COLUMNS`] empty columns and
    /// [`TABLE_NEW_ROWS`] empty rows at `position`.
    pub fn new_typed(uuid: Uuid, position: Point) -> Self {
        let columns = (1..=TABLE_NEW_COLUMNS)
            .map(|i| TableColumn::new(&format!("c{i}"), "", true))
            .collect();
        Self {
            uuid,
            position,
            title: None,
            source: TableSource::Typed,
            columns,
            rows: vec![vec![String::new(); TABLE_NEW_COLUMNS]; TABLE_NEW_ROWS],
            links: Vec::new(),
            refresh: TableRefresh::OnCompile,
            style: TableStyle::default(),
            locked: false,
        }
    }

    /// The columns a source of this kind offers, the first four shown.
    pub fn columns_for(source: &TableSource) -> Vec<TableColumn> {
        let keys: &[(&str, &str)] = match source {
            TableSource::ComponentPins { .. } => &[
                (pin_columns::PIN, "Pin"),
                (pin_columns::NAME, "Name"),
                (pin_columns::NET, "Net"),
                (pin_columns::TYPE, "Type"),
                (pin_columns::UNIT, "Unit"),
                (pin_columns::PIN_LENGTH, "Pin length"),
            ],
            TableSource::ProjectSheets { .. } => &[
                (sheet_columns::PAGE, "Page"),
                (sheet_columns::SHEET, "Sheet"),
                (sheet_columns::TITLE, "Title"),
                (sheet_columns::REV, "Rev"),
                (sheet_columns::LAST_CHANGED, "Last changed"),
            ],
            TableSource::Typed => &[],
        };
        keys.iter()
            .enumerate()
            .map(|(i, (key, title))| TableColumn::new(key, title, i < 4))
            .collect()
    }

    /// Indices of the drawn columns.
    pub fn shown_columns(&self) -> Vec<usize> {
        (0..self.columns.len())
            .filter(|&i| self.columns[i].shown)
            .collect()
    }

    /// Cell `column` of `row`; `""` past the stored cells.
    pub fn cell(&self, row: usize, column: usize) -> &str {
        self.rows
            .get(row)
            .and_then(|r| r.get(column))
            .map_or("", String::as_str)
    }

    /// Set a cell, growing the row as needed.
    pub fn set_cell(&mut self, row: usize, column: usize, text: String) {
        if self.rows.len() <= row {
            self.rows.resize(row + 1, Vec::new());
        }
        let cells = &mut self.rows[row];
        if cells.len() <= column {
            cells.resize(column + 1, String::new());
        }
        cells[column] = text;
    }

    /// The sheet file row `row` opens, when it opens one.
    pub fn link(&self, row: usize) -> Option<&str> {
        self.links
            .get(row)
            .map(String::as_str)
            .filter(|l| !l.is_empty())
    }

    /// Show or hide the column with `key`; `false` when there is none.
    pub fn set_column_shown(&mut self, key: &str, shown: bool) -> bool {
        match self.columns.iter_mut().find(|c| c.key == key) {
            Some(column) => {
                column.shown = shown;
                true
            }
            None => false,
        }
    }

    /// Where every cell goes.
    pub fn layout(&self) -> TableLayout {
        let shown = self.shown_columns();
        let text = self.style.text_size.max(0.1);
        let pad = self.style.cell_padding.max(0.0);
        let advance = text * TABLE_ADVANCE_EM;
        let mut column_x = Vec::with_capacity(shown.len());
        let mut column_widths = Vec::with_capacity(shown.len());
        let mut x = 0.0;
        for &i in &shown {
            let chars = self
                .rows
                .iter()
                .map(|r| r.get(i).map_or(0, |c| c.chars().count()))
                .chain(std::iter::once(self.columns[i].title.chars().count()))
                .max()
                .unwrap_or(0)
                .max(TABLE_MIN_COLUMN_CHARS);
            let w = chars as f64 * advance + 2.0 * pad;
            column_x.push(x);
            column_widths.push(w);
            x += w;
        }
        let row_height = text + 2.0 * pad;
        let body_rows = self.rows.len();
        let title_height = match &self.title {
            Some(t) if !t.is_empty() => text * (1.0 + TABLE_TITLE_GAP_EM),
            _ => 0.0,
        };
        TableLayout {
            origin: self.position,
            columns: shown,
            column_x,
            column_widths,
            row_height,
            body_rows,
            width: x,
            height: row_height * (body_rows + 1) as f64,
            title_height,
        }
    }

    /// The table's box, title line included.
    pub fn bounds(&self) -> (Point, Point) {
        self.layout().bounds()
    }

    /// The cell under `p`, if any.
    pub fn cell_at(&self, p: Point) -> Option<TableCell> {
        let layout = self.layout();
        let (lx, ly) = (p.x - layout.origin.x, p.y - layout.origin.y);
        if lx < 0.0 || ly < 0.0 || lx > layout.width || ly > layout.height {
            return None;
        }
        let j = layout
            .column_x
            .iter()
            .zip(&layout.column_widths)
            .position(|(x, w)| lx >= *x && lx <= x + w)?;
        let index = (ly / layout.row_height).floor() as usize;
        let row = index.checked_sub(1).filter(|r| *r < layout.body_rows);
        if index > 0 && row.is_none() {
            return None;
        }
        Some(TableCell {
            row,
            column: layout.columns[j],
        })
    }

    /// Map the table through `f` (move, rotate, mirror). A table never turns
    /// or flips: its centre follows `f` and it stays upright.
    pub fn map_points(&mut self, f: impl Fn(Point) -> Point) {
        let (lo, hi) = self.bounds();
        let centre = Point::new((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0);
        let moved = f(centre);
        self.position.x += moved.x - centre.x;
        self.position.y += moved.y - centre.y;
    }

    /// `"N rows · M columns"` counts: body rows and drawn columns.
    pub fn size(&self) -> (usize, usize) {
        (self.rows.len(), self.shown_columns().len())
    }
}

/// The net names of the sheet's power ports: a table draws these cells in
/// the power colour.
pub fn power_net_names(sheet: &SchematicSheet) -> BTreeSet<String> {
    sheet
        .symbols
        .iter()
        .filter(|s| s.is_power && !s.value.trim().is_empty())
        .map(|s| s.value.trim().to_string())
        .collect()
}
