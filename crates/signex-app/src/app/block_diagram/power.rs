//! The tab's Power tree view (mockup row 171, #1313): the view switch reads
//! the project's power tree off the update thread, with the run behind
//! `query_power_tree` (`app::power_tree`), for the variant the Variant field
//! names (the project's active one until a person picks). Each switch to
//! the view, and each pick, reads it again, so it follows the schematic. A
//! click on a part opens its sheet with the part selected. Showing it
//! changes nothing in the diagram.

use std::path::{Path, PathBuf};

use iced::Task;
use signex_types::schematic::{SelectedItem, SelectedKind};

use super::BlockDiagramMsg;
use super::state::{DiagramView, PowerTreeRead};
use crate::app::handlers::canvas::{AfterOpen, SheetOpen};
use crate::app::{Message, Signex};

/// A finished power tree run for the diagram at `path`.
#[derive(Debug, Clone, PartialEq)]
pub struct PowerTreeLanded {
    pub(crate) path: PathBuf,
    pub(crate) result: Result<PowerTreeRead, String>,
}

/// A power tree run, ready to leave the update thread.
pub(crate) type PowerTreeRun = Box<dyn FnOnce() -> Result<PowerTreeRead, String> + Send>;

impl Signex {
    /// The view switch: show `view`, reading the power tree for it.
    pub(super) fn pick_diagram_view(&mut self, path: &Path, view: DiagramView) -> Task<Message> {
        let Some(editor) = self.document_state.block_diagrams.editors.get_mut(path) else {
            return Task::none();
        };
        editor.view = view;
        editor.export = None;
        if view == DiagramView::PowerTree {
            return self.read_power_tree(path);
        }
        Task::none()
    }

    /// The Variant field: read the tree again for `variant`.
    pub(super) fn pick_power_variant(&mut self, path: &Path, variant: String) -> Task<Message> {
        let Some(editor) = self.document_state.block_diagrams.editors.get_mut(path) else {
            return Task::none();
        };
        editor.power.variant = Some(variant);
        if editor.view != DiagramView::PowerTree {
            return Task::none();
        }
        self.read_power_tree(path)
    }

    /// Start a read for the diagram's view; a read already going is
    /// overtaken (its answer is dropped if it no longer fits, see
    /// [`Self::power_tree_landed`]).
    fn read_power_tree(&mut self, path: &Path) -> Task<Message> {
        let variant = self
            .document_state
            .block_diagrams
            .editors
            .get(path)
            .and_then(|editor| editor.power.variant.clone());
        let run = self.power_tree_reader(path, variant.as_deref());
        let Some(editor) = self.document_state.block_diagrams.editors.get_mut(path) else {
            return Task::none();
        };
        let Some(run) = run else {
            editor.power.read = Some(Err(no_project(path)));
            return Task::none();
        };
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

    /// The power tree run of the project whose folder holds the diagram at
    /// `path`, for `variant` (the project's active one without it). `None`
    /// when no open project holds the diagram.
    pub(crate) fn power_tree_reader(
        &self,
        path: &Path,
        variant: Option<&str>,
    ) -> Option<PowerTreeRun> {
        let project = super::project_holding(&self.document_state, path)?;
        let variant = crate::app::power_tree::power_tree_variant(project, variant);
        let job = variant.map(|variant| self.power_tree_job(project, variant));
        Some(Box::new(move || job.and_then(|job| job.read())))
    }

    /// The variant the view wants now, resolved; `None` is Base.
    fn wanted_variant(&self, path: &Path) -> Option<String> {
        let project = super::project_holding(&self.document_state, path)?;
        let editor = self.document_state.block_diagrams.editors.get(path)?;
        crate::app::power_tree::power_tree_variant(project, editor.power.variant.as_deref())
            .ok()
            .flatten()
    }

    /// A run landed: the view shows it, unless it read a variant the view
    /// no longer asks for. Its rail stays picked while the tree has it.
    pub(super) fn power_tree_landed(&mut self, landed: PowerTreeLanded) -> Task<Message> {
        let PowerTreeLanded { path, result } = landed;
        let wanted = self.wanted_variant(&path);
        let Some(editor) = self.document_state.block_diagrams.editors.get_mut(&path) else {
            return Task::none();
        };
        if result.as_ref().is_ok_and(|read| read.variant != wanted) {
            return Task::none();
        }
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

    /// A part clicked in the view (#1313): its sheet opens at the part's
    /// sheet occurrence with the part's first unit selected.
    pub(super) fn open_power_part(&mut self, path: &Path, part: usize) -> Task<Message> {
        let target = self
            .document_state
            .block_diagrams
            .editors
            .get(path)
            .and_then(|editor| editor.power.read.as_ref()?.as_ref().ok())
            .and_then(|read| {
                let part = read.topology.parts.get(part)?;
                let sheet = read.sheet_paths.get(&part.sheet)?.clone();
                let unit = *part.units.first()?;
                Some((sheet, part.occurrence.clone(), unit))
            });
        let Some((sheet, occurrence, unit)) = target else {
            return Task::none();
        };
        let item = Some(SelectedItem::new(unit, SelectedKind::Symbol));
        self.open_sheet(
            sheet,
            SheetOpen::at(Some(occurrence)).then(AfterOpen::Reveal { item }),
        )
    }
}

/// Why there is no tree: the diagram is in no open project's folder.
fn no_project(path: &Path) -> String {
    format!(
        "{} is not in the folder of an open project",
        super::file_name(path)
    )
}
