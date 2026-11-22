//! Function audit, wave A batch g7b: mockup rows m65 to m74.
//!
//! Each test drives a row's controls through `Signex::update` and checks what
//! they change in the document, the dialog state and the dirty mark. Tests
//! state the behaviour the row or a dated decision asks for, also where the
//! code is suspected to differ.

use std::path::{Path, PathBuf};

use crate::fixture;
use crate::settle_support::settle;
use signex_app::app::contracts::Channel;
use signex_app::app::{
    EditMsg, FileMsg, FootprintDocument, InputMsg, Message, MoveSelectionMsg, NetColorMsg,
    OverlayMsg, ParameterManagerMsg as Pm, PasteIntent, PasteRequest, SelectionFilterMsg, Signex,
    TabInfo, TabKind, TextEditMsg, TextEditTarget, Tool,
};
use signex_app::canvas::CanvasEvent;
use signex_app::dock::DockMessage;
use signex_app::find_replace::{FindReplaceMsg, FindScope};
use signex_app::keymap::AppCommandId;
use signex_app::library::editor::footprint::state::SelectionFilterKind;
use signex_app::menu_bar::MenuMessage;
use signex_app::panels::PanelMsg;
use signex_app::paste_array::PasteArrayMsg;
use signex_app::paste_prefs::PasteDesignators;
use signex_app::toolbar::ToolMessage;
use signex_engine::SimilarProperty;
use signex_types::coord::Unit;
use signex_types::schematic::{
    Label, LabelType, LibSymbol, Point, SchematicSheet, SelectedItem, SelectedKind, Symbol, Wire,
    WireWidthPreset,
};
use signex_types::theme::{Color, ThemeId};
use uuid::Uuid;

#[path = "g7b/edit_find.rs"]
mod edit_find;
#[path = "g7b/nets_filter_pause.rs"]
mod nets_filter_pause;
#[path = "g7b/paste_params.rs"]
mod paste_params;

const SHEET: &str = "g7b-audit.snxsch";
const RED: Color = Color::new(0xE0, 0x54, 0x54, 255);
const TEAL: (u8, u8, u8) = (0x14, 0xB8, 0xA6);

fn part(reference: &str, value: &str, lib: &str, x: f64) -> Symbol {
    Symbol {
        uuid: Uuid::now_v7(),
        lib_id: lib.to_owned(),
        reference: reference.to_owned(),
        value: value.to_owned(),
        footprint: "R_0603".to_owned(),
        position: Point::new(x, 40.0),
        unit: 1,
        ..Default::default()
    }
}

fn wire(from: (f64, f64), to: (f64, f64)) -> Wire {
    Wire {
        uuid: Uuid::now_v7(),
        start: Point::new(from.0, from.1),
        end: Point::new(to.0, to.1),
        ..Default::default()
    }
}

fn net_label(text: &str, at: (f64, f64)) -> Label {
    Label::new(text.to_owned(), Point::new(at.0, at.1), LabelType::Net)
}

/// R1, R2 and C1 at 100k, R3 at 22k; net VOUT on two wires, VOUT_3V3 on one.
fn sheet() -> SchematicSheet {
    let mut r1 = part("R1", "100k", "lib:r", 20.0);
    r1.fields.insert("Tolerance".into(), "1%".into());
    let mut r2 = part("R2", "100k", "lib:r", 60.0);
    r2.fields.insert("Tolerance".into(), "5%".into());
    let mut sheet = SchematicSheet {
        uuid: Uuid::now_v7(),
        paper_size: "A4".to_owned(),
        symbols: vec![
            r1,
            r2,
            part("R3", "22k", "lib:r", 100.0),
            part("C1", "100k", "lib:c", 140.0),
        ],
        wires: vec![
            wire((20.0, 100.0), (40.0, 100.0)),
            wire((80.0, 100.0), (100.0, 100.0)),
            wire((20.0, 120.0), (40.0, 120.0)),
        ],
        labels: vec![
            net_label("VOUT", (20.0, 100.0)),
            net_label("VOUT_3V3", (20.0, 120.0)),
            net_label("VOUT", (80.0, 100.0)),
        ],
        ..Default::default()
    };
    for lib in ["lib:r", "lib:c"] {
        let body: LibSymbol =
            serde_json::from_str(&format!(r#"{{"id":"{lib}"}}"#)).expect("library body");
        sheet.lib_symbols.insert(body.id.clone(), body);
    }
    sheet
}

fn app_with(sheet: SchematicSheet) -> Signex {
    let path = PathBuf::from(SHEET);
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.unit = Unit::Mm;
    app.ui_state.paste_designators = PasteDesignators::Keep;
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(sheet).expect("engine"),
    );
    app.document_state.tabs.push(TabInfo {
        title: "g7b-audit".to_owned(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(path);
    app
}

fn app() -> Signex {
    app_with(sheet())
}

fn read_only_app() -> Signex {
    let mut sheet = sheet();
    sheet.unknown.newer_minor_source = Some("snxsch/1.7".into());
    app_with(sheet)
}

fn doc(app: &Signex) -> &SchematicSheet {
    app.document_state
        .active_engine()
        .expect("active sheet")
        .document()
}

fn symbol<'a>(app: &'a Signex, reference: &str) -> &'a Symbol {
    doc(app)
        .symbols
        .iter()
        .find(|s| s.reference == reference)
        .expect("part on the sheet")
}

fn label_texts(app: &Signex) -> Vec<String> {
    doc(app).labels.iter().map(|l| l.text.clone()).collect()
}

fn is_dirty(app: &Signex) -> bool {
    app.document_state.is_document_dirty(Path::new(SHEET))
}

fn send(app: &mut Signex, message: Message) {
    let _ = app.update(message);
}

fn undo(app: &mut Signex) {
    send(app, Message::Edit(EditMsg::Undo));
}

fn escape(app: &mut Signex) {
    send(app, Message::Input(InputMsg::EscapePressed { window: None }));
}

fn select_parts(app: &mut Signex, references: &[&str]) {
    let items: Vec<SelectedItem> = references
        .iter()
        .map(|r| SelectedItem::new(symbol(app, r).uuid, SelectedKind::Symbol))
        .collect();
    app.interaction_state.active_canvas_mut().selected = items;
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

fn at(app: &Signex, reference: &str) -> (f64, f64) {
    let p = symbol(app, reference).position;
    (p.x, p.y)
}

fn mv(app: &mut Signex, message: MoveSelectionMsg) {
    send(app, Message::MoveSelection(message));
}

fn open_move(app: &mut Signex, dx: &str, dy: &str) {
    select_parts(app, &["R1", "R2"]);
    mv(app, MoveSelectionMsg::Open);
    mv(app, MoveSelectionMsg::DxChanged(dx.into()));
    mv(app, MoveSelectionMsg::DyChanged(dy.into()));
}

fn find(app: &mut Signex, message: FindReplaceMsg) {
    send(app, Message::FindReplaceMsg(message));
}

fn open_replace(app: &mut Signex, query: &str, replacement: &str) {
    send(app, Message::Overlay(OverlayMsg::OpenReplace));
    find(app, FindReplaceMsg::QueryChanged(query.into()));
    find(app, FindReplaceMsg::ReplacementChanged(replacement.into()));
}

fn pm(app: &mut Signex, message: Pm) {
    send(app, Message::ParameterManager(message));
}

fn edit_cell(app: &mut Signex, reference: &str, key: &str, value: &str) {
    let symbol_uuid = symbol(app, reference).uuid;
    pm(
        app,
        Pm::Edit {
            symbol_uuid,
            key: key.into(),
            value: value.into(),
        },
    );
}

fn field<'a>(app: &'a Signex, reference: &str, key: &str) -> Option<&'a str> {
    symbol(app, reference).fields.get(key).map(String::as_str)
}

fn net(app: &mut Signex, message: NetColorMsg) {
    send(app, Message::NetColor(message));
}

fn set_net(app: &mut Signex, name: &str, color: Option<Color>) {
    net(
        app,
        NetColorMsg::Set {
            net: name.into(),
            color,
        },
    );
}
