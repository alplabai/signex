//! An export that runs the Python cells first (mockup row 200): cells run
//! on Run, Run all and export, so the pages print current results. Every
//! cell with no result or an out-of-date one runs (`Trigger::Export`), and
//! the compile prints what each shows; the source stays hidden unless the
//! export asks for it (`python_source`).

use super::{ExportError, ExportOptions, NotebookExport, export_with_cells};
use crate::cell::{
    CellCache, CellRunner, DesignValues, PageCells, RunContext, RunReport, SandboxPolicy, Trigger,
    Trust, notebook_cells, run_cells,
};
use crate::reference::Reference;
use crate::sources::NotebookSources;

/// What the cells of an export run with.
pub struct ExportCells<'a> {
    /// The design values the cells read.
    pub design: &'a dyn DesignValues,
    pub runner: &'a dyn CellRunner,
    pub policy: &'a SandboxPolicy,
    pub trust: Trust,
    /// The time the runs are stamped with, from the caller's clock.
    pub now: &'a str,
    /// The last results; the runs update it, for the caller to keep.
    pub cache: &'a mut CellCache,
}

/// Run the cells of `sources` as an export does, then export with their
/// results printed. The report says what each cell shows.
pub fn export_pdf_running_cells(
    sources: &NotebookSources,
    options: &ExportOptions,
    ident: &str,
    is_live: impl Fn(&Reference) -> bool,
    cells: ExportCells<'_>,
) -> Result<(NotebookExport, RunReport), ExportError> {
    let (python, _problems) = notebook_cells(sources);
    let context = RunContext {
        cells: &python,
        design: cells.design,
        runner: cells.runner,
        policy: cells.policy,
        trust: cells.trust,
        now: cells.now,
    };
    let report = run_cells(&context, cells.cache, Trigger::Export);
    let page = PageCells::build(&python, &report.cells, cells.cache, options.python_source);
    export_with_cells(sources, options, ident, is_live, page).map(|export| (export, report))
}
