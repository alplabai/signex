//! Footprint-editor silkscreen graphic handlers — the methods behind
//! the `FpEditor*` dock-panel messages that edit the selected
//! silk-front graphic (Line endpoints, Text position / size / content,
//! stroke width, filled toggle, delete) on the active `.snxfpt`
//! editor. Owns the `SilkLineEndpoint` / `SilkTextField` selector enums
//! the endpoint / field setters take. The dispatcher in `mod.rs` routes
//! these panel messages here.
//!
//! Pure code motion out of the former `sch_library.rs` god-file
//! (ADR-0001 #163); zero behaviour change.

use super::super::*;

/// v0.21 — Silk Line endpoint to mutate.
#[derive(Debug, Clone, Copy)]
pub(crate) enum SilkLineEndpoint {
    FromX,
    FromY,
    ToX,
    ToY,
}

/// v0.21 — Silk Text field to mutate.
#[derive(Debug, Clone, Copy)]
pub(crate) enum SilkTextField {
    PositionX,
    PositionY,
    Size,
}

impl Signex {
    pub(crate) fn fp_editor_set_silk_line_endpoint(
        &mut self,
        endpoint: SilkLineEndpoint,
        value: String,
    ) -> bool {
        let parsed = value.trim().parse::<f64>().ok();
        if let Some(parsed) = parsed {
            if let Some(editor) = self.active_footprint_editor_mut() {
                if let Some(idx) = editor.state.selected_silk_f {
                    if let Some(g) = editor.primitive_mut().silk_f.get_mut(idx) {
                        if let signex_library::primitive::footprint::FpGraphicKind::Line {
                            from,
                            to,
                        } = &mut g.kind
                        {
                            match endpoint {
                                SilkLineEndpoint::FromX => from[0] = parsed,
                                SilkLineEndpoint::FromY => from[1] = parsed,
                                SilkLineEndpoint::ToX => to[0] = parsed,
                                SilkLineEndpoint::ToY => to[1] = parsed,
                            }
                            editor.dirty = true;
                            editor.canvas_cache.clear();
                        }
                    }
                }
            }
        }
        self.refresh_panel_ctx();
        true
    }

    pub(crate) fn fp_editor_set_silk_text_field(
        &mut self,
        field: SilkTextField,
        value: String,
    ) -> bool {
        let parsed = value.trim().parse::<f64>().ok();
        if let Some(parsed) = parsed {
            if let Some(editor) = self.active_footprint_editor_mut() {
                if let Some(idx) = editor.state.selected_silk_f {
                    if let Some(g) = editor.primitive_mut().silk_f.get_mut(idx) {
                        if let signex_library::primitive::footprint::FpGraphicKind::Text {
                            position,
                            size,
                            ..
                        } = &mut g.kind
                        {
                            match field {
                                SilkTextField::PositionX => position[0] = parsed,
                                SilkTextField::PositionY => position[1] = parsed,
                                SilkTextField::Size => {
                                    if parsed > 0.0 {
                                        *size = parsed;
                                    }
                                }
                            }
                            editor.dirty = true;
                            editor.canvas_cache.clear();
                        }
                    }
                }
            }
        }
        self.refresh_panel_ctx();
        true
    }

    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_set_silk_stroke_width(
        &mut self,
        v: &str,
    ) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            let parsed = v.trim().parse::<f64>().ok();
            if let Some(idx) = editor.state.selected_silk_f {
                if let Some(g) = editor.primitive_mut().silk_f.get_mut(idx) {
                    if let Some(w) = parsed {
                        if w >= 0.0 {
                            g.stroke_width = w;
                            editor.dirty = true;
                            editor.canvas_cache.clear();
                        }
                    }
                }
            }
        }
        self.refresh_panel_ctx();
        true
    }

    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_toggle_silk_filled(
        &mut self,
        on: &bool,
    ) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            if let Some(idx) = editor.state.selected_silk_f {
                if let Some(g) = editor.primitive_mut().silk_f.get_mut(idx) {
                    g.filled = *on;
                    editor.dirty = true;
                    editor.canvas_cache.clear();
                }
            }
        }
        self.refresh_panel_ctx();
        true
    }

    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_set_silk_text(
        &mut self,
        value: &str,
    ) -> bool {
        // v0.18.24 — edit the selected silk-front graphic's
        // Text content. No-op when the selection isn't a
        // Text or no silk graphic is selected.
        let value = value.to_string();
        if let Some(editor) = self.active_footprint_editor_mut() {
            if let Some(idx) = editor.state.selected_silk_f {
                editor.with_parts(|_state, primitive| {
                    use signex_library::primitive::footprint::FpGraphicKind;
                    if let Some(g) = primitive.silk_f.get_mut(idx) {
                        if let FpGraphicKind::Text { content, .. } = &mut g.kind {
                            *content = value;
                        }
                    }
                });
                editor.dirty = true;
                editor.canvas_cache.clear();
            }
        }
        self.refresh_panel_ctx();
        true
    }

    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_delete_selected_silk(
        &mut self,
    ) -> bool {
        // v0.18.24 — delete the currently-selected silk-front
        // graphic and clear the selection.
        if let Some(editor) = self.active_footprint_editor_mut() {
            if let Some(idx) = editor.state.selected_silk_f {
                editor.with_parts(|state, primitive| {
                    if idx < primitive.silk_f.len() {
                        primitive.silk_f.remove(idx);
                        // HI-25: keep the selection cursor consistent
                        // with the new vec length / shifted indices.
                        state.selected_silk_f =
                            crate::library::editor::footprint::state::adjust_selection_after_remove(
                                state.selected_silk_f,
                                idx,
                            );
                    } else {
                        state.selected_silk_f = None;
                    }
                });
                editor.dirty = true;
                editor.canvas_cache.clear();
            }
        }
        self.refresh_panel_ctx();
        true
    }
}
