//! Where-Used navigation: open the sheet that places a library component
//! and bring the placed symbol into view, selected.

use iced::Task;
use signex_types::schematic::{SelectedItem, SelectedKind};

use super::super::*;

impl Signex {
    /// Jump to `site`: open its sheet, select the symbol whose designator is
    /// the site's instance id and fit the view around it. A sheet that no
    /// longer opens, or a symbol that is no longer on it, is reported on the
    /// Messages panel instead of silently doing nothing.
    pub(super) fn handle_jump_to_use_site(
        &mut self,
        site: signex_library::UseSite,
    ) -> Task<Message> {
        self.ensure_sheet_open_and_active(&site.sheet_path);

        let on_target_sheet = self
            .active_tab_path()
            .is_some_and(|active| active == self.resolve_held_spelling(site.sheet_path.clone()));
        if !on_target_sheet {
            crate::diagnostics::log_warning(format!(
                "Where-Used: could not open sheet {} to show {}.",
                site.sheet_path.display(),
                site.instance_id
            ));
            return Task::none();
        }

        let found = self.active_render_snapshot().and_then(|snapshot| {
            snapshot
                .symbols
                .iter()
                .find(|symbol| self.shown_designator(symbol) == site.instance_id)
                .map(|symbol| (symbol.uuid, symbol.position))
        });
        let Some((uuid, position)) = found else {
            crate::diagnostics::log_warning(format!(
                "Where-Used: {} is no longer placed on {}.",
                site.instance_id,
                site.sheet_path.display()
            ));
            return Task::none();
        };
        self.handle_focus_at(
            position.x,
            position.y,
            Some(SelectedItem::new(uuid, SelectedKind::Symbol)),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use signex_library::UseSite;
    use signex_types::schematic::{Point, SchematicSheet, SelectedKind, Symbol};

    use crate::app::Signex;
    use crate::app::handlers::menu::export::tests::app_workspace;

    fn site(sheet: &Path, instance: &str) -> UseSite {
        UseSite {
            project_path: PathBuf::from("/w/proj/p.snxprj"),
            sheet_path: sheet.to_path_buf(),
            instance_id: instance.to_string(),
        }
    }

    fn app_with_r1_at(x: f64, y: f64) -> (Signex, PathBuf, uuid::Uuid) {
        let mut app = app_workspace("/w/proj", &[]);
        let id = uuid::Uuid::new_v4();
        let path = PathBuf::from("/w/proj/a.snxsch");
        let sheet = SchematicSheet {
            uuid: uuid::Uuid::new_v4(),
            symbols: vec![Symbol {
                uuid: id,
                lib_id: "Device:R".into(),
                reference: "R1".into(),
                position: Point::new(x, y),
                ..Default::default()
            }],
            ..Default::default()
        };
        app.open_schematic_tab(path.clone(), "a".into(), sheet);
        (app, path, id)
    }

    #[test]
    fn jump_selects_the_placed_symbol_and_fits_the_view() {
        let (mut app, path, id) = app_with_r1_at(50.0, 40.0);
        app.interaction_state.canvas.pending_fit.set(None);
        app.interaction_state.active_canvas_mut().selected.clear();

        let _ = app.handle_jump_to_use_site(site(&path, "R1"));

        let selected = &app.interaction_state.active_canvas().selected;
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].uuid, id);
        assert_eq!(selected[0].kind, SelectedKind::Symbol);
        let fit = app.interaction_state.canvas.pending_fit.get().expect("fit");
        assert!(fit.x < 50.0 && 50.0 < fit.x + fit.width);
        assert!(fit.y < 40.0 && 40.0 < fit.y + fit.height);
    }

    #[test]
    fn jump_to_a_missing_instance_selects_nothing() {
        let (mut app, path, _) = app_with_r1_at(50.0, 40.0);
        app.interaction_state.canvas.pending_fit.set(None);
        app.interaction_state.active_canvas_mut().selected.clear();

        let _ = app.handle_jump_to_use_site(site(&path, "R99"));

        assert!(app.interaction_state.active_canvas().selected.is_empty());
        assert!(app.interaction_state.canvas.pending_fit.get().is_none());
    }
}
