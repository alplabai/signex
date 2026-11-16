//! Import Graphics (#1667): the lines, circles, arcs and polylines of a DXF
//! file land on the cursor as sheet drawings, in the project's shape line
//! width. What the import does not read is counted in the Messages panel.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use signex_engine::{CopiedSelection, ImportedGraphics};
use signex_i18n::ids;

use crate::app::{EditMsg, Message, PlacementExtrasMsg, Signex};

const WHAT: &str = "Import graphics";

/// Read and convert the DXF file at `path`. Blocking: run it off the UI
/// thread.
pub(crate) fn read_graphics_file(
    path: &Path,
    width_mm: f64,
) -> Result<Arc<ImportedGraphics>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("{} could not be read ({error})", path.display()))?;
    signex_engine::drawings_from_dxf(&text, width_mm)
        .map(Arc::new)
        .map_err(|error| format!("{}: {error}", path.display()))
}

impl Signex {
    /// Ask for the DXF file to import.
    pub(super) fn import_graphics_dialog(&mut self) -> Task<Message> {
        if self.fragment_target(WHAT).is_none() {
            return Task::none();
        }
        let i18n = &self.ui_state.i18n;
        let dialog = self
            .ui_state
            .io
            .picker
            .dialog()
            .title_in(i18n, ids::FILE_DIALOG_IMPORT_GRAPHICS)
            .filter_in(i18n, ids::FILE_FILTER_DXF, &["dxf"])
            .filter_in(i18n, ids::FILE_FILTER_ALL_FILES, &["*"]);
        Task::perform(dialog.pick_file(), |path| {
            Message::Edit(EditMsg::Extras(PlacementExtrasMsg::ImportGraphicsPicked(
                path,
            )))
        })
    }

    /// The line width imported drawings take: the project's shape default.
    pub(crate) fn imported_graphics_width_mm(&self) -> f64 {
        self.document_state
            .active_schematic_defaults()
            .shape_stroke_mm(0.0)
    }

    /// Read and convert the picked file off the UI thread.
    pub(super) fn read_graphics(&mut self, path: Option<PathBuf>) -> Task<Message> {
        let Some(path) = path else {
            return Task::none();
        };
        let Some(target) = self.fragment_target(WHAT) else {
            return Task::none();
        };
        let epoch = self.interaction_state.placement_epoch;
        let width_mm = self.imported_graphics_width_mm();
        Task::perform(
            async move {
                let read = path.clone();
                let graphics =
                    tokio::task::spawn_blocking(move || read_graphics_file(&read, width_mm))
                        .await
                        .unwrap_or_else(|error| Err(format!("the read failed: {error}")));
                (path, graphics)
            },
            move |(path, graphics)| {
                Message::Edit(EditMsg::Extras(PlacementExtrasMsg::GraphicsRead {
                    path,
                    target: target.clone(),
                    epoch,
                    graphics,
                }))
            },
        )
    }

    /// The file was converted: put its drawings on the cursor.
    pub(super) fn graphics_read(
        &mut self,
        path: &Path,
        target: &Path,
        epoch: u64,
        graphics: Result<Arc<ImportedGraphics>, String>,
    ) {
        if !self.fragment_read_is_current(target, epoch, WHAT) {
            return;
        }
        match self.graphics_from(path, graphics) {
            Ok(copied) => self.arm_fragment(copied, target),
            Err(reason) => crate::diagnostics::log_warning(format!("{WHAT}: {reason}")),
        }
    }

    /// The imported drawings as a copy anchored at their centre, or why there
    /// are none. Says in the Messages panel what was skipped.
    pub(crate) fn graphics_from(
        &self,
        path: &Path,
        graphics: Result<Arc<ImportedGraphics>, String>,
    ) -> Result<CopiedSelection, String> {
        let graphics = graphics?;
        if graphics.skipped > 0 {
            crate::diagnostics::log_info(format!(
                "{WHAT}: {} entit(ies) in {} are not lines, circles, arcs or polylines and were skipped.",
                graphics.skipped,
                path.display()
            ));
        }
        CopiedSelection::from_drawings(graphics.drawings.clone(), self.fragment_grid_mm())
            .map_err(|error| format!("{} could not be placed ({error})", path.display()))?
            .ok_or_else(|| format!("{} holds nothing to place", path.display()))
    }
}
