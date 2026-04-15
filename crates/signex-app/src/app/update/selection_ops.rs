use iced::Task;

use super::super::*;

impl Signex {
    pub(crate) fn handle_selection_message(
        &mut self,
        msg: selection_message::SelectionMessage,
    ) -> Task<Message> {
        match msg {
            selection_message::SelectionMessage::SelectAll => {
                if let Some(snapshot) = self.active_render_snapshot() {
                    use signex_types::schematic::{SelectedItem, SelectedKind};
                    let mut all = Vec::new();
                    for s in &snapshot.symbols {
                        all.push(SelectedItem::new(s.uuid, SelectedKind::Symbol));
                    }
                    for w in &snapshot.wires {
                        all.push(SelectedItem::new(w.uuid, SelectedKind::Wire));
                    }
                    for b in &snapshot.buses {
                        all.push(SelectedItem::new(b.uuid, SelectedKind::Bus));
                    }
                    for l in &snapshot.labels {
                        all.push(SelectedItem::new(l.uuid, SelectedKind::Label));
                    }
                    for j in &snapshot.junctions {
                        all.push(SelectedItem::new(j.uuid, SelectedKind::Junction));
                    }
                    for nc in &snapshot.no_connects {
                        all.push(SelectedItem::new(nc.uuid, SelectedKind::NoConnect));
                    }
                    for tn in &snapshot.text_notes {
                        all.push(SelectedItem::new(tn.uuid, SelectedKind::TextNote));
                    }
                    for cs in &snapshot.child_sheets {
                        all.push(SelectedItem::new(cs.uuid, SelectedKind::ChildSheet));
                    }
                    self.canvas.selected = all;
                    self.canvas.clear_overlay_cache();
                    self.update_selection_info();
                }
            }
            selection_message::SelectionMessage::HitAt { world_x, world_y } => {
                if let Some(snapshot) = self.active_render_snapshot() {
                    let hit = signex_render::schematic::hit_test::hit_test(
                        snapshot,
                        world_x,
                        world_y,
                    );
                    self.canvas.selected = hit.into_iter().collect();
                    self.canvas.clear_overlay_cache();
                    self.update_selection_info();
                }
            }
            selection_message::SelectionMessage::BoxSelect { x1, y1, x2, y2 } => {
                if let Some(snapshot) = self.active_render_snapshot() {
                    let rect = signex_types::schematic::Aabb::new(x1, y1, x2, y2);
                    self.canvas.selected =
                        signex_render::schematic::hit_test::hit_test_rect(snapshot, &rect);
                    self.canvas.clear_overlay_cache();
                    self.update_selection_info();
                }
            }
        }

        Task::none()
    }
}
