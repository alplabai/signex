use iced::Task;

use super::super::*;
use crate::active_bar::SelectionFilter;

/// Return `true` iff the currently active filter set allows selecting the
/// given hit. When no filters are active (empty set), selection is blocked
/// entirely — that matches the Altium "unselect all categories" behaviour.
pub(crate) fn passes_filter(
    item: &signex_types::schematic::SelectedItem,
    snapshot: &signex_render::schematic::SchematicRenderSnapshot,
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
        SelectedKind::ChildSheet => SelectionFilter::SheetSymbols,
        SelectedKind::Label => SelectionFilter::NetLabels,
        SelectedKind::TextNote => SelectionFilter::Texts,
        SelectedKind::SymbolRefField | SelectedKind::SymbolValField => SelectionFilter::Parameters,
        SelectedKind::Drawing => SelectionFilter::DrawingObjects,
        SelectedKind::Junction | SelectedKind::NoConnect => SelectionFilter::Other,
    };
    filters.contains(&required)
}

fn all_selectable_items(
    snapshot: &signex_render::schematic::SchematicRenderSnapshot,
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
    for text_note in &snapshot.text_notes {
        items.push(SelectedItem::new(text_note.uuid, SelectedKind::TextNote));
    }
    for child_sheet in &snapshot.child_sheets {
        items.push(SelectedItem::new(
            child_sheet.uuid,
            SelectedKind::ChildSheet,
        ));
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
    snapshot: &signex_render::schematic::SchematicRenderSnapshot,
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
                    self.interaction_state.canvas.selected = all_selectable_items(snapshot);
                    self.interaction_state.canvas.clear_overlay_cache();
                    self.update_selection_info();
                }
            }
            selection_request::SelectionRequest::StoreSlot { slot } => {
                if let Some(selection_slot) = self.interaction_state.selection_slots.get_mut(slot) {
                    *selection_slot = self.interaction_state.canvas.selected.clone();
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
                    self.interaction_state.canvas.selected = recalled;
                    self.interaction_state.canvas.clear_overlay_cache();
                    self.update_selection_info();
                }
            }
            selection_request::SelectionRequest::HitAt { world_x, world_y } => {
                if let Some(snapshot) = self.active_render_snapshot() {
                    let hit =
                        signex_render::schematic::hit_test::hit_test(snapshot, world_x, world_y);
                    let filters = &self.interaction_state.selection_filters;
                    let hit = hit.filter(|h| passes_filter(h, snapshot, filters));
                    self.interaction_state.canvas.selected = hit.into_iter().collect();
                    self.interaction_state.canvas.clear_overlay_cache();
                    self.update_selection_info();
                }
            }
            selection_request::SelectionRequest::BoxSelect { x1, y1, x2, y2 } => {
                if let Some(snapshot) = self.active_render_snapshot() {
                    let rect = signex_types::schematic::Aabb::new(x1, y1, x2, y2);
                    let filters = self.interaction_state.selection_filters.clone();
                    let mode = self.ui_state.selection_mode;
                    self.interaction_state.canvas.selected =
                        signex_render::schematic::hit_test::hit_test_rect_mode(
                            snapshot, &rect, mode,
                        )
                            .into_iter()
                            .filter(|h| passes_filter(h, snapshot, &filters))
                            .collect();
                    self.interaction_state.canvas.clear_overlay_cache();
                    self.update_selection_info();
                }
            }
            selection_request::SelectionRequest::SelectConnected { world_x, world_y } => {
                if let Some(snapshot) = self.active_render_snapshot() {
                    let hit =
                        signex_render::schematic::hit_test::hit_test(snapshot, world_x, world_y);
                    if let Some(item) = hit {
                        self.interaction_state.canvas.selected =
                            expand_to_net(snapshot, &item);
                        self.interaction_state.canvas.clear_overlay_cache();
                        self.update_selection_info();
                    }
                }
            }
            selection_request::SelectionRequest::ArmDrag => {
                // Placeholder — full immediate-drag requires shared state
                // across SchematicCanvas and CanvasState (which are updated
                // in different phases). Deferred; for now the user can still
                // drag by clicking a selected item in the standard way.
                crate::diagnostics::log_info(
                    "Active Bar 'Drag' is deferred — click a selected item to drag",
                );
            }
        }

        Task::none()
    }
}

/// Given a hit on a wire/bus/junction/label, return the full set of net-geom
/// items (wires, buses, junctions, labels) reachable by shared endpoints.
/// Symbols and their pins are intentionally excluded — this matches Altium's
/// "Select » Connection" behaviour which picks net geometry only.
fn expand_to_net(
    snapshot: &signex_render::schematic::SchematicRenderSnapshot,
    seed: &signex_types::schematic::SelectedItem,
) -> Vec<signex_types::schematic::SelectedItem> {
    use signex_types::schematic::{Point, SelectedItem, SelectedKind};

    // Seed endpoints we start walking from.
    let mut frontier: Vec<Point> = Vec::new();
    match seed.kind {
        SelectedKind::Wire => {
            if let Some(w) = snapshot.wires.iter().find(|w| w.uuid == seed.uuid) {
                frontier.push(w.start);
                frontier.push(w.end);
            } else {
                return vec![seed.clone()];
            }
        }
        SelectedKind::Bus => {
            if let Some(b) = snapshot.buses.iter().find(|b| b.uuid == seed.uuid) {
                frontier.push(b.start);
                frontier.push(b.end);
            } else {
                return vec![seed.clone()];
            }
        }
        SelectedKind::Junction => {
            if let Some(j) = snapshot.junctions.iter().find(|j| j.uuid == seed.uuid) {
                frontier.push(j.position);
            } else {
                return vec![seed.clone()];
            }
        }
        SelectedKind::Label => {
            if let Some(l) = snapshot.labels.iter().find(|l| l.uuid == seed.uuid) {
                frontier.push(l.position);
            } else {
                return vec![seed.clone()];
            }
        }
        _ => return vec![seed.clone()],
    }

    // Net tolerance — KiCad coordinates are multiples of 0.01 mm at worst.
    let eps = 1e-4_f64;
    let same = |a: &Point, b: &Point| (a.x - b.x).abs() < eps && (a.y - b.y).abs() < eps;

    let mut net_points: Vec<Point> = frontier.clone();
    let mut out: Vec<SelectedItem> = Vec::new();
    let mut used_wires = std::collections::HashSet::new();
    let mut used_buses = std::collections::HashSet::new();
    let mut used_junctions = std::collections::HashSet::new();
    let mut used_labels = std::collections::HashSet::new();

    loop {
        let before = net_points.len();
        for w in &snapshot.wires {
            if used_wires.contains(&w.uuid) {
                continue;
            }
            if net_points.iter().any(|p| same(p, &w.start) || same(p, &w.end)) {
                used_wires.insert(w.uuid);
                out.push(SelectedItem::new(w.uuid, SelectedKind::Wire));
                net_points.push(w.start);
                net_points.push(w.end);
            }
        }
        for b in &snapshot.buses {
            if used_buses.contains(&b.uuid) {
                continue;
            }
            if net_points.iter().any(|p| same(p, &b.start) || same(p, &b.end)) {
                used_buses.insert(b.uuid);
                out.push(SelectedItem::new(b.uuid, SelectedKind::Bus));
                net_points.push(b.start);
                net_points.push(b.end);
            }
        }
        for j in &snapshot.junctions {
            if used_junctions.contains(&j.uuid) {
                continue;
            }
            if net_points.iter().any(|p| same(p, &j.position)) {
                used_junctions.insert(j.uuid);
                out.push(SelectedItem::new(j.uuid, SelectedKind::Junction));
                net_points.push(j.position);
            }
        }
        for l in &snapshot.labels {
            if used_labels.contains(&l.uuid) {
                continue;
            }
            if net_points.iter().any(|p| same(p, &l.position)) {
                used_labels.insert(l.uuid);
                out.push(SelectedItem::new(l.uuid, SelectedKind::Label));
                // Labels don't carry geometry beyond their anchor.
            }
        }
        if net_points.len() == before {
            break;
        }
    }

    // Make sure the seed is in the result (in case no shared endpoints
    // matched it, e.g. an isolated label).
    if !out.iter().any(|i| i.uuid == seed.uuid) {
        out.push(seed.clone());
    }
    out
}
