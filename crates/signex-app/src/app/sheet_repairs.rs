//! Telling Messages what loading a sheet repaired (#1830).
//!
//! The parser records repairs on the sheet (`SchematicSheet::take_load_repairs`);
//! the code that installs a sheet into the app takes them once, logs them here,
//! and marks the sheet modified unless it is read-only.

use std::path::Path;

use signex_types::format::SheetLoadRepair;

/// Repairs named in full before the rest are summarised, so a file with
/// thousands of repeats does not flood Messages.
const SHOWN: usize = 20;

/// Take the repairs off a sheet a pass is about to write or keep, and log
/// them: the file will hold the repaired sheet. True when there were any.
pub(crate) fn take_and_log(
    path: &Path,
    sheet: &mut signex_types::schematic::SchematicSheet,
) -> bool {
    log_sheet_repairs(path, &sheet.take_load_repairs())
}

/// Take and log what loading repaired on a board (#1886). True when the
/// board now differs from its file and can be saved, so it must open modified;
/// a read-only (newer-minor) board only reports.
pub(crate) fn take_and_log_board(path: &Path, board: &mut signex_types::pcb::PcbBoard) -> bool {
    log_sheet_repairs(path, &board.take_load_repairs())
        && board.unknown.newer_minor_source.is_none()
}

/// The one line a read-only scan of a project says about a repaired sheet.
pub(crate) fn summary_line(path: &Path, count: usize) -> String {
    let file = path.file_name().unwrap_or_default().to_string_lossy();
    format!("{file} has {count} repeated ids; open and save it to fix")
}

/// Log `repairs` of the sheet at `path` to Messages; true when there were any.
pub(crate) fn log_sheet_repairs(path: &Path, repairs: &[SheetLoadRepair]) -> bool {
    let file = path.file_name().unwrap_or_default().to_string_lossy();
    for repair in repairs.iter().take(SHOWN) {
        crate::diagnostics::log_warning(format!("{file} repaired on load: {}", repair.describe()));
    }
    if repairs.len() > SHOWN {
        crate::diagnostics::log_warning(format!(
            "{file} repaired on load: and {} more repairs",
            repairs.len() - SHOWN
        ));
    }
    !repairs.is_empty()
}

#[cfg(test)]
mod tests {
    use signex_types::format::{RepairedObject, SheetLoadRepair};
    use signex_types::schematic::SchematicSheet;

    use crate::app::handlers::menu::export::tests::app_workspace;
    use crate::app::restore_job::RestoreJobDocument;

    fn repaired_sheet(read_only: bool) -> SchematicSheet {
        let mut sheet = SchematicSheet {
            uuid: uuid::Uuid::now_v7(),
            paper_size: "A4".into(),
            ..Default::default()
        };
        sheet
            .unknown
            .load_repairs
            .push(SheetLoadRepair::CopyDropped {
                object: RepairedObject {
                    kind: "wire",
                    name: String::new(),
                    position: 2,
                },
                uuid: uuid::Uuid::from_u128(7),
            });
        if read_only {
            sheet.unknown.newer_minor_source = Some("snxsch/1.7".into());
        }
        sheet
    }

    /// Reload a tab with a repaired sheet; whether it ends dirty.
    fn dirty_after_reload(read_only: bool) -> bool {
        let _ = crate::diagnostics::init_logging();
        let mut app = app_workspace("/w/proj", &[]);
        let path = std::path::PathBuf::from("/w/proj/reload_repair.snxsch");
        app.open_schematic_tab(path.clone(), "reload_repair".into(), repaired_sheet(false));
        app.document_state.mark_document_clean(&path);
        let idx = app.document_state.active_tab;
        let _ = app.apply_reloaded(
            idx,
            Ok(RestoreJobDocument::Schematic(Box::new(repaired_sheet(
                read_only,
            )))),
            None,
        );
        app.document_state.is_document_dirty(&path)
    }

    /// A board parsed from a file whose two tracks share one uuid.
    fn repaired_board(read_only: bool) -> signex_types::pcb::PcbBoard {
        let text = "format = \"snxpcb/1\"\npcb_id = \"00000000-0000-0000-0000-000000000009\"\n";
        let mut board = signex_types::format::SnxPcb::parse(text)
            .expect("parses")
            .board;
        let track = |end_x: f64| signex_types::pcb::Segment {
            uuid: uuid::Uuid::from_u128(7),
            start: signex_types::pcb::Point { x: 0.0, y: 0.0 },
            end: signex_types::pcb::Point { x: end_x, y: 0.0 },
            width: 0.2,
            layer: "TopCopper".into(),
            net: 0,
            unknown_columns: Default::default(),
        };
        board.segments = vec![track(1.0), track(2.0)];
        let text = signex_types::format::SnxPcb::new(board)
            .write_string()
            .expect("serialise");
        let mut board = signex_types::format::SnxPcb::parse(&text)
            .expect("parses")
            .board;
        assert_eq!(board.unknown.load_repairs.len(), 1, "fixture: one repair");
        if read_only {
            board.unknown.newer_minor_source = Some("snxpcb/1.7".into());
        }
        board
    }

    #[test]
    fn a_board_repaired_on_open_opens_modified_and_reports_once() {
        let _ = crate::diagnostics::init_logging();
        let mut app = app_workspace("/w/proj", &[]);
        let path = std::path::PathBuf::from("/w/proj/board_repair_once.snxpcb");
        app.open_pcb_tab(path.clone(), "board_repair".into(), repaired_board(false));
        assert!(app.document_state.is_document_dirty(&path));
        let told = crate::diagnostics::recent_entries()
            .iter()
            .filter(|e| {
                e.message
                    .starts_with("board_repair_once.snxpcb repaired on load")
            })
            .count();
        assert_eq!(told, 1);
        let Some(crate::app::TabDocument::Pcb(board)) = app
            .document_state
            .tabs
            .last()
            .and_then(|tab| tab.cached_document.as_ref())
        else {
            panic!("a board tab");
        };
        assert!(board.unknown.load_repairs.is_empty());
    }

    #[test]
    fn a_read_only_board_repaired_on_open_stays_clean() {
        let mut app = app_workspace("/w/proj", &[]);
        let path = std::path::PathBuf::from("/w/proj/board_ro.snxpcb");
        app.open_pcb_tab(path.clone(), "board_ro".into(), repaired_board(true));
        assert!(!app.document_state.is_document_dirty(&path));
    }

    #[test]
    fn a_repaired_reload_stays_modified() {
        assert!(dirty_after_reload(false));
    }

    #[test]
    fn a_repaired_read_only_reload_is_not_asked_to_save() {
        assert!(!dirty_after_reload(true));
    }

    #[test]
    fn a_pass_that_writes_a_closed_sheet_reports_its_repairs_and_a_scan_does_not() {
        let _ = crate::diagnostics::init_logging();
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("ClosedBulk1830.snxsch");
        let wire = |end_x: f64| signex_types::schematic::Wire {
            uuid: uuid::Uuid::from_u128(7),
            end: signex_types::schematic::Point::new(end_x, 0.0),
            ..Default::default()
        };
        let sheet = SchematicSheet {
            uuid: uuid::Uuid::now_v7(),
            paper_size: "A4".into(),
            wires: vec![wire(1.0), wire(2.0)],
            ..Default::default()
        };
        let text = signex_types::format::SnxSchematic::new(sheet)
            .write_string()
            .expect("serialise");
        std::fs::write(&path, text).expect("write");
        let app = app_workspace("/w/proj", &[]);
        let told = || {
            crate::diagnostics::recent_entries()
                .iter()
                .filter(|e| {
                    e.message
                        .starts_with("ClosedBulk1830.snxsch repaired on load")
                })
                .count()
        };
        // A scan reads the sheet through the shared read: silent.
        let mut loaded = crate::app::project_sheets::load_sheet(&app.document_state, &path)
            .expect("reads")
            .expect("present");
        assert_eq!(told(), 0);
        // A pass that writes it takes and reports the repairs.
        assert!(super::take_and_log(&path, &mut loaded));
        assert_eq!(told(), 1);
        assert!(loaded.unknown.load_repairs.is_empty());
    }

    #[test]
    fn a_repaired_sheet_stays_modified_after_an_edit_and_undo() {
        let mut app = app_workspace("/w/proj", &[]);
        let path = std::path::PathBuf::from("/w/proj/edit_undo.snxsch");
        app.open_schematic_tab(path.clone(), "edit_undo".into(), repaired_sheet(false));
        assert!(app.document_state.is_document_dirty(&path));
        let edited = app.apply_engine_command(
            signex_engine::Command::SetPaperSize {
                paper_size: "A3".to_string(),
            },
            true,
            true,
        );
        assert!(edited, "fixture: the edit must change the document");
        app.handle_undo_requested();
        assert!(
            app.document_state.is_document_dirty(&path),
            "undo returns to the repaired sheet, which the file does not hold"
        );
    }

    #[test]
    fn repairs_are_taken_once_on_open() {
        let _ = crate::diagnostics::init_logging();
        let mut app = app_workspace("/w/proj", &[]);
        let path = std::path::PathBuf::from("/w/proj/taken_once.snxsch");
        app.open_schematic_tab(path.clone(), "taken_once".into(), repaired_sheet(false));
        assert!(app.document_state.is_document_dirty(&path));
        let told = crate::diagnostics::recent_entries()
            .iter()
            .filter(|e| e.message.starts_with("taken_once.snxsch repaired on load"))
            .count();
        assert_eq!(told, 1);
        let engine = app.document_state.engines.get(&path).expect("engine");
        assert!(engine.document().unknown.load_repairs.is_empty());
    }

    #[test]
    fn a_read_only_sheet_repaired_on_open_stays_clean() {
        let mut app = app_workspace("/w/proj", &[]);
        let path = std::path::PathBuf::from("/w/proj/ro_open.snxsch");
        app.open_schematic_tab(path.clone(), "ro_open".into(), repaired_sheet(true));
        assert!(!app.document_state.is_document_dirty(&path));
    }
}
