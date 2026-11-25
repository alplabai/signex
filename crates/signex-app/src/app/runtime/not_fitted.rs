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
use crate::canvas::{CanvasSlot, NotFittedKey};

impl Signex {
    /// Re-resolve the not-fitted parts of every schematic canvas: the active
    /// one, the undocked windows and the unfocused tile panes. Each canvas
    /// recomputes only when what its parts depend on moved.
    pub(crate) fn refresh_all_not_fitted(&mut self) {
        self.refresh_canvas_not_fitted();
        self.refresh_window_not_fitted();
    }

    /// Re-resolve the active canvas's not-fitted parts, clearing its content
    /// cache only when they changed. Cheap while nothing they depend on moved.
    pub(crate) fn refresh_canvas_not_fitted(&mut self) {
        let path = self.document_state.active_path.clone();
        let stored = self
            .interaction_state
            .active_canvas()
            .not_fitted_key
            .as_ref();
        if stored.is_some_and(|key| self.not_fitted_key_matches(key, path.as_deref())) {
            return;
        }
        let key = path
            .as_deref()
            .and_then(|path| self.not_fitted_key_for(path));
        let not_fitted = self.not_fitted_of(&key);
        store_not_fitted(self.interaction_state.active_canvas_mut(), key, not_fitted);
    }

    /// Re-resolve every undocked window's and unfocused tile pane's
    /// not-fitted parts. Such a canvas is not the active one, so a variant
    /// switch or an edit of its sheet reaches it only from here.
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
            let stored = match window {
                Some(id) => self.interaction_state.canvases.get(&id),
                None => self.interaction_state.tile_panes.schematic.get(&path),
            }
            .and_then(|canvas| canvas.not_fitted_key.as_ref());
            if stored.is_some_and(|key| self.not_fitted_key_matches(key, Some(&path))) {
                continue;
            }
            let key = self.not_fitted_key_for(&path);
            let not_fitted = self.not_fitted_of(&key);
            let canvas = match window {
                Some(id) => self.interaction_state.canvases.get_mut(&id),
                None => self.interaction_state.tile_panes.schematic.get_mut(&path),
            };
            if let Some(canvas) = canvas {
                store_not_fitted(canvas, key, not_fitted);
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

    /// The parts `key` stands for; none while the preference is off.
    fn not_fitted_of(&self, key: &NotFittedKey) -> HashSet<uuid::Uuid> {
        match key {
            Some((sheet, _, _)) => self.not_fitted_on(&sheet.0),
            None => HashSet::new(),
        }
    }

    /// What the not-fitted parts of the sheet at `path` are a function of:
    /// the designators' inputs, the active variant's name and the project's
    /// variant settings. `None` while the preference is off.
    fn not_fitted_key_for(&self, path: &Path) -> NotFittedKey {
        if !self.ui_state.cross_out_not_fitted {
            return None;
        }
        let project = self.document_state.document_project(path);
        let variant = project.map_or(signex_types::project::BASE_VARIANT, |p| {
            p.data.active_variant_name()
        });
        let settings = project
            .map(|p| p.data.variant_settings.clone())
            .unwrap_or_default();
        Some((
            self.designators_key_for(path),
            variant.to_string(),
            settings,
        ))
    }

    /// Whether `stored`, computed for a canvas now showing `path`, still
    /// holds; compared without copying the variant settings.
    fn not_fitted_key_matches(&self, stored: &NotFittedKey, path: Option<&Path>) -> bool {
        let Some((sheet, variant, settings)) = stored else {
            return !self.ui_state.cross_out_not_fitted;
        };
        let Some(path) = path else {
            return false;
        };
        if !self.ui_state.cross_out_not_fitted || self.designators_key_for(path) != *sheet {
            return false;
        }
        let project = self.document_state.document_project(path);
        let now_variant = project.map_or(signex_types::project::BASE_VARIANT, |p| {
            p.data.active_variant_name()
        });
        let sheets_only = VariantSettings::default();
        let now_settings = project.map_or(&sheets_only, |p| &p.data.variant_settings);
        variant.as_str() == now_variant && now_settings == settings
    }
}

/// Record `key` on `canvas` and redraw it when its not-fitted parts changed.
fn store_not_fitted(canvas: &mut CanvasSlot, key: NotFittedKey, not_fitted: HashSet<uuid::Uuid>) {
    canvas.not_fitted_key = Some(key);
    if canvas.not_fitted != not_fitted {
        canvas.not_fitted = not_fitted;
        canvas.clear_content_cache();
    }
}
