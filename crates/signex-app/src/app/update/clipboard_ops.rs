use iced::Task;

use super::super::*;

impl Signex {
    pub(crate) fn handle_cut(&mut self) -> Task<Message> {
        self.handle_copy();
        self.handle_delete_selected();
        Task::none()
    }

    pub(crate) fn handle_copy(&mut self) {
        if let Some(ref sheet) = self.schematic {
            self.clipboard_wires.clear();
            self.clipboard_buses.clear();
            self.clipboard_labels.clear();
            self.clipboard_symbols.clear();
            self.clipboard_junctions.clear();
            self.clipboard_no_connects.clear();
            self.clipboard_text_notes.clear();
            for item in &self.canvas.selected {
                use signex_types::schematic::SelectedKind;
                match item.kind {
                    SelectedKind::Wire => {
                        if let Some(w) = sheet.wires.iter().find(|w| w.uuid == item.uuid) {
                            self.clipboard_wires.push(w.clone());
                        }
                    }
                    SelectedKind::Bus => {
                        if let Some(b) = sheet.buses.iter().find(|b| b.uuid == item.uuid) {
                            self.clipboard_buses.push(b.clone());
                        }
                    }
                    SelectedKind::Label => {
                        if let Some(l) = sheet.labels.iter().find(|l| l.uuid == item.uuid) {
                            self.clipboard_labels.push(l.clone());
                        }
                    }
                    SelectedKind::Symbol => {
                        if let Some(s) = sheet.symbols.iter().find(|s| s.uuid == item.uuid) {
                            self.clipboard_symbols.push(s.clone());
                        }
                    }
                    SelectedKind::Junction => {
                        if let Some(j) = sheet.junctions.iter().find(|j| j.uuid == item.uuid) {
                            self.clipboard_junctions.push(j.clone());
                        }
                    }
                    SelectedKind::NoConnect => {
                        if let Some(nc) = sheet.no_connects.iter().find(|n| n.uuid == item.uuid) {
                            self.clipboard_no_connects.push(nc.clone());
                        }
                    }
                    SelectedKind::TextNote => {
                        if let Some(tn) = sheet.text_notes.iter().find(|t| t.uuid == item.uuid) {
                            self.clipboard_text_notes.push(tn.clone());
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    pub(crate) fn handle_paste(&mut self) {
        if let Some(ref mut sheet) = self.schematic {
            let offset = 5.08;
            let mut cmds = Vec::new();
            for w in &self.clipboard_wires {
                let mut nw = w.clone();
                nw.uuid = uuid::Uuid::new_v4();
                nw.start.x += offset;
                nw.start.y += offset;
                nw.end.x += offset;
                nw.end.y += offset;
                cmds.push(crate::undo::EditCommand::AddWire(nw));
            }
            for b in &self.clipboard_buses {
                let mut nb = b.clone();
                nb.uuid = uuid::Uuid::new_v4();
                nb.start.x += offset;
                nb.start.y += offset;
                nb.end.x += offset;
                nb.end.y += offset;
                cmds.push(crate::undo::EditCommand::AddBus(nb));
            }
            for l in &self.clipboard_labels {
                let mut nl = l.clone();
                nl.uuid = uuid::Uuid::new_v4();
                nl.position.x += offset;
                nl.position.y += offset;
                cmds.push(crate::undo::EditCommand::AddLabel(nl));
            }
            for s in &self.clipboard_symbols {
                let mut ns = s.clone();
                ns.uuid = uuid::Uuid::new_v4();
                ns.position.x += offset;
                ns.position.y += offset;
                if let Some(ref mut rt) = ns.ref_text {
                    rt.position.x += offset;
                    rt.position.y += offset;
                }
                if let Some(ref mut vt) = ns.val_text {
                    vt.position.x += offset;
                    vt.position.y += offset;
                }
                cmds.push(crate::undo::EditCommand::AddSymbol(ns));
            }
            for j in &self.clipboard_junctions {
                let mut nj = j.clone();
                nj.uuid = uuid::Uuid::new_v4();
                nj.position.x += offset;
                nj.position.y += offset;
                cmds.push(crate::undo::EditCommand::AddJunction(nj));
            }
            for nc in &self.clipboard_no_connects {
                let mut nnc = nc.clone();
                nnc.uuid = uuid::Uuid::new_v4();
                nnc.position.x += offset;
                nnc.position.y += offset;
                cmds.push(crate::undo::EditCommand::AddNoConnect(nnc));
            }
            for tn in &self.clipboard_text_notes {
                let mut ntn = tn.clone();
                ntn.uuid = uuid::Uuid::new_v4();
                ntn.position.x += offset;
                ntn.position.y += offset;
                cmds.push(crate::undo::EditCommand::AddTextNote(ntn));
            }
            if !cmds.is_empty() {
                let batch = crate::undo::EditCommand::Batch(cmds);
                self.undo_stack.execute(sheet, batch);
                self.canvas.schematic = Some(sheet.clone());
                self.canvas.clear_content_cache();
                self.mark_dirty();
                self.commit_schematic();
            }
        }
    }
}
