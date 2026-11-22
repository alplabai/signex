//! The sheet and the small drivers every g1 row shares.

use signex_app::app::{EditMsg, Message, Signex, TabInfo, TabKind};
use signex_app::canvas::{BoxDirection, CanvasEvent};
use signex_app::dock::DockMessage;
use signex_app::panels::{PanelMsg, PlacedEditMsg, PlacedField};
use signex_types::schematic::{
    HAlign, Label, LabelType, LibSymbol, Point, SchematicSheet, SelectedItem, Symbol, TextProp,
    Wire,
};
use std::path::PathBuf;
use uuid::Uuid;

/// An app with one open sheet. The sheet's path is in a temp folder that
/// lives as long as the bench; nothing is written there unless a test does.
pub(super) struct Bench {
    pub app: Signex,
    pub path: PathBuf,
    _dir: tempfile::TempDir,
}

pub(super) fn empty_sheet() -> SchematicSheet {
    SchematicSheet {
        uuid: Uuid::now_v7(),
        paper_size: "A4".to_string(),
        ..Default::default()
    }
}

pub(super) fn bench(mut sheet: SchematicSheet) -> Bench {
    let body: LibSymbol = serde_json::from_str(r#"{"id":"lib:r"}"#).expect("a library body");
    sheet.lib_symbols.insert(body.id.clone(), body);
    bench_as_is(sheet)
}

/// A bench on `sheet` exactly as given, for a sheet read from a file.
pub(super) fn bench_as_is(sheet: SchematicSheet) -> Bench {
    let _ = signex_app::diagnostics::init_logging();
    let dir = tempfile::TempDir::new().expect("tempdir");
    let path = dir.path().join("audit.snxsch");
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(sheet.clone()).expect("engine"),
    );
    app.document_state.tabs.push(TabInfo {
        title: "audit".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path.clone());
    app.interaction_state
        .active_canvas_mut()
        .set_render_cache(Some(
            signex_app::schematic_runtime::SchematicRenderCache::from_sheet(&sheet),
        ));
    Bench {
        app,
        path,
        _dir: dir,
    }
}

impl Bench {
    pub fn panel(&mut self, msg: PanelMsg) {
        let _ = self.app.update(Message::Dock(DockMessage::Panel(msg)));
    }

    pub fn undo(&mut self) {
        let _ = self.app.update(Message::Edit(EditMsg::Undo));
    }

    pub fn redo(&mut self) {
        let _ = self.app.update(Message::Edit(EditMsg::Redo));
    }

    pub fn sheet(&self) -> &SchematicSheet {
        self.app
            .document_state
            .active_engine()
            .expect("an open sheet")
            .document()
    }

    pub fn dirty(&self) -> bool {
        self.app.document_state.is_document_dirty(&self.path)
    }

    /// Type `text` into a placed object's box, without Enter.
    pub fn type_in(&mut self, item: SelectedItem, field: PlacedField, text: &str) {
        self.panel(PanelMsg::PlacedEdit(PlacedEditMsg::Typed {
            item,
            field,
            text: text.to_string(),
        }));
    }

    /// Type `text` into a placed object's box and press Enter.
    pub fn enter(&mut self, item: SelectedItem, field: PlacedField, text: &str) {
        self.type_in(item, field.clone(), text);
        self.panel(PanelMsg::PlacedEdit(PlacedEditMsg::Submitted {
            item,
            field,
        }));
    }

    /// Whether a box of a placed object still holds a draft.
    pub fn has_draft(&self) -> bool {
        self.app.document_state.panel_ctx.placed.draft.is_some()
    }

    /// Select what a box over the whole sheet touches.
    pub fn select_everything(&mut self) {
        let _ = self
            .app
            .update(Message::CanvasEvent(CanvasEvent::BoxSelect {
                x1: -50.0,
                y1: -50.0,
                x2: 450.0,
                y2: 450.0,
                direction: BoxDirection::RightToLeft,
            }));
    }
}

fn field_text(at: Point) -> TextProp {
    TextProp {
        position: at,
        rotation: 0.0,
        font_size: 1.27,
        font_family: None,
        justify_h: HAlign::Left,
        justify_v: Default::default(),
        hidden: false,
    }
}

/// A part at `at` with its designator above and its value below.
pub(super) fn part(reference: &str, value: &str, at: (f64, f64)) -> Symbol {
    Symbol {
        uuid: Uuid::now_v7(),
        lib_id: "lib:r".into(),
        reference: reference.into(),
        value: value.into(),
        footprint: "R_0603".into(),
        position: Point::new(at.0, at.1),
        unit: 1,
        in_bom: true,
        on_board: true,
        ref_text: Some(field_text(Point::new(at.0, at.1 - 5.0))),
        val_text: Some(field_text(Point::new(at.0, at.1 + 5.0))),
        ..Default::default()
    }
}

pub(super) fn label(text: &str, at: (f64, f64)) -> Label {
    Label::new(text, Point::new(at.0, at.1), LabelType::Net)
}

pub(super) fn wire(a: (f64, f64), b: (f64, f64), width: f64) -> Wire {
    Wire {
        uuid: Uuid::now_v7(),
        start: Point::new(a.0, a.1),
        end: Point::new(b.0, b.1),
        stroke_width: width,
        ..Default::default()
    }
}
