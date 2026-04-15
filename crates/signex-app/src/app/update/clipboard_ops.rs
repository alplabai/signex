use iced::Task;

use super::super::*;

fn union_bounds(
    current: Option<signex_types::schematic::Aabb>,
    next: signex_types::schematic::Aabb,
) -> Option<signex_types::schematic::Aabb> {
    Some(match current {
        Some(bounds) => bounds.union(&next),
        None => next,
    })
}

fn clipboard_bounds(app: &Signex) -> Option<signex_types::schematic::Aabb> {
    use signex_types::schematic::Aabb;

    let mut bounds = None;

    for wire in &app.clipboard_wires {
        bounds = union_bounds(bounds, Aabb::new(wire.start.x, wire.start.y, wire.end.x, wire.end.y));
    }
    for bus in &app.clipboard_buses {
        bounds = union_bounds(bounds, Aabb::new(bus.start.x, bus.start.y, bus.end.x, bus.end.y));
    }
    for label in &app.clipboard_labels {
        bounds = union_bounds(
            bounds,
            Aabb::new(label.position.x, label.position.y, label.position.x, label.position.y)
                .expand(1.27),
        );
    }
    for symbol in &app.clipboard_symbols {
        bounds = union_bounds(
            bounds,
            Aabb::new(
                symbol.position.x - 5.08,
                symbol.position.y - 5.08,
                symbol.position.x + 5.08,
                symbol.position.y + 5.08,
            ),
        );
        if let Some(ref_text) = &symbol.ref_text {
            bounds = union_bounds(
                bounds,
                Aabb::new(
                    ref_text.position.x,
                    ref_text.position.y,
                    ref_text.position.x,
                    ref_text.position.y,
                )
                .expand(1.27),
            );
        }
        if let Some(val_text) = &symbol.val_text {
            bounds = union_bounds(
                bounds,
                Aabb::new(
                    val_text.position.x,
                    val_text.position.y,
                    val_text.position.x,
                    val_text.position.y,
                )
                .expand(1.27),
            );
        }
    }
    for junction in &app.clipboard_junctions {
        bounds = union_bounds(
            bounds,
            Aabb::new(
                junction.position.x,
                junction.position.y,
                junction.position.x,
                junction.position.y,
            )
            .expand(0.8),
        );
    }
    for no_connect in &app.clipboard_no_connects {
        bounds = union_bounds(
            bounds,
            Aabb::new(
                no_connect.position.x,
                no_connect.position.y,
                no_connect.position.x,
                no_connect.position.y,
            )
            .expand(1.27),
        );
    }
    for text_note in &app.clipboard_text_notes {
        bounds = union_bounds(
            bounds,
            Aabb::new(
                text_note.position.x,
                text_note.position.y,
                text_note.position.x,
                text_note.position.y,
            )
            .expand(1.27),
        );
    }

    bounds
}

fn smart_paste_offset(app: &Signex) -> (f64, f64) {
    let default_offset = 5.08;
    let Some(snapshot) = app.active_render_snapshot() else {
        return (default_offset, default_offset);
    };
    let Some(clipboard_bounds) = clipboard_bounds(app) else {
        return (default_offset, default_offset);
    };
    let Some(content_bounds) = snapshot.content_bounds() else {
        return (default_offset, default_offset);
    };

    let margin = app.grid_size_mm.max(2.54) as f64;
    let offset_x = (content_bounds.max_x - clipboard_bounds.min_x) + margin;
    (offset_x, margin)
}

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

    pub(crate) fn handle_smart_paste(&mut self) {
        let (offset_x, offset_y) = smart_paste_offset(self);
        self.handle_paste_with_offset(offset_x, offset_y);
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
        self.handle_paste_with_offset(5.08, 5.08);
    }

    fn handle_paste_with_offset(&mut self, offset_x: f64, offset_y: f64) {
        if self.has_active_schematic() {
            let mut commands = Vec::new();
            for w in &self.clipboard_wires {
                let mut nw = w.clone();
                nw.uuid = uuid::Uuid::new_v4();
                nw.start.x += offset_x;
                nw.start.y += offset_y;
                nw.end.x += offset_x;
                nw.end.y += offset_y;
                commands.push(signex_engine::Command::PlaceWireSegment { wire: nw });
            }
            for b in &self.clipboard_buses {
                let mut nb = b.clone();
                nb.uuid = uuid::Uuid::new_v4();
                nb.start.x += offset_x;
                nb.start.y += offset_y;
                nb.end.x += offset_x;
                nb.end.y += offset_y;
                commands.push(signex_engine::Command::PlaceBus { bus: nb });
            }
            for l in &self.clipboard_labels {
                let mut nl = l.clone();
                nl.uuid = uuid::Uuid::new_v4();
                nl.position.x += offset_x;
                nl.position.y += offset_y;
                commands.push(signex_engine::Command::PlaceLabel { label: nl });
            }
            for s in &self.clipboard_symbols {
                let mut ns = s.clone();
                ns.uuid = uuid::Uuid::new_v4();
                ns.position.x += offset_x;
                ns.position.y += offset_y;
                if let Some(ref mut rt) = ns.ref_text {
                    rt.position.x += offset_x;
                    rt.position.y += offset_y;
                }
                if let Some(ref mut vt) = ns.val_text {
                    vt.position.x += offset_x;
                    vt.position.y += offset_y;
                }
                commands.push(signex_engine::Command::PlaceSymbol { symbol: ns });
            }
            for j in &self.clipboard_junctions {
                let mut nj = j.clone();
                nj.uuid = uuid::Uuid::new_v4();
                nj.position.x += offset_x;
                nj.position.y += offset_y;
                commands.push(signex_engine::Command::PlaceJunction { junction: nj });
            }
            for nc in &self.clipboard_no_connects {
                let mut nnc = nc.clone();
                nnc.uuid = uuid::Uuid::new_v4();
                nnc.position.x += offset_x;
                nnc.position.y += offset_y;
                commands.push(signex_engine::Command::PlaceNoConnect { no_connect: nnc });
            }
            for tn in &self.clipboard_text_notes {
                let mut ntn = tn.clone();
                ntn.uuid = uuid::Uuid::new_v4();
                ntn.position.x += offset_x;
                ntn.position.y += offset_y;
                commands.push(signex_engine::Command::PlaceTextNote { text_note: ntn });
            }
            self.apply_engine_commands(commands, false, false);
        }
    }
}
