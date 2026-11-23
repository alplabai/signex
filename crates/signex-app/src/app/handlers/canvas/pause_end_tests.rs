//! What ends a paused placement from outside it (mockup row 157). Each test
//! runs with the pause in main and in an undocked window.

use std::path::{Path, PathBuf};

use signex_types::pcb::PcbBoard;
use signex_types::schematic::SchematicSheet;

use super::dimension_tool_pause_tests::{
    app_on_sheet_a, click_in, held_points, pause_in, show_tab, tab_key, undocked_over,
};
use super::placement_tests::click;
use crate::app::project_git::{SwitchDone, SwitchSurface};
use crate::app::restore_job::{RestoreFailure, RestoreJobDocument};
use crate::app::{Message, Signex, TabInfo, TabKind, Tool, ToolMessage};

const FROM: (f64, f64) = (0.0, 0.0);
const TO: (f64, f64) = (12.7, 0.0);
const TYPED_PRECISION: u8 = 4;

fn board() -> PcbBoard {
    serde_json::from_value(serde_json::json!({
        "uuid": uuid::Uuid::from_u128(1),
        "thickness": 1.6,
        "layers": [{"id": 0, "name": "TopCopper", "layer_type": "signal"}],
    }))
    .expect("the fixture board")
}

/// Sheet A in main, B undocked and C behind, with two points down and paused
/// by Tab in main or in B's window, and a precision typed in the form.
struct Paused {
    app: Signex,
    paths: Vec<PathBuf>,
    window: iced::window::Id,
    in_window: bool,
}

impl Paused {
    fn new(in_window: bool) -> Self {
        let (mut app, paths) = app_on_sheet_a(&["b.snxsch", "c.snxsch"]);
        let window = undocked_over(&mut app, &paths[1]);
        let _ = app.run_in_window(window, true, |_| iced::Task::none());
        if in_window {
            pause_in(&mut app, window);
        } else {
            click(&mut app, FROM);
            click(&mut app, TO);
            tab_key(&mut app);
        }
        app.interaction_state.annotation_tool.dimension.precision = TYPED_PRECISION;
        Self {
            app,
            paths,
            window,
            in_window,
        }
    }

    fn each(mut check: impl FnMut(Self)) {
        for in_window in [false, true] {
            check(Self::new(in_window));
        }
    }

    /// The sheet the pause was taken on.
    fn owner_sheet(&self) -> PathBuf {
        self.paths[usize::from(self.in_window)].clone()
    }

    fn is_frozen(&self) -> bool {
        let state = &self.app.interaction_state;
        state.canvas.placement_paused || state.canvases[&self.window].placement_paused
    }

    fn assert_ended(&self, why: &str) {
        let state = &self.app.interaction_state;
        assert!(state.placement_pause.is_none(), "{why}: the pause ended");
        assert!(!self.is_frozen(), "{why}: no canvas stays frozen");
        assert_eq!(held_points(&self.app), 0, "{why}: the points are gone");
        assert!(state.annotation_tool.paused_with.is_none(), "{why}");
        assert_eq!(
            state.annotation_tool.dimension.precision, TYPED_PRECISION,
            "{why}: what was typed stays for the next placement"
        );
    }

    fn assert_still_paused(&self, why: &str) {
        let state = &self.app.interaction_state;
        assert!(state.placement_pause.is_some(), "{why}: the pause stays");
        assert!(self.is_frozen(), "{why}: its canvas stays paused");
        assert_eq!(held_points(&self.app), 2, "{why}: its points stay");
    }

    fn reload(&mut self, path: &Path) {
        let tabs = &self.app.document_state.tabs;
        let idx = tabs.iter().position(|tab| tab.path == path).expect("tab");
        let sheet = self.app.document_state.engines[path].document().clone();
        let document = RestoreJobDocument::Schematic(Box::new(sheet));
        let _ = self
            .app
            .apply_reloaded_reporting(idx, Ok(document), None, false);
    }

    fn switch_branch(&mut self, root: &Path) {
        let done = SwitchDone {
            root: root.to_path_buf(),
            branch: "other".into(),
            changed: Vec::new(),
            reopen: Vec::new(),
        };
        let _ = self
            .app
            .finish_branch_switch(SwitchSurface::History, Ok(done));
    }

    fn push_tab(&mut self, name: &str, kind: TabKind) -> PathBuf {
        let path = PathBuf::from(name);
        self.app.document_state.tabs.push(TabInfo {
            title: name.to_string(),
            path: path.clone(),
            cached_document: None,
            project_id: None,
            kind,
        });
        path
    }
}

#[test]
fn a_reload_of_the_paused_sheet_ends_the_pause_and_keeps_the_tool() {
    Paused::each(|mut paused| {
        let sheet = paused.owner_sheet();
        paused.reload(&sheet);
        paused.assert_ended("reload");
        let state = &paused.app.interaction_state;
        assert_eq!(state.current_tool, Tool::LinearDimension, "still armed");
    });
}

#[test]
fn a_failed_reload_of_the_paused_sheet_ends_the_pause() {
    Paused::each(|mut paused| {
        let sheet = paused.owner_sheet();
        let tabs = &paused.app.document_state.tabs;
        let idx = tabs.iter().position(|tab| tab.path == sheet).expect("tab");
        let failure = RestoreFailure::NotReloaded {
            why: "unreadable".into(),
            unreadable: true,
        };
        let _ = paused
            .app
            .apply_reloaded_reporting(idx, Err(failure), None, false);
        paused.assert_ended("failed reload");
    });
}

#[test]
fn a_reload_of_another_sheet_keeps_the_pause() {
    Paused::each(|mut paused| {
        let other = paused.paths[2].clone();
        paused.reload(&other);
        paused.assert_still_paused("unrelated reload");
    });
}

#[test]
fn a_reload_of_the_board_main_shows_keeps_a_windows_pause() {
    let mut paused = Paused::new(true);
    let path = PathBuf::from("board.snxpcb");
    paused.app.open_pcb_tab(path, "board".into(), board());
    // Main moving to the board ended that pause: take a new one over it.
    let _ = paused.app.update(Message::Tool(ToolMessage::SelectTool(
        Tool::LinearDimension,
    )));
    pause_in(&mut paused.app, paused.window);

    let idx = paused.app.document_state.active_tab;
    let document = RestoreJobDocument::Pcb(Box::new(board()));
    let _ = paused
        .app
        .apply_reloaded_reporting(idx, Ok(document), None, false);
    paused.assert_still_paused("reload of main's board");

    let failure = RestoreFailure::NotReloaded {
        why: "unreadable".into(),
        unreadable: true,
    };
    let _ = paused
        .app
        .apply_reloaded_reporting(idx, Err(failure), None, false);
    paused.assert_still_paused("failed reload of main's board");
}

#[test]
fn a_reload_clears_the_owners_form_and_lets_the_tool_place_again() {
    let mut paused = Paused::new(true);
    let sheet = paused.owner_sheet();
    paused.reload(&sheet);
    let ctx = &paused.app.document_state.window_panel_ctx[&paused.window];
    let form = ctx.annotation.dimension_tool.as_ref().expect("armed form");
    assert!(!form.is_paused && !form.has_from, "the form starts over");

    click_in(&mut paused.app, paused.window, FROM);
    assert_eq!(held_points(&paused.app), 1, "a click is a new first point");
}

#[test]
fn a_branch_switch_ends_a_pause_on_a_sheet_of_that_repository() {
    Paused::each(|mut paused| {
        // These sheets have relative paths: the empty root holds them.
        paused.switch_branch(Path::new(""));
        paused.assert_ended("branch switch");
    });
}

#[test]
fn a_branch_switch_of_another_repository_keeps_the_pause() {
    Paused::each(|mut paused| {
        paused.switch_branch(Path::new("/elsewhere/repo"));
        paused.assert_still_paused("another repository");
    });
}

#[test]
fn main_moving_to_a_board_ends_the_pause() {
    Paused::each(|mut paused| {
        let path = PathBuf::from("board.snxpcb");
        paused.app.open_pcb_tab(path, "board".into(), board());
        paused.assert_ended("board");
    });
}

#[test]
fn main_moving_to_a_library_tab_ends_the_pause() {
    Paused::each(|mut paused| {
        let library = PathBuf::from("parts.snxlib");
        let tab = paused.push_tab("parts.snxlib", TabKind::LibraryBrowser(library));
        show_tab(&mut paused.app, &tab);
        paused.assert_ended("library");
    });
}

#[test]
fn main_moving_to_an_empty_tab_ends_the_pause() {
    Paused::each(|mut paused| {
        let tab = paused.push_tab("stack", TabKind::LayerStack);
        show_tab(&mut paused.app, &tab);
        paused.assert_ended("empty tab");
    });
}

#[test]
fn main_with_no_tab_left_ends_the_pause() {
    Paused::each(|mut paused| {
        paused.app.document_state.active_tab = paused.app.document_state.tabs.len();
        paused.app.sync_active_tab();
        paused.assert_ended("no tab");
    });
}

#[test]
fn a_main_move_clears_the_ghost_of_a_window_paused_on_a_label() {
    let (mut app, paths) = app_on_sheet_a(&["b.snxsch"]);
    let window = undocked_over(&mut app, &paths[1]);
    let _ = app.run_in_window_document(window, |app| {
        let _ = app.update(Message::Tool(ToolMessage::SelectTool(Tool::Label)));
        app.update(Message::Tool(ToolMessage::PrePlacementTab))
    });
    let slot = &mut app.interaction_state.canvases.get_mut(&window).unwrap();
    assert!(slot.placement_paused, "paused in the window");
    slot.tool_preview = Some("Label".to_string());
    let form = app.document_state.panel_ctx.pre_placement.clone();

    app.open_pcb_tab("board.snxpcb".into(), "board".into(), board());

    let slot = &app.interaction_state.canvases[&window];
    assert!(!slot.placement_paused && slot.tool_preview.is_none());
    assert!(slot.ghost_label.is_none());
    assert_eq!(
        app.document_state.panel_ctx.pre_placement.is_some(),
        form.is_some(),
        "the typed form stays for the next placement"
    );
}

#[test]
fn a_reload_keeps_the_typed_form_of_a_paused_label() {
    let (mut app, paths) = app_on_sheet_a(&[]);
    let _ = app.update(Message::Tool(ToolMessage::SelectTool(Tool::Label)));
    tab_key(&mut app);
    let form = app.document_state.panel_ctx.pre_placement.as_mut();
    form.expect("the Tab form").label_text = "TYPED".to_string();

    let sheet = SchematicSheet::default();
    let idx = app.document_state.active_tab;
    let document = RestoreJobDocument::Schematic(Box::new(sheet));
    let _ = app.apply_reloaded_reporting(idx, Ok(document), None, false);

    assert!(!app.interaction_state.canvas.placement_paused);
    assert!(app.interaction_state.placement_pause.is_none());
    let form = app.document_state.panel_ctx.pre_placement.as_ref();
    assert_eq!(form.expect("kept").label_text, "TYPED");
    assert_eq!(paths.len(), 1);
}
