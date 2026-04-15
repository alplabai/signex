use iced::Task;

use super::super::*;

impl Signex {
    pub(crate) fn handle_cut(&mut self) -> Task<Message> {
        self.handle_copy();
        self.handle_delete_selected();
        Task::none()
    }

    pub(crate) fn handle_copy(&mut self) {
        let Some(sheet) = self.active_schematic() else {
            return;
        };

        let mut clipboard_wires = Vec::new();
        let mut clipboard_buses = Vec::new();
        let mut clipboard_labels = Vec::new();
        let mut clipboard_symbols = Vec::new();
        let mut clipboard_junctions = Vec::new();
        let mut clipboard_no_connects = Vec::new();
        let mut clipboard_text_notes = Vec::new();
        for item in &self.canvas.selected {
            use signex_types::schematic::SelectedKind;
            match item.kind {
                SelectedKind::Wire => {
                    if let Some(w) = sheet.wires.iter().find(|w| w.uuid == item.uuid) {
                        clipboard_wires.push(w.clone());
                    }
                }
                SelectedKind::Bus => {
                    if let Some(b) = sheet.buses.iter().find(|b| b.uuid == item.uuid) {
                        clipboard_buses.push(b.clone());
                    }
                }
                SelectedKind::Label => {
                    if let Some(l) = sheet.labels.iter().find(|l| l.uuid == item.uuid) {
                        clipboard_labels.push(l.clone());
                    }
                }
                SelectedKind::Symbol => {
                    if let Some(s) = sheet.symbols.iter().find(|s| s.uuid == item.uuid) {
                        clipboard_symbols.push(s.clone());
                    }
                }
                SelectedKind::Junction => {
                    if let Some(j) = sheet.junctions.iter().find(|j| j.uuid == item.uuid) {
                        clipboard_junctions.push(j.clone());
                    }
                }
                SelectedKind::NoConnect => {
                    if let Some(nc) = sheet.no_connects.iter().find(|n| n.uuid == item.uuid) {
                        clipboard_no_connects.push(nc.clone());
                    }
                }
                SelectedKind::TextNote => {
                    if let Some(tn) = sheet.text_notes.iter().find(|t| t.uuid == item.uuid) {
                        clipboard_text_notes.push(tn.clone());
                    }
                }
                _ => {}
            }
        }

        self.clipboard_wires = clipboard_wires;
        self.clipboard_buses = clipboard_buses;
        self.clipboard_labels = clipboard_labels;
        self.clipboard_symbols = clipboard_symbols;
        self.clipboard_junctions = clipboard_junctions;
        self.clipboard_no_connects = clipboard_no_connects;
        self.clipboard_text_notes = clipboard_text_notes;
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
