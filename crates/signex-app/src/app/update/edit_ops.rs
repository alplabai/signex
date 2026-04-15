use super::super::*;

impl Signex {
    pub(crate) fn handle_delete_selected(&mut self) {
        if !self.canvas.selected.is_empty()
            && let Some(snapshot) = self.active_render_snapshot()
        {
            let mut has_supported_selection = false;
            for item in &self.canvas.selected {
                use signex_types::schematic::SelectedKind;
                match item.kind {
                    SelectedKind::Wire => {
                        if let Some(w) = snapshot.wires.iter().find(|w| w.uuid == item.uuid) {
                            let _wire = w;
                            has_supported_selection = true;
                        }
                    }
                    SelectedKind::Bus => {
                        if let Some(b) = snapshot.buses.iter().find(|b| b.uuid == item.uuid) {
                            let _bus = b;
                            has_supported_selection = true;
                        }
                    }
                    SelectedKind::Label => {
                        if let Some(l) = snapshot.labels.iter().find(|l| l.uuid == item.uuid) {
                            let _label = l;
                            has_supported_selection = true;
                        }
                    }
                    SelectedKind::Junction => {
                        if let Some(j) = snapshot.junctions.iter().find(|j| j.uuid == item.uuid) {
                            let _junction = j;
                            has_supported_selection = true;
                        }
                    }
                    SelectedKind::NoConnect => {
                        if let Some(nc) = snapshot.no_connects.iter().find(|n| n.uuid == item.uuid) {
                            let _no_connect = nc;
                            has_supported_selection = true;
                        }
                    }
                    SelectedKind::Symbol => {
                        if let Some(s) = snapshot.symbols.iter().find(|s| s.uuid == item.uuid) {
                            let _symbol = s;
                            has_supported_selection = true;
                        }
                    }
                    SelectedKind::TextNote => {
                        if let Some(tn) = snapshot.text_notes.iter().find(|t| t.uuid == item.uuid) {
                            let _text_note = tn;
                            has_supported_selection = true;
                        }
                    }
                    _ => {}
                }
            }
            if has_supported_selection {
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
        let undone = self.apply_engine_undo(true);

        if undone {
            self.canvas.selected.clear();
        }
    }

    pub(crate) fn handle_redo(&mut self) {
        let redone = self.apply_engine_redo(true);

        if redone {
            self.canvas.selected.clear();
        }
    }

    pub(crate) fn handle_rotate_selected(&mut self) {
        if self.canvas.selected.len() == 1 {
            let item = self.canvas.selected[0];
            if item.kind == signex_types::schematic::SelectedKind::Symbol
                && let Some(snapshot) = self.active_render_snapshot()
                && let Some(sym) = snapshot.symbols.iter().find(|s| s.uuid == item.uuid)
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
                && let Some(snapshot) = self.active_render_snapshot()
                && let Some(sym) = snapshot.symbols.iter().find(|s| s.uuid == item.uuid)
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
                && let Some(snapshot) = self.active_render_snapshot()
                && let Some(sym) = snapshot.symbols.iter().find(|s| s.uuid == item.uuid)
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
