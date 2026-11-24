//! The Pin table document tab's own state (mockup row 122b): its filter, the
//! issues-only view, the ticked rows, the bulk edit fields and the Paste
//! rows draft. Session state of one editor, like the Properties panel's;
//! never saved, never undone. Reading pasted rows is pure and lives here
//! too, so the editor applies what this module read.

use std::collections::BTreeSet;

use iced::widget::text_editor;
use signex_library::{PinDirection, PinOrientation};

/// How Apply paste uses the pasted rows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PasteMode {
    /// A row whose number a pin has updates that pin; the rest are added.
    #[default]
    UpdateByNumber,
    /// Every row is added as a new pin.
    Append,
    /// The rows replace every pin.
    ReplaceAll,
}

impl PasteMode {
    pub const ALL: [PasteMode; 3] = [Self::UpdateByNumber, Self::Append, Self::ReplaceAll];
}

/// What one pasted column is read as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteField {
    Number,
    Name,
    Electrical,
    Part,
    Length,
    Description,
    Ignore,
}

impl PasteField {
    pub const ALL: [PasteField; 7] = [
        Self::Number,
        Self::Name,
        Self::Electrical,
        Self::Part,
        Self::Length,
        Self::Description,
        Self::Ignore,
    ];
}

/// The map of the first columns before the user changes it: number, name,
/// type word and description, the order of a datasheet pin table.
const DEFAULT_COLUMNS: [PasteField; 4] = [
    PasteField::Number,
    PasteField::Name,
    PasteField::Electrical,
    PasteField::Description,
];

/// The map always offers at least this many columns.
const MIN_COLUMNS: usize = DEFAULT_COLUMNS.len();

/// The Paste rows box while it is open.
#[derive(Debug, Clone)]
pub struct PasteDraft {
    pub content: text_editor::Content,
    /// What each column is read as, by column; a column past the end is
    /// read by its default.
    pub columns: Vec<PasteField>,
    pub mode: PasteMode,
}

impl Default for PasteDraft {
    fn default() -> Self {
        Self {
            content: text_editor::Content::new(),
            columns: DEFAULT_COLUMNS.to_vec(),
            mode: PasteMode::default(),
        }
    }
}

impl PartialEq for PasteDraft {
    fn eq(&self, other: &Self) -> bool {
        self.columns == other.columns
            && self.mode == other.mode
            && self.content.text() == other.content.text()
    }
}

impl PasteDraft {
    /// What column `index` is read as.
    pub fn field_of(&self, index: usize) -> PasteField {
        self.columns
            .get(index)
            .or_else(|| DEFAULT_COLUMNS.get(index))
            .copied()
            .unwrap_or(PasteField::Ignore)
    }

    /// How many columns the map shows: the widest pasted line, at least
    /// [`MIN_COLUMNS`].
    pub fn column_count(&self) -> usize {
        column_count(&self.content.text())
    }

    /// The pasted rows, read through the column map.
    pub fn rows(&self) -> Vec<PastedRow> {
        let count = self.column_count();
        let fields: Vec<PasteField> = (0..count).map(|i| self.field_of(i)).collect();
        read_rows(&self.content.text(), &fields)
    }
}

/// The bulk edit bar: each field set is written to every ticked row; an
/// empty field keeps each row's value.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BulkEdit {
    pub electrical: Option<PinDirection>,
    pub part: String,
    pub length: String,
    pub orientation: Option<PinOrientation>,
    pub hidden: Option<bool>,
}

/// The Pin table tab of one symbol editor.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PinTableUi {
    pub filter: String,
    /// Show only the rows with an issue.
    pub issues_only: bool,
    /// The ticked rows, by pin index.
    pub checked: BTreeSet<usize>,
    pub bulk: BulkEdit,
    /// The Paste rows box, while open.
    pub paste: Option<PasteDraft>,
}

/// What the user did in the Pin table tab.
#[derive(Debug, Clone, PartialEq)]
pub enum PinTableMsg {
    FilterChanged(String),
    IssuesOnlyToggled,
    /// A row's tick box: the pin index and the new state.
    RowChecked(usize, bool),
    /// The header tick box: tick every listed row, or none.
    AllChecked(Vec<usize>, bool),
    BulkElectricalPicked(Option<PinDirection>),
    BulkPartChanged(String),
    BulkLengthChanged(String),
    BulkOrientationPicked(Option<PinOrientation>),
    BulkHiddenPicked(Option<bool>),
    /// Apply to selected: write the bulk fields to the ticked rows.
    BulkApplied,
    /// `+ Pin`: add a pin as a new row.
    AddPinPressed,
    DeleteSelectedPressed,
    /// `Paste rows…`: open the paste box, or close it.
    PasteToggled,
    PasteEdited(text_editor::Action),
    PasteColumnMapped(usize, PasteField),
    PasteModePicked(PasteMode),
    PasteCancelled,
    PasteApplied,
}

impl PinTableUi {
    /// Apply what changes this state. `false` when the message edits the
    /// symbol, which the caller does.
    pub fn apply(&mut self, msg: &PinTableMsg) -> bool {
        match msg {
            PinTableMsg::FilterChanged(query) => self.filter.clone_from(query),
            PinTableMsg::IssuesOnlyToggled => self.issues_only = !self.issues_only,
            PinTableMsg::RowChecked(idx, on) => {
                if *on {
                    self.checked.insert(*idx);
                } else {
                    self.checked.remove(idx);
                }
            }
            PinTableMsg::AllChecked(rows, on) => {
                if *on {
                    self.checked.extend(rows.iter().copied());
                } else {
                    self.checked.clear();
                }
            }
            PinTableMsg::BulkElectricalPicked(v) => self.bulk.electrical = *v,
            PinTableMsg::BulkPartChanged(v) => self.bulk.part.clone_from(v),
            PinTableMsg::BulkLengthChanged(v) => self.bulk.length.clone_from(v),
            PinTableMsg::BulkOrientationPicked(v) => self.bulk.orientation = *v,
            PinTableMsg::BulkHiddenPicked(v) => self.bulk.hidden = *v,
            PinTableMsg::PasteToggled => {
                self.paste = match self.paste.take() {
                    Some(_) => None,
                    None => Some(PasteDraft::default()),
                };
            }
            PinTableMsg::PasteEdited(action) => {
                if let Some(paste) = self.paste.as_mut() {
                    paste.content.perform(action.clone());
                }
            }
            PinTableMsg::PasteColumnMapped(index, field) => {
                if let Some(paste) = self.paste.as_mut() {
                    let wanted = (*index + 1).max(paste.columns.len());
                    let defaults: Vec<PasteField> = (paste.columns.len()..wanted)
                        .map(|i| paste.field_of(i))
                        .collect();
                    paste.columns.extend(defaults);
                    paste.columns[*index] = *field;
                }
            }
            PinTableMsg::PasteModePicked(mode) => {
                if let Some(paste) = self.paste.as_mut() {
                    paste.mode = *mode;
                }
            }
            PinTableMsg::PasteCancelled => self.paste = None,
            PinTableMsg::BulkApplied
            | PinTableMsg::AddPinPressed
            | PinTableMsg::DeleteSelectedPressed
            | PinTableMsg::PasteApplied => return false,
        }
        true
    }

    /// Forget ticks of pins that are gone (an undo, a delete elsewhere).
    pub fn keep_checks_below(&mut self, pin_count: usize) {
        self.checked.retain(|idx| *idx < pin_count);
    }
}

/// One pasted row: what each mapped column gave.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PastedRow {
    pub number: Option<String>,
    pub name: Option<String>,
    pub electrical: Option<PinDirection>,
    pub part: Option<u8>,
    pub length: Option<f64>,
    pub description: Option<String>,
}

/// `line` split into columns: on a tab, or on a run of two or more spaces.
pub fn split_columns(line: &str) -> Vec<&str> {
    let mut cells = Vec::new();
    for tabbed in line.split('\t') {
        let mut rest = tabbed;
        while let Some(at) = rest.find("  ") {
            cells.push(rest[..at].trim());
            rest = rest[at..].trim_start_matches(' ');
        }
        cells.push(rest.trim());
    }
    cells
}

/// The columns the map offers for `text`: its widest line, at least
/// [`MIN_COLUMNS`].
pub fn column_count(text: &str) -> usize {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|l| split_columns(l).len())
        .max()
        .unwrap_or(0)
        .max(MIN_COLUMNS)
}

/// The electrical type of a datasheet type word: I input, O output, I/O
/// bidirectional, P, PWR or a dash power input, OC open collector, a type
/// name as itself, anything else passive.
pub fn type_of_word(word: &str) -> PinDirection {
    let word = word.trim().to_ascii_uppercase();
    match word.as_str() {
        "I" | "IN" => PinDirection::Input,
        "O" | "OUT" => PinDirection::Output,
        "I/O" | "IO" | "B" => PinDirection::Bidirectional,
        "P" | "PWR" | "POWER" | "\u{2014}" | "\u{2013}" | "-" => PinDirection::PowerInput,
        "OC" => PinDirection::OpenDrainLow,
        _ => PinDirection::ALL
            .into_iter()
            .find(|d| d.label().to_ascii_uppercase() == word)
            .unwrap_or(PinDirection::Passive),
    }
}

/// A pasted name with the datasheet's dashes as the minus sign: every en
/// dash, and a leading hyphen (`-INA`, `V–`).
pub fn typographic_minus(name: &str) -> String {
    let name = name.replace('\u{2013}', "\u{2212}");
    match name.strip_prefix('-') {
        Some(rest) => format!("\u{2212}{rest}"),
        None => name,
    }
}

/// The rows of `text`, one per non-empty line, column `i` read as
/// `fields[i]`.
pub fn read_rows(text: &str, fields: &[PasteField]) -> Vec<PastedRow> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|line| {
            let mut row = PastedRow::default();
            for (cell, field) in split_columns(line).into_iter().zip(fields) {
                read_cell(&mut row, *field, cell);
            }
            row
        })
        .collect()
}

fn read_cell(row: &mut PastedRow, field: PasteField, cell: &str) {
    match field {
        PasteField::Number => row.number = Some(cell.to_string()),
        PasteField::Name => row.name = Some(typographic_minus(cell)),
        PasteField::Electrical => row.electrical = Some(type_of_word(cell)),
        PasteField::Part => row.part = cell.parse().ok(),
        PasteField::Length => row.length = parse_mm(cell),
        PasteField::Description => row.description = Some(cell.to_string()),
        PasteField::Ignore => {}
    }
}

/// A typed length in mm, a comma read as the decimal point; `None` when it
/// is not a finite number.
pub fn parse_mm(text: &str) -> Option<f64> {
    text.trim()
        .replace(',', ".")
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
}

/// What Apply paste would do: rows read, pins updated, pins added, and the
/// pins replaced (Replace all only).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PastePreview {
    pub rows: usize,
    pub updated: usize,
    pub added: usize,
    pub replaced: usize,
}

/// The preview of pasting `rows` in `mode` over pins numbered `numbers`.
pub fn paste_preview(rows: &[PastedRow], mode: PasteMode, numbers: &[&str]) -> PastePreview {
    let updated = match mode {
        PasteMode::UpdateByNumber => rows
            .iter()
            .filter(|r| {
                r.number
                    .as_deref()
                    .is_some_and(|n| !n.is_empty() && numbers.contains(&n))
            })
            .count(),
        PasteMode::Append | PasteMode::ReplaceAll => 0,
    };
    PastePreview {
        rows: rows.len(),
        updated,
        added: rows.len() - updated,
        replaced: if mode == PasteMode::ReplaceAll {
            numbers.len()
        } else {
            0
        },
    }
}

#[cfg(test)]
#[path = "pin_table_ui_tests.rs"]
mod tests;
