//! The part values a stale mark is judged against: every placed part's
//! designator and value, as the schematic holds them now. Captured in
//! `update` (the open sheets, with their unsaved edits), assembled off the
//! UI thread with the sheets that are not open read from disk.

use std::path::{Path, PathBuf};

use signex_notebook::attachment::DesignValues;
use signex_notebook::export::DesignNames;
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

    /// The designators, pins and nets of the design, read whole from the
    /// project in `project_dir` (the open sheets with their unsaved edits):
    /// what the PDF export's stale check reads (mockup row 202). `None`
    /// when a sheet could not be read or is missing, or the design cannot
    /// be stitched: then nothing is known to be gone. Blocking.
    pub(crate) fn names(&self, project_dir: &Path) -> Option<DesignNames> {
        let set = project_sheets::assemble_project_sheets_quiet_from(
            &self.open,
            &self.pages,
            &self.root,
            self.fold,
        );
        if !set.unreadable.is_empty() || !set.absent.is_empty() {
            return None;
        }
        let design = crate::app::notebook_references::read_design(&set, project_dir)?;
        let components = design
            .parts()
            .iter()
            .map(|part| part.designator.clone())
            .collect();
        let pins = design
            .parts()
            .iter()
            .flat_map(|part| {
                part.pins.iter().flat_map(move |pin| {
                    let by_number = format!("{}.{}", part.designator, pin.number);
                    let by_name =
                        (!pin.name.is_empty()).then(|| format!("{}.{}", part.designator, pin.name));
                    std::iter::once(by_number).chain(by_name)
                })
            })
            .collect();
        let nets = design.nets().iter().map(|net| net.name.clone()).collect();
        Some(DesignNames {
            components: Some(components),
            pins: Some(pins),
            nets: Some(nets),
        })
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
