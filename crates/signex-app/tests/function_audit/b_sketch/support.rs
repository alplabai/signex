//! The rigs and small drivers every b_sketch row shares.

use std::path::PathBuf;

use signex_app::app::{
    EditMsg, FileMsg, FootprintDocument, Message, Signex, SymbolDocument, TabDocument, TabInfo,
    TabKind,
};
use signex_app::dock::DockMessage;
use signex_app::keymap::AppCommandId;
use signex_app::library::editor::footprint::{EditorMode, SketchTool};
use signex_app::library::messages::{
    FootprintEditorMsg, LibraryMessage, PrimitiveEdit, SymbolEditorMsg,
};
use signex_app::menu_bar::MenuMessage;
use signex_app::panels::PanelMsg;
use signex_library::{Footprint, FootprintFile, Symbol, SymbolFile};
use signex_sketch::SketchData;
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_types::format::SnxPcb;
use signex_types::pcb::{LayerDef, NetDef, PcbBoard};
use tempfile::TempDir;
use uuid::Uuid;

use crate::settle_support::settle;

pub(super) fn fresh_app() -> Signex {
    let _ = signex_app::diagnostics::init_logging();
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app
}

pub(super) fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

/// A footprint open in a tab, at its save point.
pub(super) struct FpRig {
    pub app: Signex,
    pub path: PathBuf,
    _dir: TempDir,
}

pub(super) fn open_footprint(footprint: Footprint) -> FpRig {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("audit.snxfpt");
    let mut editor = FootprintDocument::new(path.clone(), FootprintFile::from_footprint(footprint));
    editor.mark_saved();
    let mut app = fresh_app();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(TabInfo {
        title: "audit".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    FpRig {
        app,
        path,
        _dir: dir,
    }
}

/// A footprint with one pad at each of `at`, at its save point.
pub(super) fn pads_rig(at: &[(f64, f64)]) -> FpRig {
    let mut rig = open_footprint(Footprint::empty("pads"));
    for &(x_mm, y_mm) in at {
        rig.send(FootprintEditorMsg::AddPad { x_mm, y_mm });
    }
    rig.editor_mut().mark_saved();
    rig
}

/// `sketch` open in Sketch mode, solved, at its save point.
pub(super) fn sketch_rig(sketch: SketchData) -> FpRig {
    let mut footprint = Footprint::empty("sketch");
    footprint.sketch = Some(sketch);
    let mut rig = open_footprint(footprint);
    rig.send(FootprintEditorMsg::SetMode(EditorMode::Sketch));
    rig.editor_mut().mark_saved();
    rig
}

impl FpRig {
    pub fn send(&mut self, msg: FootprintEditorMsg) {
        let path = self.path.clone();
        settle(
            &mut self.app,
            Message::Library(LibraryMessage::PrimitiveEditorEvent {
                path,
                msg: PrimitiveEdit::Footprint(msg),
            }),
        );
    }

    pub fn panel(&mut self, msg: PanelMsg) {
        settle(&mut self.app, Message::Dock(DockMessage::Panel(msg)));
    }

    pub fn undo(&mut self) {
        settle(&mut self.app, Message::Edit(EditMsg::Undo));
    }

    pub fn editor(&self) -> &FootprintDocument {
        &self.app.document_state.footprint_editors[&self.path]
    }

    pub fn editor_mut(&mut self) -> &mut FootprintDocument {
        self.app
            .document_state
            .footprint_editors
            .get_mut(&self.path)
            .expect("the footprint is open")
    }

    pub fn sketch(&self) -> &SketchData {
        self.editor()
            .primitive()
            .sketch
            .as_ref()
            .expect("the footprint has a sketch")
    }

    pub fn dirty(&self) -> bool {
        self.editor().is_dirty()
    }

    pub fn steps(&self) -> usize {
        self.editor().history.undo_len()
    }

    pub fn positions(&self) -> Vec<(f64, f64)> {
        self.editor()
            .state
            .pads
            .iter()
            .map(|pad| pad.position_mm)
            .collect()
    }

    pub fn select_pads(&mut self, pads: Vec<usize>) {
        self.send(FootprintEditorMsg::SelectPads(pads));
    }

    pub fn tool_click(&mut self, x_mm: f64, y_mm: f64, snap_id: Option<SketchEntityId>) {
        self.send(FootprintEditorMsg::SketchToolClick {
            x_mm,
            y_mm,
            snap_id,
        });
    }

    pub fn arm(&mut self, tool: SketchTool) {
        self.send(FootprintEditorMsg::SketchSetTool(tool));
    }
}

/// A sketch under construction: one plane, Points and Lines by position.
pub(super) struct SketchBuilder {
    pub plane: PlaneId,
    pub data: SketchData,
}

impl SketchBuilder {
    pub fn new() -> Self {
        let plane = PlaneId::new();
        let data = SketchData {
            planes: vec![Plane {
                id: plane,
                kind: PlaneKind::BoardTop,
            }],
            ..SketchData::default()
        };
        Self { plane, data }
    }

    pub fn point(&mut self, x: f64, y: f64) -> SketchEntityId {
        let id = SketchEntityId::new();
        self.data
            .entities
            .push(Entity::new(id, self.plane, EntityKind::Point { x, y }));
        id
    }

    pub fn line(&mut self, start: SketchEntityId, end: SketchEntityId) -> SketchEntityId {
        let id = SketchEntityId::new();
        self.data
            .entities
            .push(Entity::new(id, self.plane, EntityKind::Line { start, end }));
        id
    }

    pub fn entity_mut(&mut self, id: SketchEntityId) -> &mut Entity {
        self.data
            .entities
            .iter_mut()
            .find(|entity| entity.id == id)
            .expect("the entity was added")
    }
}

/// A symbol open in a tab with one pin and one rectangle, at its save point.
pub(super) struct SymRig {
    pub app: Signex,
    pub path: PathBuf,
    _dir: TempDir,
}

pub(super) fn open_symbol() -> SymRig {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("audit.snxsym");
    let file = SymbolFile::from_symbol(Symbol::empty("audit"));
    let mut app = fresh_app();
    app.document_state
        .symbol_editors
        .insert(path.clone(), SymbolDocument::new(path.clone(), file));
    app.document_state.tabs.push(TabInfo {
        title: "audit".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::SymbolEditor(path.clone()),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    let mut rig = SymRig {
        app,
        path,
        _dir: dir,
    };
    rig.send(SymbolEditorMsg::AddPin { x: 2.54, y: 5.08 });
    rig.send(SymbolEditorMsg::AddRectangle {
        from_x: 0.0,
        from_y: 0.0,
        to_x: 2.54,
        to_y: 2.54,
    });
    rig.editor_mut().mark_saved();
    rig
}

impl SymRig {
    pub fn send(&mut self, msg: SymbolEditorMsg) {
        let path = self.path.clone();
        settle(
            &mut self.app,
            Message::Library(LibraryMessage::PrimitiveEditorEvent {
                path,
                msg: PrimitiveEdit::Symbol(msg),
            }),
        );
    }

    pub fn undo(&mut self) {
        settle(&mut self.app, Message::Edit(EditMsg::Undo));
    }

    pub fn editor(&self) -> &SymbolDocument {
        &self.app.document_state.symbol_editors[&self.path]
    }

    pub fn editor_mut(&mut self) -> &mut SymbolDocument {
        self.app
            .document_state
            .symbol_editors
            .get_mut(&self.path)
            .expect("the symbol is open")
    }

    pub fn symbol(&self) -> &Symbol {
        self.editor().primitive()
    }

    /// The symbol as text, to compare a whole state.
    pub fn snapshot(&self) -> String {
        format!("{:?}", self.symbol())
    }

    pub fn dirty(&self) -> bool {
        self.editor().is_dirty()
    }
}

/// A board opened from disk.
pub(super) struct BoardRig {
    pub app: Signex,
    pub path: PathBuf,
    _dir: TempDir,
}

fn layer(id: u8, name: &str) -> LayerDef {
    LayerDef {
        id,
        name: name.to_owned(),
        layer_type: "signal".to_owned(),
        display_name: String::new(),
    }
}

/// An empty two-layer board with a fixed id.
pub(super) fn empty_board() -> PcbBoard {
    PcbBoard {
        sketch: None,
        stackup: None,
        rooms: Vec::new(),
        options: Default::default(),
        uuid: Uuid::from_u128(0xB5E7C),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: Vec::new(),
        layers: vec![layer(0, "TopCopper"), layer(31, "BottomCopper")],
        setup: None,
        nets: vec![NetDef {
            number: 1,
            name: "GND".into(),
        }],
        footprints: Vec::new(),
        segments: Vec::new(),
        vias: Vec::new(),
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        free: Default::default(),
        unknown: Default::default(),
    }
}

pub(super) fn open_board(board: PcbBoard) -> BoardRig {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("audit.snxpcb");
    let text = SnxPcb::new(board).write_string().expect("encode board");
    std::fs::write(&path, text).expect("write board");
    let mut app = fresh_app();
    settle(&mut app, Message::File(FileMsg::Opened(Some(path.clone()))));
    BoardRig {
        app,
        path,
        _dir: dir,
    }
}

impl BoardRig {
    pub fn send(&mut self, message: Message) {
        settle(&mut self.app, message);
    }

    pub fn edit(&mut self, msg: FootprintEditorMsg) {
        let path = self.path.clone();
        self.send(Message::Library(LibraryMessage::PrimitiveEditorEvent {
            path,
            msg: PrimitiveEdit::Footprint(msg),
        }));
    }

    pub fn panel(&mut self, msg: PanelMsg) {
        self.send(Message::Dock(DockMessage::Panel(msg)));
    }

    pub fn undo(&mut self) {
        self.send(Message::Edit(EditMsg::Undo));
    }

    /// Run `board_sketch` as its menu row does: enter, or leave when on.
    pub fn toggle_sketch(&mut self) {
        let id = AppCommandId::new("board_sketch").expect("a catalog command");
        self.send(Message::Menu(MenuMessage::Command(id)));
    }

    pub fn in_sketch(&self) -> bool {
        self.app.interaction_state.board_sketch.is_some()
    }

    pub fn board(&self) -> &PcbBoard {
        let docs = &self.app.document_state;
        docs.tabs[docs.active_tab]
            .cached_document
            .as_ref()
            .and_then(TabDocument::as_pcb)
            .expect("a board is open")
    }

    pub fn snapshot(&self) -> String {
        serde_json::to_string(self.board()).expect("the board serialises")
    }

    pub fn dirty(&self) -> bool {
        self.app.document_state.is_document_dirty(&self.path)
    }

    pub fn working(&self) -> SketchData {
        self.app
            .interaction_state
            .board_sketch
            .as_ref()
            .expect("sketch mode")
            .working_sketch()
    }

    pub fn click(&mut self, x_mm: f64, y_mm: f64) {
        self.edit(FootprintEditorMsg::SketchToolClick {
            x_mm,
            y_mm,
            snap_id: None,
        });
    }

    fn ids_of(&self, kind: fn(&EntityKind) -> bool) -> Vec<SketchEntityId> {
        self.working()
            .entities
            .iter()
            .filter(|entity| kind(&entity.kind))
            .map(|entity| entity.id)
            .collect()
    }

    pub fn lines(&self) -> Vec<SketchEntityId> {
        self.ids_of(|kind| matches!(kind, EntityKind::Line { .. }))
    }

    pub fn circles(&self) -> Vec<SketchEntityId> {
        self.ids_of(|kind| matches!(kind, EntityKind::Circle { .. }))
    }

    /// Draw a rectangle with the sketch tool; its four lines.
    pub fn rectangle(&mut self, from: (f64, f64), to: (f64, f64)) -> Vec<SketchEntityId> {
        let before = self.lines();
        self.edit(FootprintEditorMsg::SketchSetTool(SketchTool::Rectangle));
        self.click(from.0, from.1);
        self.click(to.0, to.1);
        self.edit(FootprintEditorMsg::ToolEscape);
        self.lines()
            .into_iter()
            .filter(|id| !before.contains(id))
            .collect()
    }

    /// Draw a 4 mm circle centred on `(x, y)`; its id.
    pub fn circle(&mut self, x: f64, y: f64) -> SketchEntityId {
        let before = self.circles();
        self.edit(FootprintEditorMsg::SketchSetTool(SketchTool::Circle));
        self.click(x, y);
        self.click(x + 2.0, y);
        self.edit(FootprintEditorMsg::ToolEscape);
        self.circles()
            .into_iter()
            .find(|id| !before.contains(id))
            .expect("the circle was drawn")
    }
}
