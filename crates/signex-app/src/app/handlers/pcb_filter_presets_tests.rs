//! #2383 — the board Selection Filter's presets and its live area chips,
//! through `Signex::update`: a preset saved from the chips applies after the
//! chips change and survives a restart; a Keepout chip turned off stops a
//! click picking the keepout. #2873: presets are edited in place.

use std::path::Path;

use iced::Task;
use pcb_geom::{ClickMode, ItemRef, PickKind};
use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{Message, Signex};
use crate::dock::DockMessage;
use crate::panels::{PanelMsg, PcbBoardMsg};
use crate::pcb_canvas::PcbCanvasMsg;

const KEEPOUT: Uuid = Uuid::from_u128(0x70);

/// A 50 x 40 mm board holding one keepout triangle on the top copper.
fn board() -> PcbBoard {
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "setup": {"grid_size": 1.0},
        "outline": [
            {"x": 0.0, "y": 0.0}, {"x": 50.0, "y": 0.0},
            {"x": 50.0, "y": 40.0}, {"x": 0.0, "y": 40.0}
        ],
        "layers": [{"id": 0, "name": "TopCopper", "layer_type": "signal"}],
        "zones": [{"uuid": KEEPOUT, "layer": "TopCopper", "fill_type": "keepout",
                   "outline": [{"x": 10.0, "y": 10.0}, {"x": 30.0, "y": 10.0},
                               {"x": 10.0, "y": 30.0}]}]
    }))
    .unwrap()
}

fn app_with_board(dir: &Path) -> Signex {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    app.open_pcb_tab(dir.join("board.snxpcb"), "board".into(), board());
    app.ui_state.pcb_filter_presets.clear();
    app
}

fn panel(app: &mut Signex, msg: PcbBoardMsg) -> Task<Message> {
    app.update(Message::Dock(DockMessage::Panel(PanelMsg::PcbBoard(msg))))
}

fn click(app: &mut Signex, x: f64, y: f64) -> Vec<ItemRef> {
    run(app.update(Message::PcbCanvas(PcbCanvasMsg::Clicked {
        at: Point::new(x, y),
        tolerance_mm: 0.05,
        mode: ClickMode::Replace,
    })));
    app.interaction_state
        .pcb_selection
        .selection()
        .items()
        .collect()
}

fn run(task: Task<Message>) {
    use futures::StreamExt;
    let Some(mut stream) = iced_runtime::task::into_stream(task) else {
        return;
    };
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    runtime.block_on(async { while stream.next().await.is_some() {} });
}

fn keepout() -> ItemRef {
    ItemRef::Zone {
        zone: KEEPOUT,
        kind: PickKind::Keepouts,
    }
}

#[test]
fn a_keepout_chip_turned_off_stops_the_keepout_being_picked() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_with_board(dir.path());
    assert_eq!(click(&mut app, 15.0, 15.0), vec![keepout()], "inside it");
    assert_eq!(click(&mut app, 20.0, 10.0), vec![keepout()], "on its edge");

    run(panel(
        &mut app,
        PcbBoardMsg::FilterKindToggled(PickKind::Keepouts),
    ));
    assert!(click(&mut app, 15.0, 15.0).is_empty());

    run(panel(
        &mut app,
        PcbBoardMsg::FilterKindToggled(PickKind::Keepouts),
    ));
    assert_eq!(click(&mut app, 15.0, 15.0), vec![keepout()]);
}

/// The one test of this file that runs the prefs writes: the others share
/// the per-process prefs file, so they leave their write tasks unrun.
#[test]
fn a_saved_preset_applies_after_the_chips_change_and_outlives_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_with_board(dir.path());
    // Only Tracks and Vias on, then saved as a preset.
    run(panel(&mut app, PcbBoardMsg::FilterAllToggled));
    for kind in [PickKind::Tracks, PickKind::Vias] {
        run(panel(&mut app, PcbBoardMsg::FilterKindToggled(kind)));
    }
    let task = panel(&mut app, PcbBoardMsg::FilterPresetAdded);
    assert!(task.units() >= 1, "the save is a task, not inline IO");
    run(task);
    run(panel(
        &mut app,
        PcbBoardMsg::FilterPresetRenamed(0, "Routing".into()),
    ));
    let summary = app
        .document_view
        .panel_ctx
        .pcb_board
        .as_ref()
        .expect("board panel");
    assert_eq!(summary.presets.len(), 1, "the panel shows the preset");

    // Every chip back on: the keepout picks again.
    run(panel(&mut app, PcbBoardMsg::FilterAllToggled));
    assert!(app.ui_state.pcb_pick_filter.is_all_on());
    assert_eq!(click(&mut app, 15.0, 15.0), vec![keepout()]);

    // Applying the preset turns on exactly its kinds.
    run(panel(&mut app, PcbBoardMsg::FilterPresetApplied(0)));
    let on: Vec<PickKind> = app.ui_state.pcb_pick_filter.on_kinds().collect();
    assert_eq!(on, vec![PickKind::Tracks, PickKind::Vias]);
    assert!(click(&mut app, 15.0, 15.0).is_empty());

    // A restart reads the preset back.
    let restarted = app_workspace(&dir.path().to_string_lossy(), &[]);
    let presets = &restarted.ui_state.pcb_filter_presets;
    assert_eq!(presets.len(), 1);
    assert_eq!(presets[0].name, "Routing");
    assert_eq!(presets[0].filters, vec![PickKind::Tracks, PickKind::Vias]);

    // #2873 (Caner, 2026-11-24) supersedes Save Chips: the applied preset
    // is selected for editing, so a chip click changes it at once and the
    // change is saved; it was All - On then Save Chips
    // (`FilterPresetUpdated`), which is gone.
    let task = panel(&mut app, PcbBoardMsg::FilterKindToggled(PickKind::Pads));
    assert!(task.units() >= 1, "the save is a task, not inline IO");
    run(task);
    let edited = vec![PickKind::Tracks, PickKind::Pads, PickKind::Vias];
    assert_eq!(app.ui_state.pcb_filter_presets[0].filters, edited);
    let restarted = app_workspace(&dir.path().to_string_lossy(), &[]);
    assert_eq!(restarted.ui_state.pcb_filter_presets[0].filters, edited);
    run(panel(&mut app, PcbBoardMsg::FilterPresetRemoved(0)));
    assert!(app.ui_state.pcb_filter_presets.is_empty());
    let restarted = app_workspace(&dir.path().to_string_lossy(), &[]);
    assert!(restarted.ui_state.pcb_filter_presets.is_empty());
}

/// A panel message, its prefs write left unrun (see the restart test).
fn send(app: &mut Signex, msg: PcbBoardMsg) -> Task<Message> {
    panel(app, msg)
}

/// The kinds the board filter has on.
fn on(app: &Signex) -> Vec<PickKind> {
    app.ui_state.pcb_pick_filter.on_kinds().collect()
}

/// The kinds of the board's preset `idx`.
fn kinds(app: &Signex, idx: usize) -> Vec<PickKind> {
    app.ui_state.pcb_filter_presets[idx].filters.clone()
}

/// The preset the board panel shows as selected for editing.
fn shown_editing(app: &Signex) -> Option<usize> {
    let summary = app.document_view.panel_ctx.pcb_board.as_ref();
    summary.expect("board panel").preset_editing
}

/// An app on the board with two saved presets, Vias (0) and Tracks (1),
/// none selected, every chip on.
fn app_with_two_presets(dir: &Path) -> Signex {
    let mut app = app_with_board(dir);
    for kind in [PickKind::Vias, PickKind::Tracks] {
        let _ = send(&mut app, PcbBoardMsg::FilterAllSet(false));
        let _ = send(&mut app, PcbBoardMsg::FilterKindToggled(kind));
        let _ = send(&mut app, PcbBoardMsg::FilterPresetAdded);
    }
    let _ = send(&mut app, PcbBoardMsg::FilterAllSet(true));
    app
}

/// #2873: a click on a preset applies it and selects it for editing.
#[test]
fn a_preset_click_applies_it_and_selects_it_for_editing() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_with_two_presets(dir.path());
    assert_eq!(app.ui_state.pcb_filter_preset_editing, None);
    let _ = send(&mut app, PcbBoardMsg::FilterPresetApplied(1));
    assert_eq!(on(&app), vec![PickKind::Tracks]);
    assert_eq!(app.ui_state.pcb_filter_preset_editing, Some(1));
    assert_eq!(shown_editing(&app), Some(1), "the panel shows it");
    let _ = send(&mut app, PcbBoardMsg::FilterPresetApplied(7));
    assert_eq!(shown_editing(&app), Some(1), "past the end: no change");
}

/// #2873: while a preset is selected, each chip click changes it at once,
/// with a prefs write, and the filter with it.
#[test]
fn a_chip_click_changes_the_selected_preset_and_the_filter() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_with_two_presets(dir.path());
    let _ = send(&mut app, PcbBoardMsg::FilterPresetApplied(0));
    let task = send(&mut app, PcbBoardMsg::FilterKindToggled(PickKind::Pads));
    assert!(task.units() >= 1, "the preset is saved");
    assert_eq!(on(&app), vec![PickKind::Pads, PickKind::Vias]);
    assert_eq!(kinds(&app, 0), vec![PickKind::Pads, PickKind::Vias]);
    assert_eq!(kinds(&app, 1), vec![PickKind::Tracks], "the other is kept");
    let _ = send(&mut app, PcbBoardMsg::FilterKindToggled(PickKind::Vias));
    assert_eq!(kinds(&app, 0), vec![PickKind::Pads]);
    assert_eq!(on(&app), vec![PickKind::Pads]);
}

/// #2873: All - On, All, None and a second click on the selected preset
/// leave editing; a chip then changes only the filter.
#[test]
fn all_on_none_or_a_second_click_leave_editing() {
    let leave = [
        PcbBoardMsg::FilterAllToggled,
        PcbBoardMsg::FilterAllSet(true),
        PcbBoardMsg::FilterAllSet(false),
        PcbBoardMsg::FilterPresetApplied(0),
    ];
    for left in leave {
        let dir = tempfile::tempdir().unwrap();
        let mut app = app_with_two_presets(dir.path());
        let _ = send(&mut app, PcbBoardMsg::FilterPresetApplied(0));
        let before = on(&app);
        let _ = send(&mut app, left.clone());
        assert_eq!(app.ui_state.pcb_filter_preset_editing, None, "{left:?}");
        assert_eq!(shown_editing(&app), None, "{left:?}");
        assert_eq!(kinds(&app, 0), vec![PickKind::Vias], "{left:?} keeps it");
        if left == PcbBoardMsg::FilterPresetApplied(0) {
            assert_eq!(on(&app), before, "a second click keeps the filter");
        }
        let had_pads = app.ui_state.pcb_pick_filter.is_on(PickKind::Pads);
        let _ = send(&mut app, PcbBoardMsg::FilterKindToggled(PickKind::Pads));
        let has_pads = app.ui_state.pcb_pick_filter.is_on(PickKind::Pads);
        assert_ne!(had_pads, has_pads, "{left:?}: the chip changes the filter");
        assert_eq!(
            kinds(&app, 0),
            vec![PickKind::Vias],
            "{left:?}: not the preset"
        );
    }
}

/// #2873: the name field renames the selected preset and Delete removes it;
/// deleting an earlier preset keeps the selection on its preset.
#[test]
fn the_name_field_renames_and_delete_removes_the_selected_preset() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_with_two_presets(dir.path());
    let _ = send(&mut app, PcbBoardMsg::FilterPresetApplied(1));
    let _ = send(
        &mut app,
        PcbBoardMsg::FilterPresetRenamed(1, "Routing".into()),
    );
    assert_eq!(app.ui_state.pcb_filter_presets[1].name, "Routing");
    let _ = send(&mut app, PcbBoardMsg::FilterPresetRemoved(0));
    assert_eq!(shown_editing(&app), Some(0));
    assert_eq!(app.ui_state.pcb_filter_presets[0].name, "Routing");
    let _ = send(&mut app, PcbBoardMsg::FilterPresetRemoved(0));
    assert!(app.ui_state.pcb_filter_presets.is_empty());
    assert_eq!(shown_editing(&app), None);
}

/// #2873: Add Preset saves the chips that are on as a new preset and
/// selects it; the four-preset cap stays.
#[test]
fn add_preset_saves_the_chips_and_selects_the_new_preset() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_with_two_presets(dir.path());
    let _ = send(&mut app, PcbBoardMsg::FilterAllSet(false));
    let _ = send(&mut app, PcbBoardMsg::FilterKindToggled(PickKind::Keepouts));
    let _ = send(&mut app, PcbBoardMsg::FilterPresetAdded);
    assert_eq!(kinds(&app, 2), vec![PickKind::Keepouts]);
    assert_eq!(shown_editing(&app), Some(2));
    let _ = send(&mut app, PcbBoardMsg::FilterPresetAdded);
    assert_eq!(shown_editing(&app), Some(3));
    let _ = send(&mut app, PcbBoardMsg::FilterPresetAdded);
    let cap = crate::active_bar::CUSTOM_FILTER_PRESET_LIMIT;
    assert_eq!(app.ui_state.pcb_filter_presets.len(), cap, "the cap holds");
    assert_eq!(
        shown_editing(&app),
        Some(3),
        "a refused add selects nothing"
    );
}

/// #2383: the board panel header's filter button opens and closes the
/// Active Bar's Selection Filter dropdown.
#[test]
fn the_header_filter_button_opens_the_active_bar_dropdown() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_with_board(dir.path());
    run(panel(&mut app, PcbBoardMsg::FilterMenuToggled));
    assert_eq!(
        app.interaction_state.pcb_bar_menu,
        Some(crate::app::PcbBarMenu::Filter)
    );
    run(panel(&mut app, PcbBoardMsg::FilterMenuToggled));
    assert_eq!(app.interaction_state.pcb_bar_menu, None);
}
