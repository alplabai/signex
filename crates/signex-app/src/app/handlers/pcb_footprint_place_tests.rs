//! Mockup row 119 (#2828): Place in the Library panel's Footprints kind puts
//! the footprint on the cursor of the open board; otherwise it stays off and
//! says why.

use std::path::{Path, PathBuf};

use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{FootprintDocument, Signex, TabInfo, TabKind};
use crate::library::state::OpenLibrary;
use crate::panels::FootprintPlaceBlock as Block;
use crate::pcb_canvas::CarryKind;

fn board() -> PcbBoard {
    serde_json::from_value(serde_json::json!({"uuid": Uuid::from_u128(1)})).unwrap()
}

fn library(dir: &Path) -> OpenLibrary {
    OpenLibrary {
        root: dir.join("lib.snxlib"),
        display_name: "lib".into(),
        library_id: Uuid::now_v7(),
        tables: Default::default(),
        cached_components: Vec::new(),
        cached_symbols: Vec::new(),
        cached_footprints: Vec::new(),
        cached_sims: Vec::new(),
        cached_classes: Vec::new(),
        observed_stamp: None,
        dangling: Default::default(),
        pricing: Default::default(),
        own_edit_stamp: None,
        display: Default::default(),
    }
}

/// `boards` open boards, then a saved `R?` footprint of an open library in
/// the active tab, with a sibling footprint after it.
fn rig(dir: &Path, boards: usize) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    app.library.open_libraries.push(library(dir));
    for n in 0..boards {
        let path = dir.join(format!("board{n}.snxpcb"));
        app.open_pcb_tab(path, format!("board{n}"), board());
    }
    let mut footprint = signex_library::Footprint::empty("SOIC-8");
    footprint.default_designator = "R?".into();
    let path = dir
        .join("footprints")
        .join(format!("{}.snxfpt", footprint.uuid));
    let mut file = signex_library::FootprintFile::from_footprint(footprint);
    file.footprints
        .push(signex_library::Footprint::empty("QFN-16"));
    app.document_state
        .footprint_editors
        .insert(path.clone(), FootprintDocument::new(path.clone(), file));
    app.document_state.tabs.push(TabInfo {
        title: "footprint".into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.refresh_panel_ctx();
    (app, path)
}

#[test]
fn place_puts_the_footprint_on_the_cursor_of_the_open_board() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = rig(dir.path(), 1);
    let row = &app
        .document_state
        .panel_ctx
        .footprint_editor
        .as_ref()
        .expect("footprint editor")
        .internal_footprints[0];
    assert_eq!(row.place_block, None, "Place is live");

    let _ = app.place_library_footprint(0);

    assert!(app.has_active_pcb(), "the board is in view");
    let carry = app.interaction_state.pcb_carry.as_ref().expect("carry");
    assert_eq!(carry.kind, CarryKind::Place);
    assert_eq!(
        carry.part.as_deref().map(|part| part.footprint_name()),
        Some("SOIC-8")
    );

    assert!(app.pcb_place_clicked(Point::new(10.0, 10.0)));
    let placed = &app.active_pcb().expect("board").footprints;
    assert_eq!(placed.len(), 1, "one click places one copy");
    assert_eq!(placed[0].reference, "R1", "numbered from its designator");
    assert_eq!(placed[0].footprint_id, "SOIC-8");
    assert!(placed[0].library_ref.is_some(), "it keeps its library link");
}

#[test]
fn place_stays_off_without_exactly_one_open_board() {
    let dir = tempfile::tempdir().unwrap();
    let (mut none, _) = rig(dir.path(), 0);
    assert_eq!(none.footprint_place_block(0), Some(Block::NoBoard));
    let _ = none.place_library_footprint(0);
    assert!(none.interaction_state.pcb_carry.is_none());

    let dir = tempfile::tempdir().unwrap();
    let (several, _) = rig(dir.path(), 2);
    assert_eq!(several.footprint_place_block(0), Some(Block::SeveralBoards));
}

#[test]
fn place_stays_off_for_unsaved_edits_and_a_later_footprint_of_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = rig(dir.path(), 1);
    assert_eq!(app.footprint_place_block(1), Some(Block::NotFirstInFile));

    app.document_state
        .footprint_editors
        .get_mut(&path)
        .unwrap()
        .mark_dirty();
    assert_eq!(app.footprint_place_block(0), Some(Block::UnsavedEdits));
    let tab = app.document_state.active_tab;
    let _ = app.place_library_footprint(0);
    assert_eq!(
        app.document_state.active_tab, tab,
        "the editor stays in view"
    );
    assert!(app.interaction_state.pcb_carry.is_none());
}
