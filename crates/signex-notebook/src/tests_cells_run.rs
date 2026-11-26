//! When Python cells run and what each shows (mockup row 200): Run, Run all
//! and export only, the cache, out-of-date marks, the sandbox ending only
//! the cell that broke a rule, the slim install and a notebook from outside
//! the project; then what the page prints.

use std::collections::BTreeMap;

use crate::cell::{
    Access, CellCache, CellRunner, CellState, CellStatus, CellSummary, DesignValues, Field, Figure,
    NoRuntime, PageCells, PythonCell, RunContext, RunOutput, RunReport, RuntimeStatus,
    SandboxPolicy, Script, ScriptedRunner, Trigger, Trust, caption, cell_states, cells_in, note,
    run_cells,
};
use crate::tests_cells::RIPPLE;
use crate::{CompileOptions, NotebookSources, PACKAGE_IMPORT, compile};

#[derive(Default)]
struct Design {
    values: BTreeMap<String, String>,
    params: BTreeMap<(String, String), String>,
}

impl Design {
    fn buck() -> Self {
        let mut design = Design::default();
        design.set_value("L1", "4.7µH");
        design.set_value("C15", "22µF");
        let params = [("C15", "ESR", "5mΩ"), ("U3", "fsw", "500kHz")];
        for (part, name, value) in params {
            let key = (part.to_string(), name.to_string());
            design.params.insert(key, value.to_string());
        }
        design
    }

    fn set_value(&mut self, part: &str, value: &str) {
        self.values.insert(part.to_string(), value.to_string());
    }
}

impl DesignValues for Design {
    fn value(&self, designator: &str) -> Option<String> {
        self.values.get(designator).cloned()
    }

    fn param(&self, designator: &str, name: &str) -> Option<String> {
        let key = (designator.to_string(), name.to_string());
        self.params.get(&key).cloned()
    }
}

const TWO_MORE: &str = "#python(label: \"efficiency\")[```python\nprint(0.91)\n```]\n\
     #python(label: \"git\")[```python\nimport subprocess\nsubprocess.run([\"git\"])\n```]\n";

const SVG: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"10\"/>";

fn runner() -> ScriptedRunner {
    let csv = Access::File {
        call: "open".into(),
        path: "notebook/attachments/ripple.csv".into(),
        write: false,
    };
    let ripple = Script {
        output: RunOutput {
            stdout: "ΔI_L = 1.02 A\nΔV_out = 16.7 mV\n".into(),
            figures: vec![Figure {
                svg: SVG.into(),
                caption: "Computed output ripple versus input voltage".into(),
            }],
            duration_ms: 212,
            ..RunOutput::default()
        },
        attempts: vec![(3, csv)],
    };
    let program = Access::StartProgram {
        call: "subprocess.run".into(),
    };
    let git = Script {
        output: RunOutput {
            stdout: "never printed".into(),
            duration_ms: 4,
            ..RunOutput::default()
        },
        attempts: vec![(2, program)],
    };
    ScriptedRunner::new("Python 3.12")
        .with("ripple-vs-vin", ripple)
        .with("git", git)
}

/// The three cells, a design, a scripted runtime and a cache.
struct Bench {
    cells: Vec<PythonCell>,
    design: Design,
    runner: ScriptedRunner,
    cache: CellCache,
}

impl Bench {
    fn new() -> Self {
        let (cells, problems) = cells_in("design.snxnbk", &format!("{RIPPLE}{TWO_MORE}"));
        assert!(problems.is_empty(), "{problems:?}");
        Bench {
            cells,
            design: Design::buck(),
            runner: runner(),
            cache: CellCache::default(),
        }
    }

    fn run(&mut self, trigger: Trigger) -> RunReport {
        self.run_as(Trust::Trusted, trigger, false)
    }

    /// A run under `trust`; on the slim install when `slim`.
    fn run_as(&mut self, trust: Trust, trigger: Trigger, slim: bool) -> RunReport {
        let policy = SandboxPolicy::new("/work/buck-3v3");
        let runner: &dyn CellRunner = if slim { &NoRuntime } else { &self.runner };
        let context = RunContext {
            cells: &self.cells,
            design: &self.design,
            runner,
            policy: &policy,
            trust,
            now: "2026-11-25T14:20",
        };
        run_cells(&context, &mut self.cache, trigger)
    }

    /// What every cell shows now, without running.
    fn shown(&self) -> Vec<CellStatus> {
        let runtime = self.runner.status();
        cell_states(&self.cells, &self.cache, &self.design, &runtime)
    }

    fn runs(&self) -> usize {
        self.runner.runs().len()
    }

    fn page(&self, statuses: &[CellStatus], print_source: bool) -> PageCells {
        PageCells::build(&self.cells, statuses, &self.cache, print_source)
    }
}

fn states(report: &RunReport) -> Vec<&CellState> {
    report.cells.iter().map(|status| &status.state).collect()
}

#[test]
fn nothing_runs_until_run_and_reopening_runs_nothing() {
    let mut bench = Bench::new();
    let shown = bench.shown();
    assert!(shown.iter().all(|s| s.state == CellState::NeverRun));
    assert_eq!(bench.runs(), 0, "showing the states runs no cell");

    let report = bench.run(Trigger::All);
    assert_eq!(bench.runner.runs(), ["ripple-vs-vin", "efficiency", "git"]);
    assert_eq!((report.summary.ran, report.summary.blocked), (2, 1));

    let again = bench.run(Trigger::All);
    assert_eq!(bench.runs(), 3, "nothing changed: nothing runs");
    let summary = CellSummary {
        cells: 3,
        cached: 2,
        blocked: 1,
        ..CellSummary::default()
    };
    assert_eq!(again.summary, summary);
}

#[test]
fn run_cell_runs_that_cell_even_when_cached() {
    let mut bench = Bench::new();
    let _ = bench.run(Trigger::All);
    let report = bench.run(Trigger::Cell("efficiency".into()));
    let last = bench.runner.runs().last().cloned();
    assert_eq!(last.as_deref(), Some("efficiency"));
    assert_eq!(bench.runs(), 4);
    assert_eq!((report.summary.ran, report.summary.cached), (1, 1));
    let unknown = bench.run(Trigger::Cell("nope".into()));
    assert_eq!(unknown.unknown_cell.as_deref(), Some("nope"));
    assert_eq!(bench.runs(), 4);
}

#[test]
fn a_changed_value_marks_only_the_cells_that_read_it() {
    let mut bench = Bench::new();
    let _ = bench.run(Trigger::All);
    bench.design.set_value("C15", "47µF");
    let shown = bench.shown();
    let CellState::OutOfDate { changes, .. } = &shown[0].state else {
        panic!("{:?}", shown[0].state);
    };
    assert_eq!(changes.len(), 1);
    let change = &changes[0];
    assert_eq!(
        (change.designator.as_str(), &change.field),
        ("C15", &Field::Value)
    );
    assert_eq!(
        (change.was.as_str(), change.now.as_str()),
        ("22 µF", "47 µF")
    );
    assert!(matches!(shown[1].state, CellState::Cached { .. }));
    assert_eq!(caption(&shown[0].state), "out of date: C15 changed");
    let text = note(&shown[0].state);
    let changed = "C15 changed from 22 µF to 47 µF in the schematic";
    assert!(text.contains(changed), "{text}");

    let report = bench.run(Trigger::All);
    assert_eq!(bench.runs(), 4, "only the out-of-date cell ran");
    assert!(matches!(states(&report)[0], CellState::Ran { .. }));
}

#[test]
fn an_edited_cell_is_out_of_date_until_it_runs() {
    let mut bench = Bench::new();
    let _ = bench.run(Trigger::All);
    bench.cells[1].source = "print(0.92)".into();
    let shown = bench.shown();
    let edited = CellState::OutOfDate {
        source_changed: true,
        runtime_changed: false,
        changes: Vec::new(),
    };
    assert_eq!(shown[1].state, edited);
    assert_eq!(bench.runs(), 3, "an edit alone runs nothing");
}

#[test]
fn the_sandbox_blocks_only_the_cell_that_broke_a_rule() {
    let mut bench = Bench::new();
    let report = bench.run(Trigger::Export);
    let found = states(&report);
    assert!(matches!(found[0], CellState::Ran { .. }), "{found:?}");
    assert!(matches!(found[1], CellState::Ran { .. }), "{found:?}");
    let CellState::Blocked { line, refusal } = found[2] else {
        panic!("{found:?}");
    };
    assert_eq!(*line, bench.cells[2].file_line(2));
    assert_eq!(refusal.call, "subprocess.run");
    assert_eq!(caption(found[2]), format!("blocked, line {line}"));
    let opening = format!("PermissionError at line {line}: subprocess.run is blocked");
    assert!(note(found[2]).starts_with(&opening), "{}", note(found[2]));
    let kept = bench.cache.get("git").expect("a blocked result is kept");
    assert!(kept.output.stdout.is_empty(), "a refusal prints nothing");
}

#[test]
fn the_slim_install_runs_nothing_and_shows_the_cached_result_greyed() {
    let mut bench = Bench::new();
    let _ = bench.run(Trigger::All);
    let report = bench.run_as(Trust::Trusted, Trigger::All, true);
    assert_eq!(report.runtime, RuntimeStatus::NotInstalled);
    assert_eq!(report.summary.not_run, 3);
    let state = states(&report)[0].clone();
    let cached_at = Some("2026-11-25T14:20".to_string());
    assert_eq!(state, CellState::NoRuntime { cached_at });
    assert_eq!(caption(&state), "not run: no Python runtime");
    let text = note(&state);
    assert!(text.starts_with("Python is not installed."), "{text}");
    assert!(text.contains("the last cached result (2026-11-25) is shown greyed above"));
    let page = bench.page(&report.cells, false);
    let ripple = &page.cells["ripple-vs-vin"];
    assert!(ripple.greyed);
    assert!(ripple.stdout.contains("16.7 mV"), "the cached result shows");
    assert_eq!(bench.cache.len(), 3, "the cache is kept");
}

#[test]
fn a_notebook_from_outside_the_project_waits_for_an_answer() {
    let mut bench = Bench::new();
    let report = bench.run_as(Trust::Unasked, Trigger::All, false);
    assert!(report.needs_trust);
    assert_eq!(bench.runs(), 0, "no cell runs before the answer");
    let waiting = states(&report);
    assert!(waiting.iter().all(|s| **s == CellState::AwaitingTrust));
    let refused = bench.run_as(Trust::Refused, Trigger::Export, false);
    assert!(!refused.needs_trust);
    assert_eq!(bench.runs(), 0);
    let refused = states(&refused);
    assert!(refused.iter().all(|s| **s == CellState::TrustRefused));
    let _ = bench.run(Trigger::All);
    assert_eq!(bench.runs(), 3);
}

#[test]
fn the_run_report_is_a_json_result() {
    let mut bench = Bench::new();
    let report = bench.run(Trigger::All);
    let json = serde_json::to_value(&report).unwrap();
    assert_eq!(json["trigger"], "all");
    assert_eq!(json["runtime"], "ready");
    assert_eq!(json["version"], "Python 3.12");
    assert_eq!(json["cells"][0]["label"], "ripple-vs-vin");
    assert_eq!(json["cells"][0]["state"], "ran");
    assert_eq!(json["cells"][0]["duration_ms"], 212);
    assert_eq!(json["cells"][2]["state"], "blocked");
    assert_eq!(json["cells"][2]["refusal"]["kind"], "start_program");
    assert_eq!(json["summary"]["cells"], 3);
}

#[test]
fn a_deleted_cell_leaves_the_cache() {
    let mut bench = Bench::new();
    let _ = bench.run(Trigger::All);
    bench.cells.truncate(1);
    let _ = bench.run(Trigger::All);
    assert_eq!(bench.cache.len(), 1);
    assert!(bench.cache.get("ripple-vs-vin").is_some());
}

#[test]
fn the_page_lists_the_values_read_with_their_schematic_source() {
    let mut bench = Bench::new();
    let report = bench.run(Trigger::All);
    let page = bench.page(&report.cells, false);
    let ripple = &page.cells["ripple-vs-vin"];
    let values: Vec<_> = ripple
        .values
        .iter()
        .map(|v| (v.symbol.as_str(), v.value.as_str(), v.source.as_str()))
        .collect();
    assert_eq!(
        values,
        [
            ("L", "4.7 µH", "L1 value"),
            ("C", "22 µF", "C15 value"),
            ("esr", "5 mΩ", "C15 ESR"),
            ("fsw", "500 kHz", "U3 fsw"),
        ]
    );
    assert_eq!(ripple.caption, "ran in 212 ms · 14:20");
    let reads = "Reads L1, C15, U3 · re-runs only when one of them or the cell changes";
    assert_eq!(ripple.reads, reads);
    assert_eq!(ripple.figures.len(), 1);
    assert!(!ripple.greyed);
    assert_eq!(page.cells["git"].state, "blocked");
    assert!(page.cells["git"].stdout.is_empty());
    let cached = caption(&bench.shown()[1].state);
    assert_eq!(cached, "cached · inputs unchanged since 2026-11-25 14:20");
}

fn compiles(text: &str, cells: PageCells) {
    let options = CompileOptions {
        cells,
        ..CompileOptions::default()
    };
    let compilation = compile(&NotebookSources::new("design.snxnbk", text), &options);
    assert!(!compilation.has_errors(), "{:?}", compilation.diagnostics);
}

#[test]
fn every_cell_state_prints_and_compiling_runs_nothing() {
    let mut bench = Bench::new();
    let unlabelled = "#python[```python\nx = 1\n```]\n";
    let text = format!("{PACKAGE_IMPORT}\n{RIPPLE}{TWO_MORE}{unlabelled}");
    compiles(&text, bench.page(&bench.shown(), false));
    assert_eq!(bench.runs(), 0, "a compile runs no cell");

    let report = bench.run(Trigger::All);
    compiles(&text, bench.page(&report.cells, true));

    bench.design.set_value("C15", "47µF");
    compiles(&text, bench.page(&bench.shown(), false));

    let slim = cell_states(
        &bench.cells,
        &bench.cache,
        &bench.design,
        &RuntimeStatus::NotInstalled,
    );
    compiles(&text, bench.page(&slim, false));
}

#[test]
fn the_source_is_hidden_in_print_unless_asked_for() {
    let page = PageCells::default();
    assert!(!page.print_source, "hidden by default");
    let json: serde_json::Value = serde_json::from_slice(&page.to_json()).unwrap();
    assert_eq!(json["print_source"], false);
    assert_eq!(json["cells"], serde_json::json!({}));
    let shown = PageCells {
        print_source: true,
        ..PageCells::default()
    };
    let json: serde_json::Value = serde_json::from_slice(&shown.to_json()).unwrap();
    assert_eq!(json["print_source"], true);
}
