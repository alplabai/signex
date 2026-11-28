//! mockup row 192, item D1 (decision 9, Caner 2026-11-20) — editing an attribute of a
//! patterned pad asks "This pad / All pads" every time. This pad stores one
//! per-instance override and only that pad changes, through a save and a
//! reload; All pads changes every instance; Cancel changes nothing.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use signex_app::app::{FileMsg, FootprintDocument, Message, Signex, TabInfo, TabKind};
use signex_app::dock::DockMessage;
use signex_app::library::editor::footprint::PadScope;
use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use signex_app::panels::PanelMsg;
use signex_library::{Footprint, FootprintFile};
use signex_sketch::SketchData;
use signex_sketch::array::{Array, ArrayId, ArrayKind, NumberingScheme};
use signex_sketch::attr::{PadAttr, PadKind, PadShape, PadSide};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::solver::Solver;
use tempfile::TempDir;

/// A 4-pad row: the source pad "1" at the origin, a linear pattern of it at
/// a 1 mm pitch, its pads baked into the footprint.
fn row() -> (Footprint, ArrayId) {
    let plane = PlaneId::new();
    let source = SketchEntityId::new();
    let mut point = Entity::new(source, plane, EntityKind::Point { x: 0.0, y: 0.0 });
    point.pad = Some(PadAttr {
        number: "1".into(),
        kind: PadKind::Smd,
        side: PadSide::Top,
        shape: PadShape::Rect,
        size_x_expr: "0.5mm".into(),
        size_y_expr: "0.5mm".into(),
        ..PadAttr::default()
    });
    let array = ArrayId::new();
    let sketch = SketchData {
        planes: vec![Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        }],
        entities: vec![point],
        arrays: vec![Array {
            id: array,
            kind: ArrayKind::Linear {
                source,
                count_expr: "4".into(),
                dx_expr: "1mm".into(),
                dy_expr: "0mm".into(),
                depopulation: None,
            },
            numbering: NumberingScheme::default(),
        }],
        ..SketchData::default()
    };
    let solved = Solver::default()
        .solve(&sketch, &HashMap::new())
        .expect("the row solves");
    let (mut pads, mut warnings) = (Vec::new(), Vec::new());
    signex_bake::bake_pads(&sketch, &solved, &HashMap::new(), &mut pads, &mut warnings).unwrap();
    signex_bake::bake_arrays(&sketch, &solved, &HashMap::new(), &mut pads, &mut warnings).unwrap();
    assert_eq!(pads.len(), 4, "{warnings:?}");
    let mut fp = Footprint::empty("row");
    fp.pads = pads;
    fp.sketch = Some(sketch);
    (fp, array)
}

fn open(stem: &str) -> (Signex, PathBuf, ArrayId) {
    let (fp, array) = row();
    let path = PathBuf::from(format!("{stem}.snxfpt"));
    let app = open_at(&path, FootprintFile::from_footprint(fp));
    (app, path, array)
}

fn open_at(path: &Path, file: FootprintFile) -> Signex {
    let editor = FootprintDocument::new(path.to_path_buf(), file);
    let (mut app, _initial_task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.to_path_buf(), editor);
    app.document_state.tabs.push(TabInfo {
        title: "row".into(),
        path: path.to_path_buf(),
        project_id: None,
        kind: TabKind::FootprintEditor(path.to_path_buf()),
    });
    app.document_state.active_tab = 0;
    app
}

fn editor<'a>(app: &'a Signex, path: &PathBuf) -> &'a FootprintDocument {
    app.document_state.footprint_editors.get(path).unwrap()
}

/// The index of the pad numbered `number`.
fn idx_of(app: &Signex, path: &PathBuf, number: &str) -> usize {
    editor(app, path)
        .state
        .pads
        .iter()
        .position(|p| p.number == number)
        .unwrap_or_else(|| panic!("no pad {number}"))
}

fn set_net(app: &mut Signex, path: &PathBuf, number: &str, net: &str) {
    let idx = idx_of(app, path, number);
    drop(app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::FpEditorSetSelectedPadNet {
            idx,
            value: net.into(),
        },
    ))));
}

fn answer(app: &mut Signex, path: &Path, scope: PadScope) {
    drop(
        app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
            path: path.to_path_buf(),
            msg: PrimitiveEdit::Footprint(FootprintEditorMsg::PadScopeAnswered(scope)),
        })),
    );
}

/// `(number, net)` of every baked pad, sorted by number.
fn nets(fp: &Footprint) -> Vec<(String, String)> {
    let mut out: Vec<_> = fp
        .pads
        .iter()
        .map(|p| (p.number.clone(), p.net.clone()))
        .collect();
    out.sort();
    out
}

fn expected(gnd: &[&str]) -> Vec<(String, String)> {
    ["1", "2", "3", "4"]
        .iter()
        .map(|n| {
            let net = if gnd.contains(n) { "GND" } else { "" };
            (n.to_string(), net.to_string())
        })
        .collect()
}

#[test]
fn editing_the_net_of_a_patterned_pad_asks_first() {
    let (mut app, path, array) = open("scope-ask");
    let undo_before = editor(&app, &path).history.undo_len();

    set_net(&mut app, &path, "3", "GND");

    let prompt = editor(&app, &path)
        .state
        .pad_scope
        .as_deref()
        .expect("the question is open");
    assert_eq!(prompt.instance, (array, 2, 0));
    assert_eq!(
        nets(editor(&app, &path).primitive()),
        expected(&[]),
        "nothing written yet"
    );
    assert_eq!(editor(&app, &path).history.undo_len(), undo_before);
    let idx = idx_of(&app, &path, "3");
    let state = &editor(&app, &path).state;
    assert_eq!(
        state.pads[idx].net, "",
        "the pad list keeps the pad as it was"
    );
    assert_eq!(
        state.shown_pad(idx).map(|p| p.net.as_str()),
        Some("GND"),
        "the panel shows the held value"
    );
}

/// A Save while the question waits writes the pad as it was, and a Cancel
/// after the save leaves the editor clean and equal to the file on disk.
#[test]
fn a_save_while_the_question_waits_never_writes_the_held_value() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("scope-save.snxfpt");
    let (fp, _) = row();
    let file = FootprintFile::from_footprint(fp);
    std::fs::write(&path, file.to_toml_string().expect("serialise")).expect("write");
    let read = FootprintFile::from_bytes(&std::fs::read(&path).unwrap()).expect("parse");
    let mut app = open_at(&path, read);
    drop(app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::FpEditorSetFootprintDescription("edited".into()),
    ))));
    set_net(&mut app, &path, "3", "GND");
    assert!(editor(&app, &path).state.pad_scope.is_some());

    crate::settle_support::settle(&mut app, Message::File(FileMsg::Save));

    let written = FootprintFile::from_bytes(&std::fs::read(&path).unwrap()).expect("reads back");
    let fp = &written.footprints[0];
    assert_eq!(fp.description, "edited", "the save ran");
    assert_eq!(nets(fp), expected(&[]), "the held net is not on disk");
    assert!(fp.sketch.as_ref().unwrap().pad_overrides.is_empty());

    answer(&mut app, &path, PadScope::Cancel);
    let doc = editor(&app, &path);
    assert!(doc.state.pad_scope.is_none());
    assert!(
        !doc.is_dirty(),
        "Cancel after the save leaves nothing unsaved"
    );
    assert_eq!(nets(doc.primitive()), expected(&[]));
}

#[test]
fn this_pad_writes_one_override_and_only_that_pad_changes_through_a_reload() {
    let (mut app, path, array) = open("scope-this");
    let undo_before = editor(&app, &path).history.undo_len();

    set_net(&mut app, &path, "3", "GND");
    answer(&mut app, &path, PadScope::ThisPad);

    let doc = editor(&app, &path);
    assert!(doc.state.pad_scope.is_none());
    assert_eq!(doc.history.undo_len(), undo_before + 1, "one undo step");
    let sketch = doc.primitive().sketch.as_ref().unwrap();
    assert_eq!(sketch.pad_overrides.len(), 1);
    let o = sketch
        .override_for(array, 2, 0)
        .expect("keyed by the instance");
    assert_eq!(o.net.as_deref(), Some("GND"));
    assert_eq!(o.number, None, "only what changed is stored");
    assert_eq!(nets(doc.primitive()), expected(&["3"]));

    let text = doc.file.to_toml_string().expect("saves");
    assert!(text.starts_with("format = \"snxfpt/1.2\"\n"), "{text}");
    let back = FootprintFile::from_toml_str(&text).expect("reads back");
    let sketch = back.footprints[0].sketch.as_ref().unwrap();
    assert_eq!(
        sketch
            .override_for(array, 2, 0)
            .and_then(|o| o.net.as_deref()),
        Some("GND")
    );
    assert_eq!(nets(&back.footprints[0]), expected(&["3"]));
}

#[test]
fn all_pads_changes_every_instance() {
    let (mut app, path, _) = open("scope-all");

    set_net(&mut app, &path, "3", "GND");
    answer(&mut app, &path, PadScope::AllPads);

    let doc = editor(&app, &path);
    let sketch = doc.primitive().sketch.as_ref().unwrap();
    assert!(sketch.pad_overrides.is_empty());
    assert_eq!(nets(doc.primitive()), expected(&["1", "2", "3", "4"]));
}

#[test]
fn cancel_changes_nothing() {
    let (mut app, path, _) = open("scope-cancel");
    let before = editor(&app, &path).file.clone();
    let undo_before = editor(&app, &path).history.undo_len();

    set_net(&mut app, &path, "3", "GND");
    answer(&mut app, &path, PadScope::Cancel);

    let doc = editor(&app, &path);
    assert!(doc.state.pad_scope.is_none());
    assert_eq!(doc.file, before);
    assert_eq!(doc.history.undo_len(), undo_before);
    let pad = &doc.state.pads[idx_of(&app, &path, "3")];
    assert_eq!(pad.net, "", "the panel shows the pad as it was");
}

#[test]
fn the_question_comes_back_on_the_next_edit() {
    let (mut app, path, _) = open("scope-again");

    set_net(&mut app, &path, "3", "GND");
    answer(&mut app, &path, PadScope::ThisPad);
    set_net(&mut app, &path, "3", "VCC");

    assert!(
        editor(&app, &path).state.pad_scope.is_some(),
        "no answer is remembered"
    );
    set_net(&mut app, &path, "2", "VCC");
    let prompt = editor(&app, &path).state.pad_scope.as_deref().unwrap();
    assert_eq!(prompt.instance.1, 1, "an edit of another pad asks about it");
}

#[test]
fn the_source_pad_is_edited_without_a_question() {
    let (mut app, path, _) = open("scope-source");

    set_net(&mut app, &path, "1", "GND");

    assert!(editor(&app, &path).state.pad_scope.is_none());
}

#[test]
fn esc_closes_the_question_and_changes_nothing() {
    let (mut app, path, _) = open("scope-esc");
    let before = editor(&app, &path).file.clone();

    set_net(&mut app, &path, "2", "GND");
    assert!(editor(&app, &path).state.pad_scope.is_some());
    drop(
        app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
            path: path.clone(),
            msg: PrimitiveEdit::Footprint(FootprintEditorMsg::ToolEscape),
        })),
    );

    let doc = editor(&app, &path);
    assert!(doc.state.pad_scope.is_none(), "Esc answers Cancel first");
    assert_eq!(doc.file, before);
    assert_eq!(doc.state.pads[idx_of(&app, &path, "2")].net, "");
}
