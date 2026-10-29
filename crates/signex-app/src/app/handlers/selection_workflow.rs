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
        let uuid = match drawing {
            signex_types::schematic::SchDrawing::Line { uuid, .. }
            | signex_types::schematic::SchDrawing::Rect { uuid, .. }
            | signex_types::schematic::SchDrawing::Circle { uuid, .. }
            | signex_types::schematic::SchDrawing::Arc { uuid, .. }
            | signex_types::schematic::SchDrawing::Polyline { uuid, .. } => *uuid,
        };
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
                    self.interaction_state.active_canvas_mut().selected =
                        all_selectable_items(snapshot);
                    self.interaction_state
                        .active_canvas_mut()
                        .clear_overlay_cache();
                    self.update_selection_info();
                }
            }
            selection_request::SelectionRequest::ClearAll => {
                let canvas = self.interaction_state.active_canvas_mut();
                canvas.selected.clear();
                canvas.clear_overlay_cache();
                self.update_selection_info();
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
                    self.interaction_state.active_canvas_mut().selected = recalled;
                    self.interaction_state
                        .active_canvas_mut()
                        .clear_overlay_cache();
                    self.update_selection_info();
                }
            }
            selection_request::SelectionRequest::HitAt { world_x, world_y } => {
                if let Some(snapshot) = self.active_render_snapshot() {
                    let hit = crate::schematic_runtime::hit_test::hit_test_styled(
                        snapshot,
                        &self.interaction_state.active_canvas().designators,
                        world_x,
                        world_y,
                        self.ui_state.net_class_styles.styles(),
                    );
                    let filters = &self.interaction_state.selection_filters;
                    let hit = hit.filter(|h| passes_filter(h, snapshot, filters));
                    self.interaction_state.active_canvas_mut().selected = hit.into_iter().collect();
                    self.interaction_state
                        .active_canvas_mut()
                        .clear_overlay_cache();
                    self.update_selection_info();
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
                        )
                        .into_iter()
                        .filter(|h| passes_filter(h, snapshot, &filters))
                        .collect();
                    self.interaction_state
                        .active_canvas_mut()
                        .clear_overlay_cache();
                    self.update_selection_info();
                }
            }
            selection_request::SelectionRequest::SelectConnected { world_x, world_y } => {
                if let Some(snapshot) = self.active_render_snapshot() {
                    let hit = crate::schematic_runtime::hit_test::hit_test_styled(
                        snapshot,
                        &self.interaction_state.active_canvas().designators,
                        world_x,
                        world_y,
                        self.ui_state.net_class_styles.styles(),
                    );
                    if let Some(item) = hit {
                        self.interaction_state.active_canvas_mut().selected =
                            expand_to_net(snapshot, &item);
                        self.interaction_state
                            .active_canvas_mut()
                            .clear_overlay_cache();
                        self.update_selection_info();
                    }
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
            let pp_w = self
                .document_state
                .panel_ctx
                .pre_placement
                .as_ref()
                .map(|pp| pp.shape_width_mm)
                .unwrap_or(0.0);
            let pp_fill = self
                .document_state
                .panel_ctx
                .pre_placement
                .as_ref()
                .map(|pp| pp.shape_fill)
                .unwrap_or(signex_types::schematic::FillType::None);
            let pts = std::mem::take(&mut self.interaction_state.polyline_points);
            let drawing = signex_types::schematic::SchDrawing::Polyline {
                uuid: uuid::Uuid::new_v4(),
                points: pts,
                width: pp_w,
                fill: pp_fill,
                stroke_color: None,
            };
            self.apply_engine_command(
                signex_engine::Command::PlaceSchDrawing { drawing },
                false,
                false,
            );
            self.interaction_state
                .active_canvas_mut()
                .polyline_points
                .clear();
            self.interaction_state
                .active_canvas_mut()
                .clear_overlay_cache();
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
        if self.open_selected_child_sheet() {
            return Task::none();
        }
        Task::none()
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

/// Given a hit on a wire/bus/junction/label, return the full set of net-geom
/// items (wires, buses, junctions, labels) reachable by shared endpoints.
/// Symbols and their pins are intentionally excluded — this matches Altium's
/// "Select » Connection" behaviour which picks net geometry only.
///
/// Endpoints are quantised to 0.001 mm (1 µm) and stored in a HashSet, so
/// lookup is O(1) and the overall walk is O(N · passes) instead of the naive
/// O(P²·N²). Critical for power nets with hundreds of wires.
///
/// Known limit (deferred): connectivity here is endpoint-sharing only, so it
/// can over-select across buses and misses T-junctions; the authoritative
/// netlist lives in `signex-net`.
fn expand_to_net(
    snapshot: &crate::schematic_runtime::SchematicRenderSnapshot,
    seed: &signex_types::schematic::SelectedItem,
) -> Vec<signex_types::schematic::SelectedItem> {
    use signex_types::schematic::{Point, SelectedItem, SelectedKind};
    use std::collections::HashSet;

    // Quantise to 0.001 mm so endpoints compare as exact integer keys.
    // Standard schematic coordinates are multiples of 0.01 mm at worst, so
    // a 0.001 mm bucket is tight enough to avoid false unions.
    let key = |p: &Point| -> (i64, i64) {
        ((p.x * 1000.0).round() as i64, (p.y * 1000.0).round() as i64)
    };

    let mut net_keys: HashSet<(i64, i64)> = HashSet::new();
    match seed.kind {
        SelectedKind::Wire => {
            if let Some(w) = snapshot.wires.iter().find(|w| w.uuid == seed.uuid) {
                net_keys.insert(key(&w.start));
                net_keys.insert(key(&w.end));
            } else {
                return vec![*seed];
            }
        }
        SelectedKind::Bus => {
            if let Some(b) = snapshot.buses.iter().find(|b| b.uuid == seed.uuid) {
                net_keys.insert(key(&b.start));
                net_keys.insert(key(&b.end));
            } else {
                return vec![*seed];
            }
        }
        SelectedKind::Junction => {
            if let Some(j) = snapshot.junctions.iter().find(|j| j.uuid == seed.uuid) {
                net_keys.insert(key(&j.position));
            } else {
                return vec![*seed];
            }
        }
        SelectedKind::Label => {
            if let Some(l) = snapshot.labels.iter().find(|l| l.uuid == seed.uuid) {
                net_keys.insert(key(&l.position));
            } else {
                return vec![*seed];
            }
        }
        _ => return vec![*seed],
    }

    let mut out: Vec<SelectedItem> = Vec::new();
    let mut used_wires: HashSet<uuid::Uuid> = HashSet::new();
    let mut used_buses: HashSet<uuid::Uuid> = HashSet::new();
    let mut used_junctions: HashSet<uuid::Uuid> = HashSet::new();
    let mut used_labels: HashSet<uuid::Uuid> = HashSet::new();

    loop {
        let before = net_keys.len();
        for w in &snapshot.wires {
            if used_wires.contains(&w.uuid) {
                continue;
            }
            let (ks, ke) = (key(&w.start), key(&w.end));
            if net_keys.contains(&ks) || net_keys.contains(&ke) {
                used_wires.insert(w.uuid);
                out.push(SelectedItem::new(w.uuid, SelectedKind::Wire));
                net_keys.insert(ks);
                net_keys.insert(ke);
            }
        }
        for b in &snapshot.buses {
            if used_buses.contains(&b.uuid) {
                continue;
            }
            let (ks, ke) = (key(&b.start), key(&b.end));
            if net_keys.contains(&ks) || net_keys.contains(&ke) {
                used_buses.insert(b.uuid);
                out.push(SelectedItem::new(b.uuid, SelectedKind::Bus));
                net_keys.insert(ks);
                net_keys.insert(ke);
            }
        }
        for j in &snapshot.junctions {
            if used_junctions.contains(&j.uuid) {
                continue;
            }
            if net_keys.contains(&key(&j.position)) {
                used_junctions.insert(j.uuid);
                out.push(SelectedItem::new(j.uuid, SelectedKind::Junction));
                // Junctions sit at shared points we already know about;
                // nothing new to add to net_keys.
            }
        }
        for l in &snapshot.labels {
            if used_labels.contains(&l.uuid) {
                continue;
            }
            if net_keys.contains(&key(&l.position)) {
                used_labels.insert(l.uuid);
                out.push(SelectedItem::new(l.uuid, SelectedKind::Label));
            }
        }
        if net_keys.len() == before {
            break;
        }
    }

    // Make sure the seed is in the result (in case no shared endpoints
    // matched it, e.g. an isolated label).
    if !out.iter().any(|i| i.uuid == seed.uuid) {
        out.push(*seed);
    }
    out
}
