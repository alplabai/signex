//! The boards and the small drivers every board row shares.

use std::path::PathBuf;

use pcb_geom::ClickMode;
use signex_app::app::{EditMsg, FileMsg, Message, Signex, TabDocument};
use signex_app::dock::DockMessage;
use signex_app::keymap::AppCommandId;
use signex_app::menu_bar::MenuMessage;
use signex_app::panels::pcb_primitive_properties::{
    PcbPrimitiveFacts, PrimitiveField, PrimitiveTarget,
};
use signex_app::panels::{PanelMsg, PcbBoardMsg, PcbPrimitiveMsg};
use signex_app::pcb_canvas::PcbCanvasMsg;
use signex_renderer::pcb::PcbLayerView;
use signex_types::coord::Unit;
use signex_types::format::SnxPcb;
use signex_types::pcb::{LayerDef, PcbBoard, Point};
use uuid::Uuid;

use crate::settle_support::settle;

pub(super) const FOOTPRINT: Uuid = Uuid::from_u128(0xB01);
pub(super) const PAD_TH: Uuid = Uuid::from_u128(0xB02);
pub(super) const PAD_SMD: Uuid = Uuid::from_u128(0xB03);
pub(super) const TRACK: Uuid = Uuid::from_u128(0xB04);
pub(super) const VIA: Uuid = Uuid::from_u128(0xB05);
pub(super) const ARC: Uuid = Uuid::from_u128(0xB06);
pub(super) const POUR: Uuid = Uuid::from_u128(0xB07);
pub(super) const TEXT: Uuid = Uuid::from_u128(0xB08);
pub(super) const DIM: Uuid = Uuid::from_u128(0xB09);
pub(super) const LINE: Uuid = Uuid::from_u128(0xB0A);

fn square(x: f64, y: f64, side: f64) -> serde_json::Value {
    serde_json::json!([
        {"x": x, "y": y}, {"x": x + side, "y": y},
        {"x": x + side, "y": y + side}, {"x": x, "y": y + side}
    ])
}

/// On a 1 mm grid: U1 at (60, 10) with a through-hole pad at (58, 10) and a
/// surface pad at (62, 10); a GND track along y = 0 to x = 10; a GND via at
/// (20, 0); an arc round (20, 20); a GND pour square at (50, 50); a string at
/// (5, 30); a dimension along y = 10 from x = 30 to 40; a silkscreen line
/// along y = 40. No two overlap, so one click picks one item.
pub(super) fn all_kinds() -> PcbBoard {
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(0xB00),
        "setup": {"grid_size": 1.0, "trace_width": 0.25},
        "nets": [{"number": 1, "name": "GND"}, {"number": 2, "name": "VCC"}],
        "footprints": [{
            "uuid": FOOTPRINT, "reference": "U1", "value": "MCU",
            "footprint_id": "QFN-48", "position": {"x": 60.0, "y": 10.0},
            "layer": "TopCopper",
            "pads": [
                {"uuid": PAD_TH, "number": "1", "pad_type": "thru", "shape": "circle",
                 "position": {"x": -2.0, "y": 0.0}, "size": {"x": 1.2, "y": 1.2},
                 "drill": {"diameter": 0.5, "shape": ""},
                 "layers": ["TopCopper", "BottomCopper"], "net": null},
                {"uuid": PAD_SMD, "number": "2", "pad_type": "smd", "shape": "rect",
                 "position": {"x": 2.0, "y": 0.0}, "size": {"x": 1.0, "y": 0.6},
                 "drill": null, "layers": ["TopCopper"], "net": null}
            ]
        }],
        "segments": [{"uuid": TRACK, "start": {"x": 0.0, "y": 0.0},
                      "end": {"x": 10.0, "y": 0.0}, "width": 0.25,
                      "layer": "TopCopper", "net": 1}],
        "vias": [{"uuid": VIA, "position": {"x": 20.0, "y": 0.0}, "diameter": 0.6,
                  "drill": 0.3, "layers": ["TopCopper", "BottomCopper"], "net": 1}],
        "zones": [{"uuid": POUR, "net": 1, "net_name": "GND", "layer": "TopCopper",
                   "fill_type": "pour", "outline": square(50.0, 50.0, 10.0)}],
        "texts": [{"uuid": TEXT, "text": "A", "position": {"x": 5.0, "y": 30.0},
                   "layer": "TopSilkscreen", "font_size": 1.0, "rotation": 0.0}],
        "graphics": [{"uuid": LINE, "graphic_type": "line", "layer": "TopSilkscreen",
                      "width": 0.2, "start": {"x": 0.0, "y": 40.0},
                      "end": {"x": 10.0, "y": 40.0}}],
        "free": {
            "arcs": [{"uuid": ARC, "start": {"x": 25.0, "y": 20.0},
                      "mid": {"x": 20.0, "y": 25.0}, "end": {"x": 15.0, "y": 20.0},
                      "width": 0.3, "layer": "TopCopper", "net": 0}],
            "dimensions": [{"uuid": DIM, "start": {"x": 30.0, "y": 10.0},
                            "end": {"x": 40.0, "y": 10.0}, "offset": 2.0,
                            "layer": "Dimensions", "line_width": 0.1,
                            "text_height": 1.0, "precision": 2}]
        }
    }))
    .expect("the audit board")
}

/// [`all_kinds`] with four copper layers named in its stack.
pub(super) fn four_layers() -> PcbBoard {
    let mut board = all_kinds();
    board.layers = ["TopCopper", "Inner1Copper", "Inner2Copper", "BottomCopper"]
        .iter()
        .enumerate()
        .map(|(id, name)| LayerDef {
            id: id as u8,
            name: (*name).to_string(),
            layer_type: "signal".to_string(),
            display_name: String::new(),
        })
        .collect();
    board
}

/// An app with `board` opened from its file, at its save point. Object and
/// axis snapping start off, so a test turns on exactly what it checks.
pub(super) struct Bench {
    pub app: Signex,
    pub path: PathBuf,
    pub main: iced::window::Id,
    _dir: tempfile::TempDir,
}

pub(super) fn open(board: PcbBoard) -> Bench {
    let _ = signex_app::diagnostics::init_logging();
    let dir = tempfile::TempDir::new().expect("tempdir");
    let path = dir.path().join("audit.snxpcb");
    let text = SnxPcb::new(board).write_string().expect("encode board");
    std::fs::write(&path, text).expect("write board");
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let main = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(main);
    app.ui_state.unit = Unit::Mm;
    app.ui_state.snap_enabled = true;
    app.ui_state.pcb_snap.objects.clear();
    app.ui_state.pcb_snap.to_axes = false;
    settle(&mut app, Message::File(FileMsg::Opened(Some(path.clone()))));
    Bench {
        app,
        path,
        main,
        _dir: dir,
    }
}

impl Bench {
    /// Send `message` and run every task it starts.
    pub fn send(&mut self, message: Message) {
        settle(&mut self.app, message);
    }

    /// Send `message` and drop its task: a preference write or a focus
    /// request, which must not reach the user's settings from a test.
    pub fn post(&mut self, message: Message) {
        let _ = self.app.update(message);
    }

    pub fn panel(&mut self, msg: PanelMsg) {
        self.send(Message::Dock(DockMessage::Panel(msg)));
    }

    pub fn board_panel(&mut self, msg: PcbBoardMsg) {
        self.panel(PanelMsg::PcbBoard(msg));
    }

    pub fn primitive(&mut self, msg: PcbPrimitiveMsg) {
        self.panel(PanelMsg::PcbPrimitive(msg));
    }

    pub fn command(&mut self, id: &str) {
        let id = AppCommandId::new(id).expect("a catalog command");
        self.send(Message::Menu(MenuMessage::Command(id)));
    }

    pub fn undo(&mut self) {
        self.send(Message::Edit(EditMsg::Undo));
    }

    pub fn redo(&mut self) {
        self.send(Message::Edit(EditMsg::Redo));
    }

    pub fn board(&self) -> PcbBoard {
        self.app
            .document_state
            .tabs
            .iter()
            .find(|tab| tab.path == self.path)
            .and_then(|tab| tab.cached_document.as_ref())
            .and_then(TabDocument::as_pcb)
            .expect("the board is open")
            .clone()
    }

    pub fn json(&self) -> String {
        serde_json::to_string(&self.board()).expect("the board as JSON")
    }

    pub fn dirty(&self) -> bool {
        self.app.document_state.is_document_dirty(&self.path)
    }

    pub fn layer_view(&self) -> PcbLayerView {
        self.app
            .ui_state
            .pcb_layer_views
            .get(&self.path)
            .cloned()
            .unwrap_or_default()
    }

    pub fn click(&mut self, x: f64, y: f64) {
        self.click_with(x, y, ClickMode::Replace);
    }

    pub fn click_with(&mut self, x: f64, y: f64, mode: ClickMode) {
        self.send(Message::PcbCanvas(PcbCanvasMsg::Clicked {
            at: Point::new(x, y),
            tolerance_mm: 0.05,
            mode,
        }));
    }

    pub fn facts(&self) -> PcbPrimitiveFacts {
        self.app
            .document_state
            .panel_ctx
            .pcb_board
            .as_ref()
            .and_then(|summary| summary.primitive.clone())
            .expect("a primitive form shows")
    }

    /// Click the item at `(x, y)`; the target its form edits.
    pub fn pick(&mut self, x: f64, y: f64) -> PrimitiveTarget {
        self.click(x, y);
        self.facts().common().target.clone()
    }

    /// Type `text` into a primitive field and press Enter.
    pub fn enter(&mut self, target: &PrimitiveTarget, field: PrimitiveField, text: &str) {
        self.primitive(PcbPrimitiveMsg::FieldTyped {
            target: target.clone(),
            field,
            text: text.to_string(),
        });
        self.primitive(PcbPrimitiveMsg::FieldSubmitted {
            target: target.clone(),
            field,
        });
    }

    /// `edit` changes the board in one undo step and marks it dirty; undo
    /// goes back to the save point and redo replays it. The board is left
    /// undone; the edited board is returned.
    pub fn one_step(&mut self, edit: impl FnOnce(&mut Self)) -> PcbBoard {
        assert!(!self.dirty(), "the board starts at its save point");
        let before = self.json();
        edit(self);
        let after = self.json();
        assert_ne!(before, after, "the edit changed the board");
        assert!(self.dirty(), "the edit marks the board dirty");
        let edited = self.board();
        self.undo();
        assert_eq!(self.json(), before, "one undo restores the board");
        assert!(!self.dirty(), "undo returns to the save point");
        self.redo();
        assert_eq!(self.json(), after, "redo replays the edit");
        assert!(self.dirty(), "redo marks the board dirty again");
        self.undo();
        assert_eq!(self.json(), before, "the edit was a single step");
        edited
    }

    /// `edit` changes neither the board nor its dirty mark.
    pub fn refused(&mut self, edit: impl FnOnce(&mut Self)) {
        let before = self.json();
        let was_dirty = self.dirty();
        edit(self);
        assert_eq!(self.json(), before, "the board is unchanged");
        assert_eq!(self.dirty(), was_dirty, "the dirty mark is unchanged");
    }

    /// `edit` changes what is shown only: the board stays as saved.
    pub fn view_only(&mut self, edit: impl FnOnce(&mut Self)) {
        assert!(!self.dirty(), "the board starts at its save point");
        self.refused(edit);
        assert!(!self.dirty(), "a view change never marks the board dirty");
    }
}

/// Two coordinates to the nanometre a board file stores.
pub(super) fn assert_at(at: Point, x: f64, y: f64) {
    assert!(
        (at.x - x).abs() < 1e-6 && (at.y - y).abs() < 1e-6,
        "expected ({x}, {y}), got ({}, {})",
        at.x,
        at.y
    );
}
