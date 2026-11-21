//! A History restore that lands on a sheet the user switched away from
//! rewrites the file behind the engines: background reads taken before it
//! (Number Schematic Sheets, ERC) must see that they are out of date.

use std::path::{Path, PathBuf};

use signex_types::schematic::{ChildSheet, FillType, Point, SchematicSheet, Wire};

use super::restore_commit_tests::{click_restore, commit_revision, finish_restore};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::sheet_numbering::{NumberingProblem, SheetNumberingMsg};
use crate::app::{Message, Signex};

/// The child pages the root places, so Apply can swap their numbers.
const CHILDREN: [(u128, &str); 2] = [(0xC41D, "x"), (0xC42D, "y")];
/// The wire only the restored revision holds.
const RESTORED: u128 = 0x01D;

fn child_symbol((id, name): (u128, &str)) -> ChildSheet {
    ChildSheet {
        uuid: uuid::Uuid::from_u128(id),
        name: name.into(),
        filename: format!("{name}.snxsch"),
        position: Point::new(0.0, 0.0),
        size: (10.0, 10.0),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: Vec::new(),
        instances: Vec::new(),
    }
}

/// The root sheet, placing the same children either way; `marker` adds a wire
/// so the two revisions differ only off the sheet symbols.
fn root_sheet(marker: Option<u128>) -> SchematicSheet {
    SchematicSheet {
        uuid: uuid::Uuid::from_u128(0x2007),
        paper_size: "A4".into(),
        child_sheets: CHILDREN.map(child_symbol).to_vec(),
        wires: marker
            .map(|id| Wire {
                uuid: uuid::Uuid::from_u128(id),
                start: Point::new(10.16, 10.16),
                end: Point::new(30.48, 10.16),
                stroke_width: 0.0,
                color: None,
                locked: false,
                unknown_columns: Default::default(),
            })
            .into_iter()
            .collect(),
        ..Default::default()
    }
}

fn encode(sheet: SchematicSheet) -> String {
    signex_types::format::SnxSchematic::new(sheet)
        .write_string()
        .expect("encode")
}

struct Rig {
    app: Signex,
    root: PathBuf,
    other: PathBuf,
    sha: String,
    _dir: tempfile::TempDir,
}

/// A git project whose root sheet has an older revision to restore, open
/// next to a second sheet; the root tab is active.
fn rig() -> Rig {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("root.snxsch");
    let other = dir.path().join("other.snxsch");
    for (_, name) in CHILDREN {
        std::fs::write(
            dir.path().join(format!("{name}.snxsch")),
            encode(SchematicSheet::default()),
        )
        .expect("child on disk");
    }
    let sha = commit_revision(dir.path(), &root, &encode(root_sheet(Some(RESTORED))));
    commit_revision(dir.path(), &root, &encode(root_sheet(None)));
    let mut app = app_workspace(&dir.path().to_string_lossy(), &["root.snxsch"]);
    app.document_state.projects[0].data.enable_git = true;
    app.open_schematic_tab(other.clone(), "other".into(), SchematicSheet::default());
    app.open_schematic_tab(root.clone(), "root".into(), root_sheet(None));
    Rig {
        app,
        root,
        other,
        sha,
        _dir: dir,
    }
}

fn holds_restored(app: &Signex, path: &Path) -> bool {
    app.document_state.engines[path]
        .document()
        .wires
        .iter()
        .any(|w| w.uuid == uuid::Uuid::from_u128(RESTORED))
}

/// Restore the root while the other sheet is the active tab.
fn restore_root_in_background(rig: &mut Rig) {
    let messages = click_restore(&mut rig.app, &rig.sha);
    assert!(rig.app.activate_tab_for_path(&rig.other));
    finish_restore(&mut rig.app, messages);
    assert_eq!(
        rig.app.document_state.active_path.as_deref(),
        Some(rig.other.as_path()),
        "the restore did not take focus"
    );
    assert!(holds_restored(&rig.app, &rig.root), "the restore installed");
}

#[test]
fn a_background_history_restore_moves_the_document_and_sheet_edit_epochs() {
    let mut rig = rig();
    let messages = click_restore(&mut rig.app, &rig.sha);
    assert!(rig.app.activate_tab_for_path(&rig.other));
    let documents = rig.app.document_state.documents_epoch;
    let sheets = rig.app.document_state.sheet_edit_epoch;

    finish_restore(&mut rig.app, messages);

    assert!(holds_restored(&rig.app, &rig.root));
    assert!(
        rig.app.document_state.documents_epoch > documents,
        "a read taken before the restore is stale"
    );
    assert!(
        rig.app.document_state.sheet_edit_epoch > sheets,
        "ERC results taken before the restore are stale"
    );
}

fn send(app: &mut Signex, msg: SheetNumberingMsg) {
    crate::task_driver::TaskDriver::new()
        .send(app, Message::from(msg))
        .expect("settled");
}

#[test]
fn sheet_numbering_read_before_a_background_restore_is_refused_at_apply() {
    let mut rig = rig();
    send(&mut rig.app, SheetNumberingMsg::Requested);
    let state = rig.app.ui_state.sheet_numbering.as_ref().expect("dialog");
    let placed: Vec<(usize, usize)> = state
        .rows()
        .iter()
        .enumerate()
        .filter(|(_, r)| r.placement.is_some())
        .map(|(row, r)| (row, r.number))
        .collect();
    assert_eq!(placed.len(), 2, "both child pages are listed");
    // Swap the two child pages' numbers.
    for (&(row, _), &(_, number)) in placed.iter().zip(placed.iter().rev()) {
        let text = number.to_string();
        send(&mut rig.app, SheetNumberingMsg::NumberEdited { row, text });
    }
    let state = rig.app.ui_state.sheet_numbering.as_ref().expect("dialog");
    assert!(state.can_apply(), "the swap is ready to apply");

    restore_root_in_background(&mut rig);
    send(&mut rig.app, SheetNumberingMsg::ApplyRequested);

    let state = rig
        .app
        .ui_state
        .sheet_numbering
        .as_ref()
        .expect("still open");
    assert_eq!(state.problem(), Some(&NumberingProblem::Stale));
    let root = rig.app.document_state.engines[&rig.root].document();
    assert!(
        root.child_sheets.iter().all(|c| c.instances.is_empty()),
        "nothing was renumbered"
    );
}
