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
    "place_component",
    "lasso_select",
    "cancel_current_tool",
    "measure_distance",
    "move_selection",
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
        other => panic!("unknown scenario {other}"),
    }
}

/// A headless app with one clean sheet open, and the driver that settles its
/// tasks.
struct Session {
    app: Signex,
    driver: TaskDriver,
    path: PathBuf,
    _root: tempfile::TempDir,
}

impl Session {
    fn boot() -> Self {
        let root = tempfile::tempdir().expect("override root");
        crate::config_root::set_config_root_override(root.path().to_path_buf()).expect("override");
        let rules = RuleProfile::new("Headless", "1", ErcRules::default());
        let (mut app, boot) = Signex::boot_headless(Vec::new(), &rules).expect("boot");
        let mut driver = TaskDriver::new();
        driver.settle(&mut app, boot).expect("boot settled");
        let path = root.path().join("sheet.snxsch");
        fixture::add_sheet(&mut app, &path);
        app.document_state.mark_document_clean(&path);
        Self {
            app,
            driver,
            path,
            _root: root,
        }
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
            "{}\ndirty={}\nwires={} symbols={} junctions={}\nundo={}",
            interaction_snapshot(&self.app),
            self.app.document_state.is_document_dirty(&self.path),
            sheet.wires.len(),
            sheet.symbols.len(),
            sheet.junctions.len(),
            engine.undo_position(),
        )
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
    assert_eq!(session.run("copy"), CommandOutcome::Ran);
    assert_eq!(session.selected(), selected, "copy keeps the selection");
    assert_eq!(session.run("unselect_all"), CommandOutcome::Ran);
    assert_eq!(session.selected(), 0);
    assert_eq!(session.run("run_erc"), CommandOutcome::Ran);
}
