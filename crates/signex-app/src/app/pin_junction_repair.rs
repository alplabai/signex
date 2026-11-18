//! Pin ends that touch a wire mid-span with no dot (2026-11-18).
//!
//! An edit that makes such a contact gets its dot from the engine. A sheet
//! saved before that, or by another tool, is opened as it is: opening never
//! changes a file. Messages says how many pins touch a wire without a
//! junction, once per open, and Repair Pin Junctions adds the dots as one undo
//! step (`signex_engine::pin_junction_repairs`).

use std::path::Path;

use signex_types::schematic::SchematicSheet;

use super::Signex;

/// The Messages line for `count` bare contacts in the file `file`.
pub(crate) fn bare_taps_line(file: &str, count: usize) -> String {
    let pins = if count == 1 {
        "1 pin touches".to_string()
    } else {
        format!("{count} pins touch")
    };
    format!("{file}: {pins} a wire without a junction; Repair Pin Junctions adds the dots")
}

/// Tell Messages how many pin ends of the sheet opened from `path` lie inside
/// a wire with no dot; the sheet is left as it is. The count reported.
pub(crate) fn report_bare_pin_taps(path: &Path, sheet: &SchematicSheet) -> usize {
    let count = signex_engine::bare_pin_taps(sheet).len();
    if count > 0 {
        let file = path.file_name().unwrap_or_default().to_string_lossy();
        crate::diagnostics::log_warning(bare_taps_line(&file, count));
    }
    count
}

impl Signex {
    /// Repair Pin Junctions: a dot on every bare contact of the active sheet,
    /// one undo step. Says so in Messages when there is nothing to repair.
    pub(crate) fn repair_pin_junctions(&mut self) {
        let Some(engine) = self.document_state.active_engine() else {
            return;
        };
        let mut ids = signex_types::object_id::UuidV7Ids;
        let repairs = signex_engine::pin_junction_repairs(engine.document(), &mut ids);
        if repairs.is_empty() {
            crate::diagnostics::log_info("No pin touches a wire without a junction");
            return;
        }
        let count = repairs.len();
        if self.apply_engine_commands(repairs, true, true) {
            crate::diagnostics::log_info(format!("Added {count} pin junction(s)"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::{
        LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, Symbol, Wire,
    };

    /// TP1's pin end inside a wire (0,10)-(40,10), with no dot.
    fn sheet_with_bare_tap() -> SchematicSheet {
        let mut lib: LibSymbol =
            serde_json::from_value(serde_json::json!({ "id": "TP" })).expect("lib symbol");
        lib.pins.push(LibPin {
            unit: 0,
            body_style: 1,
            pin: Pin {
                direction: PinDirection::Passive,
                shape_style: PinShapeStyle::Plain,
                position: Point::new(0.0, 0.0),
                rotation: 0.0,
                length: 0.0,
                name: String::new(),
                number: "1".to_string(),
                visible: true,
                name_visible: true,
                number_visible: true,
            },
        });
        let mut sheet = SchematicSheet::default();
        sheet.lib_symbols.insert("TP".to_string(), lib);
        sheet.symbols.push(Symbol {
            uuid: uuid::Uuid::now_v7(),
            lib_id: "TP".to_string(),
            reference: "TP1".to_string(),
            position: Point::new(20.0, 10.0),
            unit: 1,
            ..Symbol::default()
        });
        sheet.wires.push(Wire {
            uuid: uuid::Uuid::now_v7(),
            start: Point::new(0.0, 10.0),
            end: Point::new(40.0, 10.0),
            ..Wire::default()
        });
        sheet
    }

    fn app_with(sheet: SchematicSheet) -> (Signex, std::path::PathBuf) {
        let (mut app, _task) = Signex::new();
        let path = std::path::PathBuf::from("bare_tap.snxsch");
        let engine = signex_engine::Engine::new(sheet).unwrap();
        app.document_state.engines.insert(path.clone(), engine);
        app.document_state.tabs.push(crate::app::TabInfo {
            title: "bare_tap".to_string(),
            path: path.clone(),
            cached_document: None,
            project_id: None,
            kind: crate::app::TabKind::Schematic,
        });
        app.document_state.active_tab = app.document_state.tabs.len() - 1;
        app.document_state.active_path = Some(path.clone());
        (app, path)
    }

    #[test]
    fn an_opened_sheet_is_reported_once_and_left_as_it_is() {
        let sheet = sheet_with_bare_tap();
        let path = Path::new("reported_once.snxsch");
        let line = bare_taps_line("reported_once.snxsch", 1);
        let before = crate::diagnostics::logged_count(&line);
        assert_eq!(report_bare_pin_taps(path, &sheet), 1);
        assert_eq!(crate::diagnostics::logged_count(&line), before + 1);
        assert!(sheet.junctions.is_empty(), "the report changes nothing");
    }

    #[test]
    fn repair_adds_the_dots_in_one_undo_step() {
        let (mut app, path) = app_with(sheet_with_bare_tap());
        app.repair_pin_junctions();
        let dots = |app: &Signex| app.document_state.engines[&path].document().junctions.len();
        assert_eq!(dots(&app), 1);
        let engine = app.document_state.engines.get_mut(&path).unwrap();
        assert!(signex_engine::bare_pin_taps(engine.document()).is_empty());
        engine.undo().unwrap();
        assert_eq!(dots(&app), 0, "one undo takes the repair back");
    }

    #[test]
    fn the_repair_message_reaches_the_handler() {
        let (mut app, path) = app_with(sheet_with_bare_tap());
        let message = crate::app::Message::Erc(crate::app::ErcMsg::PinJunctionRepairRequested);
        let _ = app.update(message);
        let junctions = &app.document_state.engines[&path].document().junctions;
        assert_eq!(junctions.len(), 1);
    }
}
