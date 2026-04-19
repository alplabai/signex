use super::*;

impl Signex {
    /// Clear every cursor-following ghost preview. Call before arming a new
    /// ghost or switching to a tool that doesn't have one, so a previously
    /// armed ghost from another tool doesn't linger on the canvas.
    pub(crate) fn clear_ghost_previews(&mut self) {
        self.interaction_state.canvas.ghost_label = None;
        self.interaction_state.canvas.ghost_symbol = None;
        self.interaction_state.canvas.ghost_text = None;
    }

    /// Mirror `ui_state.lasso_polygon` into the canvas widget's copy and
    /// invalidate the overlay cache. Iced's `canvas::Program` only sees
    /// the widget's own state, so the canvas needs its own snapshot; any
    /// mutation to the in-flight lasso must route through this helper so
    /// the two copies never diverge.
    pub(crate) fn sync_lasso_polygon_to_canvas(&mut self) {
        self.interaction_state.canvas.lasso_polygon = self.ui_state.lasso_polygon.clone();
        self.interaction_state.canvas.clear_overlay_cache();
    }

    fn component_value_from_lib_id(lib_id: &str) -> String {
        lib_id
            .rsplit(':')
            .next()
            .filter(|value| !value.is_empty())
            .unwrap_or(lib_id)
            .to_string()
    }

    fn component_prefix_from_lib_id(lib_id: &str) -> String {
        let value = Self::component_value_from_lib_id(lib_id);
        let prefix: String = value
            .chars()
            .take_while(|ch| ch.is_ascii_alphabetic())
            .collect();
        if prefix.is_empty() {
            "U".to_string()
        } else {
            prefix.to_ascii_uppercase()
        }
    }

    fn increment_designator(reference: &str) -> Option<String> {
        let digit_start = reference
            .char_indices()
            .find(|(_, ch)| ch.is_ascii_digit())
            .map(|(idx, _)| idx)?;
        let prefix = &reference[..digit_start];
        let digits = &reference[digit_start..];
        let value = digits.parse::<u32>().ok()?;
        Some(format!(
            "{prefix}{:0width$}",
            value + 1,
            width = digits.len()
        ))
    }

    fn next_designator_for_prefix(&self, prefix: &str) -> String {
        let next_index = self
            .active_render_snapshot()
            .map(|snapshot| {
                snapshot
                    .symbols
                    .iter()
                    .filter_map(|symbol| {
                        let reference = symbol.reference.trim();
                        if !reference.starts_with(prefix) {
                            return None;
                        }
                        reference[prefix.len()..].parse::<u32>().ok()
                    })
                    .max()
                    .unwrap_or(0)
                    + 1
            })
            .unwrap_or(1);

        format!("{prefix}{next_index}")
    }

    pub(crate) fn current_component_defaults(&self) -> Option<(String, String)> {
        let lib_id = self.document_state.panel_ctx.selected_component.as_ref()?;
        let value = Self::component_value_from_lib_id(lib_id);
        let designator = self
            .document_state
            .panel_ctx
            .pre_placement
            .as_ref()
            .map(|pp| pp.designator.trim().to_string())
            .filter(|designator| !designator.is_empty())
            .unwrap_or_else(|| {
                self.next_designator_for_prefix(&Self::component_prefix_from_lib_id(lib_id))
            });
        Some((value, designator))
    }

    pub(crate) fn place_selected_component(&mut self, wx: f64, wy: f64) -> bool {
        let Some(lib_id) = self.document_state.panel_ctx.selected_component.clone() else {
            return false;
        };

        let default_value = Self::component_value_from_lib_id(&lib_id);
        let reference = self
            .document_state
            .panel_ctx
            .pre_placement
            .as_ref()
            .map(|pp| pp.designator.trim().to_string())
            .filter(|designator| !designator.is_empty())
            .unwrap_or_else(|| {
                self.next_designator_for_prefix(&Self::component_prefix_from_lib_id(&lib_id))
            });
        let value = self
            .document_state
            .panel_ctx
            .pre_placement
            .as_ref()
            .map(|pp| pp.label_text.trim().to_string())
            .filter(|text| !text.is_empty() && text != "NET")
            .unwrap_or(default_value.clone());
        let rotation = self
            .document_state
            .panel_ctx
            .pre_placement
            .as_ref()
            .map(|pp| pp.rotation)
            .unwrap_or(0.0);

        let symbol = signex_types::schematic::Symbol {
            uuid: uuid::Uuid::new_v4(),
            lib_id: lib_id.clone(),
            reference: reference.clone(),
            value,
            footprint: String::new(),
            datasheet: String::new(),
            position: signex_types::schematic::Point::new(wx, wy),
            rotation,
            mirror_x: false,
            mirror_y: false,
            unit: 1,
            is_power: false,
            ref_text: Some(signex_types::schematic::TextProp {
                position: signex_types::schematic::Point::new(wx, wy - 2.54),
                rotation,
                font_size: signex_types::schematic::SCHEMATIC_TEXT_MM,
                justify_h: signex_types::schematic::HAlign::Center,
                justify_v: signex_types::schematic::VAlign::default(),
                hidden: false,
            }),
            val_text: Some(signex_types::schematic::TextProp {
                position: signex_types::schematic::Point::new(wx, wy + 2.54),
                rotation,
                font_size: signex_types::schematic::SCHEMATIC_TEXT_MM,
                justify_h: signex_types::schematic::HAlign::Center,
                justify_v: signex_types::schematic::VAlign::default(),
                hidden: false,
            }),
            fields_autoplaced: true,
            dnp: false,
            in_bom: true,
            on_board: true,
            exclude_from_sim: false,
            locked: false,
            fields: std::collections::HashMap::new(),
            pin_uuids: std::collections::HashMap::new(),
            instances: Vec::new(),
        };
        self.apply_engine_command(signex_engine::Command::PlaceSymbol { symbol }, false, false);

        let next_designator = Self::increment_designator(&reference).unwrap_or_else(|| {
            self.next_designator_for_prefix(&Self::component_prefix_from_lib_id(&lib_id))
        });

        if let Some(ref mut pp) = self.document_state.panel_ctx.pre_placement {
            pp.label_text = default_value;
            pp.designator = next_designator;
        }

        true
    }

    pub(crate) fn clear_transient_schematic_tool_state(&mut self) {
        self.interaction_state.pending_power = None;
        self.interaction_state.pending_port = None;
        self.interaction_state.canvas.ghost_label = None;
        self.interaction_state.canvas.ghost_symbol = None;
        self.interaction_state.canvas.ghost_text = None;
        self.interaction_state.canvas.tool_preview = None;
        self.interaction_state.canvas.placement_paused = false;
        // Drop any configured pre-placement defaults so the next tool
        // session starts fresh instead of inheriting the previous one.
        self.document_state.panel_ctx.pre_placement = None;
        self.interaction_state.editing_text = None;
        // Escape / right-click also cancels the net-colour pen, the
        // z-order reference picker, and any in-flight lasso —
        // Altium-parity "one terminator kills every armed mode".
        self.ui_state.pending_net_color = None;
        self.interaction_state.canvas.pending_net_color = None;
        self.ui_state.reorder_picker = None;
        self.ui_state.lasso_polygon = None;
        self.sync_lasso_polygon_to_canvas();

        if self.interaction_state.wire_drawing {
            self.interaction_state.wire_drawing = false;
            self.interaction_state.wire_points.clear();
            self.interaction_state.canvas.wire_preview.clear();
            self.interaction_state.canvas.drawing_mode = false;
        }
        // Drop any in-flight arc / polyline click buffers.
        self.interaction_state.arc_points.clear();
        self.interaction_state.polyline_points.clear();
        self.interaction_state.canvas.arc_points.clear();
        self.interaction_state.canvas.polyline_points.clear();
    }

    pub(crate) fn align_selected(&mut self, action: &crate::active_bar::ActiveBarAction) {
        use crate::active_bar::ActiveBarAction;

        if self.interaction_state.canvas.selected.len() < 2
            && !matches!(action, ActiveBarAction::AlignToGrid)
        {
            return;
        }
        let Some(engine) = self.document_state.engine.as_ref() else {
            return;
        };

        let positions = engine.selection_anchors(&self.interaction_state.canvas.selected);

        if positions.is_empty() {
            return;
        }

        let min_x = positions
            .iter()
            .map(|anchor| anchor.x)
            .fold(f64::INFINITY, f64::min);
        let max_x = positions
            .iter()
            .map(|anchor| anchor.x)
            .fold(f64::NEG_INFINITY, f64::max);
        let min_y = positions
            .iter()
            .map(|anchor| anchor.y)
            .fold(f64::INFINITY, f64::min);
        let max_y = positions
            .iter()
            .map(|anchor| anchor.y)
            .fold(f64::NEG_INFINITY, f64::max);
        let center_x = (min_x + max_x) / 2.0;
        let center_y = (min_y + max_y) / 2.0;
        let gs = self.ui_state.grid_size_mm as f64;

        let mut engine_commands = Vec::new();
        for anchor in &positions {
            let (target_x, target_y) = match action {
                ActiveBarAction::AlignLeft => (min_x, anchor.y),
                ActiveBarAction::AlignRight => (max_x, anchor.y),
                ActiveBarAction::AlignTop => (anchor.x, min_y),
                ActiveBarAction::AlignBottom => (anchor.x, max_y),
                ActiveBarAction::AlignHorizontalCenters => (center_x, anchor.y),
                ActiveBarAction::AlignVerticalCenters => (anchor.x, center_y),
                ActiveBarAction::AlignToGrid => {
                    ((anchor.x / gs).round() * gs, (anchor.y / gs).round() * gs)
                }
                _ => (anchor.x, anchor.y),
            };
            let dx = target_x - anchor.x;
            let dy = target_y - anchor.y;
            if dx.abs() > 0.001 || dy.abs() > 0.001 {
                let items = vec![signex_types::schematic::SelectedItem::new(
                    anchor.uuid,
                    anchor.kind,
                )];
                engine_commands.push(signex_engine::Command::MoveSelection { items, dx, dy });
            }
        }

        if matches!(
            action,
            ActiveBarAction::DistributeHorizontally | ActiveBarAction::DistributeVertically
        ) && positions.len() > 2
        {
            engine_commands.clear();
            let mut sorted = positions.clone();
            let n = sorted.len();
            match action {
                ActiveBarAction::DistributeHorizontally => {
                    sorted.sort_by(|a, b| a.x.total_cmp(&b.x));
                    let step = (max_x - min_x) / (n - 1) as f64;
                    for (index, anchor) in sorted.iter().enumerate() {
                        let target_x = min_x + step * index as f64;
                        let dx = target_x - anchor.x;
                        if dx.abs() > 0.001 {
                            let items = vec![signex_types::schematic::SelectedItem::new(
                                anchor.uuid,
                                anchor.kind,
                            )];
                            engine_commands.push(signex_engine::Command::MoveSelection {
                                items,
                                dx,
                                dy: 0.0,
                            });
                        }
                    }
                }
                ActiveBarAction::DistributeVertically => {
                    sorted.sort_by(|a, b| a.y.total_cmp(&b.y));
                    let step = (max_y - min_y) / (n - 1) as f64;
                    for (index, anchor) in sorted.iter().enumerate() {
                        let target_y = min_y + step * index as f64;
                        let dy = target_y - anchor.y;
                        if dy.abs() > 0.001 {
                            let items = vec![signex_types::schematic::SelectedItem::new(
                                anchor.uuid,
                                anchor.kind,
                            )];
                            engine_commands.push(signex_engine::Command::MoveSelection {
                                items,
                                dx: 0.0,
                                dy,
                            });
                        }
                    }
                }
                _ => {}
            }
        }

        if !engine_commands.is_empty() {
            self.apply_engine_commands(engine_commands, true, false);
        }
    }
}
