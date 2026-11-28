//! The checklist as CSV (mockup row 206, More ▸ Export CSV…): one row per
//! item in list order under a header of the file's own key names, so the
//! columns read the same in every language. Lines end in CRLF.

use super::DesignChecklist;
use super::link::ChecklistLink;

/// The header row's columns.
pub const CSV_COLUMNS: [&str; 9] = [
    "id", "stage", "state", "title", "owner", "link", "reason", "notes", "updated",
];

/// One cell, quoted when it holds a comma, a quote or a line break.
fn cell(text: &str) -> String {
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_string()
    }
}

fn line(cells: &[&str]) -> String {
    let cells: Vec<String> = cells.iter().map(|text| cell(text)).collect();
    cells.join(",") + "\r\n"
}

impl DesignChecklist {
    /// The checklist as CSV text.
    pub fn to_csv(&self) -> String {
        let mut out = line(&CSV_COLUMNS);
        for item in &self.items {
            let link = item
                .link
                .as_ref()
                .map(ChecklistLink::to_text)
                .unwrap_or_default();
            out.push_str(&line(&[
                item.id.as_str(),
                item.stage.key(),
                item.state.key(),
                item.title.as_str(),
                item.owner.as_deref().unwrap_or_default(),
                link.as_str(),
                item.reason.as_deref().unwrap_or_default(),
                item.notes.as_deref().unwrap_or_default(),
                item.updated.as_deref().unwrap_or_default(),
            ]));
        }
        out
    }
}
