//! Step 2, preview: what the dry run reported. Pure reads of the report;
//! the list filter and the severity chips, which the loss report after the
//! import shares. A dry run writes only the report (contract §2.2), so the
//! list is the report's items.

use super::{ImportDialogState, PreviewState};
use crate::library_import::RunOutcome;
use crate::library_import::report::{ReportItem, Severity};

/// The severities the chips show, most severe first.
pub const CHIP_SEVERITIES: [Severity; 4] = [
    Severity::Error,
    Severity::Dropped,
    Severity::Approximated,
    Severity::Info,
];

/// How many list rows are drawn; the caption says how many more match.
pub const MAX_ROWS: usize = 200;

impl ImportDialogState {
    /// The finished dry run, when step 2 has one.
    pub fn preview_outcome(&self) -> Option<&RunOutcome> {
        match &self.preview {
            Some(PreviewState::Ready(outcome)) => Some(outcome),
            _ => None,
        }
    }

    /// The shown report's items ([`Self::shown_report`]): the dry run's
    /// on step 2, the finished run's or the stored one after step 3.
    pub fn report_items(&self) -> &[ReportItem] {
        self.shown_report().map_or(&[], |r| r.items.as_slice())
    }

    /// `(severity, count)` for each chip severity the report has.
    pub fn severity_counts(&self) -> Vec<(Severity, usize)> {
        let items = self.report_items();
        CHIP_SEVERITIES
            .iter()
            .map(|&sev| (sev, items.iter().filter(|i| i.severity == sev).count()))
            .filter(|&(_, n)| n > 0)
            .collect()
    }

    /// The items the list shows, with their index in the report: not of a
    /// hidden severity, and matching the filter text in their source name,
    /// message or code (case-insensitive).
    pub fn visible_items(&self) -> Vec<(usize, &ReportItem)> {
        let needle = self.filter.trim().to_lowercase();
        self.report_items()
            .iter()
            .enumerate()
            .filter(|(_, item)| !self.hidden.contains(&item.severity))
            .filter(|(_, item)| {
                needle.is_empty()
                    || [&item.source, &item.message, &item.code]
                        .iter()
                        .any(|field| field.to_lowercase().contains(&needle))
            })
            .collect()
    }

    /// The selected item, while the filter still shows it.
    pub fn selected_report_item(&self) -> Option<&ReportItem> {
        let index = self.selected_item?;
        self.visible_items()
            .into_iter()
            .find(|(i, _)| *i == index)
            .map(|(_, item)| item)
    }

    /// Show or hide `severity` in the list.
    pub fn toggle_severity(&mut self, severity: Severity) {
        match self.hidden.iter().position(|&s| s == severity) {
            Some(at) => {
                self.hidden.remove(at);
            }
            None => self.hidden.push(severity),
        }
    }

    /// Parts the import would write: the dry run's count.
    pub fn parts_to_import(&self) -> u64 {
        self.preview_outcome().map_or(0, |o| o.summary.counts.parts)
    }
}

/// The `detail` object of an item as `(key, value)` lines, its scalar
/// values as written and anything nested as compact JSON.
pub fn detail_lines(item: &ReportItem) -> Vec<(String, String)> {
    let Some(object) = item.detail.as_object() else {
        return Vec::new();
    };
    object
        .iter()
        .map(|(key, value)| {
            let shown = match value {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            (key.clone(), shown)
        })
        .collect()
}
