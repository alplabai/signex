//! Where a resolved notebook request is handed on. Each body belongs to the
//! slice that builds that surface: the notebook tab (mockup row 195), the
//! Notebook badges (row 197), the generator (row 201) and the PDF export
//! (row 202). Each request is also said in Messages, so a row is never
//! silent even where its surface draws nothing new.

use iced::Task;

use super::NotebookTarget;
use crate::app::{Message, Signex};

impl Signex {
    /// Show the notebook tab of `target`, with the empty state when its file
    /// does not exist yet.
    pub(crate) fn show_design_notebook(&mut self, target: NotebookTarget) -> Task<Message> {
        say("Open", &target);
        self.open_notebook_tab(target.project)
    }

    /// Open the Generate options of `target`'s project.
    pub(crate) fn generate_design_notebook(&mut self, target: NotebookTarget) -> Task<Message> {
        say("Generate", &target);
        self.dispatch_notebook_generate_message(crate::app::NotebookGenerateMsg::DialogOpened)
    }

    /// Open the Export notebook PDF dialog of `target`.
    pub(crate) fn export_design_notebook_pdf(&mut self, target: NotebookTarget) -> Task<Message> {
        say("Export PDF", &target);
        // The notebook tab passes the page it shows (1-based) when it is
        // the tab in view.
        let in_view =
            self.notebook_tab_index(target.project) == Some(self.document_state.active_tab);
        let current_page = in_view
            .then(|| self.ui_state.notebook_tabs.get(&target.project))
            .flatten()
            .map(|tab| tab.page + 1);
        self.dispatch_notebook_pdf_message(crate::app::NotebookPdfMsg::Requested { current_page })
    }

    /// Show or hide the Notebook badges on the schematic.
    pub(crate) fn toggle_notebook_badges(&mut self) -> Task<Message> {
        crate::diagnostics::log_info("Design Notebook: Notebook badges toggled");
        self.handle_notebook_reference_message(
            crate::app::notebook_references::NotebookReferenceMsg::BadgesToggled,
        )
    }
}

fn say(action: &str, target: &NotebookTarget) {
    crate::diagnostics::log_info(format!(
        "Design Notebook: {action} {}",
        target.path.display()
    ));
}
