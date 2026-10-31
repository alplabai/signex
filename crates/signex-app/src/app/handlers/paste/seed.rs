//! Reading the project's designator numbers for Next free, off the UI thread.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;
use signex_types::designator::seed_counters;

use crate::app::state::PasteCounters;
use crate::app::{EditMsg, Message, PasteIntent, PasteRequest, Signex};

/// What the numbering read found.
#[derive(Debug, Default)]
pub struct PasteSeed {
    /// The highest number in use per prefix, across every project sheet.
    pub counters: HashMap<String, u32>,
    /// Every numbered designator in use across the project sheets.
    pub used: HashSet<String>,
    /// Sheets that exist but could not be read, with why.
    pub unreadable: Vec<(PathBuf, String)>,
    /// The read itself did not finish; `counters` says nothing.
    pub failed: bool,
}

impl Signex {
    /// The task that reads the numbers `request` will hand out from, or
    /// `None` when its numbering needs none (Keep and Reset) or there is no
    /// sheet. Paste Array reads them only when its dialog opens on Next free;
    /// a later choice of Next free asks for them with `ArraySeed`.
    pub(crate) fn paste_seed_task(&self, request: &PasteRequest) -> Option<Task<Message>> {
        let needs = request.intent == PasteIntent::ArraySeed
            || self.ui_state.paste_designators.reads_project();
        if !needs {
            return None;
        }
        let target = request.path.clone()?;
        let plan =
            crate::app::project_sheets::plan_project_sheets_for(&self.document_state, &target);
        let request = request.clone();
        Some(Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let set = plan.assemble();
                    let references = || {
                        set.sheets
                            .values()
                            .flat_map(|sheet| sheet.symbols.iter())
                            .filter(|s| !s.is_power && !s.reference.starts_with('#'))
                            .map(|s| s.reference.as_str())
                    };
                    PasteSeed {
                        failed: false,
                        counters: seed_counters(references()),
                        used: references().map(|r| r.trim().to_owned()).collect(),
                        unreadable: set.unreadable.clone(),
                    }
                })
                .await
                .unwrap_or_else(|error| PasteSeed {
                    failed: true,
                    counters: HashMap::new(),
                    used: HashSet::new(),
                    unreadable: vec![(target, format!("the numbering task failed: {error}"))],
                })
            },
            move |seed| {
                Message::Edit(EditMsg::PasteCountersSeeded {
                    request: request.clone(),
                    seed: Arc::new(seed),
                })
            },
        ))
    }

    /// Keep the numbers for the commits that follow, and name any sheet that
    /// could not be counted.
    pub(crate) fn store_paste_seed(&mut self, request: &PasteRequest, seed: &PasteSeed) {
        for (path, why) in &seed.unreadable {
            crate::diagnostics::log_warning(format!(
                "Designators on paste: {} could not be read ({why}), so its designators are not counted.",
                path.display()
            ));
        }
        // A read that did not finish has no numbers: what an earlier read
        // stored for this sheet stays, as it can only skip numbers.
        if seed.failed {
            return;
        }
        if let Some(target) = request.path.clone() {
            self.interaction_state.paste_counters = Some(PasteCounters {
                target,
                counters: seed.counters.clone(),
                used: seed.used.clone(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(path: &str) -> PasteRequest {
        PasteRequest {
            intent: PasteIntent::Paste,
            epoch: 0,
            path: Some(PathBuf::from(path)),
            dialog: 0,
        }
    }

    #[test]
    fn a_failed_read_keeps_the_numbers_an_earlier_read_stored() {
        let (mut app, _task) = Signex::new();
        let good = PasteSeed {
            counters: HashMap::from([("R".to_owned(), 4)]),
            ..PasteSeed::default()
        };
        app.store_paste_seed(&request("a.snxsch"), &good);
        let failed = PasteSeed {
            failed: true,
            unreadable: vec![(
                PathBuf::from("a.snxsch"),
                "the numbering task failed".into(),
            )],
            ..PasteSeed::default()
        };
        app.store_paste_seed(&request("a.snxsch"), &failed);
        let kept = app.interaction_state.paste_counters.as_ref().unwrap();
        assert_eq!(kept.counters.get("R"), Some(&4));
    }
}
