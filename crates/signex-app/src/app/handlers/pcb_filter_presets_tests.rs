//! #2383 — the board Selection Filter's presets and its live area chips,
//! through `Signex::update`: a preset saved from the chips applies after the
//! chips change and survives a restart; a Keepout chip turned off stops a
//! click picking the keepout.

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
        .document_state
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

    // Updated from the chips, then removed.
    run(panel(&mut app, PcbBoardMsg::FilterAllToggled));
    run(panel(&mut app, PcbBoardMsg::FilterPresetUpdated(0)));
    assert_eq!(
        app.ui_state.pcb_filter_presets[0].filters.len(),
        PickKind::ALL.len()
    );
    run(panel(&mut app, PcbBoardMsg::FilterPresetRemoved(0)));
    assert!(app.ui_state.pcb_filter_presets.is_empty());
    let restarted = app_workspace(&dir.path().to_string_lossy(), &[]);
    assert!(restarted.ui_state.pcb_filter_presets.is_empty());
}
