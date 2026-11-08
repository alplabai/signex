//! #1718 — hand-placed silk in a sketch-mode footprint survives a re-bake
//! and a save; the silk the sketch baked is shown but not edited in Pads
//! mode, and the Properties selection follows its entry across a re-bake.

use signex_app::app::{FileMsg, FootprintDocument, Message, Signex, TabInfo, TabKind};
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
use signex_types::pcb_layer::{PcbLayer, TechnicalLayer};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// The stock polarised 0805 as trunk wrote it before #1718: one baked,
/// unmarked cathode bar.
const PRE_1718_STOCK: &str = include_str!(
    "../../../signex-library/tests/fixtures/snxfpt/pre_1718_chip_0805_polarised.snxfpt"
);

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
            layer: PcbLayer::Technical(TechnicalLayer::SilkscreenTop),
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
    open_editor_at(PathBuf::from(format!("{stem}.snxfpt")), file)
}

fn open_editor_at(path: PathBuf, file: FootprintFile) -> (Signex, PathBuf) {
    let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
    let editor = FootprintDocument::new(path.clone(), file);
    let (mut app, _initial_task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(TabInfo {
        title: stem,
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

/// Write `file` to `path` and open it from those bytes, as File > Open does.
fn open_from_disk(path: &Path, file: &FootprintFile) -> Signex {
    std::fs::write(path, file.to_toml_string().expect("serialise")).expect("write");
    let read = FootprintFile::from_bytes(&std::fs::read(path).unwrap()).expect("parse");
    open_editor_at(path.to_path_buf(), read).0
}

/// Save through the real path: File > Save, the merge into the file on
/// disk, the atomic write.
fn save(app: &mut Signex) {
    crate::settle_support::settle(app, Message::File(FileMsg::Save));
}

fn reopen_from_disk(path: &Path) -> Signex {
    let read = FootprintFile::from_bytes(&std::fs::read(path).unwrap()).expect("parse");
    open_editor_at(path.to_path_buf(), read).0
}

/// A baked footprint, then Place String and a two-click Place Track.
fn editor_with_hand_silk(stem: &str) -> (Signex, PathBuf, SketchEntityId, SketchEntityId) {
    let (fp, outline, spare) = footprint_with_silk_line();
    let (mut app, path) = open_editor(stem, FootprintFile::from_footprint(fp));
    add_hand_silk(&mut app, &path);
    (app, path, outline, spare)
}

fn add_hand_silk(app: &mut Signex, path: &Path) {
    rebake(app, path);
    assert_eq!(silk(app, path).len(), 1, "the outline bakes");
    edit(
        app,
        path,
        FootprintEditorMsg::AddText {
            x_mm: 0.0,
            y_mm: 3.0,
        },
    );
    edit(
        app,
        path,
        FootprintEditorMsg::TrackClick {
            x_mm: 2.0,
            y_mm: 2.0,
        },
    );
    edit(
        app,
        path,
        FootprintEditorMsg::TrackClick {
            x_mm: 3.0,
            y_mm: 2.0,
        },
    );
    edit(app, path, FootprintEditorMsg::TrackCancel);
    assert_eq!(silk(app, path).len(), 3);
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
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("hand-silk-1718.snxfpt");
    let (fp, outline, _) = footprint_with_silk_line();
    let mut app = open_from_disk(&path, &FootprintFile::from_footprint(fp));
    add_hand_silk(&mut app, &path);
    rebake(&mut app, &path);
    assert_outline_and_hand_silk(&silk(&app, &path), outline);

    save(&mut app);
    assert!(!app.document_state.is_document_dirty(&path), "saved");
    let mut app = reopen_from_disk(&path);
    assert_outline_and_hand_silk(&silk(&app, &path), outline);
    rebake(&mut app, &path);
    assert_outline_and_hand_silk(&silk(&app, &path), outline);
}

/// Opening a file an older build saved marks its baked silk in memory; an
/// edit and a save must not be reported as replacing another window's
/// change, and the file then carries the markers and the hand silk.
#[test]
fn saving_an_edit_to_an_older_builds_footprint_writes_markers_without_an_overwrite() {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("older-build-1718.snxfpt");
    let mut file = FootprintFile::from_toml_str(PRE_1718_STOCK).expect("fixture");
    file.footprints[0].silk_f.push(FpGraphic::drawn(
        FpGraphicKind::Line {
            from: [0.0, 3.0],
            to: [1.0, 3.0],
        },
        0.12,
        false,
    ));
    let mut app = open_from_disk(&path, &file);
    assert!(silk(&app, &path)[0].is_baked(), "classified on open");

    panel(
        &mut app,
        PanelMsg::FpEditorSetFootprintDescription("edited".into()),
    );
    save(&mut app);

    let shown = path.display().to_string();
    let overwrites = signex_app::diagnostics::recent_entries()
        .into_iter()
        .filter(|e| e.message.contains(&shown) && e.message.contains("another window"))
        .count();
    assert_eq!(overwrites, 0, "a false overwrite was reported");
    let on_disk = FootprintFile::from_bytes(&std::fs::read(&path).unwrap()).unwrap();
    let fp = &on_disk.footprints[0];
    assert_eq!(fp.description, "edited");
    assert_eq!(fp.silk_f.len(), 2);
    assert!(fp.silk_f[0].is_baked() && !fp.silk_f[1].is_baked());
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
