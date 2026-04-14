use super::super::*;

impl Signex {
    pub(crate) fn handle_delete_selected(&mut self) {
        if !self.canvas.selected.is_empty()
            && let Some(ref mut sheet) = self.schematic
        {
            let mut cmds = Vec::new();
            for item in &self.canvas.selected {
                use signex_types::schematic::SelectedKind;
                match item.kind {
                    SelectedKind::Wire => {
                        if let Some(w) = sheet.wires.iter().find(|w| w.uuid == item.uuid) {
                            cmds.push(crate::undo::EditCommand::RemoveWire(w.clone()));
                        }
                    }
                    SelectedKind::Bus => {
                        if let Some(b) = sheet.buses.iter().find(|b| b.uuid == item.uuid) {
                            cmds.push(crate::undo::EditCommand::RemoveBus(b.clone()));
                        }
                    }
                    SelectedKind::Label => {
                        if let Some(l) = sheet.labels.iter().find(|l| l.uuid == item.uuid) {
                            cmds.push(crate::undo::EditCommand::RemoveLabel(l.clone()));
                        }
                    }
                    SelectedKind::Junction => {
                        if let Some(j) = sheet.junctions.iter().find(|j| j.uuid == item.uuid) {
                            cmds.push(crate::undo::EditCommand::RemoveJunction(j.clone()));
                        }
                    }
                    SelectedKind::NoConnect => {
                        if let Some(nc) = sheet.no_connects.iter().find(|n| n.uuid == item.uuid) {
                            cmds.push(crate::undo::EditCommand::RemoveNoConnect(nc.clone()));
                        }
                    }
                    SelectedKind::Symbol => {
                        if let Some(s) = sheet.symbols.iter().find(|s| s.uuid == item.uuid) {
                            cmds.push(crate::undo::EditCommand::RemoveSymbol(s.clone()));
                        }
                    }
                    SelectedKind::TextNote => {
                        if let Some(tn) = sheet.text_notes.iter().find(|t| t.uuid == item.uuid) {
                            cmds.push(crate::undo::EditCommand::RemoveTextNote(tn.clone()));
                        }
                    }
                    _ => {}
                }
            }
            if !cmds.is_empty() {
                let batch = crate::undo::EditCommand::Batch(cmds);
                self.undo_stack.execute(sheet, batch);
                self.canvas.schematic = Some(sheet.clone());
                self.canvas.selected.clear();
                self.canvas.clear_content_cache();
                self.canvas.clear_overlay_cache();
                self.mark_dirty();
                self.commit_schematic();
                self.update_selection_info();
            }
        }
    }

    pub(crate) fn handle_undo(&mut self) {
        if let Some(ref mut sheet) = self.schematic
            && self.undo_stack.undo(sheet)
        {
            self.canvas.schematic = Some(sheet.clone());
            self.canvas.selected.clear();
            self.canvas.clear_content_cache();
            self.canvas.clear_overlay_cache();
            self.mark_dirty();
            self.commit_schematic();
            self.update_selection_info();
        }
    }

    pub(crate) fn handle_redo(&mut self) {
        if let Some(ref mut sheet) = self.schematic
            && self.undo_stack.redo(sheet)
        {
            self.canvas.schematic = Some(sheet.clone());
            self.canvas.selected.clear();
            self.canvas.clear_content_cache();
            self.canvas.clear_overlay_cache();
            self.mark_dirty();
            self.commit_schematic();
            self.update_selection_info();
        }
    }

    pub(crate) fn handle_rotate_selected(&mut self) {
        if self.canvas.selected.len() == 1 {
            let item = self.canvas.selected[0];
            if item.kind == signex_types::schematic::SelectedKind::Symbol
                && let Some(ref mut sheet) = self.schematic
                && let Some(sym) = sheet.symbols.iter().find(|s| s.uuid == item.uuid)
            {
                let old_rotation = sym.rotation;
                let new_rotation = (old_rotation + 90.0) % 360.0;
                let cmd = crate::undo::EditCommand::RotateSymbol {
                    uuid: item.uuid,
                    old_rotation,
                    new_rotation,
                };
                self.undo_stack.execute(sheet, cmd);
                self.canvas.schematic = Some(sheet.clone());
                self.canvas.clear_content_cache();
                self.canvas.clear_overlay_cache();
                self.mark_dirty();
                self.commit_schematic();
                self.update_selection_info();
            }
        }
    }

    pub(crate) fn handle_mirror_selected_x(&mut self) {
        if self.canvas.selected.len() == 1 {
            let item = self.canvas.selected[0];
            if item.kind == signex_types::schematic::SelectedKind::Symbol
                && let Some(ref mut sheet) = self.schematic
                && let Some(sym) = sheet.symbols.iter().find(|s| s.uuid == item.uuid)
            {
                let cmd = crate::undo::EditCommand::MirrorSymbol {
                    uuid: item.uuid,
                    axis: crate::undo::MirrorAxis::X,
                    old_mirror_x: sym.mirror_x,
                    old_mirror_y: sym.mirror_y,
                };
                self.undo_stack.execute(sheet, cmd);
                self.canvas.schematic = Some(sheet.clone());
                self.canvas.clear_content_cache();
                self.canvas.clear_overlay_cache();
                self.mark_dirty();
                self.commit_schematic();
                self.update_selection_info();
            }
        }
    }

    pub(crate) fn handle_mirror_selected_y(&mut self) {
        if self.canvas.selected.len() == 1 {
            let item = self.canvas.selected[0];
            if item.kind == signex_types::schematic::SelectedKind::Symbol
                && let Some(ref mut sheet) = self.schematic
                && let Some(sym) = sheet.symbols.iter().find(|s| s.uuid == item.uuid)
            {
                let cmd = crate::undo::EditCommand::MirrorSymbol {
                    uuid: item.uuid,
                    axis: crate::undo::MirrorAxis::Y,
                    old_mirror_x: sym.mirror_x,
                    old_mirror_y: sym.mirror_y,
                };
                self.undo_stack.execute(sheet, cmd);
                self.canvas.schematic = Some(sheet.clone());
                self.canvas.clear_content_cache();
                self.canvas.clear_overlay_cache();
                self.mark_dirty();
                self.commit_schematic();
                self.update_selection_info();
            }
        }
    }
}
