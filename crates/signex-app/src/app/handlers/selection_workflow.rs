use iced::Task;

use super::super::*;
use crate::active_bar::SelectionFilter;

/// Return `true` iff the currently active filter set allows selecting the
/// given hit. When no filters are active (empty set), selection is blocked
/// entirely — that matches the Altium "unselect all categories" behaviour.
pub(crate) fn passes_filter(
    item: &signex_types::schematic::SelectedItem,
    snapshot: &crate::schematic_runtime::SchematicRenderSnapshot,
    filters: &std::collections::HashSet<SelectionFilter>,
) -> bool {
    use signex_types::schematic::SelectedKind;
    let required = match item.kind {
        SelectedKind::Symbol => {
            let is_power = snapshot
                .symbols
                .iter()
                .find(|s| s.uuid == item.uuid)
                .map(|s| s.is_power)
                .unwrap_or(false);
            if is_power {
                SelectionFilter::PowerPorts
            } else {
                SelectionFilter::Components
            }
        }
        SelectedKind::Wire => SelectionFilter::Wires,
        SelectedKind::Bus | SelectedKind::BusEntry => SelectionFilter::Buses,
        SelectedKind::ChildSheet | SelectedKind::SheetPin => SelectionFilter::SheetSymbols,
        SelectedKind::Label => SelectionFilter::NetLabels,
        SelectedKind::TextNote => SelectionFilter::Texts,
        SelectedKind::SymbolRefField | SelectedKind::SymbolValField => SelectionFilter::Parameters,
        SelectedKind::Drawing => SelectionFilter::DrawingObjects,
        SelectedKind::Junction | SelectedKind::NoConnect | SelectedKind::NoErcDirective => {
            SelectionFilter::Other
        }
    };
    filters.contains(&required)
}

fn all_selectable_items(
    snapshot: &crate::schematic_runtime::SchematicRenderSnapshot,
) -> Vec<signex_types::schematic::SelectedItem> {
    use signex_types::schematic::{SelectedItem, SelectedKind};

    let mut items = Vec::new();
    for symbol in &snapshot.symbols {
        items.push(SelectedItem::new(symbol.uuid, SelectedKind::Symbol));
    }
    for wire in &snapshot.wires {
        items.push(SelectedItem::new(wire.uuid, SelectedKind::Wire));
    }
    for bus in &snapshot.buses {
        items.push(SelectedItem::new(bus.uuid, SelectedKind::Bus));
    }
    for label in &snapshot.labels {
        items.push(SelectedItem::new(label.uuid, SelectedKind::Label));
    }
    for junction in &snapshot.junctions {
        items.push(SelectedItem::new(junction.uuid, SelectedKind::Junction));
    }
    for no_connect in &snapshot.no_connects {
        items.push(SelectedItem::new(no_connect.uuid, SelectedKind::NoConnect));
    }
    for directive in &snapshot.no_erc_directives {
        items.push(SelectedItem::new(
            directive.uuid,
            SelectedKind::NoErcDirective,
        ));
    }
    for text_note in &snapshot.text_notes {
        items.push(SelectedItem::new(text_note.uuid, SelectedKind::TextNote));
    }
    for child_sheet in &snapshot.child_sheets {
        items.push(SelectedItem::new(
            child_sheet.uuid,
            SelectedKind::ChildSheet,
        ));
        for sheet_pin in &child_sheet.pins {
            items.push(SelectedItem::new(sheet_pin.uuid, SelectedKind::SheetPin));
        }
    }
    for bus_entry in &snapshot.bus_entries {
        items.push(SelectedItem::new(bus_entry.uuid, SelectedKind::BusEntry));
    }
    for drawing in &snapshot.drawings {
        let uuid = drawing.uuid();
        items.push(SelectedItem::new(uuid, SelectedKind::Drawing));
    }

    items
}

fn valid_selection_items(
    snapshot: &crate::schematic_runtime::SchematicRenderSnapshot,
    items: &[signex_types::schematic::SelectedItem],
) -> Vec<signex_types::schematic::SelectedItem> {
    use signex_types::schematic::SelectedKind;

    let valid_items: std::collections::HashSet<_> = all_selectable_items(snapshot)
        .into_iter()
        .flat_map(|item| match item.kind {
            SelectedKind::Symbol => vec![
                item,
                signex_types::schematic::SelectedItem::new(item.uuid, SelectedKind::SymbolRefField),
                signex_types::schematic::SelectedItem::new(item.uuid, SelectedKind::SymbolValField),
            ],
            _ => vec![item],
        })
        .collect();

    items
        .iter()
        .copied()
        .filter(|item| valid_items.contains(item))
        .collect()
}

impl Signex {
    pub(crate) fn handle_selection_request(
        &mut self,
        request: selection_request::SelectionRequest,
    ) -> Task<Message> {
        match request {
            selection_request::SelectionRequest::SelectAll => {
                if let Some(snapshot) = self.active_render_snapshot() {
                    let items = all_selectable_items(snapshot);
                    self.replace_selection(items);
                }
            }
            selection_request::SelectionRequest::ClearAll => {
                self.replace_selection(Vec::new());
            }
            selection_request::SelectionRequest::StoreSlot { slot } => {
                let selected = self.interaction_state.canvas.selected.clone();
                if let Some(selection_slot) = self.interaction_state.selection_slots.get_mut(slot) {
                    *selection_slot = selected;
                }
            }
            selection_request::SelectionRequest::RecallSlot { slot } => {
                if let Some(snapshot) = self.active_render_snapshot() {
                    let recalled = self
                        .interaction_state
                        .selection_slots
                        .get(slot)
                        .map(|items| valid_selection_items(snapshot, items))
                        .unwrap_or_default();
                    self.interaction_state.clicked_pin = None;
                    self.replace_selection(recalled);
                }
            }
            selection_request::SelectionRequest::HitAt { world_x, world_y } => {
                if let Some(snapshot) = self.active_render_snapshot() {
                    let hit = crate::schematic_runtime::hit_test::hit_test_styled(
                        snapshot,
                        &self.interaction_state.active_canvas().designators,
                        world_x,
                        world_y,
                        self.active_wire_widths(),
                    );
                    let filters = &self.interaction_state.selection_filters;
                    let hit = hit.filter(|h| passes_filter(h, snapshot, filters));
                    let at = signex_types::schematic::Point::new(world_x, world_y);
                    let clicked_pin = hit.and_then(|h| {
                        let sheet = self.document_state.active_path.as_deref()?;
                        self.pin_under(sheet, &h, at)
                            .map(|pin| crate::app::state::ClickedPin {
                                sheet: sheet.to_path_buf(),
                                placement: self.shown_placement(sheet),
                                symbol: h.uuid,
                                pin,
                            })
                    });
                    if clicked_pin.is_some() {
                        // A pin click opens the Pins section; the user can
                        // collapse it again and that stands until the next
                        // pin click.
                        self.document_state
                            .panel_ctx
                            .collapsed_sections
                            .remove(crate::panels::PINS_SECTION_KEY);
                    }
                    self.interaction_state.clicked_pin = clicked_pin;
                    self.interaction_state.active_canvas_mut().selected = hit.into_iter().collect();
                    self.interaction_state
                        .active_canvas_mut()
                        .clear_overlay_cache();
                    self.update_selection_info();
                    // The Pins rows read their nets from the project net index.
                    // A cold index is built now, and the rows refresh when it
                    // lands (`refresh_pin_rows`).
                    if hit.is_some_and(|h| h.kind == signex_types::schematic::SelectedKind::Symbol)
                    {
                        return self.warm_net_index_for_pin_rows();
                    }
                }
            }
            selection_request::SelectionRequest::BoxSelect {
                x1,
                y1,
                x2,
                y2,
                direction,
            } => {
                if let Some(snapshot) = self.active_render_snapshot() {
                    let rect = signex_types::schematic::Aabb::new(x1, y1, x2, y2);
                    let filters = self.interaction_state.selection_filters.clone();
                    let mode = crate::schematic_runtime::hit_test::SelectionMode::for_box_drag(
                        direction,
                        self.ui_state.drag_direction_select,
                        self.ui_state.selection_mode,
                    );
                    self.interaction_state.active_canvas_mut().selected =
                        crate::schematic_runtime::hit_test::hit_test_rect_mode(
                            snapshot,
                            &self.interaction_state.active_canvas().designators,
                            &rect,
                            mode,
                            self.active_wire_widths(),
                        )
                        .into_iter()
                        .filter(|h| passes_filter(h, snapshot, &filters))
                        .collect();
                    self.interaction_state.clicked_pin = None;
                    self.interaction_state
                        .active_canvas_mut()
                        .clear_overlay_cache();
                    self.update_selection_info();
                }
            }
            selection_request::SelectionRequest::SelectConnected { seed } => {
                if let Some(snapshot) = self.active_render_snapshot() {
                    let connected = signex_net::connected_net_items(snapshot, seed);
                    self.interaction_state.active_canvas_mut().selected = connected;
                    self.interaction_state
                        .active_canvas_mut()
                        .clear_overlay_cache();
                    self.update_selection_info();
                }
            }
            selection_request::SelectionRequest::FindSimilar(msg) => {
                return self.handle_find_similar_message(msg);
            }
            selection_request::SelectionRequest::DragKeepingConnections => {
                self.arm_drag_keeping_connections();
            }
            selection_request::SelectionRequest::CycleMode => return self.cycle_selection_mode(),
            selection_request::SelectionRequest::LassoCommit => {
                return self.commit_lasso_or_polyline();
            }
            selection_request::SelectionRequest::ArmDrag => {
                // Placeholder — full immediate-drag requires shared state
                // across CanvasSlot and CanvasState (which are updated
                // in different phases). Deferred; for now the user can still
                // drag by clicking a selected item in the standard way.
                crate::diagnostics::log_info(
                    "Active Bar 'Drag' is deferred — click a selected item to drag",
                );
            }
        }

        Task::none()
    }

    /// Enter key: commit whichever multi-click buffer is armed (polyline
    /// drawing or lasso polygon), then fall through to opening a selected
    /// child sheet.
    fn commit_lasso_or_polyline(&mut self) -> Task<Message> {
        // Altium-style single terminator — Enter commits
        // whichever multi-click buffer is currently armed:
        //   - Lasso: selects inside the polygon.
        //   - Polyline (Tool::Polyline): writes a SchDrawing.
        //   - Arc (Tool::Arc): arms need 3 clicks regardless.
        if self.interaction_state.current_tool == Tool::Polyline
            && self.interaction_state.polyline_points.len() >= 2
        {
            self.commit_polyline();
            let canvas = self.interaction_state.active_canvas_mut();
            canvas.polyline_points.clear();
            canvas.clear_overlay_cache();
            return Task::none();
        }
        if let Some(pts) = self.ui_state.lasso_polygon.take()
            && pts.len() >= 3
            && let Some(snapshot) = self.active_render_snapshot()
        {
            let poly: Vec<(f64, f64)> = pts.iter().map(|p| (p.x, p.y)).collect();
            let filters = self.interaction_state.selection_filters.clone();
            self.interaction_state.active_canvas_mut().selected =
                crate::schematic_runtime::hit_test::hit_test_polygon(snapshot, &poly)
                    .into_iter()
                    .filter(|h| passes_filter(h, snapshot, &filters))
                    .collect();
            self.update_selection_info();
        }
        self.sync_lasso_polygon_to_canvas();
        self.open_selected_child_sheet().unwrap_or_else(Task::none)
    }

    /// Cycle the rubber-band selection mode (Shift+S).
    fn cycle_selection_mode(&mut self) -> Task<Message> {
        use crate::schematic_runtime::hit_test::SelectionMode;
        self.ui_state.selection_mode = match self.ui_state.selection_mode {
            SelectionMode::Inside => SelectionMode::Touching,
            SelectionMode::Touching => SelectionMode::Inside,
            SelectionMode::Single => SelectionMode::Inside,
        };
        crate::diagnostics::log_info(format!(
            "Selection mode: {:?}",
            self.ui_state.selection_mode
        ));
        if self.ui_state.drag_direction_select {
            crate::diagnostics::log_info(
                "Select by drag direction is on: the drag direction decides box selection \
                 (Preferences > Editing turns it off)",
            );
        }
        Task::none()
    }
}
