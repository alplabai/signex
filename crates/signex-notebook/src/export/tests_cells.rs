//! Mockup row 200: an export runs the Python cells before it compiles, so
//! the pages print current results, with the source hidden unless the
//! export asks for it. The runtime is a scripted fake.

use super::*;
use crate::PACKAGE_IMPORT;
use crate::cell::{
    CellCache, CellState, DesignValues, RunOutput, SandboxPolicy, Script, ScriptedRunner, Trust,
};
use crate::reference::Reference;
use crate::sources::NotebookSources;

const IDENT: &str = "buck-3v3/notebook/design.snxnbk";

/// A design with no values: the cell reads none.
struct NoValues;

impl DesignValues for NoValues {
    fn value(&self, _designator: &str) -> Option<String> {
        None
    }

    fn param(&self, _designator: &str, _name: &str) -> Option<String> {
        None
    }
}

fn notebook() -> NotebookSources {
    NotebookSources::new(
        "design.snxnbk",
        format!(
            "{PACKAGE_IMPORT}\n= Efficiency\n\
             #python(label: \"efficiency\")[```python\nprint(\"eta = 0.91\")\n```]\n"
        ),
    )
}

fn runner() -> ScriptedRunner {
    let script = Script {
        output: RunOutput {
            stdout: "eta = 0.91\n".into(),
            duration_ms: 12,
            ..RunOutput::default()
        },
        attempts: Vec::new(),
    };
    ScriptedRunner::new("Python 3.12").with("efficiency", script)
}

fn live(_: &Reference) -> bool {
    true
}

fn export_running(
    runner: &ScriptedRunner,
    cache: &mut CellCache,
    options: &ExportOptions,
) -> (NotebookExport, crate::cell::RunReport) {
    let policy = SandboxPolicy::new("/work/buck-3v3");
    let cells = ExportCells {
        design: &NoValues,
        runner,
        policy: &policy,
        trust: Trust::Trusted,
        now: "2026-11-26T09:00",
        cache,
    };
    match export_pdf_running_cells(&notebook(), options, IDENT, live, cells) {
        Ok(done) => done,
        Err(error) => panic!("the export failed: {error}"),
    }
}

#[test]
fn an_export_runs_the_cells_first_and_prints_their_results() {
    let runner = runner();
    let mut cache = CellCache::default();
    let options = ExportOptions::default();
    let (with_results, report) = export_running(&runner, &mut cache, &options);
    assert_eq!(runner.runs(), ["efficiency"], "the export ran the cell");
    assert!(
        matches!(report.cells[0].state, CellState::Ran { .. }),
        "{:?}",
        report.cells[0].state
    );
    assert!(cache.get("efficiency").is_some(), "the result is kept");
    let plain = export_pdf(&notebook(), &options, IDENT, live).expect("exports");
    assert_ne!(
        with_results.pdf, plain.pdf,
        "the pages print the result, not a cell never run"
    );

    // A second export finds the result current and runs nothing.
    let (_, report) = export_running(&runner, &mut cache, &options);
    assert_eq!(runner.runs(), ["efficiency"]);
    assert!(
        matches!(report.cells[0].state, CellState::Cached { .. }),
        "{:?}",
        report.cells[0].state
    );
}

#[test]
fn the_source_prints_only_when_the_export_asks_for_it() {
    let runner = runner();
    let mut cache = CellCache::default();
    let hidden = ExportOptions::default();
    assert!(!hidden.python_source);
    let (without, _) = export_running(&runner, &mut cache, &hidden);
    let shown = ExportOptions {
        python_source: true,
        ..ExportOptions::default()
    };
    let (with, _) = export_running(&runner, &mut cache, &shown);
    assert_ne!(without.pdf, with.pdf, "the source adds to the page");
}
