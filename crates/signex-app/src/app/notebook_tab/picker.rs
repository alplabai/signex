//! The reference picker's state (mockup row 195): typing `@` or pressing
//! Insert reference opens it over the editor; the list is the project's
//! parts and net labels on its open sheets, filtered as you type
//! (`signex_notebook::matching`); Enter inserts the stored call. Pins come
//! with the reference resolution (`.` after a part lists its pins, mockup
//! row 196).

use signex_notebook::{Candidate, MAX_ROWS, Reference, matching};

use crate::app::Signex;
use crate::app::state::ProjectId;

/// The id of the picker's query field, focused when it opens.
pub(crate) const PICKER_INPUT_ID: &str = "notebook-reference-picker";

/// An open picker.
#[derive(Debug, Clone, Default)]
pub struct Picker {
    pub query: String,
    /// The candidates when it opened, in sheet order.
    pub candidates: Vec<Candidate>,
    /// The highlighted row of the filtered list.
    pub row: usize,
}

impl Picker {
    pub fn new(candidates: Vec<Candidate>) -> Self {
        Picker {
            query: String::new(),
            candidates,
            row: 0,
        }
    }

    /// The rows shown: the best matches, at most [`MAX_ROWS`].
    pub fn rows(&self) -> Vec<&Candidate> {
        let mut rows = matching(&self.candidates, &self.query);
        rows.truncate(MAX_ROWS);
        rows
    }

    /// How many candidates match, shown or not.
    pub fn match_count(&self) -> usize {
        matching(&self.candidates, &self.query).len()
    }

    /// Move the highlight one row, staying on the list.
    pub fn step(&mut self, down: bool) {
        let last = self.rows().len().saturating_sub(1);
        self.row = if down {
            (self.row + 1).min(last)
        } else {
            self.row.saturating_sub(1)
        };
    }

    /// The reference of row `row`, or of the highlighted row.
    pub fn chosen(&self, row: Option<usize>) -> Option<Reference> {
        let rows = self.rows();
        rows.get(row.unwrap_or(self.row))
            .map(|candidate| candidate.reference.clone())
    }
}

impl Signex {
    /// The parts and net labels of `id`'s open sheets, each once, in sheet
    /// path order.
    pub(super) fn notebook_candidates(&self, id: ProjectId) -> Vec<Candidate> {
        let docs = &self.document_state;
        let mut paths: Vec<&std::path::PathBuf> = docs
            .engines
            .keys()
            .filter(|path| {
                docs.document_project(path)
                    .is_some_and(|project| project.id == id)
            })
            .collect();
        paths.sort();
        let mut found: Vec<Candidate> = Vec::new();
        for path in paths {
            let Some(engine) = docs.engines.get(path) else {
                continue;
            };
            let sheet = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            let document = engine.document();
            for symbol in &document.symbols {
                let name = symbol.reference.trim();
                // Power ports and unannotated parts name no part.
                if name.is_empty() || name.starts_with('#') || name.ends_with('?') {
                    continue;
                }
                let detail = [symbol.value.trim(), symbol.footprint.trim()]
                    .into_iter()
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join(" · ");
                push_once(
                    &mut found,
                    Reference::Component(name.to_string()),
                    detail,
                    &sheet,
                );
            }
            for label in &document.labels {
                let name = label.text.trim();
                if !name.is_empty() {
                    push_once(
                        &mut found,
                        Reference::Net(name.to_string()),
                        String::new(),
                        &sheet,
                    );
                }
            }
        }
        found
    }
}

fn push_once(found: &mut Vec<Candidate>, reference: Reference, detail: String, sheet: &str) {
    if found.iter().any(|held| held.reference == reference) {
        return;
    }
    found.push(Candidate {
        reference,
        detail,
        sheet: sheet.to_string(),
    });
}
