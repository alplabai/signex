use signex_types::schematic::{
    Bus, Junction, Label, NoConnect, SchDrawing, SelectedItem, SelectedKind, Symbol, TextNote,
    Wire, SCHEMATIC_PT_TO_MM,
};

use super::Engine;

// ---------------------------------------------------------------------------
// Selection data types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct ClipboardSelection {
    pub wires: Vec<Wire>,
    pub buses: Vec<Bus>,
    pub labels: Vec<Label>,
    pub symbols: Vec<Symbol>,
    pub junctions: Vec<Junction>,
    pub no_connects: Vec<NoConnect>,
    pub text_notes: Vec<TextNote>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SelectionAnchor {
    pub uuid: uuid::Uuid,
    pub kind: SelectedKind,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone)]
pub struct SelectionDetails {
    pub selected_uuid: uuid::Uuid,
    pub selected_kind: SelectedKind,
    pub info: Vec<(String, String)>,
}

// ---------------------------------------------------------------------------
// impl Engine — selection queries
// ---------------------------------------------------------------------------

impl Engine {
    pub fn has_selected_items(&self, items: &[SelectedItem]) -> bool {
        items.iter().any(|item| self.contains_selected_item(item))
    }

    pub fn selection_is_single_symbol(&self, items: &[SelectedItem]) -> bool {
        matches!(items, [item] if item.kind == SelectedKind::Symbol
            && self.contains_selected_item(item))
    }

    pub fn collect_selection_clipboard(&self, items: &[SelectedItem]) -> ClipboardSelection {
        let mut clipboard = ClipboardSelection::default();

        for item in items {
            match item.kind {
                SelectedKind::Wire => {
                    if let Some(wire) =
                        self.document.wires.iter().find(|w| w.uuid == item.uuid)
                    {
                        clipboard.wires.push(wire.clone());
                    }
                }
                SelectedKind::Bus => {
                    if let Some(bus) =
                        self.document.buses.iter().find(|b| b.uuid == item.uuid)
                    {
                        clipboard.buses.push(bus.clone());
                    }
                }
                SelectedKind::Label => {
                    if let Some(label) =
                        self.document.labels.iter().find(|l| l.uuid == item.uuid)
                    {
                        clipboard.labels.push(label.clone());
                    }
                }
                SelectedKind::Symbol => {
                    if let Some(symbol) =
                        self.document.symbols.iter().find(|s| s.uuid == item.uuid)
                    {
                        clipboard.symbols.push(symbol.clone());
                    }
                }
                SelectedKind::Junction => {
                    if let Some(j) =
                        self.document.junctions.iter().find(|j| j.uuid == item.uuid)
                    {
                        clipboard.junctions.push(j.clone());
                    }
                }
                SelectedKind::NoConnect => {
                    if let Some(nc) =
                        self.document.no_connects.iter().find(|nc| nc.uuid == item.uuid)
                    {
                        clipboard.no_connects.push(nc.clone());
                    }
                }
                SelectedKind::TextNote => {
                    if let Some(tn) =
                        self.document.text_notes.iter().find(|t| t.uuid == item.uuid)
                    {
                        clipboard.text_notes.push(tn.clone());
                    }
                }
                _ => {}
            }
        }

        clipboard
    }

    pub fn selection_anchors(&self, items: &[SelectedItem]) -> Vec<SelectionAnchor> {
        let mut anchors = Vec::new();

        for item in items {
            let position = match item.kind {
                SelectedKind::Symbol => self
                    .document
                    .symbols
                    .iter()
                    .find(|s| s.uuid == item.uuid)
                    .map(|s| (s.position.x, s.position.y)),
                SelectedKind::Label => self
                    .document
                    .labels
                    .iter()
                    .find(|l| l.uuid == item.uuid)
                    .map(|l| (l.position.x, l.position.y)),
                SelectedKind::Junction => self
                    .document
                    .junctions
                    .iter()
                    .find(|j| j.uuid == item.uuid)
                    .map(|j| (j.position.x, j.position.y)),
                SelectedKind::NoConnect => self
                    .document
                    .no_connects
                    .iter()
                    .find(|nc| nc.uuid == item.uuid)
                    .map(|nc| (nc.position.x, nc.position.y)),
                SelectedKind::TextNote => self
                    .document
                    .text_notes
                    .iter()
                    .find(|t| t.uuid == item.uuid)
                    .map(|t| (t.position.x, t.position.y)),
                SelectedKind::SheetPin => self
                    .document
                    .child_sheets
                    .iter()
                    .find_map(|cs| {
                        cs.pins
                            .iter()
                            .find(|p| p.uuid == item.uuid)
                    })
                    .map(|p| (p.position.x, p.position.y)),
                SelectedKind::Wire => self
                    .document
                    .wires
                    .iter()
                    .find(|w| w.uuid == item.uuid)
                    .map(|w| {
                        (
                            (w.start.x + w.end.x) / 2.0,
                            (w.start.y + w.end.y) / 2.0,
                        )
                    }),
                SelectedKind::Bus => self
                    .document
                    .buses
                    .iter()
                    .find(|b| b.uuid == item.uuid)
                    .map(|b| {
                        (
                            (b.start.x + b.end.x) / 2.0,
                            (b.start.y + b.end.y) / 2.0,
                        )
                    }),
                _ => None,
            };

            if let Some((x, y)) = position {
                anchors.push(SelectionAnchor {
                    uuid: item.uuid,
                    kind: item.kind,
                    x,
                    y,
                });
            }
        }

        anchors
    }

    pub fn describe_single_selection(
        &self,
        items: &[SelectedItem],
    ) -> Option<SelectionDetails> {
        let [item] = items else {
            return None;
        };

        let h_align_label = |align| match align {
            signex_types::schematic::HAlign::Left => "Left",
            signex_types::schematic::HAlign::Center => "Center",
            signex_types::schematic::HAlign::Right => "Right",
        };
        let v_align_label = |align| match align {
            signex_types::schematic::VAlign::Top => "Top",
            signex_types::schematic::VAlign::Center => "Center",
            signex_types::schematic::VAlign::Bottom => "Bottom",
        };
        let fill_type_label = |fill| match fill {
            signex_types::schematic::FillType::None => "None",
            signex_types::schematic::FillType::Outline => "Outline",
            signex_types::schematic::FillType::Background => "Background",
        };

        let mut info: Vec<(String, String)> = Vec::new();

        match item.kind {
            SelectedKind::Symbol => {
                let symbol = self
                    .document
                    .symbols
                    .iter()
                    .find(|s| s.uuid == item.uuid)?;
                let type_label = if symbol.is_power { "Power Port" } else { "Symbol" };
                info.push(("Type".into(), type_label.into()));
                info.push(("Reference".into(), symbol.reference.clone()));
                info.push(("Value".into(), symbol.value.clone()));
                if let Some(lib_sym) = self.document.lib_symbols.get(&symbol.lib_id)
                    && !lib_sym.description.is_empty()
                {
                    info.push(("Description".into(), lib_sym.description.clone()));
                }
                info.push(("Library ID".into(), symbol.lib_id.clone()));
                info.push(("Footprint".into(), symbol.footprint.clone()));
                if !symbol.datasheet.is_empty() {
                    info.push(("Datasheet".into(), symbol.datasheet.clone()));
                }
                info.push((
                    "Position".into(),
                    format!("{:.2}, {:.2} mm", symbol.position.x, symbol.position.y),
                ));
                info.push(("Rotation".into(), format!("{:.0}\u{00b0}", symbol.rotation)));
                if symbol.mirror_x {
                    info.push(("Mirror".into(), "X".into()));
                }
                if symbol.mirror_y {
                    info.push(("Mirror".into(), "Y".into()));
                }
                if symbol.unit > 1 {
                    info.push(("Unit".into(), symbol.unit.to_string()));
                }
                info.push((
                    "Locked".into(),
                    if symbol.locked { "Yes" } else { "No" }.into(),
                ));
                info.push(("DNP".into(), if symbol.dnp { "Yes" } else { "No" }.into()));
                let mut custom: Vec<(&String, &String)> = symbol.fields.iter().collect();
                custom.sort_by(|a, b| a.0.cmp(b.0));
                for (name, value) in custom {
                    info.push((format!("Param: {name}"), value.clone()));
                }
            }
            SelectedKind::Wire => {
                let wire = self
                    .document
                    .wires
                    .iter()
                    .find(|w| w.uuid == item.uuid)?;
                let dx = wire.end.x - wire.start.x;
                let dy = wire.end.y - wire.start.y;
                let len = (dx * dx + dy * dy).sqrt();
                info.push(("Type".into(), "Wire".into()));
                info.push((
                    "Start".into(),
                    format!("{:.2}, {:.2}", wire.start.x, wire.start.y),
                ));
                info.push((
                    "End".into(),
                    format!("{:.2}, {:.2}", wire.end.x, wire.end.y),
                ));
                info.push(("Length".into(), format!("{:.2} mm", len)));
            }
            SelectedKind::Label => {
                let label = self
                    .document
                    .labels
                    .iter()
                    .find(|l| l.uuid == item.uuid)?;
                info.push(("Type".into(), format!("{:?} Label", label.label_type)));
                info.push(("Text".into(), label.text.clone()));
                info.push(("Net Name".into(), label.text.clone()));
                info.push((
                    "Position".into(),
                    format!("{:.2}, {:.2}", label.position.x, label.position.y),
                ));
                info.push(("Rotation".into(), format!("{:.0}°", label.rotation)));
                info.push((
                    "Text Size".into(),
                    format!(
                        "{}",
                        (label.font_size.max(0.0) / SCHEMATIC_PT_TO_MM).round() as i32
                    ),
                ));
                info.push((
                    "Horizontal Justification".into(),
                    h_align_label(label.justify).into(),
                ));
            }
            SelectedKind::Junction => {
                let junction = self
                    .document
                    .junctions
                    .iter()
                    .find(|j| j.uuid == item.uuid)?;
                info.push(("Type".into(), "Junction".into()));
                info.push((
                    "Position".into(),
                    format!("{:.2}, {:.2}", junction.position.x, junction.position.y),
                ));
            }
            SelectedKind::NoConnect => {
                let no_connect = self
                    .document
                    .no_connects
                    .iter()
                    .find(|nc| nc.uuid == item.uuid)?;
                info.push(("Type".into(), "No Connect".into()));
                info.push((
                    "Position".into(),
                    format!("{:.2}, {:.2}", no_connect.position.x, no_connect.position.y),
                ));
            }
            SelectedKind::TextNote => {
                let text_note = self
                    .document
                    .text_notes
                    .iter()
                    .find(|t| t.uuid == item.uuid)?;
                info.push(("Type".into(), "Text Note".into()));
                info.push(("Text".into(), text_note.text.clone()));
                info.push((
                    "Position".into(),
                    format!("{:.2}, {:.2}", text_note.position.x, text_note.position.y),
                ));
                info.push(("Rotation".into(), format!("{:.0}°", text_note.rotation)));
                info.push((
                    "Text Size".into(),
                    format!(
                        "{}",
                        (text_note.font_size.max(0.0) / SCHEMATIC_PT_TO_MM).round() as i32
                    ),
                ));
                info.push((
                    "Horizontal Justification".into(),
                    h_align_label(text_note.justify_h).into(),
                ));
                info.push((
                    "Vertical Justification".into(),
                    v_align_label(text_note.justify_v).into(),
                ));
            }
            SelectedKind::ChildSheet => {
                let child_sheet = self
                    .document
                    .child_sheets
                    .iter()
                    .find(|cs| cs.uuid == item.uuid)?;
                info.push(("Type".into(), "Hierarchical Sheet".into()));
                info.push(("Name".into(), child_sheet.name.clone()));
                info.push(("File".into(), child_sheet.filename.clone()));
                info.push((
                    "Position".into(),
                    format!(
                        "{:.2}, {:.2}",
                        child_sheet.position.x, child_sheet.position.y
                    ),
                ));
                info.push((
                    "Size".into(),
                    format!("{:.1} x {:.1} mm", child_sheet.size.0, child_sheet.size.1),
                ));
            }
            SelectedKind::SheetPin => {
                let (sheet_name, sheet_file, sheet_pin) = self
                    .document
                    .child_sheets
                    .iter()
                    .find_map(|cs| {
                        cs.pins
                            .iter()
                            .find(|p| p.uuid == item.uuid)
                            .map(|p| (cs.name.clone(), cs.filename.clone(), p))
                    })?;
                info.push(("Type".into(), "Sheet Pin".into()));
                info.push(("Sheet".into(), sheet_name));
                info.push(("Sheet File".into(), sheet_file));
                info.push(("Name".into(), sheet_pin.name.clone()));
                info.push(("Direction".into(), format!("{:?}", sheet_pin.direction)));
                info.push((
                    "Position".into(),
                    format!("{:.2}, {:.2}", sheet_pin.position.x, sheet_pin.position.y),
                ));
            }
            SelectedKind::Bus => {
                let bus = self
                    .document
                    .buses
                    .iter()
                    .find(|b| b.uuid == item.uuid)?;
                info.push(("Type".into(), "Bus".into()));
                info.push((
                    "Start".into(),
                    format!("{:.2}, {:.2}", bus.start.x, bus.start.y),
                ));
                info.push(("End".into(), format!("{:.2}, {:.2}", bus.end.x, bus.end.y)));
            }
            SelectedKind::BusEntry => {
                info.push(("Type".into(), "Bus Entry".into()));
            }
            SelectedKind::Drawing => {
                let d = self.document.drawings.iter().find(|d| {
                    let u = match d {
                        SchDrawing::Line { uuid, .. }
                        | SchDrawing::Rect { uuid, .. }
                        | SchDrawing::Circle { uuid, .. }
                        | SchDrawing::Arc { uuid, .. }
                        | SchDrawing::Polyline { uuid, .. } => *uuid,
                    };
                    u == item.uuid
                })?;
                match d {
                    SchDrawing::Line { start, end, width, .. } => {
                        info.push(("Type".into(), "Line".into()));
                        info.push(("Start".into(), format!("{:.2}, {:.2}", start.x, start.y)));
                        info.push(("End".into(), format!("{:.2}, {:.2}", end.x, end.y)));
                        info.push(("Width".into(), format!("{width:.3}")));
                    }
                    SchDrawing::Rect { start, end, width, fill, .. } => {
                        info.push(("Type".into(), "Rectangle".into()));
                        let x0 = start.x.min(end.x);
                        let y0 = start.y.min(end.y);
                        let w = (end.x - start.x).abs();
                        let h = (end.y - start.y).abs();
                        info.push(("Position".into(), format!("{x0:.2}, {y0:.2}")));
                        info.push(("Width".into(), format!("{w:.2}")));
                        info.push(("Height".into(), format!("{h:.2}")));
                        info.push(("Border".into(), format!("{width:.3}")));
                        info.push(("Fill".into(), fill_type_label(*fill).into()));
                    }
                    SchDrawing::Circle { center, radius, width, fill, .. } => {
                        info.push(("Type".into(), "Circle".into()));
                        info.push((
                            "Center".into(),
                            format!("{:.2}, {:.2}", center.x, center.y),
                        ));
                        info.push(("Radius".into(), format!("{radius:.3}")));
                        info.push(("Border".into(), format!("{width:.3}")));
                        info.push(("Fill".into(), fill_type_label(*fill).into()));
                    }
                    SchDrawing::Arc { start, mid, end, width, .. } => {
                        info.push(("Type".into(), "Arc".into()));
                        if let Some((cx, cy, radius)) =
                            circumcircle((start.x, start.y), (mid.x, mid.y), (end.x, end.y))
                        {
                            let sa: f64 = (start.y - cy).atan2(start.x - cx);
                            let ea: f64 = (end.y - cy).atan2(end.x - cx);
                            let norm = |a: f64| -> f64 {
                                let mut t = a.to_degrees() % 360.0;
                                if t < 0.0 {
                                    t += 360.0;
                                }
                                t
                            };
                            info.push(("Center".into(), format!("{cx:.2}, {cy:.2}")));
                            info.push(("Radius".into(), format!("{radius:.3}")));
                            info.push(("Start Angle".into(), format!("{:.3}", norm(sa))));
                            info.push(("End Angle".into(), format!("{:.3}", norm(ea))));
                        } else {
                            info.push(("Start".into(), format!("{:.2}, {:.2}", start.x, start.y)));
                            info.push(("Mid".into(), format!("{:.2}, {:.2}", mid.x, mid.y)));
                            info.push(("End".into(), format!("{:.2}, {:.2}", end.x, end.y)));
                        }
                        info.push(("Width".into(), format!("{width:.3}")));
                    }
                    SchDrawing::Polyline { points, width, fill, .. } => {
                        info.push(("Type".into(), "Polygon".into()));
                        info.push(("Vertices".into(), format!("{}", points.len())));
                        info.push(("Border".into(), format!("{width:.3}")));
                        info.push(("Fill".into(), fill_type_label(*fill).into()));
                    }
                }
            }
            SelectedKind::SymbolRefField => {
                let symbol = self
                    .document
                    .symbols
                    .iter()
                    .find(|s| s.uuid == item.uuid)?;
                let ref_text = symbol.ref_text.as_ref()?;
                info.push(("Type".into(), "Reference Field".into()));
                info.push(("Text".into(), symbol.reference.clone()));
                info.push(("Reference".into(), symbol.reference.clone()));
                info.push((
                    "Position".into(),
                    format!("{:.2}, {:.2} mm", ref_text.position.x, ref_text.position.y),
                ));
                let effective_rot = (symbol.rotation + ref_text.rotation).rem_euclid(360.0);
                info.push(("Rotation".into(), format!("{effective_rot:.0}°")));
                info.push((
                    "Text Size".into(),
                    format!(
                        "{}",
                        (ref_text.font_size.max(0.0) / SCHEMATIC_PT_TO_MM).round() as i32
                    ),
                ));
                info.push((
                    "Horizontal Justification".into(),
                    h_align_label(ref_text.justify_h).into(),
                ));
                info.push((
                    "Vertical Justification".into(),
                    v_align_label(ref_text.justify_v).into(),
                ));
                info.push((
                    "Visible".into(),
                    if ref_text.hidden { "No" } else { "Yes" }.into(),
                ));
                info.push((
                    "Fields Autoplaced".into(),
                    if symbol.fields_autoplaced { "Yes" } else { "No" }.into(),
                ));
            }
            SelectedKind::SymbolValField => {
                let symbol = self
                    .document
                    .symbols
                    .iter()
                    .find(|s| s.uuid == item.uuid)?;
                let value_text = symbol.val_text.as_ref()?;
                info.push(("Type".into(), "Value Field".into()));
                info.push(("Text".into(), symbol.value.clone()));
                info.push(("Value".into(), symbol.value.clone()));
                info.push((
                    "Position".into(),
                    format!(
                        "{:.2}, {:.2} mm",
                        value_text.position.x, value_text.position.y
                    ),
                ));
                let effective_rot = (symbol.rotation + value_text.rotation).rem_euclid(360.0);
                info.push(("Rotation".into(), format!("{effective_rot:.0}°")));
                info.push((
                    "Text Size".into(),
                    format!(
                        "{}",
                        (value_text.font_size.max(0.0) / SCHEMATIC_PT_TO_MM).round() as i32
                    ),
                ));
                info.push((
                    "Horizontal Justification".into(),
                    h_align_label(value_text.justify_h).into(),
                ));
                info.push((
                    "Vertical Justification".into(),
                    v_align_label(value_text.justify_v).into(),
                ));
                info.push((
                    "Visible".into(),
                    if value_text.hidden { "No" } else { "Yes" }.into(),
                ));
                info.push((
                    "Fields Autoplaced".into(),
                    if symbol.fields_autoplaced { "Yes" } else { "No" }.into(),
                ));
            }
        }

        Some(SelectionDetails {
            selected_uuid: item.uuid,
            selected_kind: item.kind,
            info,
        })
    }
}

// ---------------------------------------------------------------------------
// Arc geometry helper
// ---------------------------------------------------------------------------

/// Circle through three non-collinear points — converts Standard's
/// (start, mid, end) arc storage into (center, radius, angles).
fn circumcircle(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> Option<(f64, f64, f64)> {
    let (ax, ay) = a;
    let (bx, by) = b;
    let (cx, cy) = c;
    let d = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
    if d.abs() < 1e-9 {
        return None;
    }
    let ux = ((ax * ax + ay * ay) * (by - cy)
        + (bx * bx + by * by) * (cy - ay)
        + (cx * cx + cy * cy) * (ay - by))
        / d;
    let uy = ((ax * ax + ay * ay) * (cx - bx)
        + (bx * bx + by * by) * (ax - cx)
        + (cx * cx + cy * cy) * (bx - ax))
        / d;
    let r = ((ax - ux) * (ax - ux) + (ay - uy) * (ay - uy)).sqrt();
    Some((ux, uy, r))
}
