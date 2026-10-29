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
        let before: std::collections::HashSet<uuid::Uuid> =
            engine.document().wires.iter().map(|w| w.uuid).collect();
        // A locked wire is refused with its own notice by the gateway.
        self.apply_engine_command(signex_engine::Command::BreakWire { wire, at }, true, true);
        self.paint_new_half_like(wire, &before);
    }

    /// The half a break adds keeps the net colour of the wire it came from
    /// (colours are kept per wire uuid, so the new uuid has none yet). An
    /// engine undo drops the half and leaves this entry unused; a redo
    /// brings the same uuid back, painted.
    fn paint_new_half_like(
        &mut self,
        wire: uuid::Uuid,
        before: &std::collections::HashSet<uuid::Uuid>,
    ) {
        let Some(color) = self.ui_state.wire_color_overrides.get(&wire).copied() else {
            return;
        };
        let Some(engine) = self.document_state.active_engine() else {
            return;
        };
        let added: Vec<uuid::Uuid> = engine
            .document()
            .wires
            .iter()
            .map(|w| w.uuid)
            .filter(|id| !before.contains(id))
            .collect();
        // A refused or no-op break adds nothing: leave the colours and the
        // tab's dirty state alone.
        if added.is_empty() {
            return;
        }
        for id in added {
            self.ui_state.wire_color_overrides.insert(id, color);
        }
        if let Some(path) = self.document_state.active_path.clone() {
            self.mark_net_colors_dirty(&path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::{SchematicSheet, Wire};

    fn app_with_wire() -> (Signex, uuid::Uuid) {
        app_with_wire_at(4321.0)
    }

    /// `y` keeps each test's log lines distinct: the diagnostics ring is shared.
    fn app_with_wire_at(y: f64) -> (Signex, uuid::Uuid) {
        let _ = crate::diagnostics::init_logging();
        let id = uuid::Uuid::new_v4();
        let sheet = SchematicSheet {
            uuid: uuid::Uuid::new_v4(),
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

    #[test]
    fn a_refused_break_of_a_coloured_wire_leaves_the_colours_and_revision_alone() {
        let (mut app, id) = app_with_wire_at(4322.0);
        let red = signex_types::theme::Color::new(255, 0, 0, 255);
        app.ui_state.wire_color_overrides.insert(id, red);
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
        let revisions = app.ui_state.net_color_revision.clone();

        app.break_wire_click(8.0, 4322.0);

        assert_eq!(wire_count(&app), 1);
        assert_eq!(app.ui_state.wire_color_overrides.len(), 1);
        assert_eq!(app.ui_state.net_color_revision, revisions);
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
