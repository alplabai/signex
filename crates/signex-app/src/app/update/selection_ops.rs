use iced::Task;

use super::super::*;

impl Signex {
    pub(crate) fn handle_selection_message(
        &mut self,
        msg: selection_message::SelectionMessage,
    ) -> Task<Message> {
        match msg {
            selection_message::SelectionMessage::SelectAll => {
                if let Some(ref sheet) = self.schematic {
                    use signex_types::schematic::{SelectedItem, SelectedKind};
                    let mut all = Vec::new();
                    for s in &sheet.symbols {
                        all.push(SelectedItem::new(s.uuid, SelectedKind::Symbol));
                    }
                    for w in &sheet.wires {
                        all.push(SelectedItem::new(w.uuid, SelectedKind::Wire));
                    }
                    for b in &sheet.buses {
                        all.push(SelectedItem::new(b.uuid, SelectedKind::Bus));
                    }
                    for l in &sheet.labels {
                        all.push(SelectedItem::new(l.uuid, SelectedKind::Label));
                    }
                    for j in &sheet.junctions {
                        all.push(SelectedItem::new(j.uuid, SelectedKind::Junction));
                    }
                    for nc in &sheet.no_connects {
                        all.push(SelectedItem::new(nc.uuid, SelectedKind::NoConnect));
                    }
                    for tn in &sheet.text_notes {
                        all.push(SelectedItem::new(tn.uuid, SelectedKind::TextNote));
                    }
                    for cs in &sheet.child_sheets {
                        all.push(SelectedItem::new(cs.uuid, SelectedKind::ChildSheet));
                    }
                    self.canvas.selected = all;
                    self.canvas.clear_overlay_cache();
                    self.update_selection_info();
                }
            }
            selection_message::SelectionMessage::HitAt { world_x, world_y } => {
                if let Some(ref sheet) = self.schematic {
                    let hit = signex_render::schematic::hit_test::hit_test(sheet, world_x, world_y);
                    self.canvas.selected = hit.into_iter().collect();
                    self.canvas.clear_overlay_cache();
                    self.update_selection_info();
                }
            }
            selection_message::SelectionMessage::BoxSelect { x1, y1, x2, y2 } => {
                if let Some(ref sheet) = self.schematic {
                    use signex_types::schematic::{SelectedItem, SelectedKind};
                    let mut selected = Vec::new();

                    for s in &sheet.symbols {
                        let px = s.position.x;
                        let py = s.position.y;
                        if px >= x1 && px <= x2 && py >= y1 && py <= y2 {
                            selected.push(SelectedItem::new(s.uuid, SelectedKind::Symbol));
                        }
                    }
                    for w in &sheet.wires {
                        let in_box = |p: &signex_types::schematic::Point| {
                            p.x >= x1 && p.x <= x2 && p.y >= y1 && p.y <= y2
                        };
                        if in_box(&w.start) || in_box(&w.end) {
                            selected.push(SelectedItem::new(w.uuid, SelectedKind::Wire));
                        }
                    }
                    for b in &sheet.buses {
                        let in_box = |p: &signex_types::schematic::Point| {
                            p.x >= x1 && p.x <= x2 && p.y >= y1 && p.y <= y2
                        };
                        if in_box(&b.start) || in_box(&b.end) {
                            selected.push(SelectedItem::new(b.uuid, SelectedKind::Bus));
                        }
                    }
                    for l in &sheet.labels {
                        if l.position.x >= x1
                            && l.position.x <= x2
                            && l.position.y >= y1
                            && l.position.y <= y2
                        {
                            selected.push(SelectedItem::new(l.uuid, SelectedKind::Label));
                        }
                    }
                    for j in &sheet.junctions {
                        if j.position.x >= x1
                            && j.position.x <= x2
                            && j.position.y >= y1
                            && j.position.y <= y2
                        {
                            selected.push(SelectedItem::new(j.uuid, SelectedKind::Junction));
                        }
                    }
                    for nc in &sheet.no_connects {
                        if nc.position.x >= x1
                            && nc.position.x <= x2
                            && nc.position.y >= y1
                            && nc.position.y <= y2
                        {
                            selected.push(SelectedItem::new(nc.uuid, SelectedKind::NoConnect));
                        }
                    }
                    for tn in &sheet.text_notes {
                        if tn.position.x >= x1
                            && tn.position.x <= x2
                            && tn.position.y >= y1
                            && tn.position.y <= y2
                        {
                            selected.push(SelectedItem::new(tn.uuid, SelectedKind::TextNote));
                        }
                    }

                    self.canvas.selected = selected;
                    self.canvas.clear_overlay_cache();
                    self.update_selection_info();
                }
            }
        }

        Task::none()
    }
}
