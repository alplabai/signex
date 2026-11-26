//! The tab's Power tree view (mockup row 171, #1313): the view switch reads
//! the project's power tree off the update thread, with the run behind
//! `query_power_tree` (`app::power_tree`), for the project's active
//! variant. Each switch to the view reads it again, so it follows the
//! schematic. Showing it changes nothing in the diagram.

use std::path::{Path, PathBuf};

use iced::Task;

use super::BlockDiagramMsg;
use super::state::{DiagramView, PowerTreeRead};
use crate::app::{Message, Signex};

/// A finished power tree run for the diagram at `path`.
#[derive(Debug, Clone, PartialEq)]
pub struct PowerTreeLanded {
    pub(crate) path: PathBuf,
    pub(crate) result: Result<PowerTreeRead, String>,
}

impl Signex {
    /// The view switch: show `view`, reading the power tree for it.
    pub(super) fn pick_diagram_view(&mut self, path: &Path, view: DiagramView) -> Task<Message> {
        let run = (view == DiagramView::PowerTree)
            .then(|| self.power_tree_run(path))
            .flatten();
        let Some(editor) = self.document_state.block_diagrams.editors.get_mut(path) else {
            return Task::none();
        };
        editor.view = view;
        editor.export = None;
        let Some(run) = run else {
            if view == DiagramView::PowerTree && !editor.power.loading {
                editor.power.read = Some(Err(no_project(path)));
            }
            return Task::none();
        };
        if editor.power.loading {
            return Task::none();
        }
        editor.power.loading = true;
        let landed_path = path.to_path_buf();
        Task::perform(
            crate::app::path_op_jobs::run_blocking(run),
            move |ran: Result<Result<PowerTreeRead, String>, String>| {
                Message::BlockDiagram(BlockDiagramMsg::PowerTreeRead(Box::new(PowerTreeLanded {
                    path: landed_path.clone(),
                    result: ran.and_then(|read| read),
                })))
            },
        )
    }

    /// The run for the project whose folder holds the diagram.
    fn power_tree_run(
        &self,
        path: &Path,
    ) -> Option<impl FnOnce() -> Result<PowerTreeRead, String> + Send + 'static> {
        let project = super::project_holding(&self.document_state, path)?;
        let variant = crate::app::power_tree::power_tree_variant(project, None);
        let job = variant.map(|variant| self.power_tree_job(project, variant));
        Some(move || job.and_then(|job| job.read()))
    }

    /// A run landed: the view shows it. Its rail stays picked while the
    /// tree still has it.
    pub(super) fn power_tree_landed(&mut self, landed: PowerTreeLanded) -> Task<Message> {
        let PowerTreeLanded { path, result } = landed;
        let Some(editor) = self.document_state.block_diagrams.editors.get_mut(&path) else {
            return Task::none();
        };
        let power = &mut editor.power;
        power.loading = false;
        let kept = match (&result, &power.rail) {
            (Ok(read), Some(rail)) => read.tree.rail(rail).is_some(),
            _ => false,
        };
        if !kept {
            power.rail = result
                .as_ref()
                .ok()
                .and_then(|read| read.tree.rails.first())
                .map(|rail| rail.net.clone());
        }
        power.read = Some(result);
        Task::none()
    }

    /// A rail picked in the view.
    pub(super) fn pick_power_rail(&mut self, path: &Path, rail: String) {
        if let Some(editor) = self.document_state.block_diagrams.editors.get_mut(path) {
            editor.power.rail = Some(rail);
        }
    }
}

/// Why there is no tree: the diagram is in no open project's folder.
fn no_project(path: &Path) -> String {
    format!(
        "{} is not in the folder of an open project",
        super::file_name(path)
    )
}
