//! #2480: a board view is saved through the debounced loop and restored when
//! the board opens again in the next launch.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use signex_renderer::pcb::{BoardObject, PcbLayerView};
use signex_types::pcb::PcbBoard;
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};

use crate::app::{FileMsg, Message, PcbMsg, PcbViewMsg, Signex};
use crate::board_view_prefs::{BoardViewSync, read_board_views_from};
use crate::fonts::PrefsBoardViewStore;

const BOTTOM: PcbLayer = PcbLayer::Copper(CopperLayer::Bottom);
const OVERLAY: PcbLayer = PcbLayer::Technical(TechnicalLayer::SilkscreenTop);
const BOARD: &str = "/w/kept.snxpcb";

/// Top and Bottom copper and the top overlay.
fn board() -> PcbBoard {
    serde_json::from_str(
        r#"{
          "uuid": "8d15b2f9-8f86-41d7-9ec4-0f54a4f3a651", "version": 1,
          "generator": "test", "thickness": 1.6, "outline": [],
          "layers": [
            { "id": 0, "name": "TopCopper", "layer_type": "signal" },
            { "id": 1, "name": "TopSilkscreen", "layer_type": "" },
            { "id": 31, "name": "BottomCopper", "layer_type": "signal" }
          ],
          "setup": null, "nets": [], "footprints": [],
          "segments": [], "vias": [], "zones": [], "graphics": [], "texts": []
        }"#,
    )
    .expect("valid board")
}

fn launch() -> Signex {
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app
}

fn open(app: &mut Signex) -> PathBuf {
    let path = PathBuf::from(BOARD);
    let _ = app.update(Message::File(FileMsg::PcbOpenFinished {
        disk: None,
        path: path.clone(),
        title: "kept".to_string(),
        result: Ok(Box::new(board())),
    }));
    path
}

fn view_event(app: &mut Signex, event: PcbViewMsg) {
    let _ = app.update(Message::Pcb(PcbMsg::View(event)));
}

fn read(prefs: &Path) -> Option<serde_json::Value> {
    let bytes = std::fs::read(prefs).ok()?;
    serde_json::from_slice(&bytes).ok()
}

#[test]
fn a_changed_board_view_is_saved_and_restored_in_the_next_launch() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let prefs = dir.path().join("prefs.json");

    let mut app = launch();
    *app.ui_state.pcb_layer_view_sync = BoardViewSync::new(
        Arc::new(PrefsBoardViewStore::at(prefs.clone())),
        Default::default(),
    );
    let path = open(&mut app);
    view_event(&mut app, PcbViewMsg::LayerVisibilityToggled(OVERLAY));
    view_event(&mut app, PcbViewMsg::ActiveLayerPicked(BOTTOM));
    view_event(&mut app, PcbViewMsg::BoardFlipToggled);
    view_event(&mut app, PcbViewMsg::RatsnestVisibilityToggled);
    view_event(&mut app, PcbViewMsg::UsedLayersOnlySet(true));
    view_event(
        &mut app,
        PcbViewMsg::ObjectVisibilitySet {
            kind: Some(BoardObject::Vias),
            visible: false,
        },
    );
    view_event(
        &mut app,
        PcbViewMsg::ObjectTransparencySet {
            kind: BoardObject::Tracks,
            percent: 40,
        },
    );
    // Not kept: it ends with the board.
    view_event(&mut app, PcbViewMsg::LayerHighlightToggled(BOTTOM));
    let changed = app.pcb_layer_view(&path);
    assert_ne!(changed.kept(), PcbLayerView::default(), "the view changed");

    app.ui_state
        .pcb_layer_view_sync
        .flush()
        .expect("the change waits for its write")
        .run()
        .expect("written");
    let kept = read_board_views_from(read(&prefs).as_ref());
    assert_eq!(kept.get(&path), Some(&changed.kept()));

    let mut next = launch();
    *next.ui_state.saved_pcb_layer_views = kept;
    assert_eq!(
        next.pcb_layer_view(&path),
        PcbLayerView::default(),
        "not open yet"
    );
    open(&mut next);
    assert_eq!(
        next.pcb_layer_view(&path),
        changed.kept(),
        "restored on open"
    );
    assert!(
        next.ui_state.saved_pcb_layer_views.is_empty(),
        "the session view took it over"
    );
}

#[test]
fn an_unchanged_view_arms_no_write() {
    let mut app = launch();
    open(&mut app);
    assert!(
        app.ui_state.pcb_layer_view_sync.flush().is_none(),
        "opening a board writes nothing"
    );
}
