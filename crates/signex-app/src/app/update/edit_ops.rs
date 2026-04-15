use super::super::*;

impl Signex {
    pub(crate) fn handle_delete_selected(&mut self) {
        if !self.canvas.selected.is_empty()
            && let Some(sheet) = self.schematic.as_ref()
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
                if self.apply_engine_command(
                    signex_engine::Command::DeleteSelection {
                        items: self.canvas.selected.clone(),
                    },
                    true,
                    true,
                ) {
                    self.canvas.selected.clear();
                }
            }
        }
    }

    pub(crate) fn handle_undo(&mut self) {
        let undone = match self.undo_stack.peek_undo_origin() {
            Some(crate::undo::CommandOrigin::EngineMirrored) => self.apply_engine_undo(true),
            Some(crate::undo::CommandOrigin::Legacy) => self.apply_undo(true),
            None => false,
        };

        if undone {
            self.canvas.selected.clear();
        }
    }

    pub(crate) fn handle_redo(&mut self) {
        let redone = match self.undo_stack.peek_redo_origin() {
            Some(crate::undo::CommandOrigin::EngineMirrored) => self.apply_engine_redo(true),
            Some(crate::undo::CommandOrigin::Legacy) => self.apply_redo(true),
            None => false,
        };

        if redone {
            self.canvas.selected.clear();
        }
    }

    pub(crate) fn handle_rotate_selected(&mut self) {
        if self.canvas.selected.len() == 1 {
            let item = self.canvas.selected[0];
            if item.kind == signex_types::schematic::SelectedKind::Symbol
                && let Some(ref mut sheet) = self.schematic
                && let Some(sym) = sheet.symbols.iter().find(|s| s.uuid == item.uuid)
            {
                let _rotation = sym.rotation;
                self.apply_engine_command(
                    signex_engine::Command::RotateSelection {
                        items: vec![item],
                        angle_degrees: 90.0,
                    },
                    true,
                    true,
                );
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
                let _mirror = (sym.mirror_x, sym.mirror_y);
                self.apply_engine_command(
                    signex_engine::Command::MirrorSelection {
                        items: vec![item],
                        axis: signex_engine::MirrorAxis::Vertical,
                    },
                    true,
                    true,
                );
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
                let _mirror = (sym.mirror_x, sym.mirror_y);
                self.apply_engine_command(
                    signex_engine::Command::MirrorSelection {
                        items: vec![item],
                        axis: signex_engine::MirrorAxis::Horizontal,
                    },
                    true,
                    true,
                );
            }
        }
    }
}
