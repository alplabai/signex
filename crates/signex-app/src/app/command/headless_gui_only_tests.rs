//! #2170: a headless caller is refused a command whose whole effect is window
//! UI, with a reason and nothing changed; a command with a document, selection
//! or engine effect still runs. The config-root override is process-wide, so
//! each scenario runs in a child process of this test binary ([`probe`]).

#![allow(
    clippy::disallowed_methods,
    reason = "the test re-runs its own binary as a child process"
)]

use std::path::PathBuf;

use signex_types::project::{ErcRules, RuleProfile};

use super::{CommandArgs, CommandOutcome};
use crate::app::Signex;
use crate::app::command::interaction_snapshot_tests::interaction_snapshot;
use crate::app::handlers::editing_commands::net_color_driver as fixture;
use crate::keymap::{AppCommandId, HeadlessUse, all_command_ids, metadata_for_str};
use crate::task_driver::TaskDriver;

const PROBE_ENV: &str = "SIGNEX_HEADLESS_GUI_ONLY_PROBE";

/// Window UI that opens a modal or a dialog.
const MODALS: &[&str] = &[
    "open_erc_dialog",
    "report_manager_bom",
    "open_parameter_manager",
    "open_preferences",
    "open_command_palette",
    "open_grid_picker",
    "find",
    "annotate_schematic",
    "reset_schematic_designators",
];

/// Window UI that shows, hides or focuses a panel or a tab.
const PANELS: &[&str] = &[
    "toggle_properties_panel",
    "toggle_search_panel",
    "toggle_floating_panels",
    "open_components_panel",
    "next_document_tab",
    "toggle_schematic_list_panel",
];

/// Window UI that moves the view: zoom, grid, units, display toggles.
const VIEW: &[&str] = &[
    "zoom_to_fit",
    "zoom_in_at_cursor",
    "next_grid",
    "cycle_unit",
    "toggle_visible_grid",
    "toggle_electrical_grid",
    "toggle_auto_focus",
    "center_on_cursor",
];

/// Window UI that arms a tool or an interactive gesture.
const TOOLS: &[&str] = &[
    "clear_net_color",
    "draw_arc",
    "lasso_select",
    "cancel_current_tool",
    "measure_distance",
    "move_selection",
    "paste",
    "paste_array",
    "bring_to_front_of",
    "send_to_back_of",
];

/// Window-only selection and highlight modes, and in-place edits.
const OTHERS: &[&str] = &[
    "cycle_selection_mode",
    "toggle_cross_select_mode",
    "clear_net_highlighting",
    "edit_value",
    "break_wire",
    "next_sheet",
];

fn id(name: &str) -> AppCommandId {
    AppCommandId::new(name).expect("a valid id")
}

fn run_probe(scenario: &str) {
    let test = module_path!()
        .split_once("::")
        .map(|(_, rest)| format!("{rest}::probe"))
        .expect("a module path");
    let output = std::process::Command::new(std::env::current_exe().expect("test binary"))
        .args([test.as_str(), "--exact", "--nocapture", "--test-threads=1"])
        .env(PROBE_ENV, scenario)
        .output()
        .expect("run the probe");
    assert!(
        output.status.success(),
        "scenario {scenario} failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn a_modal_or_dialog_command_is_refused_headless() {
    run_probe("modals");
}

#[test]
fn a_panel_or_tab_command_is_refused_headless() {
    run_probe("panels");
}

#[test]
fn a_zoom_or_view_command_is_refused_headless() {
    run_probe("view");
}

#[test]
fn a_tool_arming_command_is_refused_headless() {
    run_probe("tools");
}

#[test]
fn a_selection_mode_or_in_place_edit_command_is_refused_headless() {
    run_probe("others");
}

#[test]
fn every_window_only_catalog_command_is_refused_headless() {
    run_probe("every_window_only");
}

#[test]
fn a_command_with_a_document_effect_still_runs_headless() {
    run_probe("document_effect");
}

#[test]
fn a_selection_step_a_copy_and_a_check_still_run_headless() {
    run_probe("selection_copy_check");
}

#[test]
fn every_running_command_changes_something_when_it_reports_ran() {
    run_probe("runs_rows");
}

#[test]
fn probe() {
    let Ok(scenario) = std::env::var(PROBE_ENV) else {
        return;
    };
    match scenario.as_str() {
        "modals" => refused(MODALS),
        "panels" => refused(PANELS),
        "view" => refused(VIEW),
        "tools" => refused(TOOLS),
        "others" => refused(OTHERS),
        "every_window_only" => every_window_only(),
        "document_effect" => document_effect(),
        "selection_copy_check" => selection_copy_check(),
        "runs_rows" => runs_rows_do_their_work(),
        other => panic!("unknown scenario {other}"),
    }
}

/// A headless app with one clean sheet open, and the driver that settles its
/// tasks.
struct Session {
    app: Signex,
    driver: TaskDriver,
    path: PathBuf,
}

impl Session {
    fn boot() -> Self {
        static ROOT: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
        let root = ROOT.get_or_init(|| {
            let root = tempfile::tempdir().expect("override root");
            crate::config_root::set_config_root_override(root.path().to_path_buf())
                .expect("override");
            root
        });
        let rules = RuleProfile::new("Headless", "1", ErcRules::default());
        let (mut app, boot) = Signex::boot_headless(Vec::new(), &rules).expect("boot");
        let mut driver = TaskDriver::new();
        driver.settle(&mut app, boot).expect("boot settled");
        let path = root.path().join("sheet.snxsch");
        fixture::add_sheet_with(&mut app, &path, content());
        app.document_state.mark_document_clean(&path);
        Self { app, driver, path }
    }

    fn run(&mut self, name: &str) -> CommandOutcome {
        let (outcome, task) = self
            .app
            .dispatch_command_headless(&id(name), CommandArgs::none());
        self.driver.settle(&mut self.app, task).expect("settled");
        outcome
    }

    /// What a person sees and what the file holds: the interaction snapshot,
    /// the dirty flag, the element counts and the undo position.
    fn state(&self) -> String {
        let engine = &self.app.document_state.engines[&self.path];
        let sheet = engine.document();
        format!(
            "{}\nreset_confirm={} erc_dialog={} erc_run={:?}\ndirty={}\nwires={} symbols={} junctions={}\nundo={}\ndoc={:?}",
            interaction_snapshot(&self.app),
            self.app.ui_state.annotate_reset_confirm,
            self.app.ui_state.erc_dialog_open,
            self.app.ui_state.erc_run_rule_set,
            self.app.document_state.is_document_dirty(&self.path),
            sheet.wires.len(),
            sheet.symbols.len(),
            sheet.junctions.len(),
            engine.undo_position(),
            sheet,
        )
    }

    /// What a command did that is not window UI: the document, the selection
    /// and the ERC results.
    fn effects(&self) -> String {
        let engine = &self.app.document_state.engines[&self.path];
        format!(
            "{:?}|{}|{}|{:?}|{:?}",
            engine.document(),
            engine.undo_position(),
            self.app.document_state.is_document_dirty(&self.path),
            self.app.interaction_state.active_canvas().selected,
            self.app.ui_state.erc_run_rule_set,
        )
    }

    fn element_count(&self) -> usize {
        let sheet = self.app.document_state.engines[&self.path].document();
        sheet.wires.len() + sheet.symbols.len() + sheet.junctions.len()
    }

    fn drops(&self) -> u64 {
        self.driver.dropped_window_actions()
    }

    fn refusals(&self) -> u64 {
        self.app.headless_answers().map_or(0, |a| a.refusals())
    }

    fn selected(&self) -> usize {
        self.app.interaction_state.active_canvas().selected.len()
    }
}

fn assert_refused(session: &mut Session, name: &str) {
    let before = session.state();
    let outcome = session.run(name);
    assert_eq!(
        outcome,
        CommandOutcome::Disabled {
            reason: format!("`{name}` only works in a window")
        },
        "{name}"
    );
    assert_eq!(session.state(), before, "{name} changed something");
}

/// Every id in `group` is a catalog row classified window only, and is
/// refused with the reason and no change.
fn refused(group: &[&str]) {
    let mut session = Session::boot();
    for name in group {
        let meta = metadata_for_str(name).unwrap_or_else(|| panic!("{name} is not a command"));
        assert_eq!(meta.headless_use(), Some(HeadlessUse::WindowOnly), "{name}");
        assert_refused(&mut session, name);
    }
}

fn every_window_only() {
    let mut session = Session::boot();
    let mut refused = 0;
    for name in all_command_ids() {
        let window_only = metadata_for_str(name)
            .is_some_and(|meta| meta.headless_use() == Some(HeadlessUse::WindowOnly));
        if window_only {
            assert_refused(&mut session, name);
            refused += 1;
        }
    }
    assert!(refused > 100, "{refused}");
}

/// Select everything, delete it: the sheet loses its elements, goes dirty and
/// gains an undo step.
fn document_effect() {
    let mut session = Session::boot();
    let before = session.state();
    assert_eq!(session.run("select_all"), CommandOutcome::Ran);
    assert!(session.selected() > 0);
    assert_eq!(session.run("delete_selection"), CommandOutcome::Ran);
    let engine = &session.app.document_state.engines[&session.path];
    assert!(engine.document().wires.is_empty());
    assert!(engine.document().symbols.is_empty());
    assert!(session.app.document_state.is_document_dirty(&session.path));
    assert_ne!(session.state(), before);
    assert_eq!(engine.undo_position(), 1, "one undo step");
}

/// The commands that act without a window and without touching the document:
/// a selection step, a copy, an ERC run.
fn selection_copy_check() {
    let mut session = Session::boot();
    assert_eq!(session.selected(), 0);
    assert_eq!(session.run("select_all"), CommandOutcome::Ran);
    let selected = session.selected();
    assert!(selected > 0);
    let count = session.element_count();
    assert_eq!(session.run("copy"), CommandOutcome::Ran);
    assert_eq!(session.selected(), selected, "copy keeps the selection");
    // The copy is real: pasting it back adds the copied elements.
    assert_eq!(session.run("smart_paste"), CommandOutcome::Ran);
    assert!(
        session.element_count() > count,
        "smart_paste after copy adds elements: {count} -> {}",
        session.element_count()
    );
    assert_eq!(session.run("unselect_all"), CommandOutcome::Ran);
    assert_eq!(session.selected(), 0);
    assert!(session.app.ui_state.erc_run_rule_set.is_none());
    assert_eq!(session.run("run_erc"), CommandOutcome::Ran);
    assert!(
        session.app.ui_state.erc_run_rule_set.is_some() && !session.app.ui_state.erc_running,
        "the ERC run finished and left its results"
    );
}

/// Every command classified as running, run without operands on a sheet with
/// content and a selection: one that reports `Ran` must have changed the
/// document, the selection or the clipboard. A `Ran` whose only trace is a
/// dropped window or a dialog flag is a fake one: classify it window only.
fn runs_rows_do_their_work() {
    let mut fakes = Vec::new();
    for name in all_command_ids() {
        let meta = metadata_for_str(name).expect("row");
        if meta.headless_use() != Some(HeadlessUse::Runs) {
            continue;
        }
        let mut session = Session::boot();
        if name != "select_all" {
            session.run("select_all");
            session.run("copy");
        }
        let (effects, window, drops, refusals) = (
            session.effects(),
            session.state(),
            session.drops(),
            session.refusals(),
        );
        let clip = session.driver.clipboard_text().map(str::to_string);
        let outcome = session.run(name);
        let worked = session.effects() != effects
            || session.driver.clipboard_text().map(str::to_string) != clip;
        let row = format!(
            "{name:<36} {:<18} effect={worked:<5} window_state={:<5} dropped={} refusals={}",
            format!("{outcome:?}").chars().take(18).collect::<String>(),
            session.state() != window,
            session.drops() - drops,
            session.refusals() - refusals,
        );
        println!("{row}");
        if outcome == CommandOutcome::Ran && (!worked || session.drops() > drops) {
            fakes.push(row);
        }
    }
    assert!(
        fakes.is_empty(),
        "Ran without an effect:\n{}",
        fakes.join("\n")
    );
}

/// A sheet with content a command can act on: two unannotated parts apart
/// from each other, two wires and a junction, all at different places.
fn content() -> signex_types::schematic::SchematicSheet {
    use signex_types::schematic::{Junction, Point, SchematicSheet, Symbol, Wire};
    let part = |reference: &str, at: (f64, f64)| Symbol {
        uuid: uuid::Uuid::now_v7(),
        lib_id: "Device:R".to_string(),
        reference: reference.to_string(),
        position: Point::new(at.0, at.1),
        ..Default::default()
    };
    let wire = |from: (f64, f64), to: (f64, f64)| Wire {
        uuid: uuid::Uuid::now_v7(),
        start: Point::new(from.0, from.1),
        end: Point::new(to.0, to.1),
        ..Default::default()
    };
    SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        symbols: vec![part("R?", (20.0, 20.0)), part("R?", (60.0, 45.0))],
        wires: vec![
            wire((0.0, 0.0), (10.0, 0.0)),
            wire((30.0, 5.0), (30.0, 25.0)),
        ],
        junctions: vec![Junction {
            uuid: uuid::Uuid::now_v7(),
            position: Point::new(7.0, 3.0),
            ..Default::default()
        }],
        ..Default::default()
    }
}
