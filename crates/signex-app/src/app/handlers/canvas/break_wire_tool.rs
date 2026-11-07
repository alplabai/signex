//! Break Wire tool (#1254): one click on a wire splits it in two there.

use signex_types::schematic::Point;

use super::super::super::*;

impl Signex {
    /// One snapped canvas click while `Tool::BreakWire` is active. A click on
    /// a wire's interior cuts it (one undo step); a click anywhere else, or at
    /// a wire's end, cuts nothing and says so in the Messages panel. The tool
    /// stays armed for the next cut.
    pub(super) fn break_wire_click(&mut self, world_x: f64, world_y: f64) {
        let at = Point::new(world_x, world_y);
        let Some(engine) = self.document_state.active_engine() else {
            return;
        };
        let Some(wire) = engine.wire_to_break_at(at) else {
            crate::diagnostics::log_info(
                "Break Wire: click on a wire, away from its ends, to split it there",
            );
            return;
        };
        // A locked wire is refused with its own notice by the gateway. The
        // engine paints the new half like the wire it was cut from.
        self.apply_engine_command(signex_engine::Command::BreakWire { wire, at }, true, true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::handlers::editing_commands::net_color_driver::paint;
    use signex_types::schematic::{SchematicSheet, Wire};

    fn app_with_wire() -> (Signex, uuid::Uuid) {
        app_with_wire_at(4321.0)
    }

    /// `y` keeps each test's log lines distinct: the diagnostics ring is shared.
    fn app_with_wire_at(y: f64) -> (Signex, uuid::Uuid) {
        let _ = crate::diagnostics::init_logging();
        let id = uuid::Uuid::now_v7();
        let sheet = SchematicSheet {
            uuid: uuid::Uuid::now_v7(),
            wires: vec![Wire {
                uuid: id,
                start: Point::new(0.0, y),
                end: Point::new(20.0, y),
                ..Default::default()
            }],
            ..Default::default()
        };
        let path = std::path::PathBuf::from("break-wire.snxsch");
        let (mut app, _task) = Signex::new();
        app.document_state.engines.insert(
            path.clone(),
            signex_engine::Engine::new(sheet).expect("engine"),
        );
        app.document_state.tabs.push(TabInfo {
            title: "break-wire".to_string(),
            path: path.clone(),
            cached_document: None,
            project_id: None,
            kind: TabKind::Schematic,
        });
        app.document_state.active_tab = 0;
        app.document_state.active_path = Some(path);
        app.interaction_state.current_tool = Tool::BreakWire;
        (app, id)
    }

    fn wire_count(app: &Signex) -> usize {
        app.document_state
            .active_engine()
            .expect("engine")
            .document()
            .wires
            .len()
    }

    fn logged(needle: &str) -> usize {
        crate::diagnostics::recent_entries()
            .iter()
            .filter(|entry| entry.message.contains(needle))
            .count()
    }

    #[test]
    fn a_click_on_a_wire_splits_it_and_one_undo_joins_it_again() {
        let (mut app, _) = app_with_wire();
        app.break_wire_click(8.0, 4321.0);
        assert_eq!(wire_count(&app), 2);

        let engine = app.document_state.active_engine_mut().expect("engine");
        assert!(engine.undo().is_ok_and(|undone| undone.is_some()));
        assert_eq!(wire_count(&app), 1);
    }

    #[test]
    fn a_click_off_every_wire_does_nothing_and_says_so() {
        let (mut app, _) = app_with_wire();
        let needle = "Break Wire: click on a wire";
        let before = logged(needle);

        app.break_wire_click(8.0, 4330.0);

        assert_eq!(wire_count(&app), 1);
        assert_eq!(logged(needle), before + 1);
    }

    fn painted_wires(app: &Signex) -> usize {
        let document = app
            .document_state
            .active_engine()
            .expect("engine")
            .document();
        document
            .wires
            .iter()
            .filter(|w| document.net_colors.contains_key(&w.uuid))
            .count()
    }

    #[test]
    fn both_halves_of_a_broken_coloured_wire_keep_the_colour_through_undo_and_redo() {
        let (mut app, id) = app_with_wire_at(4323.0);
        let red = signex_types::theme::Color::new(255, 0, 0, 255);
        paint(&mut app, &[id], red);

        app.break_wire_click(8.0, 4323.0);
        assert_eq!((wire_count(&app), painted_wires(&app)), (2, 2));

        app.handle_undo_requested();
        assert_eq!((wire_count(&app), painted_wires(&app)), (1, 1));

        app.handle_redo_requested();
        assert_eq!((wire_count(&app), painted_wires(&app)), (2, 2));
    }

    #[test]
    fn undoing_a_break_back_to_the_save_point_reads_clean_on_a_coloured_sheet() {
        let (mut app, id) = app_with_wire_at(4324.0);
        let path = app.document_state.active_path.clone().expect("path");
        let red = signex_types::theme::Color::new(255, 0, 0, 255);
        paint(&mut app, &[id], red);
        app.document_state.mark_document_clean(&path);

        app.break_wire_click(8.0, 4324.0);
        assert!(app.document_state.is_document_dirty(&path), "the cut");

        app.handle_undo_requested();
        assert!(!app.document_state.is_document_dirty(&path), "back at save");
    }

    #[test]
    fn a_refused_break_of_a_coloured_wire_leaves_the_colours_and_revision_alone() {
        let (mut app, id) = app_with_wire_at(4322.0);
        let red = signex_types::theme::Color::new(255, 0, 0, 255);
        paint(&mut app, &[id], red);
        let engine = app.document_state.active_engine_mut().expect("engine");
        engine
            .execute(signex_engine::Command::SetItemLocked {
                item: signex_types::schematic::SelectedItem::new(
                    id,
                    signex_types::schematic::SelectedKind::Wire,
                ),
                locked: true,
            })
            .expect("lock");
        let revision = app
            .document_state
            .active_engine()
            .expect("engine")
            .revision();

        app.break_wire_click(8.0, 4322.0);

        let engine = app.document_state.active_engine().expect("engine");
        assert_eq!(wire_count(&app), 1);
        assert_eq!(engine.document().net_colors.len(), 1);
        assert_eq!(engine.revision(), revision);
    }

    #[test]
    fn a_click_on_a_locked_wire_is_refused_in_the_messages_panel() {
        let (mut app, id) = app_with_wire();
        let engine = app.document_state.active_engine_mut().expect("engine");
        engine
            .execute(signex_engine::Command::SetItemLocked {
                item: signex_types::schematic::SelectedItem::new(
                    id,
                    signex_types::schematic::SelectedKind::Wire,
                ),
                locked: true,
            })
            .expect("lock");
        let needle = "cannot be broken";
        let before = logged(needle);

        app.break_wire_click(8.0, 4321.0);

        assert_eq!(wire_count(&app), 1);
        // At least one: a parallel test refusing its own locked wire logs the
        // same line into the shared ring.
        assert!(logged(needle) > before);
    }
}
