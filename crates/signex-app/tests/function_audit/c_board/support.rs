//! The documents and the small drivers every wave C board row shares.

use std::path::{Path, PathBuf};

use signex_app::app::{
    BoardOutputDialog, BoardPrintState, BoardReportsState, EditMsg, FabOutputsState, FileMsg,
    Message, PreviewState, Signex, TabDocument, TabInfo, TabKind,
};
use signex_app::io_ports::{HeadlessAnswers, IoPorts};
use signex_app::keymap::AppCommandId;
use signex_app::menu_bar::MenuMessage;
use signex_types::coord::Unit;
use signex_types::format::SnxPcb;
use signex_types::pcb::PcbBoard;
use signex_types::schematic::{LibSymbol, Point, SchematicSheet, Symbol};
use uuid::Uuid;

use crate::settle_support::settle;

pub(super) const R1: Uuid = Uuid::from_u128(0xC01);
pub(super) const C1: Uuid = Uuid::from_u128(0xC02);

fn smd_pad(id: u128, number: &str, x: f64, net: u32, name: &str) -> serde_json::Value {
    serde_json::json!({
        "uuid": Uuid::from_u128(id), "number": number, "pad_type": "smd", "shape": "rect",
        "position": {"x": x, "y": 0.0}, "size": {"x": 1.0, "y": 1.2}, "drill": null,
        "layers": ["TopCopper", "TopSolderMask", "TopPaste"],
        "net": {"number": net, "name": name}
    })
}

/// A 40 x 30 mm board: R1 (10k) at (5, 5) and C1 (100n) at (15, 5), each
/// with a GND pad and a VCC pad; a 10 mm GND track and a via, a 30 mm VCC
/// track, a net SPARE with no copper, and a silkscreen line.
pub(super) fn audit_board() -> PcbBoard {
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(0xC00),
        "thickness": 1.6,
        "outline": [{"x": 0.0, "y": 0.0}, {"x": 40.0, "y": 0.0},
                    {"x": 40.0, "y": 30.0}, {"x": 0.0, "y": 30.0}],
        "setup": {"grid_size": 1.0, "trace_width": 0.25},
        "nets": [{"number": 1, "name": "GND"}, {"number": 2, "name": "VCC"},
                 {"number": 3, "name": "SPARE"}],
        "footprints": [
            {"uuid": R1, "reference": "R1", "value": "10k", "footprint_id": "R_0603",
             "position": {"x": 5.0, "y": 5.0}, "layer": "TopCopper",
             "pads": [smd_pad(0xC11, "1", -1.0, 1, "GND"), smd_pad(0xC12, "2", 1.0, 2, "VCC")]},
            {"uuid": C1, "reference": "C1", "value": "100n", "footprint_id": "C_0603",
             "position": {"x": 15.0, "y": 5.0}, "layer": "TopCopper",
             "pads": [smd_pad(0xC13, "1", -1.0, 1, "GND"), smd_pad(0xC14, "2", 1.0, 2, "VCC")]}
        ],
        "segments": [
            {"uuid": Uuid::from_u128(0xC21), "start": {"x": 1.0, "y": 12.0},
             "end": {"x": 11.0, "y": 12.0}, "width": 0.25, "layer": "TopCopper", "net": 1},
            {"uuid": Uuid::from_u128(0xC22), "start": {"x": 2.0, "y": 20.0},
             "end": {"x": 32.0, "y": 20.0}, "width": 0.25, "layer": "TopCopper", "net": 2}
        ],
        "vias": [{"uuid": Uuid::from_u128(0xC23), "position": {"x": 20.0, "y": 12.0},
                  "diameter": 0.6, "drill": 0.3,
                  "layers": ["TopCopper", "BottomCopper"], "net": 1}],
        "graphics": [{"uuid": Uuid::from_u128(0xC24), "graphic_type": "line",
                      "layer": "TopSilkscreen", "width": 0.2,
                      "start": {"x": 2.0, "y": 25.0}, "end": {"x": 20.0, "y": 25.0}}]
    }))
    .expect("the audit board")
}

fn part(reference: &str, value: &str, footprint: &str, x: f64) -> Symbol {
    Symbol {
        uuid: Uuid::now_v7(),
        lib_id: "lib:r".into(),
        reference: reference.into(),
        value: value.into(),
        footprint: footprint.into(),
        position: Point::new(x, 50.0),
        unit: 1,
        in_bom: true,
        on_board: true,
        ..Default::default()
    }
}

/// R1 and R2 (10k, one BOM line), C1 (100n) and D1, which is not fitted.
pub(super) fn audit_sheet() -> SchematicSheet {
    let mut led = part("D1", "LED", "LED_0603", 130.0);
    led.dnp = true;
    let mut sheet = SchematicSheet {
        uuid: Uuid::now_v7(),
        paper_size: "A4".to_string(),
        symbols: vec![
            part("R1", "10k", "R_0603", 40.0),
            part("R2", "10k", "R_0603", 70.0),
            part("C1", "100n", "C_0603", 100.0),
            led,
        ],
        ..Default::default()
    };
    let body: LibSymbol = serde_json::from_str(r#"{"id":"lib:r"}"#).expect("a library body");
    sheet.lib_symbols.insert(body.id.clone(), body);
    sheet
}

/// An app with one document open at its save point, its file dialogs
/// answered from `answers`.
pub(super) struct Bench {
    pub app: Signex,
    pub path: PathBuf,
    pub answers: HeadlessAnswers,
    pub dir: tempfile::TempDir,
}

fn headless_app() -> (Signex, HeadlessAnswers) {
    let _ = signex_app::diagnostics::init_logging();
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.unit = Unit::Mm;
    let answers = HeadlessAnswers::default();
    app.ui_state.io = IoPorts::headless(answers.clone());
    (app, answers)
}

/// `board` opened from its file `audit.snxpcb`.
pub(super) fn open_board(board: PcbBoard) -> Bench {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let path = dir.path().join("audit.snxpcb");
    let text = SnxPcb::new(board).write_string().expect("encode board");
    std::fs::write(&path, text).expect("write board");
    let (mut app, answers) = headless_app();
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    settle(&mut app, Message::File(FileMsg::Opened(Some(path.clone()))));
    Bench {
        app,
        path,
        answers,
        dir,
    }
}

/// [`audit_sheet`] open at its save point.
pub(super) fn open_sheet() -> Bench {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let path = dir.path().join("audit.snxsch");
    let (mut app, answers) = headless_app();
    let mut engine = signex_engine::Engine::new(audit_sheet()).expect("engine");
    engine.mark_saved();
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "audit".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path.clone());
    Bench {
        app,
        path,
        answers,
        dir,
    }
}

impl Bench {
    /// Send `message` and run every task it starts.
    pub fn send(&mut self, message: Message) {
        settle(&mut self.app, message);
    }

    pub fn command(&mut self, id: &str) {
        let id = AppCommandId::new(id).expect("a catalog command");
        self.send(Message::Menu(MenuMessage::Command(id)));
    }

    pub fn undo(&mut self) {
        self.send(Message::Edit(EditMsg::Undo));
    }

    /// The next file dialog answers `path`.
    pub fn answer(&self, path: &Path) {
        self.answers
            .replace_all([("path".to_string(), path.display().to_string())]);
    }

    /// The next file dialog is cancelled.
    pub fn cancel_dialog(&self) {
        self.answers
            .replace_all(std::iter::empty::<(String, String)>());
    }

    pub fn out(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
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

    pub fn can_undo_board(&self) -> bool {
        self.app
            .document_state
            .tabs
            .iter()
            .find(|tab| tab.path == self.path)
            .and_then(|tab| tab.cached_document.as_ref())
            .and_then(TabDocument::as_pcb_engine)
            .expect("a board engine")
            .can_undo()
    }

    /// The open document as text, to compare before and after.
    pub fn doc_text(&self) -> String {
        match self.app.document_state.engines.get(&self.path) {
            Some(engine) => serde_json::to_string(engine.document()).expect("the sheet as JSON"),
            None => serde_json::to_string(&self.board()).expect("the board as JSON"),
        }
    }

    pub fn dirty(&self) -> bool {
        self.app.document_state.is_document_dirty(&self.path)
    }

    /// `edit` changes neither the document nor its dirty mark.
    pub fn leaves_document(&mut self, edit: impl FnOnce(&mut Self)) {
        let before = self.doc_text();
        let was_dirty = self.dirty();
        edit(self);
        assert_eq!(self.doc_text(), before, "the document is unchanged");
        assert_eq!(self.dirty(), was_dirty, "the dirty mark is unchanged");
    }

    pub fn preview(&self) -> &PreviewState {
        self.app
            .document_state
            .preview
            .as_ref()
            .expect("Export PDF is open")
    }

    pub fn fab(&self) -> &FabOutputsState {
        self.app
            .ui_state
            .fab_outputs
            .as_deref()
            .expect("Fabrication Outputs is open")
    }

    pub fn print(&self) -> &BoardPrintState {
        match self.app.ui_state.board_output.dialog.as_deref() {
            Some(BoardOutputDialog::Print(state)) => state,
            _ => panic!("Board Print / PDF is open"),
        }
    }

    pub fn reports(&self) -> &BoardReportsState {
        match self.app.ui_state.board_output.dialog.as_deref() {
            Some(BoardOutputDialog::Reports(state)) => state,
            _ => panic!("Board Reports is open"),
        }
    }

    pub fn has_notice(&self) -> bool {
        self.app.document_state.error_notice.is_some()
    }
}
