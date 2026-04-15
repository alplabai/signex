use iced::Task;

use super::super::*;

impl Signex {
    pub(crate) fn handle_cut(&mut self) -> Task<Message> {
        self.handle_copy();
        self.handle_delete_selected();
        Task::none()
    }

    pub(crate) fn handle_duplicate(&mut self) {
        if self.canvas.selected.is_empty() || !self.has_active_schematic() {
            return;
        }

        self.handle_copy();
        self.handle_paste();
    }

    pub(crate) fn handle_copy(&mut self) {
        let Some(engine) = self.engine.as_ref() else {
            return;
        };

        let clipboard = engine.collect_selection_clipboard(&self.canvas.selected);

        self.clipboard_wires = clipboard.wires;
        self.clipboard_buses = clipboard.buses;
        self.clipboard_labels = clipboard.labels;
        self.clipboard_symbols = clipboard.symbols;
        self.clipboard_junctions = clipboard.junctions;
        self.clipboard_no_connects = clipboard.no_connects;
        self.clipboard_text_notes = clipboard.text_notes;
    }

    pub(crate) fn handle_paste(&mut self) {
        if self.has_active_schematic() {
            let offset = 5.08;
            let mut commands = Vec::new();
            for w in &self.clipboard_wires {
                let mut nw = w.clone();
                nw.uuid = uuid::Uuid::new_v4();
                nw.start.x += offset;
                nw.start.y += offset;
                nw.end.x += offset;
                nw.end.y += offset;
                commands.push(signex_engine::Command::PlaceWireSegment { wire: nw });
            }
            for b in &self.clipboard_buses {
                let mut nb = b.clone();
                nb.uuid = uuid::Uuid::new_v4();
                nb.start.x += offset;
                nb.start.y += offset;
                nb.end.x += offset;
                nb.end.y += offset;
                commands.push(signex_engine::Command::PlaceBus { bus: nb });
            }
            for l in &self.clipboard_labels {
                let mut nl = l.clone();
                nl.uuid = uuid::Uuid::new_v4();
                nl.position.x += offset;
                nl.position.y += offset;
                commands.push(signex_engine::Command::PlaceLabel { label: nl });
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
                commands.push(signex_engine::Command::PlaceSymbol { symbol: ns });
            }
            for j in &self.clipboard_junctions {
                let mut nj = j.clone();
                nj.uuid = uuid::Uuid::new_v4();
                nj.position.x += offset;
                nj.position.y += offset;
                commands.push(signex_engine::Command::PlaceJunction { junction: nj });
            }
            for nc in &self.clipboard_no_connects {
                let mut nnc = nc.clone();
                nnc.uuid = uuid::Uuid::new_v4();
                nnc.position.x += offset;
                nnc.position.y += offset;
                commands.push(signex_engine::Command::PlaceNoConnect { no_connect: nnc });
            }
            for tn in &self.clipboard_text_notes {
                let mut ntn = tn.clone();
                ntn.uuid = uuid::Uuid::new_v4();
                ntn.position.x += offset;
                ntn.position.y += offset;
                commands.push(signex_engine::Command::PlaceTextNote { text_note: ntn });
            }
            self.apply_engine_commands(commands, false, false);
        }
    }
}
