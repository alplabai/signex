use signex_types::schematic::{ChildSheet, LabelType, SchematicSheet, SheetPin, GRID_MM};

use crate::command::SheetPort;

use super::Engine;

impl Engine {
    /// Returns the ports this sheet exposes to a parent hierarchical symbol.
    /// Hierarchical labels have precedence over global labels with the same name.
    pub fn collect_exposed_sheet_ports(&self) -> Vec<SheetPort> {
        let mut ports: std::collections::BTreeMap<String, (u8, String)> =
            std::collections::BTreeMap::new();

        for label in &self.document.labels {
            let Some(priority) = port_label_priority(label.label_type) else {
                continue;
            };

            let name = label.text.trim();
            if name.is_empty() {
                continue;
            }

            let direction = normalize_sheet_pin_direction(&label.shape).to_string();

            match ports.get(name) {
                Some((existing_priority, _)) if *existing_priority <= priority => {}
                _ => {
                    ports.insert(name.to_string(), (priority, direction));
                }
            }
        }

        ports
            .into_iter()
            .map(|(name, (_, direction))| SheetPort { name, direction })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Sheet-pin helpers (used by sheet.rs and transform.rs)
// ---------------------------------------------------------------------------

fn port_label_priority(label_type: LabelType) -> Option<u8> {
    match label_type {
        LabelType::Hierarchical => Some(0),
        LabelType::Global => Some(1),
        _ => None,
    }
}

pub(crate) fn normalize_sheet_pin_direction(direction_or_shape: &str) -> &'static str {
    let normalized = direction_or_shape.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "input" => "input",
        "output" => "output",
        "bidirectional" => "bidirectional",
        "tri_state" => "tri_state",
        "passive" => "passive",
        _ => "bidirectional",
    }
}

fn pin_anchor_for_direction(child: &ChildSheet, direction: &str, slot: usize) -> (f64, f64, f64) {
    let y_min = child.position.y + GRID_MM;
    let y_max = (child.position.y + child.size.1 - GRID_MM).max(y_min);
    let y = (y_min + GRID_MM * slot as f64).clamp(y_min, y_max);
    if direction == "output" {
        (child.position.x + child.size.0, y, 180.0)
    } else {
        (child.position.x, y, 0.0)
    }
}

pub(crate) fn lock_sheet_pin_to_child_edge(
    pin: &mut SheetPin,
    dx: f64,
    dy: f64,
    left_x: f64,
    right_x: f64,
    y_min: f64,
    y_max: f64,
) {
    let target_x = pin.position.x + dx;
    let target_y = (pin.position.y + dy).clamp(y_min, y_max);

    let dist_left = (target_x - left_x).abs();
    let dist_right = (target_x - right_x).abs();
    if dist_right < dist_left {
        pin.position.x = right_x;
        pin.position.y = target_y;
        pin.rotation = 180.0;
    } else {
        pin.position.x = left_x;
        pin.position.y = target_y;
        pin.rotation = 0.0;
    }
}

fn same_sheet_pin(lhs: &SheetPin, rhs: &SheetPin) -> bool {
    lhs.uuid == rhs.uuid
        && lhs.name == rhs.name
        && lhs.direction == rhs.direction
        && (lhs.position.x - rhs.position.x).abs() < 1e-9
        && (lhs.position.y - rhs.position.y).abs() < 1e-9
        && (lhs.rotation - rhs.rotation).abs() < 1e-9
        && lhs.auto_generated == rhs.auto_generated
        && lhs.user_moved == rhs.user_moved
}

pub(crate) fn reconcile_child_sheet_pins(child: &mut ChildSheet, ports: &[SheetPort]) -> bool {
    let before = child.pins.clone();
    let mut existing = std::mem::take(&mut child.pins);
    let mut next_pins = Vec::new();
    let mut left_slot = 0usize;
    let mut right_slot = 0usize;

    for port in ports {
        let direction = normalize_sheet_pin_direction(&port.direction).to_string();
        let existing_idx = existing.iter().position(|pin| pin.name == port.name);

        if let Some(idx) = existing_idx {
            let mut pin = existing.swap_remove(idx);
            pin.direction = direction.clone();
            pin.auto_generated = true;
            if !pin.user_moved {
                let slot = if direction == "output" {
                    let slot = right_slot;
                    right_slot += 1;
                    slot
                } else {
                    let slot = left_slot;
                    left_slot += 1;
                    slot
                };
                let (x, y, rotation) = pin_anchor_for_direction(child, &direction, slot);
                pin.position.x = x;
                pin.position.y = y;
                pin.rotation = rotation;
            }
            next_pins.push(pin);
        } else {
            let slot = if direction == "output" {
                let slot = right_slot;
                right_slot += 1;
                slot
            } else {
                let slot = left_slot;
                left_slot += 1;
                slot
            };
            let (x, y, rotation) = pin_anchor_for_direction(child, &direction, slot);
            next_pins.push(SheetPin {
                uuid: uuid::Uuid::new_v4(),
                name: port.name.clone(),
                direction,
                position: signex_types::schematic::Point::new(x, y),
                rotation,
                auto_generated: true,
                user_moved: false,
            });
        }
    }

    for pin in existing {
        if !pin.auto_generated {
            next_pins.push(pin);
        }
    }

    child.pins = next_pins;

    before.len() != child.pins.len()
        || before
            .iter()
            .zip(child.pins.iter())
            .any(|(lhs, rhs)| !same_sheet_pin(lhs, rhs))
}

// Keep the unused import silent — SchematicSheet is referenced via super.
#[allow(unused_imports)]
use SchematicSheet as _;
