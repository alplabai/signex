//! Where a resolved notebook request is handed on. Each body belongs to the
//! slice that builds that surface: the notebook tab (mockup row 195), the
//! Notebook badges (row 197), the generator (row 201) and the PDF export
//! (row 202). Until a slice lands, its request is said in Messages, so the
//! row is never silent.

use iced::Task;

use super::NotebookTarget;
use crate::app::{Message, Signex};

impl Signex {
    /// Show the notebook tab of `target`, with the empty state when its file
    /// does not exist yet.
    pub(super) fn show_design_notebook(&mut self, target: NotebookTarget) -> Task<Message> {
        say("Open", &target);
        Task::none()
    }

    /// Open the Generate options of `target`'s project.
    pub(super) fn generate_design_notebook(&mut self, target: NotebookTarget) -> Task<Message> {
        say("Generate", &target);
        Task::none()
    }

    /// Open the Export notebook PDF dialog of `target`.
    pub(super) fn export_design_notebook_pdf(&mut self, target: NotebookTarget) -> Task<Message> {
        say("Export PDF", &target);
        Task::none()
    }

    /// Show or hide the Notebook badges on the schematic.
    pub(super) fn toggle_notebook_badges(&mut self) -> Task<Message> {
        crate::diagnostics::log_info("Design Notebook: Notebook badges toggled");
        Task::none()
    }
}

fn say(action: &str, target: &NotebookTarget) {
    crate::diagnostics::log_info(format!(
        "Design Notebook: {action} {}",
        target.path.display()
    ));
}
