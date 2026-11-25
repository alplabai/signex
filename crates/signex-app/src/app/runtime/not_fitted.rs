//! The parts a canvas draws greyed and crossed out (#1294): marked DNP, or
//! not fitted in the project's active variant, while "Cross out not-fitted
//! parts" is on. Which parts is decided by
//! `signex_output::not_fitted_symbols`, the PDF and SVG export's own rule;
//! this file only feeds it the sheet, the shown placement and the project.

use std::collections::HashSet;
use std::path::Path;

use signex_types::project::VariantSettings;
use signex_types::schematic::InstancePath;

use super::super::*;

impl Signex {
    /// Re-resolve the active canvas's not-fitted parts, clearing its content
    /// cache only when they changed. Cheap while nothing they depend on moved.
    pub(crate) fn refresh_canvas_not_fitted(&mut self) {
        if self.canvas_not_fitted_current() {
            return;
        }
        let key = self.not_fitted_key();
        let not_fitted = match &key {
            Some((designators, _, _)) => self.not_fitted_on(&designators.0),
            None => HashSet::new(),
        };
        let canvas = self.interaction_state.active_canvas_mut();
        canvas.not_fitted_key = Some(key);
        if canvas.not_fitted != not_fitted {
            canvas.not_fitted = not_fitted;
            canvas.clear_content_cache();
        }
    }

    /// Redraw every undocked window's and unfocused tile pane's not-fitted
    /// parts; such a canvas is otherwise only refreshed while it is the
    /// active one.
    pub(crate) fn refresh_window_not_fitted(&mut self) {
        let mut paths: Vec<(Option<iced::window::Id>, std::path::PathBuf)> = self
            .ui_state
            .windows
            .iter()
            .filter_map(|(id, kind)| match kind {
                crate::app::state::WindowKind::UndockedTab { path, .. } => {
                    Some((Some(*id), path.clone()))
                }
                _ => None,
            })
            .collect();
        let panes = &self.interaction_state.tile_panes.schematic;
        paths.extend(panes.keys().map(|path| (None, path.clone())));
        for (window, path) in paths {
            let not_fitted = if self.ui_state.cross_out_not_fitted {
                self.not_fitted_on(&path)
            } else {
                HashSet::new()
            };
            let canvas = match window {
                Some(id) => self.interaction_state.canvases.get_mut(&id),
                None => self.interaction_state.tile_panes.schematic.get_mut(&path),
            };
            if let Some(canvas) = canvas
                && canvas.not_fitted != not_fitted
            {
                canvas.not_fitted = not_fitted;
                canvas.not_fitted_key = None;
                canvas.clear_content_cache();
            }
        }
    }

    /// The symbols of the sheet at `path` that are not fitted in its
    /// project's active variant, at the sheet's shown placement (the root
    /// chain when none is known). Empty when `path` has no engine.
    pub(crate) fn not_fitted_on(&self, path: &Path) -> HashSet<uuid::Uuid> {
        let Some(engine) = self.document_state.engines.get(path) else {
            return HashSet::new();
        };
        let placement = self
            .shown_placement(path)
            .unwrap_or_else(InstancePath::root);
        let project = self.document_state.document_project(path);
        let sheets_only = VariantSettings::default();
        signex_output::not_fitted_symbols(
            engine.document(),
            &placement,
            project.map(|p| p.data.active_variant_name()),
            project.map_or(&sheets_only, |p| &p.data.variant_settings),
        )
    }

    /// What the active canvas's not-fitted parts are a function of; `None`
    /// while the preference is off or no sheet is active.
    fn not_fitted_key(&self) -> crate::canvas::NotFittedKey {
        if !self.ui_state.cross_out_not_fitted {
            return None;
        }
        let designators = self.designators_key()?;
        let project = self.document_state.document_project(&designators.0);
        let variant = project.map_or(signex_types::project::BASE_VARIANT, |p| {
            p.data.active_variant_name()
        });
        let settings = project
            .map(|p| p.data.variant_settings.clone())
            .unwrap_or_default();
        Some((designators, variant.to_string(), settings))
    }

    /// Whether the active canvas's not-fitted parts were computed from what
    /// they depend on now, compared without copying the variant settings.
    fn canvas_not_fitted_current(&self) -> bool {
        let Some(stored) = &self.interaction_state.active_canvas().not_fitted_key else {
            return false;
        };
        let Some((designators, variant, settings)) = stored else {
            return !self.ui_state.cross_out_not_fitted;
        };
        if !self.ui_state.cross_out_not_fitted
            || self.designators_key().as_ref() != Some(designators)
        {
            return false;
        }
        let project = self.document_state.document_project(&designators.0);
        let now_variant = project.map_or(signex_types::project::BASE_VARIANT, |p| {
            p.data.active_variant_name()
        });
        let sheets_only = VariantSettings::default();
        let now_settings = project.map_or(&sheets_only, |p| &p.data.variant_settings);
        variant.as_str() == now_variant && now_settings == settings
    }
}
