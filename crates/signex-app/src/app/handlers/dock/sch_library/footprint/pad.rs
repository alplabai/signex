//! Footprint-editor active-tab accessors and pad-property setters —
//! the helper methods behind the `FpEditor*` pad-defaults / selected-
//! pad / sketch-pad dock-panel messages. They mutate `next_pad_defaults`
//! or the selected pad on the active `.snxfpt` editor and re-bake as
//! needed; the dispatcher in `mod.rs` routes the panel messages here.
//!
//! Text a pad field does not take is kept for the panel (`pad_input`).

use super::super::*;
use super::pad_input::optional_mm_edit;
use super::pad_trapezoid::size_fits_shape;
use crate::library::editor::footprint::pad_field;

impl Signex {
    /// v0.16.3 — sibling of [`Self::active_symbol_editor_mut`] for
    /// `.snxfpt` editor tabs. Drives the Properties-panel pad-defaults
    /// form so it can mutate `next_pad_defaults` without round-
    /// tripping through `LibraryMessage::PrimitiveEditorEvent`.
    /// Read-only sibling of [`active_footprint_editor_mut`].
    /// v0.18.11 — used by the Grid Properties modal open handler
    /// to seed the dialog buffers from the live snap step.
    pub(crate) fn active_footprint_editor(&self) -> Option<&crate::app::FootprintDocument> {
        // A board in sketch mode edits through the same editor (#2234).
        if let Some(editor) = self.board_sketch_editor() {
            return Some(editor);
        }
        let path = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .and_then(|t| match &t.kind {
                crate::app::TabKind::FootprintEditor(p) => Some(p.clone()),
                _ => None,
            })?;
        self.document_state.footprint_editors.get(&path)
    }

    pub(crate) fn active_footprint_editor_mut(
        &mut self,
    ) -> Option<&mut crate::app::FootprintDocument> {
        if self.board_sketch_editor().is_some() {
            return self.board_sketch_editor_mut();
        }
        let path = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .and_then(|t| match &t.kind {
                crate::app::TabKind::FootprintEditor(p) => Some(p.clone()),
                _ => None,
            })?;
        // Refused while the editor's library is being deleted.
        if !self.editor_edit_allowed(&path) {
            return None;
        }
        self.document_state.footprint_editors.get_mut(&path)
    }

    /// The document a footprint dialog opened now applies to: the board in
    /// sketch mode or the footprint in view, in the window it opens from.
    pub(crate) fn footprint_dialog_target(&self) -> Option<std::path::PathBuf> {
        match self.board_sketch_in_view() {
            Some(session) => Some(session.board_path.clone()),
            None => self.active_footprint_editor_path(),
        }
    }

    /// [`Self::footprint_editor_at_mut`], to read.
    pub(crate) fn footprint_editor_at(
        &self,
        target: &std::path::Path,
    ) -> Option<&crate::app::FootprintDocument> {
        let sketch = self.interaction_state.board_sketch.as_ref();
        match sketch.filter(|session| session.board_path == target) {
            Some(session) => Some(&*session.editor),
            None => self.document_state.footprint_editors.get(target),
        }
    }

    /// The editor of a dialog's `target`, whatever tab main shows by then.
    /// `None` once it is closed, its board has left sketch mode, or the
    /// Import DXF card holds that sketch.
    pub(crate) fn footprint_editor_at_mut(
        &mut self,
        target: &std::path::Path,
    ) -> Option<&mut crate::app::FootprintDocument> {
        let sketch = &self.interaction_state.board_sketch;
        if sketch.as_ref().is_some_and(|s| s.board_path == target) {
            let session = self.interaction_state.board_sketch.as_mut()?;
            if session.dxf_dialog.is_some() {
                return None;
            }
            return Some(&mut *session.editor);
        }
        if !self.editor_edit_allowed(target) {
            return None;
        }
        self.document_state.footprint_editors.get_mut(target)
    }

    pub(crate) fn fp_editor_set_next_pad_designator(&mut self, value: String) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.designator_override =
                if value.is_empty() { None } else { Some(value) };
        }
        self.refresh_panel_ctx();
        true
    }

    /// A size above zero that still makes copper with the shape is taken;
    /// anything else is refused and kept as typed text (`pad_input`).
    pub(crate) fn fp_editor_set_next_pad_size_x(&mut self, value: String) -> bool {
        let parsed = crate::numeric_field::parse_field_f64(&value).filter(|v| *v > 0.0);
        let taken = parsed.is_some_and(|parsed| {
            self.active_footprint_editor_mut().is_some_and(|editor| {
                let defaults = &mut editor.state.next_pad_defaults;
                let fits = size_fits_shape(&defaults.shape, (parsed, defaults.size_y_mm));
                if fits {
                    defaults.size_x_mm = parsed;
                }
                fits
            })
        });
        self.pad_field_typed(None, pad_field::SIZE_X, (!taken).then_some(value.as_str()));
        self.refresh_panel_ctx();
        true
    }

    pub(crate) fn fp_editor_set_next_pad_size_y(&mut self, value: String) -> bool {
        let parsed = crate::numeric_field::parse_field_f64(&value).filter(|v| *v > 0.0);
        let taken = parsed.is_some_and(|parsed| {
            self.active_footprint_editor_mut().is_some_and(|editor| {
                let defaults = &mut editor.state.next_pad_defaults;
                let fits = size_fits_shape(&defaults.shape, (defaults.size_x_mm, parsed));
                if fits {
                    defaults.size_y_mm = parsed;
                }
                fits
            })
        });
        self.pad_field_typed(None, pad_field::SIZE_Y, (!taken).then_some(value.as_str()));
        self.refresh_panel_ctx();
        true
    }

    pub(crate) fn fp_editor_set_next_pad_side(
        &mut self,
        side: crate::library::editor::footprint::state::PadSide,
    ) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.side = side;
        }
        self.refresh_panel_ctx();
        true
    }

    pub(crate) fn fp_editor_set_next_pad_rotation(&mut self, value: String) -> bool {
        let parsed = crate::numeric_field::parse_field_f64(&value);
        if let Some(parsed) = parsed
            && let Some(editor) = self.active_footprint_editor_mut()
        {
            editor.state.next_pad_defaults.rotation_deg = parsed;
        }
        let refused = parsed.is_none().then_some(value.as_str());
        self.pad_field_typed(None, pad_field::ROTATION, refused);
        self.refresh_panel_ctx();
        true
    }

    pub(crate) fn fp_editor_set_selected_pad_rotation(
        &mut self,
        idx: usize,
        value: String,
    ) -> bool {
        let parsed = crate::numeric_field::parse_field_f64(&value);
        let refused = parsed.is_none().then_some(value.as_str());
        self.pad_field_typed(Some(idx), pad_field::ROTATION, refused);
        if let Some(parsed) = parsed
            && let Some(editor) = self.active_footprint_editor_mut()
        {
            editor.record_edit(Some(("pad_rotation", idx as u128)), |editor| {
                if editor.state.pads.get(idx).is_none() {
                    return false;
                }
                editor.with_parts(|state, primitive| {
                    use crate::library::editor::footprint::pad_to_sketch;
                    let Some(pad) = state.pads.get_mut(idx) else {
                        return;
                    };
                    pad.rotation_deg = parsed;
                    // The pad FRAME moved, exactly as it does on
                    // the active-bar Rotate arm — so the sidecar
                    // is regenerated through its one owner, not
                    // re-placed corner by corner. Moving only the
                    // four bbox corners turns them into the new
                    // frame and leaves a parametric shape's
                    // anchors in the old one, which is a
                    // self-crossing outline rather than a stale
                    // one.
                    if !pad_to_sketch::remint_pad_geometry(pad, primitive) {
                        pad_to_sketch::warn_profile_pad_untransformed("Rotate", &pad.number);
                    }
                    crate::library::editor::footprint::state::FootprintEditorState::sync_pads_to_primitive(state, primitive);
                });
                editor.mark_dirty();
                editor.canvas_cache.clear();
                true
            });
        }
        self.refresh_panel_ctx();
        true
    }

    // v0.20 — Altium-parity Pad Properties / Pad Stack / Pad Features
    // form handlers. Each method mutates a slice of
    // `editor.state.next_pad_defaults` so the next `add_pad_at` mints
    // a pad with the user-selected stack / feature / testpoint
    // configuration. None of these are dirty-marking on their own —
    // they're "pre-placement defaults" — but the panel `refresh` runs
    // so the form re-reads the new value.
    pub(crate) fn fp_editor_set_next_pad_shape(&mut self, shape: signex_library::PadShape) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            let defaults = &mut editor.state.next_pad_defaults;
            defaults.shape =
                super::pad_trapezoid::seeded_trapezoid(shape, &defaults.shape, defaults.size_x_mm);
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
        true
    }
    pub(crate) fn fp_editor_set_next_pad_kind(&mut self, kind: signex_library::PadKind) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.kind = kind;
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
        true
    }
    /// An optional-mm next-pad default: empty clears it, a number is
    /// stored through `set`, other text is refused and kept for the panel.
    pub(super) fn set_next_pad_optional_mm(
        &mut self,
        field: &str,
        value: &str,
        set: impl FnOnce(&mut crate::library::editor::footprint::NextPadDefaults, Option<f64>),
    ) -> bool {
        match optional_mm_edit(value) {
            Some(parsed) => {
                if let Some(editor) = self.active_footprint_editor_mut() {
                    set(&mut editor.state.next_pad_defaults, parsed);
                }
                self.pad_field_typed(None, field, None);
            }
            None => self.pad_field_typed(None, field, Some(value)),
        }
        self.refresh_panel_ctx();
        true
    }
    pub(crate) fn fp_editor_set_next_pad_drill_diameter(&mut self, value: String) -> bool {
        self.set_next_pad_optional_mm(pad_field::DRILL_DIAMETER, &value, |d, v| {
            d.drill_diameter_mm = v
        })
    }
    pub(crate) fn fp_editor_set_next_pad_drill_slot_length(&mut self, value: String) -> bool {
        self.set_next_pad_optional_mm(pad_field::DRILL_SLOT_LENGTH, &value, |d, v| {
            d.drill_slot_length_mm = v
        })
    }
    /// #599, #2792 — empty clears; out of range or unreadable text leaves
    /// the stored default alone and stays in the box. Collapsing them into
    /// one `None` used to erase the value and drop the key from the saved
    /// `.snxfpt`.
    pub(crate) fn fp_editor_set_next_pad_corner_radius_pct(&mut self, value: String) -> bool {
        let edit = fp_parse_optional_number_in(
            &value,
            numeric_input::CORNER_RADIUS_PCT_MIN,
            numeric_input::CORNER_RADIUS_PCT_MAX,
        );
        let stored = fp_resolve_optional_number("Corner radius %, new pad default", &value, edit);
        if let Some(stored) = stored
            && let Some(editor) = self.active_footprint_editor_mut()
        {
            editor.state.next_pad_defaults.stack.corner_radius_pct = stored;
        }
        let refused = stored.is_none().then_some(value.as_str());
        self.pad_field_typed(None, pad_field::CORNER_RADIUS, refused);
        self.refresh_panel_ctx();
        true
    }
    pub(crate) fn fp_editor_set_next_pad_template(&mut self, value: String) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.template = value;
        }
        self.refresh_panel_ctx();
        true
    }
    pub(crate) fn fp_editor_set_next_pad_paste_margin_top(&mut self, value: String) -> bool {
        self.set_next_pad_optional_mm(pad_field::PASTE_TOP, &value, |d, v| {
            d.stack.paste_margin_top = v
        })
    }
    pub(crate) fn fp_editor_set_next_pad_paste_margin_bottom(&mut self, value: String) -> bool {
        self.set_next_pad_optional_mm(pad_field::PASTE_BOTTOM, &value, |d, v| {
            d.stack.paste_margin_bottom = v
        })
    }
    pub(crate) fn fp_editor_toggle_next_pad_paste_enabled_top(&mut self, on: bool) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.stack.paste_enabled_top = on;
        }
        self.refresh_panel_ctx();
        true
    }
    pub(crate) fn fp_editor_toggle_next_pad_paste_enabled_bottom(&mut self, on: bool) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.stack.paste_enabled_bottom = on;
        }
        self.refresh_panel_ctx();
        true
    }
    pub(crate) fn fp_editor_set_next_pad_mask_margin_top(&mut self, value: String) -> bool {
        self.set_next_pad_optional_mm(pad_field::MASK_TOP, &value, |d, v| {
            d.stack.mask_margin_top = v
        })
    }
    pub(crate) fn fp_editor_set_next_pad_mask_margin_bottom(&mut self, value: String) -> bool {
        self.set_next_pad_optional_mm(pad_field::MASK_BOTTOM, &value, |d, v| {
            d.stack.mask_margin_bottom = v
        })
    }
    pub(crate) fn fp_editor_toggle_next_pad_mask_tented_top(&mut self, on: bool) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.stack.mask_tented_top = on;
        }
        self.refresh_panel_ctx();
        true
    }
    pub(crate) fn fp_editor_toggle_next_pad_mask_tented_bottom(&mut self, on: bool) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.stack.mask_tented_bottom = on;
        }
        self.refresh_panel_ctx();
        true
    }
    pub(crate) fn fp_editor_toggle_next_pad_thermal_relief(&mut self, on: bool) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.stack.thermal_relief = on;
        }
        self.refresh_panel_ctx();
        true
    }
    pub(crate) fn fp_editor_set_next_pad_feature_top(
        &mut self,
        f: signex_sketch::attr::PadFeature,
    ) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.feature_top = f;
        }
        self.refresh_panel_ctx();
        true
    }
    pub(crate) fn fp_editor_set_next_pad_feature_bottom(
        &mut self,
        f: signex_sketch::attr::PadFeature,
    ) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.feature_bottom = f;
        }
        self.refresh_panel_ctx();
        true
    }
    pub(crate) fn fp_editor_toggle_next_pad_testpoint_top_assembly(&mut self, on: bool) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.testpoint.top_assembly = on;
        }
        self.refresh_panel_ctx();
        true
    }
    pub(crate) fn fp_editor_toggle_next_pad_testpoint_top_fab(&mut self, on: bool) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.testpoint.top_fab = on;
        }
        self.refresh_panel_ctx();
        true
    }
    pub(crate) fn fp_editor_toggle_next_pad_testpoint_bottom_assembly(&mut self, on: bool) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.testpoint.bottom_assembly = on;
        }
        self.refresh_panel_ctx();
        true
    }
    pub(crate) fn fp_editor_toggle_next_pad_testpoint_bottom_fab(&mut self, on: bool) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.testpoint.bottom_fab = on;
        }
        self.refresh_panel_ctx();
        true
    }

    // v0.20 — Selected-pad editing handlers. Each one mutates a slice
    // of `state.pads[idx]` and dirty-marks the editor + clears the
    // canvas cache so the new value renders. The `with_parts` block
    // syncs the pad list back onto the underlying primitive so the
    // saved file picks up the change.
    pub(in crate::app::handlers::dock::sch_library) fn with_selected_pad<F>(
        &mut self,
        idx: usize,
        f: F,
    ) -> bool
    where
        F: FnOnce(&mut crate::library::editor::footprint::state::EditorPad),
    {
        self.with_selected_pad_field(idx, None, f)
    }

    /// [`Self::with_selected_pad`] for a text-input field: `field`
    /// names it, so keystrokes into the same pad's same field share
    /// one undo step instead of one per character.
    pub(in crate::app::handlers::dock::sch_library) fn with_selected_pad_field<F>(
        &mut self,
        idx: usize,
        field: Option<&'static str>,
        f: F,
    ) -> bool
    where
        F: FnOnce(&mut crate::library::editor::footprint::state::EditorPad),
    {
        // A patterned pad's edit waits for "This pad / All pads" (mockup row 192, item D1).
        let Some(f) = crate::library::editor::footprint::hold_patterned_pad_edit(
            self.active_footprint_editor_mut(),
            idx,
            f,
        ) else {
            self.refresh_panel_ctx();
            return true;
        };
        if let Some(editor) = self.active_footprint_editor_mut() {
            let key = field.map(|name| (name, idx as u128));
            editor.record_edit(key, |editor| {
                if editor.state.pads.get(idx).is_none() {
                    return false;
                }
                editor.with_parts(|state, primitive| {
                    use crate::library::editor::footprint::pad_to_sketch;
                    let Some(pad) = state.pads.get_mut(idx) else {
                        return;
                    };
                    // This funnel carries `size_mm` and `shape` edits.
                    // A size change moves the outline extents; a shape
                    // change replaces the outline outright — a Rect
                    // that becomes a RoundRect owns arcs and anchors
                    // that were never minted, and the corner mover
                    // cannot mint them. Both are re-mints.
                    //
                    // But only when the geometry ACTUALLY changed. A
                    // re-mint drops the constraints the user authored
                    // against the outline, and most of what comes
                    // through this funnel is an ordinary field edit —
                    // designator, layers, testpoint flags. Wiping
                    // their constraints on every keystroke would be
                    // far worse than the bug being fixed.
                    let before = (pad.shape.clone(), pad.size_mm);
                    f(pad);
                    let changed = (pad.shape.clone(), pad.size_mm) != before;
                    if changed {
                        if !pad_to_sketch::remint_pad_geometry(pad, primitive) {
                            pad_to_sketch::warn_profile_pad_untransformed("Edit", &pad.number);
                        }
                    } else {
                        // Pure non-geometry edit: the outline is still
                        // correct, so leave the entities — and the
                        // constraints on them — exactly where they are.
                        pad_to_sketch::mirror_move_pad_in_sketch(pad, primitive);
                    }
                    crate::library::editor::footprint::state::FootprintEditorState::sync_pads_to_primitive(state, primitive);
                });
                editor.mark_dirty();
                editor.canvas_cache.clear();
                true
            });
        }
        self.refresh_panel_ctx();
        true
    }

    /// v0.21 — sketch-mode counterpart of `with_selected_pad`. Looks
    /// up the sketch entity by id, runs the closure on its `PadAttr`
    /// (creating one only if it already exists; non-pad entities are
    /// silently skipped), then dirty-marks the editor + clears the
    /// canvas cache. Solve+bake is queued on the next mutation cycle.
    pub(in crate::app::handlers::dock::sch_library) fn with_selected_sketch_pad<F>(
        &mut self,
        id: signex_sketch::id::SketchEntityId,
        f: F,
    ) -> bool
    where
        F: FnOnce(&mut signex_sketch::attr::PadAttr),
    {
        self.with_selected_sketch_pad_field(id, None, f)
    }

    /// [`Self::with_selected_sketch_pad`] for a text-input field: keystrokes
    /// into the same pad's `field` share one undo step.
    pub(super) fn with_selected_sketch_pad_field<F>(
        &mut self,
        id: signex_sketch::id::SketchEntityId,
        field: Option<&'static str>,
        f: F,
    ) -> bool
    where
        F: FnOnce(&mut signex_sketch::attr::PadAttr),
    {
        if let Some(editor) = self.active_footprint_editor_mut() {
            let key = field.map(|name| (name, id.0.as_u128()));
            editor.record_edit(key, |editor| {
                let sketch = editor.primitive_mut().sketch.as_mut();
                let Some(sketch) = sketch else {
                    return false;
                };
                let Some(attr) = sketch
                    .entities
                    .iter_mut()
                    .find(|e| e.id == id)
                    .and_then(|e| e.pad.as_mut())
                else {
                    return false;
                };
                f(attr);
                editor.mark_dirty();
                editor.canvas_cache.clear();
                true
            });
        }
        self.refresh_panel_ctx();
        true
    }
    pub(crate) fn fp_editor_set_selected_pad_designator(
        &mut self,
        idx: usize,
        value: String,
    ) -> bool {
        self.with_selected_pad_field(idx, Some("pad_number"), |pad| pad.number = value);
        true
    }
    pub(crate) fn fp_editor_set_selected_pad_side(
        &mut self,
        idx: usize,
        side: crate::library::editor::footprint::state::PadSide,
    ) -> bool {
        use crate::library::editor::footprint::state::PadSide;
        use signex_library::LayerId;
        let layers = match side {
            PadSide::Top => vec![
                LayerId::new("F.Cu"),
                LayerId::new("F.Mask"),
                LayerId::new("F.Paste"),
            ],
            PadSide::Bottom => vec![
                LayerId::new("B.Cu"),
                LayerId::new("B.Mask"),
                LayerId::new("B.Paste"),
            ],
            PadSide::All => vec![
                LayerId::new("*.Cu"),
                LayerId::new("F.Mask"),
                LayerId::new("B.Mask"),
            ],
        };
        let default_drill = self
            .active_footprint_editor_mut()
            .and_then(|editor| editor.state.next_pad_defaults.drill_diameter_mm)
            .unwrap_or(crate::library::editor::footprint::state::DEFAULT_THROUGH_HOLE_DRILL_MM);
        self.with_selected_pad(idx, |pad| {
            pad.layers = layers;
            pad.apply_side_drill(side, default_drill);
        });
        true
    }
    pub(crate) fn fp_editor_set_selected_pad_shape(
        &mut self,
        idx: usize,
        shape: signex_library::PadShape,
    ) -> bool {
        self.with_selected_pad(idx, |pad| {
            pad.shape = super::pad_trapezoid::seeded_trapezoid(shape, &pad.shape, pad.size_mm.0);
        });
        true
    }
    pub(crate) fn fp_editor_set_selected_pad_kind(
        &mut self,
        idx: usize,
        kind: signex_library::PadKind,
    ) -> bool {
        self.with_selected_pad(idx, |pad| pad.kind = kind);
        true
    }
    pub(crate) fn fp_editor_set_selected_pad_size_x(&mut self, idx: usize, value: String) -> bool {
        self.set_selected_pad_size(idx, 0, &value)
    }
    pub(crate) fn fp_editor_set_selected_pad_size_y(&mut self, idx: usize, value: String) -> bool {
        self.set_selected_pad_size(idx, 1, &value)
    }
    /// A size of the selected pad (`axis` 0 is X, 1 is Y): taken when it
    /// is above zero and the shape still makes copper, else refused and
    /// kept as typed text for the panel.
    fn set_selected_pad_size(&mut self, idx: usize, axis: usize, value: &str) -> bool {
        let (field, undo_key) = if axis == 0 {
            (pad_field::SIZE_X, "pad_size_x")
        } else {
            (pad_field::SIZE_Y, "pad_size_y")
        };
        let with_axis =
            |size: (f64, f64), v: f64| if axis == 0 { (v, size.1) } else { (size.0, v) };
        let parsed = crate::numeric_field::parse_field_f64(value).filter(|v| *v > 0.0);
        let fits = parsed.is_some_and(|v| {
            self.active_footprint_editor()
                .and_then(|editor| editor.state.pads.get(idx))
                .is_some_and(|pad| size_fits_shape(&pad.shape, with_axis(pad.size_mm, v)))
        });
        match parsed {
            Some(v) if fits => {
                self.pad_field_typed(Some(idx), field, None);
                self.with_selected_pad_field(idx, Some(undo_key), |pad| {
                    if size_fits_shape(&pad.shape, with_axis(pad.size_mm, v)) {
                        pad.size_mm = with_axis(pad.size_mm, v);
                    }
                });
            }
            _ => self.pad_field_refused(Some(idx), field, value),
        }
        true
    }
    /// An optional-mm field of the selected pad: empty clears it, a number
    /// is stored through `set` (one undo step per field while typing), and
    /// other text is refused and kept for the panel.
    pub(super) fn set_selected_pad_optional_mm(
        &mut self,
        idx: usize,
        (field, undo_key): (&'static str, &'static str),
        value: &str,
        set: impl FnOnce(&mut crate::library::editor::footprint::EditorPad, Option<f64>),
    ) -> bool {
        match optional_mm_edit(value) {
            Some(parsed) => {
                self.pad_field_typed(Some(idx), field, None);
                self.with_selected_pad_field(idx, Some(undo_key), |pad| set(pad, parsed));
            }
            None => self.pad_field_refused(Some(idx), field, value),
        }
        true
    }
    pub(crate) fn fp_editor_set_selected_pad_drill_diameter(
        &mut self,
        idx: usize,
        value: String,
    ) -> bool {
        let keys = (pad_field::DRILL_DIAMETER, "pad_drill_diameter");
        self.set_selected_pad_optional_mm(idx, keys, &value, |pad, parsed| {
            pad.drill_diameter_mm = parsed;
            // A slot length belongs to a drill; with no drill it must
            // not survive to resurface when a diameter is typed again.
            if parsed.is_none() {
                pad.passthrough.drill_slot_length_mm = None;
            }
        })
    }
    /// Slot length of the selected pad's drill; empty clears it back to
    /// a round hole. Without a drill there is nothing to slot, so the
    /// value is not stored on a drill-less pad.
    pub(crate) fn fp_editor_set_selected_pad_drill_slot_length(
        &mut self,
        idx: usize,
        value: String,
    ) -> bool {
        let keys = (pad_field::DRILL_SLOT_LENGTH, "pad_drill_slot_length");
        self.set_selected_pad_optional_mm(idx, keys, &value, |pad, parsed| {
            if pad.drill_diameter_mm.is_some() {
                pad.passthrough.drill_slot_length_mm = parsed;
            }
        })
    }
    /// #599 — sibling of [`Self::fp_editor_set_next_pad_corner_radius_pct`];
    /// an unreadable buffer is refused rather than written as `None`.
    pub(crate) fn fp_editor_set_selected_pad_corner_radius_pct(
        &mut self,
        idx: usize,
        value: String,
    ) -> bool {
        let edit = fp_parse_optional_number_in(
            &value,
            numeric_input::CORNER_RADIUS_PCT_MIN,
            numeric_input::CORNER_RADIUS_PCT_MAX,
        );
        match fp_resolve_optional_number("Corner radius %, selected pad", &value, edit) {
            Some(stored) => {
                self.pad_field_typed(Some(idx), pad_field::CORNER_RADIUS, None);
                self.with_selected_pad_field(idx, Some("pad_corner_radius"), |pad| {
                    pad.stack.corner_radius_pct = stored
                });
            }
            None => self.pad_field_refused(Some(idx), pad_field::CORNER_RADIUS, &value),
        }
        true
    }
    pub(crate) fn fp_editor_set_selected_pad_template(
        &mut self,
        idx: usize,
        value: String,
    ) -> bool {
        self.with_selected_pad_field(idx, Some("pad_template"), |pad| pad.template = value);
        true
    }
    /// A Template row for the next pad: the name and its library together.
    pub(crate) fn fp_editor_pick_next_pad_template(
        &mut self,
        template: String,
        library: String,
    ) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.template = template;
            editor.state.next_pad_defaults.template_library = library;
        }
        self.refresh_panel_ctx();
        true
    }
    /// A Template row for the selected pad, as one undo step.
    pub(crate) fn fp_editor_pick_selected_pad_template(
        &mut self,
        idx: usize,
        template: String,
        library: String,
    ) -> bool {
        self.with_selected_pad_field(idx, None, |pad| {
            pad.template = template;
            pad.template_library = library;
        });
        true
    }
    pub(crate) fn fp_editor_set_selected_pad_paste_margin_top(
        &mut self,
        idx: usize,
        value: String,
    ) -> bool {
        let keys = (pad_field::PASTE_TOP, "pad_paste_margin_top");
        self.set_selected_pad_optional_mm(idx, keys, &value, |pad, parsed| {
            pad.stack.paste_margin_top = parsed
        })
    }
    pub(crate) fn fp_editor_set_selected_pad_paste_margin_bottom(
        &mut self,
        idx: usize,
        value: String,
    ) -> bool {
        let keys = (pad_field::PASTE_BOTTOM, "pad_paste_margin_bottom");
        self.set_selected_pad_optional_mm(idx, keys, &value, |pad, parsed| {
            pad.stack.paste_margin_bottom = parsed
        })
    }
    pub(crate) fn fp_editor_toggle_selected_pad_paste_enabled_top(
        &mut self,
        idx: usize,
        on: bool,
    ) -> bool {
        self.with_selected_pad(idx, |pad| pad.stack.paste_enabled_top = on);
        true
    }
    pub(crate) fn fp_editor_toggle_selected_pad_paste_enabled_bottom(
        &mut self,
        idx: usize,
        on: bool,
    ) -> bool {
        self.with_selected_pad(idx, |pad| pad.stack.paste_enabled_bottom = on);
        true
    }
    pub(crate) fn fp_editor_set_selected_pad_mask_margin_top(
        &mut self,
        idx: usize,
        value: String,
    ) -> bool {
        let keys = (pad_field::MASK_TOP, "pad_mask_margin_top");
        self.set_selected_pad_optional_mm(idx, keys, &value, |pad, parsed| {
            pad.stack.mask_margin_top = parsed
        })
    }
    pub(crate) fn fp_editor_set_selected_pad_mask_margin_bottom(
        &mut self,
        idx: usize,
        value: String,
    ) -> bool {
        let keys = (pad_field::MASK_BOTTOM, "pad_mask_margin_bottom");
        self.set_selected_pad_optional_mm(idx, keys, &value, |pad, parsed| {
            pad.stack.mask_margin_bottom = parsed
        })
    }
    pub(crate) fn fp_editor_toggle_selected_pad_mask_tented_top(
        &mut self,
        idx: usize,
        on: bool,
    ) -> bool {
        self.with_selected_pad(idx, |pad| pad.stack.mask_tented_top = on);
        true
    }
    pub(crate) fn fp_editor_toggle_selected_pad_mask_tented_bottom(
        &mut self,
        idx: usize,
        on: bool,
    ) -> bool {
        self.with_selected_pad(idx, |pad| pad.stack.mask_tented_bottom = on);
        true
    }
    pub(crate) fn fp_editor_toggle_selected_pad_thermal_relief(
        &mut self,
        idx: usize,
        on: bool,
    ) -> bool {
        self.with_selected_pad(idx, |pad| pad.stack.thermal_relief = on);
        true
    }
    pub(crate) fn fp_editor_set_selected_pad_feature_top(
        &mut self,
        idx: usize,
        value: signex_sketch::attr::PadFeature,
    ) -> bool {
        self.with_selected_pad(idx, |pad| pad.feature_top = value);
        true
    }
    pub(crate) fn fp_editor_set_selected_pad_feature_bottom(
        &mut self,
        idx: usize,
        value: signex_sketch::attr::PadFeature,
    ) -> bool {
        self.with_selected_pad(idx, |pad| pad.feature_bottom = value);
        true
    }
    pub(crate) fn fp_editor_toggle_selected_pad_testpoint_top_assembly(
        &mut self,
        idx: usize,
        on: bool,
    ) -> bool {
        self.with_selected_pad(idx, |pad| pad.testpoint.top_assembly = on);
        true
    }
    pub(crate) fn fp_editor_toggle_selected_pad_testpoint_top_fab(
        &mut self,
        idx: usize,
        on: bool,
    ) -> bool {
        self.with_selected_pad(idx, |pad| pad.testpoint.top_fab = on);
        true
    }
    pub(crate) fn fp_editor_toggle_selected_pad_testpoint_bottom_assembly(
        &mut self,
        idx: usize,
        on: bool,
    ) -> bool {
        self.with_selected_pad(idx, |pad| pad.testpoint.bottom_assembly = on);
        true
    }
    pub(crate) fn fp_editor_toggle_selected_pad_testpoint_bottom_fab(
        &mut self,
        idx: usize,
        on: bool,
    ) -> bool {
        self.with_selected_pad(idx, |pad| pad.testpoint.bottom_fab = on);
        true
    }

    // v0.20/v0.21 — Pad Stack tab + placement-default Net / Locked /
    // Electrical Type / Hole detail / Plated toggles. Each mutates a
    // slice of `editor.state.next_pad_defaults` (or `pad_stack_tab`)
    // so the next placement click picks up the new value; the panel
    // refresh re-reads the form.
    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_set_pad_stack_tab(
        &mut self,
        tab: &crate::library::editor::footprint::state::PadStackTab,
    ) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.pad_stack_tab = *tab;
        }
        self.refresh_panel_ctx();
        true
    }

    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_set_next_pad_electrical_type(
        &mut self,
        v: &signex_sketch::attr::ElectricalType,
    ) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.electrical_type = *v;
        }
        self.refresh_panel_ctx();
        true
    }

    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_set_next_pad_net(
        &mut self,
        v: &str,
    ) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.net = v.to_string();
        }
        self.refresh_panel_ctx();
        true
    }

    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_toggle_next_pad_locked(
        &mut self,
        on: &bool,
    ) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.locked = *on;
        }
        self.refresh_panel_ctx();
        true
    }

    /// #599 — an unreadable rotation leaves the stored default alone
    /// instead of clearing it.
    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_set_next_pad_hole_rotation(
        &mut self,
        v: &str,
    ) -> bool {
        let edit = fp_parse_optional_number(v);
        if let Some(stored) =
            fp_resolve_optional_number("Hole rotation (°), new pad default", v, edit)
            && let Some(editor) = self.active_footprint_editor_mut()
        {
            editor.state.next_pad_defaults.hole_rotation_deg = stored;
        }
        self.refresh_panel_ctx();
        true
    }

    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_toggle_next_pad_plated(
        &mut self,
        plated: &bool,
    ) -> bool {
        use signex_library::PadKind as Pk;
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.kind = if *plated { Pk::Tht } else { Pk::NptHole };
        }
        self.refresh_panel_ctx();
        true
    }
}
