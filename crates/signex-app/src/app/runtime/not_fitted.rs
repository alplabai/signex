//! The parts a canvas draws greyed and crossed out (#1294): marked DNP, or
//! not fitted in the project's active variant, while "Cross out not-fitted
//! parts" is on. Which parts is decided by `signex_output::PlacedFits`, the
//! PDF and SVG export's own rule: across the project's open sheets, so a
//! part whose units sit on several sheets reads as the BOM reads it. A
//! repeated sheet shows every channel at once, so a part is crossed out only
//! when no channel fits it (#2662). This file only feeds it the sheets,
//! their placements and the project.

use std::borrow::Cow;
use std::collections::HashSet;
use std::path::Path;

use signex_net::ChannelDesignators;
use signex_types::project::VariantSettings;
use signex_types::schematic::{InstancePath, SchematicSheet};

use super::super::*;
use crate::canvas::{CanvasSlot, FitSheetKey, NotFittedKey};

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

    /// The parts `key` stands for; none while the preference is off.
    fn not_fitted_of(&self, key: &NotFittedKey) -> HashSet<uuid::Uuid> {
        match key {
            Some((sheet, sheets, _, _)) => self.not_fitted_across(&sheet.0, sheets),
            None => HashSet::new(),
        }
    }

    /// The symbols of the sheet at `path` not fitted in its project's active
    /// variant at every placement its shown placement (the root chain when
    /// none is known) compiles to, the fit decided across `sheets`. Empty
    /// when `path` has no engine.
    fn not_fitted_across(&self, path: &Path, sheets: &[FitSheetKey]) -> HashSet<uuid::Uuid> {
        let Some(engine) = self.document_state.engines.get(path) else {
            return HashSet::new();
        };
        let project = self.document_state.document_project(path);
        let sheets_only = VariantSettings::default();
        let settings = project.map_or(&sheets_only, |p| &p.data.variant_settings);
        let variant = project.map(|p| p.data.active_variant_name());
        let naming = project
            .map(|p| p.data.schematic_design.channel_naming.clone())
            .unwrap_or_default();
        let symbols = self.walk_sheet_symbols(path);
        let mut copies: Vec<(Cow<'_, SchematicSheet>, InstancePath)> = Vec::new();
        for (sheet_path, _, placements) in sheets {
            let Some(sheet) = self.document_state.engines.get(sheet_path) else {
                continue;
            };
            for placement in placements {
                let placed = symbols.iter().map(|(uuid, name)| (*uuid, name.as_str()));
                let channels = ChannelDesignators::of_placements(placement, placed, &naming);
                let copy = if channels.has_channels() {
                    // Only the symbols decide a fit; the rest is not copied.
                    let mut stamped = SchematicSheet {
                        symbols: sheet.document().symbols.clone(),
                        ..SchematicSheet::default()
                    };
                    channels.stamp(&mut stamped, placement);
                    Cow::Owned(stamped)
                } else {
                    Cow::Borrowed(sheet.document())
                };
                copies.push((copy, placement.clone()));
            }
        }
        let placed: Vec<(&SchematicSheet, InstancePath)> = copies
            .iter()
            .map(|(sheet, placement)| (&**sheet, placement.clone()))
            .collect();
        let fits = signex_output::PlacedFits::of(&placed, variant, settings);
        let shown = self.compiled_shown_placements(path, &symbols);
        fits.not_fitted_in_every(engine.document(), &shown)
    }

    /// The sheet symbols (uuid, designator) of the hierarchy walk describing
    /// `path`; empty when none is known.
    fn walk_sheet_symbols(&self, path: &Path) -> Vec<(uuid::Uuid, String)> {
        self.hierarchy_describing(path)
            .map(|(hierarchy, _)| {
                hierarchy
                    .sheet_symbols()
                    .into_iter()
                    .map(|(uuid, name)| (uuid, name.to_string()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Every placement the shown placement of `path` compiles to: each
    /// channel of a repeated sheet, else the shown placement itself.
    fn compiled_shown_placements(
        &self,
        path: &Path,
        symbols: &[(uuid::Uuid, String)],
    ) -> Vec<InstancePath> {
        let shown = self
            .shown_placement(path)
            .unwrap_or_else(InstancePath::root);
        let placed = symbols.iter().map(|(uuid, name)| (*uuid, name.as_str()));
        ChannelDesignators::compiled_paths(&shown, placed)
    }

    /// Every open sheet the walk describing `path` reaches, with the
    /// placements it compiles to (a repeated sheet's channels), in path
    /// order: the sheets a part's fit is decided across. `path` itself at
    /// its shown placement when the walk does not reach it. A sheet that is
    /// not open is not counted (#2662, see `docs/MULTI_CHANNEL.md`).
    fn fit_sheets(&self, path: &Path) -> Vec<FitSheetKey> {
        let engine_key = |at: &Path| {
            self.document_state
                .engines
                .get(at)
                .map(|engine| (engine.instance_id(), engine.revision()))
        };
        let symbols = self.walk_sheet_symbols(path);
        let mut out: Vec<FitSheetKey> = Vec::new();
        if let Some((hierarchy, _)) = self.hierarchy_describing(path) {
            for open in self.document_state.engines.keys() {
                let placements: Vec<InstancePath> = hierarchy
                    .placements_of(open)
                    .iter()
                    .flat_map(|logical| {
                        let placed = symbols.iter().map(|(uuid, name)| (*uuid, name.as_str()));
                        ChannelDesignators::compiled_paths(logical, placed)
                    })
                    .collect();
                if !placements.is_empty() {
                    out.push((open.clone(), engine_key(open.as_path()), placements));
                }
            }
        }
        if !out.iter().any(|(at, ..)| at == path) {
            let shown = self.compiled_shown_placements(path, &symbols);
            out.push((path.to_path_buf(), engine_key(path), shown));
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    /// What the not-fitted parts of the sheet at `path` are a function of:
    /// the designators' inputs, the sheets the fit is decided across, the
    /// active variant's name and the project's variant settings. `None`
    /// while the preference is off.
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
            self.fit_sheets(path),
            variant.to_string(),
            settings,
        ))
    }

    /// Whether `stored`, computed for a canvas now showing `path`, still
    /// holds; compared without copying the variant settings.
    fn not_fitted_key_matches(&self, stored: &NotFittedKey, path: Option<&Path>) -> bool {
        let Some((sheet, sheets, variant, settings)) = stored else {
            return !self.ui_state.cross_out_not_fitted;
        };
        let Some(path) = path else {
            return false;
        };
        if !self.ui_state.cross_out_not_fitted
            || self.designators_key_for(path) != *sheet
            || self.fit_sheets(path) != *sheets
        {
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
