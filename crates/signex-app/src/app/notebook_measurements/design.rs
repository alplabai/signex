//! The part values a stale mark is judged against: every placed part's
//! designator and value, as the schematic holds them now. Captured in
//! `update` (the open sheets, with their unsaved edits), assembled off the
//! UI thread with the sheets that are not open read from disk.

use std::path::PathBuf;

use signex_notebook::attachment::DesignValues;
use signex_types::schematic::SchematicSheet;

use crate::app::CaseFold;
use crate::app::project_sheets::{self, OpenSheets};
use crate::app::state::LoadedProject;
use crate::app::state::project_case::fold_of;

/// What the design's part values are read from.
pub(crate) struct DesignInputs {
    open: OpenSheets,
    pages: Vec<PathBuf>,
    root: PathBuf,
    fold: CaseFold,
}

impl DesignInputs {
    /// The inputs of `project`, with the open sheets `open`. `None` when
    /// the project has no sheet to start from.
    pub(crate) fn of(project: &LoadedProject, open: OpenSheets) -> Option<Self> {
        let pages = project_sheets::project_pages(Some(project));
        let root =
            project_sheets::project_root_sheet_path(project).or_else(|| pages.first().cloned())?;
        Some(Self {
            open,
            pages,
            root,
            fold: fold_of(Some(project)),
        })
    }

    /// The part values of the design. Empty when a sheet could not be read
    /// or is missing: a part there would wrongly read as changed or gone.
    pub(crate) fn values(&self) -> DesignValues {
        let set = project_sheets::assemble_project_sheets_quiet_from(
            &self.open,
            &self.pages,
            &self.root,
            self.fold,
        );
        if !set.unreadable.is_empty() || !set.absent.is_empty() {
            return DesignValues::new();
        }
        values_of(set.sheets.values())
    }
}

/// The designator and value of every placed part of `sheets`: power
/// symbols and parts not annotated yet (`R?`) are left out.
pub(crate) fn values_of<'a>(sheets: impl IntoIterator<Item = &'a SchematicSheet>) -> DesignValues {
    sheets
        .into_iter()
        .flat_map(|sheet| sheet.symbols.iter())
        .filter(|symbol| !symbol.is_power)
        .filter(|symbol| !symbol.reference.is_empty() && !symbol.reference.ends_with('?'))
        .map(|symbol| (symbol.reference.clone(), symbol.value.clone()))
        .collect()
}
