//! The Messages line for an edit refused on a read-only sheet (#1402).
//!
//! A sheet written by a newer Signex loads for viewing only. The mutation
//! gateway asks [`Signex::refuse_if_active_read_only`] before it touches the
//! engine, so the document never changes and the tab never turns dirty.

use super::*;

/// The line for the Messages panel: file name only, the reason and the way out.
pub(crate) fn refusal_message(path: Option<&std::path::Path>, format_token: &str) -> String {
    let name = path
        .and_then(|p| p.file_name())
        .map_or_else(|| "This sheet".into(), |n| n.to_string_lossy());
    format!(
        "Not changed: {name} is read-only, written by a newer Signex ({format_token}). Edit it in that version."
    )
}

impl Signex {
    /// `Some(reason)` when the active sheet is read-only, so the caller must
    /// not run its command. The line is logged once per opened engine
    /// (`instance_id`), so a drag of many events does not flood Messages; a
    /// reopen makes a new engine and re-arms it.
    pub(crate) fn refuse_if_active_read_only(&mut self) -> Option<String> {
        let engine = self.document_state.active_engine()?;
        let document = engine.document();
        if !document.is_read_only() {
            return None;
        }
        let token = document.unknown.newer_minor_source.as_deref().unwrap_or("");
        let message = refusal_message(self.document_state.active_path.as_deref(), token);
        let instance = engine.instance_id();
        if self.document_state.read_only_refusal_noted.insert(instance) {
            crate::diagnostics::log_warning(&message);
        }
        Some(message)
    }
}

#[cfg(test)]
mod tests {
    use super::refusal_message;
    use crate::app::handlers::menu::export::tests as fx;
    use crate::app::{Message, Signex};
    use signex_types::schematic::{SchematicSheet, SelectedItem, SelectedKind};

    #[test]
    fn message_names_the_file_not_the_path_and_stays_short() {
        let path = std::path::Path::new("/Users/someone/projects/board/power.snxsch");
        let text = refusal_message(Some(path), "snxsch/1.7");
        assert!(text.contains("power.snxsch"), "{text}");
        assert!(!text.contains("/Users"), "{text}");
        assert!(text.len() < 160, "{} bytes", text.len());
    }

    fn read_only(mut sheet: SchematicSheet) -> SchematicSheet {
        sheet.unknown.newer_minor_source = Some("snxsch/1.7".into());
        sheet
    }

    fn top() -> std::path::PathBuf {
        std::path::PathBuf::from("/w/ro/top.snxsch")
    }

    /// A workspace with `top.snxsch` open and active as a clean tab, a wire
    /// selected on the main canvas, holding `sheet`.
    fn app_on(sheet: SchematicSheet) -> (Signex, iced::window::Id) {
        let mut app = fx::app_workspace("/w/ro", &["top.snxsch"]);
        let wire = SelectedItem::new(sheet.wires[0].uuid, SelectedKind::Wire);
        fx::open_with(&mut app.document_state, &top(), sheet.clone());
        app.document_state.tabs.push(crate::app::TabInfo {
            title: "top".to_string(),
            path: top(),
            cached_document: None,
            project_id: None,
            kind: crate::app::TabKind::Schematic,
        });
        app.document_state.active_tab = 0;
        app.document_state.active_path = Some(top());
        let main = iced::window::Id::unique();
        app.ui_state.main_window_id = Some(main);
        app.interaction_state.canvas.set_render_cache(Some(
            crate::schematic_runtime::SchematicRenderCache::from_sheet(&sheet),
        ));
        app.interaction_state.canvas.selected = vec![wire];
        (app, main)
    }

    fn wire_sheet() -> SchematicSheet {
        fx::sheet_with_net("R_TOP", "TOP_NET", &[])
    }

    fn drag(app: &mut Signex, window_id: iced::window::Id) {
        let event = crate::canvas::CanvasEvent::MoveSelected {
            dx: 10.0,
            dy: 0.0,
            connections: Default::default(),
        };
        let _task = app.dispatch_update(Message::CanvasEventInWindow { window_id, event });
    }

    fn wire_end(app: &Signex) -> f64 {
        app.document_state.active_engine().unwrap().document().wires[0]
            .end
            .x
    }

    fn is_dirty(app: &Signex) -> bool {
        app.document_state.has_dirty_documents() || app.document_state.is_document_dirty(&top())
    }

    #[test]
    fn edit_on_read_only_sheet_is_refused_and_tab_stays_clean() {
        let (mut app, main) = app_on(read_only(wire_sheet()));
        let before = wire_end(&app);
        drag(&mut app, main);
        assert_eq!(wire_end(&app), before, "the document must not change");
        assert!(!is_dirty(&app), "a refused edit must not dirty the tab");
        assert!(
            app.document_state
                .active_engine()
                .unwrap()
                .document()
                .is_read_only()
        );

        // Control: the same drag on a writable sheet applies.
        let (mut app, main) = app_on(wire_sheet());
        let before = wire_end(&app);
        drag(&mut app, main);
        assert_ne!(wire_end(&app), before);
        assert!(is_dirty(&app));
    }

    #[test]
    fn refusal_is_reported_once_per_open() {
        let (mut app, main) = app_on(read_only(wire_sheet()));
        assert!(app.document_state.read_only_refusal_noted.is_empty());
        drag(&mut app, main);
        drag(&mut app, main);
        let first = app.document_state.active_engine().unwrap().instance_id();
        assert_eq!(
            app.document_state.read_only_refusal_noted.len(),
            1,
            "two refusals, one open, one report"
        );
        assert!(app.document_state.read_only_refusal_noted.contains(&first));

        // Reopening builds a new engine, which re-arms the report.
        fx::open_with(&mut app.document_state, &top(), read_only(wire_sheet()));
        drag(&mut app, main);
        assert_eq!(app.document_state.read_only_refusal_noted.len(), 2);
    }

    #[test]
    fn net_colour_on_read_only_sheet_is_refused() {
        let sheet = read_only(wire_sheet());
        let wire = sheet.wires[0].uuid;
        let (mut app, _main) = app_on(sheet);
        assert!(!app.record_net_color_undo());
        assert!(!is_dirty(&app), "no colour step, no dirty mark");
        assert!(app.ui_state.net_color_undo.is_empty());
        assert!(!app.ui_state.wire_color_overrides.contains_key(&wire));
    }

    #[test]
    fn clear_all_net_colors_keeps_read_only_sheet_colours() {
        let sheet = read_only(wire_sheet());
        let wire = sheet.wires[0].uuid;
        let (mut app, _main) = app_on(sheet);
        let colour = signex_types::theme::Color::new(9, 0, 0, 255);
        app.ui_state.wire_color_overrides.insert(wire, colour);
        app.clear_all_net_colors_undoably();
        assert_eq!(app.ui_state.wire_color_overrides.get(&wire), Some(&colour));
        assert!(app.ui_state.net_color_undo.is_empty());
        assert!(!is_dirty(&app));
    }
}
