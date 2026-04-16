use iced::Task;

use super::super::*;

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
        items.push(SelectedItem::new(child_sheet.uuid, SelectedKind::ChildSheet));
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
                    let hit = signex_render::schematic::hit_test::hit_test(
                        snapshot,
                        world_x,
                        world_y,
                    );
                    self.interaction_state.canvas.selected = hit.into_iter().collect();
                    self.interaction_state.canvas.clear_overlay_cache();
                    self.update_selection_info();
                }
            }
            selection_request::SelectionRequest::BoxSelect { x1, y1, x2, y2 } => {
                if let Some(snapshot) = self.active_render_snapshot() {
                    let rect = signex_types::schematic::Aabb::new(x1, y1, x2, y2);
                    self.interaction_state.canvas.selected =
                        signex_render::schematic::hit_test::hit_test_rect(snapshot, &rect);
                    self.interaction_state.canvas.clear_overlay_cache();
                    self.update_selection_info();
                }
            }
        }

        Task::none()
    }
}