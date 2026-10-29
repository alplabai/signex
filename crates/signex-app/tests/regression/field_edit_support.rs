//! A sheet with parts, labels and wires, drawn and selectable, for the
//! tests of in-place and multi-selection field edits.

use signex_app::app::{Message, Signex, TabInfo, TabKind};
use signex_app::canvas::{BoxDirection, CanvasEvent};
use signex_types::schematic::{
    HAlign, Label, LabelType, LibSymbol, Point, SchematicSheet, Symbol, TextProp, Wire,
};
use std::path::PathBuf;
use uuid::Uuid;

pub(crate) const SHEET: &str = "field-edit.snxsch";

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

/// A part at `at` with its designator 5 mm above and its value 5 mm below.
pub(crate) fn part(reference: &str, value: &str, at: (f64, f64)) -> Symbol {
    Symbol {
        uuid: Uuid::new_v4(),
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

pub(crate) fn label(text: &str, at: (f64, f64)) -> Label {
    Label {
        uuid: Uuid::new_v4(),
        text: text.into(),
        position: Point::new(at.0, at.1),
        rotation: 0.0,
        label_type: LabelType::Net,
        shape: String::new(),
        font_size: 1.27,
        font_family: None,
        justify: HAlign::Left,
        justify_v: Default::default(),
        unknown_columns: Default::default(),
        locked: false,
    }
}

pub(crate) fn wire(a: (f64, f64), b: (f64, f64), width: f64) -> Wire {
    Wire {
        unknown_columns: Default::default(),
        uuid: Uuid::new_v4(),
        start: Point::new(a.0, a.1),
        end: Point::new(b.0, b.1),
        stroke_width: width,
        locked: false,
    }
}

/// An app whose active sheet is `sheet`, rendered so hit-testing works.
pub(crate) fn app_with(mut sheet: SchematicSheet) -> Signex {
    let body: LibSymbol = serde_json::from_str(r#"{"id":"lib:r"}"#).unwrap();
    sheet.lib_symbols.insert(body.id.clone(), body);
    let _ = signex_app::diagnostics::init_logging();
    let path = PathBuf::from(SHEET);
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(sheet.clone()).expect("engine"),
    );
    app.document_state.tabs.push(TabInfo {
        title: "field-edit".to_string(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    app.interaction_state
        .active_canvas_mut()
        .set_render_cache(Some(
            signex_app::schematic_runtime::SchematicRenderCache::from_sheet(&sheet),
        ));
    app
}

pub(crate) fn empty_sheet() -> SchematicSheet {
    SchematicSheet {
        uuid: Uuid::new_v4(),
        paper_size: "A4".to_string(),
        ..Default::default()
    }
}

pub(crate) fn doc(app: &Signex) -> &SchematicSheet {
    app.document_state
        .active_engine()
        .expect("engine")
        .document()
}

/// Select everything the box 0..200 touches.
pub(crate) fn select_everything(app: &mut Signex) {
    let _ = app.update(Message::CanvasEvent(CanvasEvent::BoxSelect {
        x1: -50.0,
        y1: -50.0,
        x2: 250.0,
        y2: 250.0,
        direction: BoxDirection::RightToLeft,
    }));
}

/// A Messages-panel line containing every needle.
pub(crate) fn logged(needles: &[&str]) -> bool {
    signex_app::diagnostics::recent_entries()
        .iter()
        .any(|e| needles.iter().all(|n| e.message.contains(n)))
}
