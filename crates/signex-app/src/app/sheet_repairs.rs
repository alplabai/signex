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

    #[test]
    fn a_repaired_reload_stays_modified() {
        assert!(dirty_after_reload(false));
    }

    #[test]
    fn a_repaired_read_only_reload_is_not_asked_to_save() {
        assert!(!dirty_after_reload(true));
    }

    #[test]
    fn a_closed_sheet_read_for_a_bulk_pass_reports_its_repairs() {
        // Annotate, Reset, Replace, re-prefix and reference fixes read closed
        // sheets through `load_sheet` and may write them back (#1830).
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
        let loaded = crate::app::project_sheets::load_sheet(&app.document_state, &path)
            .expect("reads")
            .expect("present");
        assert_eq!(loaded.wires.len(), 2);
        assert!(loaded.unknown.load_repairs.is_empty(), "taken at the read");
        let told = crate::diagnostics::recent_entries().iter().any(|e| {
            e.message
                .starts_with("ClosedBulk1830.snxsch repaired on load: wire #2")
        });
        assert!(told, "Messages must name the repair");
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
        let mut app = app_workspace("/w/proj", &[]);
        let path = std::path::PathBuf::from("/w/proj/taken_once.snxsch");
        app.open_schematic_tab(path.clone(), "taken_once".into(), repaired_sheet(false));
        assert!(app.document_state.is_document_dirty(&path));
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
