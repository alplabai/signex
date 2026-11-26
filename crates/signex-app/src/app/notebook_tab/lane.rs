//! The margin lane beside the preview (mockup rows 196 and 199): a
//! reference's card, a target's measurement timeline and the cards of the
//! stale attachments. What it shows is read with each compile
//! (`compile.rs`, `Lane`) and from the project's notebook read (the
//! resolved references); this module handles what its controls publish.

use std::path::PathBuf;

use iced::Task;
use signex_i18n::{ids, t};
use signex_notebook::{Reference, ReferenceReport, reference_card};

use super::open::MAIN_FILE;
use super::{NotebookMsg, TimelineTarget, ViewMode};
use crate::app::state::ProjectId;
use crate::app::{Message, Signex};

impl Signex {
    /// The notebook file's references resolved against the design, once
    /// the project's notebook read for `id` has landed.
    pub(crate) fn notebook_references_report(&self, id: ProjectId) -> Option<&ReferenceReport> {
        let annotations = &self.ui_state.annotations;
        annotations
            .target
            .as_ref()
            .filter(|(project, _)| *project == id)?;
        let scan = annotations.scan.as_ref()?;
        scan.references
            .iter()
            .find(|file| file.name == MAIN_FILE)
            .map(|file| &file.report)
    }

    /// A References row was pressed: its card shows in the margin lane, or
    /// the lane's card closes with `None`.
    pub(super) fn notebook_reference_shown(
        &mut self,
        id: ProjectId,
        reference: Option<&Reference>,
    ) {
        let card = reference.and_then(|reference| {
            let report = self.notebook_references_report(id)?;
            let entry = report.get(reference)?;
            let scan = self.ui_state.annotations.scan.as_ref()?;
            let design = scan.design.as_deref()?;
            let badges = crate::panels::annotations::notebook_badges(scan);
            let stale = report.stale_count();
            Some((reference_card(entry, design, &badges, stale), stale))
        });
        if let Some(tab) = self.ui_state.notebook_tabs.get_mut(&id) {
            if card.is_some() && tab.view == ViewMode::Source {
                tab.view = ViewMode::Split;
            }
            tab.card = card;
        }
    }

    /// Open the timeline of `reference` in the margin lane: a tab beside
    /// the ones already open, shown, read with a compile.
    pub(super) fn notebook_timeline_opened(
        &mut self,
        id: ProjectId,
        reference: Reference,
    ) -> Task<Message> {
        let Some(tab) = self.ui_state.notebook_tabs.get_mut(&id) else {
            return Task::none();
        };
        let mut targets = tab
            .timeline
            .take()
            .map(|open| open.targets)
            .unwrap_or_default();
        if !targets.contains(&reference) {
            targets.push(reference.clone());
        }
        tab.timeline = Some(TimelineTarget {
            targets,
            shown: reference,
        });
        if tab.view == ViewMode::Source {
            tab.view = ViewMode::Split;
        }
        self.request_notebook_compile(id)
    }

    /// A timeline reading was pressed: the cursor moves to its block when
    /// it is in the notebook file.
    pub(super) fn notebook_block_shown(&mut self, id: ProjectId, file: &str, line: u32) {
        let in_main = file.is_empty() || file == MAIN_FILE;
        if in_main {
            self.notebook_line_picked(id, line);
        }
    }

    /// Export CSV under the timeline: where to write it.
    pub(super) fn pick_notebook_timeline_csv(&mut self, id: ProjectId) -> Task<Message> {
        let folder = self
            .document_state
            .project_by_id(id)
            .map(|project| project.dir().to_path_buf())
            .unwrap_or_default();
        let picker = self.ui_state.io.picker.clone();
        let i18n = self.ui_state.i18n.clone();
        Task::perform(
            async move {
                picker
                    .dialog()
                    .title_in(&i18n, ids::NOTEBOOK_TIMELINE_EXPORT_TITLE)
                    .filter_in(&i18n, ids::FILE_FILTER_CSV_EXT, &["csv"])
                    .set_directory(folder)
                    .set_file_name("timeline.csv".to_string())
                    .save_file()
                    .await
            },
            move |path| {
                Message::from(NotebookMsg::TimelineExportChosen {
                    project_id: id,
                    path,
                })
            },
        )
    }

    /// Write the timeline the lane shows to `path` as CSV, off the UI
    /// thread; the outcome is said in Messages.
    pub(super) fn write_notebook_timeline_csv(
        &mut self,
        id: ProjectId,
        path: Option<PathBuf>,
    ) -> Task<Message> {
        let Some(path) = path else {
            return Task::none();
        };
        let csv = self
            .ui_state
            .notebook_tabs
            .get(&id)
            .and_then(|tab| tab.compile.lane())
            .and_then(|lane| lane.timeline.as_ref())
            .map(signex_notebook::measurement::Timeline::to_csv);
        let Some(csv) = csv else {
            return Task::none();
        };
        let i18n = self.ui_state.i18n.clone();
        Task::perform(
            async move {
                let target = path.clone();
                let written = tokio::task::spawn_blocking(move || {
                    signex_types::atomic_io::atomic_write(&target, csv.as_bytes())
                        .map_err(|error| error.to_string())
                })
                .await
                .map_err(|error| error.to_string())
                .and_then(|done| done);
                let shown = path.display().to_string();
                match written {
                    Ok(()) => t!(i18n, ids::NOTEBOOK_TIMELINE_EXPORTED, "path" => shown),
                    Err(why) => t!(
                        i18n, ids::NOTEBOOK_TIMELINE_EXPORT_FAILED,
                        "path" => shown, "why" => why
                    ),
                }
                .into_string()
            },
            |line| {
                crate::diagnostics::log_info(line);
                Message::noop()
            },
        )
    }

    /// Mark still valid on a stale card: the attachment on `line` of the
    /// notebook file stores the part values of the design now. An edit of
    /// the notebook's text, written on Save.
    pub(super) fn mark_notebook_attachment_valid_in_tab(
        &mut self,
        id: ProjectId,
        line: u32,
    ) -> Task<Message> {
        let values = self.notebook_design_values(id);
        let Some(text) = self.notebook_tab_file_text(id) else {
            return Task::none();
        };
        let Some(marked) = signex_notebook::attachment::mark_still_valid(&text, line, &values)
        else {
            return Task::none();
        };
        match self.replace_notebook_tab_file_text(id, &marked) {
            Ok(task) => task,
            Err(why) => {
                crate::diagnostics::log_info(
                    signex_i18n::RenderTranslatable::localized(&why, &self.ui_state.i18n)
                        .into_string(),
                );
                Task::none()
            }
        }
    }
}
