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
/// a wire with no dot; the sheet is left as it is. The count reported. Its
/// Messages row links to Repair Pin Junctions on that sheet, wherever the view
/// is when the link is pressed (`panels::messages_row::row_action`).
pub(crate) fn report_bare_pin_taps(path: &Path, sheet: &SchematicSheet) -> usize {
    let count = signex_engine::bare_pin_taps(sheet).len();
    if count > 0 {
        let file = path.file_name().unwrap_or_default().to_string_lossy();
        crate::diagnostics::log_warning_with_link(
            crate::panels::messages_row::PIN_JUNCTIONS_TARGET,
            bare_taps_line(&file, count),
            crate::diagnostics::MessageLink::RepairPinJunctions(path.to_path_buf()),
        );
    }
    count
}

impl Signex {
    /// Repair Pin Junctions on the sheet at `path`, the one a Messages row
    /// names: its tab comes to the front first. A sheet no longer open is
    /// refused with a notice; the sheet in view is left alone.
    pub(crate) fn repair_pin_junctions_at(&mut self, path: &Path) {
        if !self.activate_tab_for_path(path) {
            let file = path.file_name().unwrap_or_default().to_string_lossy();
            crate::diagnostics::log_warning(format!(
                "{file} is not open; open it to repair its pin junctions"
            ));
            return;
        }
        self.repair_pin_junctions();
    }

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
pub(crate) mod tests {
    use super::*;
    use signex_types::schematic::{
        LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, Symbol, Wire,
    };

    /// TP1's pin end inside a wire (0,10)-(40,10), with no dot.
    pub(crate) fn sheet_with_bare_tap() -> SchematicSheet {
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

    /// The report's Messages row offers Repair Pin Junctions as a link.
    #[test]
    fn the_report_row_links_to_the_repair() {
        let line = bare_taps_line("linked_report.snxsch", 1);
        let before = crate::diagnostics::logged_count(&line); // installs the logger
        report_bare_pin_taps(Path::new("linked_report.snxsch"), &sheet_with_bare_tap());
        let entry = (crate::diagnostics::recent_entries().into_iter())
            .rev()
            .find(|entry| entry.message == line)
            .expect("the report is logged");
        assert_eq!(crate::diagnostics::logged_count(&line), before + 1);
        let (link, _) = crate::panels::messages_row::row_action(&entry).expect("a link");
        let named = std::path::PathBuf::from("linked_report.snxsch");
        assert_eq!(
            link,
            crate::diagnostics::MessageLink::RepairPinJunctions(named)
        );
        let code = crate::diagnostics::diagnostic_code_from_target(
            crate::panels::messages_row::PIN_JUNCTIONS_TARGET,
        );
        assert_eq!(entry.code, code, "the row keeps its own source");
    }

    /// The link repairs the sheet its row names, not the one in view: that
    /// sheet's tab comes to the front and only it gets the dot. A sheet that
    /// is no longer open is refused and nothing changes.
    #[test]
    fn the_link_repairs_the_sheet_its_row_names() {
        let (mut app, named) = app_with(sheet_with_bare_tap());
        let other = std::path::PathBuf::from("in_view.snxsch");
        let engine = signex_engine::Engine::new(sheet_with_bare_tap()).unwrap();
        app.document_state.engines.insert(other.clone(), engine);
        app.document_state.tabs.push(crate::app::TabInfo {
            title: "in_view".to_string(),
            path: other.clone(),
            project_id: None,
            kind: crate::app::TabKind::Schematic,
        });
        app.document_state.active_tab = app.document_state.tabs.len() - 1;
        app.document_state.active_path = Some(other.clone());

        let link = crate::diagnostics::MessageLink::RepairPinJunctions(named.clone());
        let pressed = crate::panels::PanelMsg::MessageLinkPressed(link);
        let _ = app.update(crate::app::Message::Dock(crate::dock::DockMessage::Panel(
            pressed,
        )));
        let dots = |app: &Signex, path: &std::path::PathBuf| {
            app.document_state.engines[path].document().junctions.len()
        };
        assert_eq!(dots(&app, &named), 1, "the named sheet is repaired");
        assert_eq!(dots(&app, &other), 0, "the sheet in view is not");
        assert_eq!(app.document_state.active_path.as_ref(), Some(&named));

        let gone = std::path::PathBuf::from("closed.snxsch");
        let notice = "closed.snxsch is not open; open it to repair its pin junctions";
        let before = crate::diagnostics::logged_count(notice);
        app.repair_pin_junctions_at(&gone);
        assert_eq!(crate::diagnostics::logged_count(notice), before + 1);
        assert_eq!(dots(&app, &other), 0, "nothing else is repaired");
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
