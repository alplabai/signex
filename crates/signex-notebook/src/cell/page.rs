//! What the page prints for each cell (mockup row 200): a header with the
//! state, the values it read with their schematic source, its printed
//! output and its figures, then the out-of-date, sandbox or slim-install
//! note. The compile hands this to the bundled package as `cells.json`;
//! the Python source itself is hidden in print unless the export asks for
//! it.

use std::collections::BTreeMap;

use serde::Serialize;

use super::cache::CellCache;
use super::model::PythonCell;
use super::sandbox::SANDBOX_RULE;
use super::schedule::{CellState, CellStatus, InputChange};

/// Every cell's page content, by label.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct PageCells {
    /// Print each cell's Python source above its result. Off by default:
    /// the source is hidden in print.
    pub print_source: bool,
    pub cells: BTreeMap<String, PageCell>,
}

/// One cell as printed.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct PageCell {
    /// The state, as [`CellState`] names it in JSON (`ran`, `cached`, ...).
    pub state: String,
    /// The header's caption: `ran in 212 ms · 14:20`.
    pub caption: String,
    /// The result is from before a change, or shown without a runtime.
    pub greyed: bool,
    pub values: Vec<PageValue>,
    pub stdout: String,
    pub figures: Vec<PageFigure>,
    /// The note under the result; empty for none.
    pub note: String,
    /// `Reads L1, C15, U3 · re-runs only when one of them or the cell
    /// changes`; empty when it reads none.
    pub reads: String,
}

/// One value a cell read: `L`, `4.7 µH`, `L1 value`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PageValue {
    pub symbol: String,
    pub value: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PageFigure {
    pub svg: String,
    pub caption: String,
}

impl PageCells {
    /// The page content of `cells` in their states `statuses`, with the
    /// results `cache` holds.
    pub fn build(
        cells: &[PythonCell],
        statuses: &[CellStatus],
        cache: &CellCache,
        print_source: bool,
    ) -> Self {
        let mut page = PageCells {
            print_source,
            cells: BTreeMap::new(),
        };
        for status in statuses {
            let Some(cell) = cells.iter().find(|cell| cell.label == status.label) else {
                continue;
            };
            page.cells
                .insert(cell.label.clone(), page_cell(cell, &status.state, cache));
        }
        page
    }

    /// The `cells.json` the bundled package reads.
    pub fn to_json(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_else(|_| EMPTY_JSON.as_bytes().to_vec())
    }
}

/// The `cells.json` of a compile with no cell results.
const EMPTY_JSON: &str = r#"{"print_source":false,"cells":{}}"#;

fn page_cell(cell: &PythonCell, state: &CellState, cache: &CellCache) -> PageCell {
    let cached = cache.get(&cell.label);
    let shows_result = !matches!(
        state,
        CellState::Blocked { .. } | CellState::NeverRun | CellState::TrustRefused
    );
    let mut page = PageCell {
        state: state_name(state),
        caption: caption(state),
        greyed: matches!(
            state,
            CellState::OutOfDate { .. } | CellState::NoRuntime { .. } | CellState::AwaitingTrust
        ),
        note: note(state),
        ..PageCell::default()
    };
    let Some(cached) = cached else {
        return page;
    };
    page.reads = reads_line(cached.inputs.iter().map(|input| input.designator.as_str()));
    if !shows_result {
        return page;
    }
    page.values = cached
        .inputs
        .iter()
        .map(|input| PageValue {
            symbol: input.symbol.clone().unwrap_or_default(),
            value: input.display(),
            source: input.source_name(),
        })
        .collect();
    page.stdout = cached.output.stdout.trim_end().to_string();
    page.figures = cached
        .output
        .figures
        .iter()
        .map(|figure| PageFigure {
            svg: figure.svg.clone(),
            caption: figure.caption.clone(),
        })
        .collect();
    page
}

/// The state's JSON name.
fn state_name(state: &CellState) -> String {
    serde_json::to_value(state)
        .ok()
        .and_then(|value| value.get("state")?.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// The header caption of `state`, as mockup row 200 draws it.
pub fn caption(state: &CellState) -> String {
    match state {
        CellState::NeverRun => "not run yet".to_string(),
        CellState::Ran {
            duration_ms,
            ran_at,
        } => format!("ran in {duration_ms} ms · {}", clock_time(ran_at)),
        CellState::Cached { ran_at } => {
            format!("cached · inputs unchanged since {}", date_time(ran_at))
        }
        CellState::OutOfDate {
            changes,
            source_changed,
            ..
        } => match changes.first() {
            Some(change) => format!("out of date: {} changed", change.designator),
            None if *source_changed => "out of date: the cell changed".to_string(),
            None => "out of date: the Python runtime changed".to_string(),
        },
        CellState::Blocked { line, .. } => format!("blocked, line {line}"),
        CellState::Failed {
            line: Some(line), ..
        } => format!("error, line {line}"),
        CellState::Failed { line: None, .. } => "error".to_string(),
        CellState::NoRuntime { .. } => "not run: no Python runtime".to_string(),
        CellState::AwaitingTrust => "not run: waiting for permission".to_string(),
        CellState::TrustRefused => "not run: not allowed for this notebook".to_string(),
    }
}

/// The note printed under a cell in `state`; empty for none.
pub fn note(state: &CellState) -> String {
    match state {
        CellState::NeverRun => "Run the cell, or Run all, to compute it.".to_string(),
        CellState::OutOfDate { changes, .. } if !changes.is_empty() => {
            let changed: Vec<String> = changes.iter().map(change_sentence).collect();
            let earlier = if changes.len() == 1 {
                "value"
            } else {
                "values"
            };
            format!(
                "Out of date: {}. The result above was computed with the earlier {earlier}.",
                changed.join("; "),
            )
        }
        CellState::OutOfDate { .. } => {
            "Out of date: the result above was computed before the last change.".to_string()
        }
        CellState::Blocked { line, refusal } => format!(
            "PermissionError at line {line}: {} {SANDBOX_RULE} The rest of the page compiled; \
             this cell shows no result until line {line} is removed.",
            refusal.blocked_sentence()
        ),
        CellState::Failed {
            line,
            kind,
            message,
        } => match line {
            Some(line) => format!("{kind} at line {line}: {message}"),
            None => format!("{kind}: {message}"),
        },
        CellState::NoRuntime { cached_at } => {
            let shown = match cached_at {
                Some(at) => format!(
                    "the last cached result ({}) is shown greyed above",
                    date_only(at)
                ),
                None => "this cell has no cached result".to_string(),
            };
            format!(
                "Python is not installed. This copy of Signex is the slim install, which leaves \
                 out the Python runtime that cells need. The page still compiles; {shown}."
            )
        }
        CellState::AwaitingTrust => "This notebook comes from outside the project. Its cells run \
             only after you allow it."
            .to_string(),
        CellState::TrustRefused => "Running cells was not allowed for this notebook.".to_string(),
        CellState::Ran { .. } | CellState::Cached { .. } => String::new(),
    }
}

/// `C15 changed from 22 µF to 47 µF in the schematic`.
fn change_sentence(change: &InputChange) -> String {
    let what = match &change.field {
        super::inputs::Field::Value => change.designator.clone(),
        super::inputs::Field::Param(name) => format!("{} {name}", change.designator),
    };
    format!(
        "{what} changed from {} to {} in the schematic",
        change.was, change.now
    )
}

/// `Reads L1, C15, U3 · re-runs only when one of them or the cell changes`.
fn reads_line<'a>(designators: impl Iterator<Item = &'a str>) -> String {
    let mut names: Vec<&str> = Vec::new();
    for name in designators {
        if !names.contains(&name) {
            names.push(name);
        }
    }
    if names.is_empty() {
        return String::new();
    }
    format!(
        "Reads {} · re-runs only when one of them or the cell changes",
        names.join(", ")
    )
}

/// `14:20` of `2026-11-25T14:20[:ss]`; the text as given when it has no
/// time.
fn clock_time(at: &str) -> String {
    match at.split_once('T') {
        Some((_, time)) => time.get(..5).unwrap_or(time).to_string(),
        None => at.to_string(),
    }
}

/// `2026-11-24 14:02` of `2026-11-24T14:02[:ss]`.
fn date_time(at: &str) -> String {
    match at.split_once('T') {
        Some((date, _)) => format!("{date} {}", clock_time(at)),
        None => at.to_string(),
    }
}

/// `2026-11-24` of `2026-11-24T14:02`.
fn date_only(at: &str) -> String {
    at.split_once('T').map_or(at, |(date, _)| date).to_string()
}
