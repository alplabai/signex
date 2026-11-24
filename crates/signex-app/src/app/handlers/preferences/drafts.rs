//! The Preferences drafts: which page each draft belongs to (the one answer
//! to "is there unsaved work, and where"), and the `PrefMsg::Draft*` and
//! component-class arms that edit them. Split out of `mod.rs`.

use iced::Task;

use super::super::super::*;
use crate::app::state::UiState;
use crate::preferences::{PrefMsg, PrefNav};

impl UiState {
    /// Whether `page` holds a draft that differs from its saved value. Every
    /// draft belongs to exactly one page (mockup row 143); a new draft must be added
    /// here or the dialog will not count it as unsaved.
    ///
    /// The three Symbol Editor terms joined in #629; before that they wrote
    /// themselves to `prefs.json` on change.
    pub fn preferences_page_differs(&self, page: PrefNav) -> bool {
        match page {
            PrefNav::Appearance => {
                self.preferences_draft_theme != self.theme_id
                    || self.preferences_draft_font != self.ui_font_name
                    || self.preferences_draft_grid_style != self.grid_style
                    || self.preferences_draft_symbol_grid_style != self.symbol_grid_style
                    // Exact `!=` on the f32 is deliberate: the picker assigns
                    // from `canvas::grid::GRID_SIZES_MM`, so draft and
                    // committed are the same constant unless the user moved
                    // it. A tolerance would let a real change read as clean.
                    || self.preferences_draft_symbol_grid_size_mm != self.symbol_grid_size_mm
                    || self.preferences_draft_symbol_pin_selection != self.symbol_pin_selection
                    || self.preferences_draft_pcb_gpu_render != self.pcb_gpu_render
                    || self.preferences_draft_antialiasing != self.antialiasing
                    || self.preferences_draft_ui_scale_percent
                        != crate::ui_scale::scale_to_percent(self.ui_scale)
                    || self.preferences_draft_wheel_mode != self.wheel_mode
                    || self.preferences_draft_layer_switcher != self.layer_switcher
                    || self.preferences_draft_selected_segment != self.selected_segment
                    || self.preferences_draft_trackpad_pan != self.trackpad_pan
                    || self.preferences_draft_zoom_step_percent != self.zoom_step_percent
                    || self.preferences_draft_window_controls_placement
                        != self.window_controls_placement
                    || self.preferences_draft_remember_window_geometry
                        != self.remember_window_geometry
            }
            PrefNav::Language => self.preferences_pages.draft_language != *self.i18n.pref(),
            PrefNav::Keyboard => self
                .preferences_keymap_editor
                .differs_from(&self.keymap_profiles),
            PrefNav::Annotation => self.preferences_draft_annotate_undo != self.annotate_undo,
            PrefNav::Editing => {
                self.preferences_draft_move_keeps_wires != self.move_keeps_wires
                    || self.preferences_draft_drag_direction_select != self.drag_direction_select
                    || self.preferences_draft_net_hover_enabled != self.net_hover_enabled
                    || self.preferences_draft_edge_pan != self.edge_pan
                    || self.preferences_draft_increment_step != self.increment_step
                    || self.preferences_draft_paste_designators != self.paste_designators
                    || self.preferences_draft_autosave_enabled != self.autosave_enabled
                    || self.preferences_draft_agent_connections_enabled
                        != self.agent_connections_enabled
                    || self.fast_grid_drafts_differ()
            }
            PrefNav::SchematicDisplay => {
                self.preferences_draft_symbol_body_fill != self.symbol_body_fill
            }
            PrefNav::PcbDefaults => self
                .preferences_pcb_defaults
                .differs_from(&self.pcb_dimension_defaults),
            // Rule profiles save to their files at once, and distributor
            // settings live in the library state: neither has drafts.
            PrefNav::RuleProfiles | PrefNav::LibraryDistributors => false,
            PrefNav::ComponentClasses => {
                self.preferences_draft_component_classes != self.component_classes
            }
        }
    }

    /// The pages with unsaved drafts, in nav order (the nav's dots).
    pub fn preferences_unsaved_pages(&self) -> Vec<PrefNav> {
        PrefNav::ALL
            .iter()
            .copied()
            .filter(|&page| self.preferences_page_differs(page))
            .collect()
    }

    /// True when any Preferences draft differs from its saved live value:
    /// the single predicate behind every `PrefMsg::Draft*` handler, the
    /// footer and every dirty-close guard, so an appearance recompute cannot
    /// report "clean" while a pending rebind or class edit would be lost.
    pub fn preferences_draft_differs(&self) -> bool {
        PrefNav::ALL
            .iter()
            .any(|&page| self.preferences_page_differs(page))
    }

    /// True when closing Preferences without Save would lose work — drives
    /// the Save/Discard footer + every dirty-close guard.
    pub fn preferences_has_unsaved_changes(&self) -> bool {
        self.preferences_draft_differs()
    }
}

impl Signex {
    /// Recompute the cached dirty flag and the pages it comes from. Every
    /// Preferences draft mutation routes through this — never assign
    /// `preferences_dirty` from an ad-hoc comparison; that drift class is
    /// exactly what let imperative edits be clobbered back to "clean"
    /// (review #308 finding 1).
    pub(crate) fn recompute_preferences_dirty(&mut self) {
        self.ui_state.preferences_pages.dirty_pages = self.ui_state.preferences_unsaved_pages();
        self.ui_state.preferences_dirty = !self.ui_state.preferences_pages.dirty_pages.is_empty();
    }

    /// Handle a `PrefMsg::Draft*` or component-class message.
    ///
    /// Only reachable for the variants the caller lists, so the fall-through
    /// arm is dead by construction; it reports rather than panicking, like
    /// the keymap handler.
    pub(super) fn handle_preferences_draft(&mut self, msg: PrefMsg) -> Task<Message> {
        match msg {
            PrefMsg::DraftTheme(id) => {
                self.ui_state.preferences_draft_theme = id;
                // Schematic canvas colours are derived from
                // `preferences_draft_theme` every frame (#631), but the PCB
                // canvases hold pushed colours and every canvas caches its
                // paper, grid and content. `sync_theme` recolours the PCB
                // canvases and drops the caches on every open canvas,
                // docked or undocked (#1749).
                self.sync_theme();
            }
            PrefMsg::DraftFont(name) => self.ui_state.preferences_draft_font = name,
            PrefMsg::DraftGridStyle(style) => {
                self.ui_state.preferences_draft_grid_style = style;
                // Live preview: the grid sits in the camera-keyed bg layer,
                // so drop that cache on every schematic canvas.
                for canvas in self.interaction_state.all_schematic_canvases_mut() {
                    canvas.clear_bg_cache();
                }
            }
            PrefMsg::DraftAnnotateUndo(scope) => {
                self.ui_state.preferences_draft_annotate_undo = scope;
            }
            PrefMsg::DraftAutosaveEnabled(enabled) => {
                self.ui_state.preferences_draft_autosave_enabled = enabled;
            }
            PrefMsg::DraftAgentConnectionsEnabled(enabled) => {
                self.ui_state.preferences_draft_agent_connections_enabled = enabled;
            }
            PrefMsg::DraftDragDirectionSelect(enabled) => {
                self.ui_state.preferences_draft_drag_direction_select = enabled;
            }
            PrefMsg::DraftNetHoverEnabled(enabled) => {
                self.ui_state.preferences_draft_net_hover_enabled = enabled;
            }
            PrefMsg::DraftEdgePanEnabled(enabled) => {
                self.ui_state.preferences_draft_edge_pan.enabled = enabled;
            }
            PrefMsg::DraftEdgePanSpeed(speed) => {
                self.ui_state.preferences_draft_edge_pan.speed = speed;
            }
            PrefMsg::SymbolBodyFill(msg) => self.ui_state.apply_body_fill_message(msg),
            PrefMsg::DraftTrackpadPan(enabled) => {
                self.ui_state.preferences_draft_trackpad_pan = enabled;
            }
            PrefMsg::DraftWheelMode(mode) => self.ui_state.preferences_draft_wheel_mode = mode,
            PrefMsg::DraftLayerSwitcher(switcher) => {
                self.ui_state.preferences_draft_layer_switcher = switcher;
            }
            PrefMsg::DraftSelectedSegment(look) => {
                self.ui_state.preferences_draft_selected_segment = look;
            }
            PrefMsg::DraftRememberWindowGeometry(enabled) => {
                self.ui_state.preferences_draft_remember_window_geometry = enabled;
            }
            PrefMsg::DraftIncrementStep(step) => {
                // Zero would stamp one text forever: it keeps the direction
                // the slider came from instead of landing on it.
                let previous = self.ui_state.preferences_draft_increment_step;
                self.ui_state.preferences_draft_increment_step = match step {
                    0 if previous < 0 => -1,
                    step => signex_types::text_increment::clamp_step(i64::from(step)),
                };
            }
            PrefMsg::DraftPasteDesignators(mode) => {
                self.ui_state.preferences_draft_paste_designators = mode;
            }
            PrefMsg::DraftMoveKeepsWires(enabled) => {
                self.ui_state.preferences_draft_move_keeps_wires = enabled;
            }
            PrefMsg::DraftWindowControlsPlacement(placement) => {
                self.ui_state.preferences_draft_window_controls_placement = placement;
            }
            PrefMsg::DraftAntialiasing(enabled) => {
                // No live preview: iced fixes it at start. Saved on Save,
                // read at the next launch.
                self.ui_state.preferences_draft_antialiasing = enabled;
            }
            PrefMsg::DraftPcbGpuRender(enabled) => {
                self.ui_state.preferences_draft_pcb_gpu_render = enabled;
                // Live preview: push the draft into the widget's effective
                // flag and repaint the PCB canvas so the toggle shows
                // immediately. Persisted only on Save; reverted on Discard.
                for canvas in self.interaction_state.all_pcb_canvases_mut() {
                    canvas.gpu_render = enabled;
                    canvas.clear_content_cache();
                    canvas.clear_bg_cache();
                }
            }
            // #629 — these three used to write `prefs.json` here, before the
            // user reached the footer. They are drafts like the rest now.
            PrefMsg::DraftSymbolGridSize(size) => {
                self.ui_state.preferences_draft_symbol_grid_size_mm = size;
            }
            PrefMsg::DraftFastGrid(index, text) => {
                if let Some(field) = self.ui_state.preferences_draft_fast_grids.get_mut(index) {
                    *field = text;
                }
            }
            PrefMsg::DraftSymbolGridStyle(style) => {
                self.ui_state.preferences_draft_symbol_grid_style = style;
                // Live preview only — `revert_preferences_drafts` pushes
                // the committed style back if the user discards.
                self.document_state.panel_ctx.symbol_grid_style = style;
            }
            PrefMsg::DraftZoomStepPercent(percent) => {
                // Out-of-range input is clamped, never stored raw.
                self.ui_state.preferences_draft_zoom_step_percent =
                    crate::canvas::Camera::clamp_zoom_step_percent(percent as i64);
            }
            PrefMsg::DraftUiScalePercent(percent) => {
                // Snapped to the 10 % steps and clamped, never stored raw.
                self.ui_state.preferences_draft_ui_scale_percent =
                    crate::ui_scale::snap_percent(percent);
            }
            PrefMsg::DraftSymbolPinSelection(mode) => {
                self.ui_state.preferences_draft_symbol_pin_selection = mode;
            }
            PrefMsg::DraftLanguage(pref) => {
                // A draft (mockup row 144a): only the preview catalog changes now; Save
                // switches the UI through `set_language`.
                self.ui_state.preferences_pages.draft_language = pref;
                self.refresh_language_preview();
            }
            PrefMsg::ComponentClassEditKey { index, key } => {
                let Some(entry) = self
                    .ui_state
                    .preferences_draft_component_classes
                    .get_mut(index)
                else {
                    return Task::none();
                };
                entry.key = key;
            }
            PrefMsg::ComponentClassEditLabel { index, label } => {
                let Some(entry) = self
                    .ui_state
                    .preferences_draft_component_classes
                    .get_mut(index)
                else {
                    return Task::none();
                };
                entry.label = label;
            }
            PrefMsg::ComponentClassAdd => {
                self.ui_state.preferences_draft_component_classes.push(
                    crate::fonts::ComponentClassEntry {
                        key: String::new(),
                        label: String::new(),
                    },
                );
            }
            PrefMsg::ComponentClassRemove { index } => {
                if index >= self.ui_state.preferences_draft_component_classes.len() {
                    return Task::none();
                }
                self.ui_state
                    .preferences_draft_component_classes
                    .remove(index);
            }
            PrefMsg::ComponentClassResetDefaults => {
                self.ui_state.preferences_draft_component_classes =
                    crate::fonts::default_component_classes();
            }
            other => {
                tracing::error!(
                    target: "signex::prefs",
                    "a non-draft Preferences message reached the draft handler: {other:?}"
                );
                return Task::none();
            }
        }
        self.recompute_preferences_dirty();
        Task::none()
    }
}
