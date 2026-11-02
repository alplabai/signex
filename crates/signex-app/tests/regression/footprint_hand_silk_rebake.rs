//! #1718 — hand-placed silk in a sketch-mode footprint survives a re-bake
//! and a save; the silk the sketch baked is shown but not edited in Pads
//! mode, and the Properties selection follows its entry across a re-bake.

use signex_app::app::{FootprintDocument, Message, Signex, TabInfo, TabKind};
use signex_app::dock::DockMessage;
use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit, RoleTag};
use signex_app::panels::PanelMsg;
use signex_library::primitive::footprint::{FpGraphic, FpGraphicKind};
use signex_library::{Footprint, FootprintFile};
use signex_sketch::attr::SilkAttr;
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::sketch::SketchData;
use signex_types::layer::SignexLayer;
use std::path::{Path, PathBuf};

/// Two free lines; the first is tagged top silk. Returns the footprint and
/// both line ids.
fn footprint_with_silk_line() -> (Footprint, SketchEntityId, SketchEntityId) {
    let plane = PlaneId::new();
    let mut sketch = SketchData::default();
    sketch.planes.push(Plane {
        id: plane,
        kind: PlaneKind::BoardTop,
    });
    let mut line = |y: f64, silk: bool| {
        let (a, b, id) = (
            SketchEntityId::new(),
            SketchEntityId::new(),
            SketchEntityId::new(),
        );
        sketch
            .entities
            .push(Entity::new(a, plane, EntityKind::Point { x: -1.0, y }));
        sketch
            .entities
            .push(Entity::new(b, plane, EntityKind::Point { x: 1.0, y }));
        let mut line = Entity::new(id, plane, EntityKind::Line { start: a, end: b });
        line.silk = silk.then_some(SilkAttr {
            layer: SignexLayer::TopSilk,
        });
        sketch.entities.push(line);
        id
    };
    let (outline, spare) = (line(-1.0, true), line(1.0, false));
    let mut fp = Footprint::empty("hand-silk");
    fp.sketch = Some(sketch);
    (fp, outline, spare)
}

fn open_editor(stem: &str, file: FootprintFile) -> (Signex, PathBuf) {
    let path = PathBuf::from(format!("{stem}.snxfpt"));
    let editor = FootprintDocument::new(path.clone(), file);
    let (mut app, _initial_task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(TabInfo {
        title: stem.into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;
    (app, path)
}

fn edit(app: &mut Signex, path: &Path, msg: FootprintEditorMsg) {
    drop(
        app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
            path: path.to_path_buf(),
            msg: PrimitiveEdit::Footprint(msg),
        })),
    );
}

fn panel(app: &mut Signex, msg: PanelMsg) {
    drop(app.update(Message::Dock(DockMessage::Panel(msg))));
}

/// Any sketch edit that solves inline re-bakes every field, silk included.
fn rebake(app: &mut Signex, path: &Path) {
    edit(
        app,
        path,
        FootprintEditorMsg::SketchEditParameter {
            name: "unrelated".into(),
            expr: "1mm".into(),
        },
    );
}

fn silk(app: &Signex, path: &Path) -> Vec<FpGraphic> {
    app.document_state.footprint_editors[path]
        .primitive()
        .silk_f
        .clone()
}

/// A baked footprint, then Place String and a two-click Place Track.
fn editor_with_hand_silk(stem: &str) -> (Signex, PathBuf, SketchEntityId, SketchEntityId) {
    let (fp, outline, spare) = footprint_with_silk_line();
    let (mut app, path) = open_editor(stem, FootprintFile::from_footprint(fp));
    rebake(&mut app, &path);
    assert_eq!(silk(&app, &path).len(), 1, "the outline bakes");
    edit(
        &mut app,
        &path,
        FootprintEditorMsg::AddText {
            x_mm: 0.0,
            y_mm: 3.0,
        },
    );
    edit(
        &mut app,
        &path,
        FootprintEditorMsg::TrackClick {
            x_mm: 2.0,
            y_mm: 2.0,
        },
    );
    edit(
        &mut app,
        &path,
        FootprintEditorMsg::TrackClick {
            x_mm: 3.0,
            y_mm: 2.0,
        },
    );
    edit(&mut app, &path, FootprintEditorMsg::TrackCancel);
    assert_eq!(silk(&app, &path).len(), 3);
    (app, path, outline, spare)
}

fn assert_outline_and_hand_silk(silk: &[FpGraphic], outline: SketchEntityId) {
    assert_eq!(silk.len(), 3, "{silk:?}");
    assert_eq!(silk[0].sketch_entity, Some(outline));
    assert!(matches!(&silk[1].kind, FpGraphicKind::Text { content, .. } if content == "TEXT"));
    assert!(matches!(
        silk[2].kind,
        FpGraphicKind::Line {
            from: [2.0, 2.0],
            to: [3.0, 2.0]
        }
    ));
    assert!(!silk[1].is_baked() && !silk[2].is_baked());
}

#[test]
fn hand_text_and_line_survive_a_rebake_save_and_reload() {
    let (mut app, path, outline, _) = editor_with_hand_silk("hand-silk-1718");
    rebake(&mut app, &path);
    assert_outline_and_hand_silk(&silk(&app, &path), outline);

    let saved = app.document_state.footprint_editors[&path]
        .file
        .to_toml_string()
        .expect("serialise");
    let reopened = FootprintFile::from_toml_str(&saved).expect("reparse");
    let (mut app, path) = open_editor("hand-silk-1718-reopened", reopened);
    assert_outline_and_hand_silk(&silk(&app, &path), outline);
    rebake(&mut app, &path);
    assert_outline_and_hand_silk(&silk(&app, &path), outline);
}

#[test]
fn a_selected_hand_entry_stays_selected_when_the_bake_grows() {
    let (mut app, path, outline, spare) = editor_with_hand_silk("silk-select-1718");
    edit(&mut app, &path, FootprintEditorMsg::SelectSilkF(Some(1)));
    edit(
        &mut app,
        &path,
        FootprintEditorMsg::SketchSetRole {
            id: spare,
            role: RoleTag::SilkTop,
        },
    );
    let before = silk(&app, &path);
    assert_eq!(before.len(), 4, "two baked lines, then the hand silk");

    panel(&mut app, PanelMsg::FpEditorSetSilkText("U1".into()));

    let after = silk(&app, &path);
    assert_eq!(after[0].sketch_entity, Some(outline));
    assert_eq!(after[1].sketch_entity, Some(spare));
    assert!(
        matches!(&after[2].kind, FpGraphicKind::Text { content, .. } if content == "U1"),
        "the edit reached another entry: {after:?}"
    );
    assert_eq!(after[3], before[3]);
}

#[test]
fn pads_mode_refuses_to_edit_or_delete_a_baked_entry() {
    let _ = signex_app::diagnostics::init_logging();
    let (mut app, path, _, _) = editor_with_hand_silk("silk-refuse-1718");
    let before = silk(&app, &path);
    edit(&mut app, &path, FootprintEditorMsg::SelectSilkF(Some(0)));

    panel(&mut app, PanelMsg::FpEditorSetSilkLineFromX("5".into()));
    panel(&mut app, PanelMsg::FpEditorSetSilkStrokeWidth("0.5".into()));
    panel(&mut app, PanelMsg::FpEditorToggleSilkFilled(true));
    panel(&mut app, PanelMsg::FpEditorDeleteSelectedSilk);
    edit(&mut app, &path, FootprintEditorMsg::DeleteSilkF);

    assert_eq!(
        silk(&app, &path),
        before,
        "a baked entry changed in Pads mode"
    );
    assert!(
        signex_app::diagnostics::recent_entries().iter().any(|e| e
            .message
            .contains("baked from the sketch; edit it in Sketch mode")),
        "the refusal is reported"
    );
}
