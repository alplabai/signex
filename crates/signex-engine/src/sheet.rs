use signex_net::{SheetPinContacts, pt_key};
use signex_types::schematic::{
    ChildSheet, GRID_MM, LabelType, Point, SchematicSheet, SheetPin, normalize_port_name,
};

use crate::command::SheetPort;

use super::Engine;

impl Engine {
    /// Returns the ports this sheet exposes to a parent hierarchical symbol.
    /// Hierarchical labels have precedence over global labels with the same name.
    pub fn collect_exposed_sheet_ports(&self) -> Vec<SheetPort> {
        exposed_sheet_ports(&self.document)
    }
}

/// The ports `child` exposes to a parent: its Hierarchical and Global labels,
/// de-duplicated by name (Hierarchical wins), each carrying the direction of
/// the winning label's shape. This is the one port model the reconcile
/// command consumes; `Engine::collect_exposed_sheet_ports` delegates here.
pub fn exposed_sheet_ports(child: &SchematicSheet) -> Vec<SheetPort> {
    let mut ports: std::collections::BTreeMap<String, (u8, String)> =
        std::collections::BTreeMap::new();

    for label in &child.labels {
        let Some(name) = label.port_name() else {
            continue;
        };
        let priority = port_label_priority(label.label_type);

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

/// The ports to hand `Command::ReconcileChildSheetPins` for the sheet symbols
/// of `parent` that reference `child_filename`.
///
/// Direction policy: a NEW pin takes the direction of its child label's shape
/// (`bidirectional` when the label carries none); a pin that already exists on
/// a symbol keeps the direction the user (or an earlier sync) gave it. The
/// reconcile command never touches a matched pin's direction, so this only
/// echoes the existing direction back to keep the port list self-consistent.
pub fn ports_for_pin_sync(
    child: &SchematicSheet,
    parent: &SchematicSheet,
    child_filename: &str,
) -> Vec<SheetPort> {
    let mut ports = exposed_sheet_ports(child);
    for port in &mut ports {
        let existing = parent
            .child_sheets
            .iter()
            .filter(|symbol| symbol.filename == child_filename)
            .flat_map(|symbol| symbol.pins.iter())
            .find(|pin| normalize_port_name(&pin.name) == Some(port.name.as_str()));
        if let Some(pin) = existing {
            port.direction = pin.direction.clone();
        }
    }
    ports
}

// ---------------------------------------------------------------------------
// Sheet-pin helpers (used by sheet.rs and transform.rs)
// ---------------------------------------------------------------------------

/// Lower wins. Only called for labels `Label::port_name` accepted.
fn port_label_priority(label_type: LabelType) -> u8 {
    match label_type {
        LabelType::Hierarchical => 0,
        _ => 1,
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
    child_x: f64,
    child_y: f64,
    child_w: f64,
    child_h: f64,
) {
    let left_x = child_x;
    let right_x = child_x + child_w;
    let top_y = child_y;
    let bottom_y = child_y + child_h;

    let x_min = left_x + GRID_MM;
    let x_max = (right_x - GRID_MM).max(left_x);
    let y_min = top_y + GRID_MM;
    let y_max = (bottom_y - GRID_MM).max(top_y);

    let target_x = pin.position.x + dx;
    let target_y = pin.position.y + dy;

    let dist_left = (target_x - left_x).abs();
    let dist_right = (target_x - right_x).abs();
    let dist_top = (target_y - top_y).abs();
    let dist_bottom = (target_y - bottom_y).abs();

    // Pick the nearest edge.
    let mut best = dist_left;
    let mut edge = 0u8; // 0=left 1=right 2=top 3=bottom
    if dist_right < best {
        best = dist_right;
        edge = 1;
    }
    if dist_top < best {
        best = dist_top;
        edge = 2;
    }
    // Last candidate — `best` is never read again, so only `edge` moves.
    if dist_bottom < best {
        edge = 3;
    }

    match edge {
        1 => {
            pin.position.x = right_x;
            pin.position.y = target_y.clamp(y_min, y_max);
            pin.rotation = 180.0;
        }
        2 => {
            pin.position.x = target_x.clamp(x_min, x_max);
            pin.position.y = top_y;
            pin.rotation = 270.0;
        }
        3 => {
            pin.position.x = target_x.clamp(x_min, x_max);
            pin.position.y = bottom_y;
            pin.rotation = 90.0;
        }
        _ => {
            pin.position.x = left_x;
            pin.position.y = target_y.clamp(y_min, y_max);
            pin.rotation = 0.0;
        }
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

/// Reconcile `child`'s pins with `ports` without ever moving an existing pin.
///
/// A pin that matches a port (by the shared trimmed port name) keeps its
/// position, rotation, direction, uuid and `auto_generated` flag, so wires
/// attached to it keep their nets and a hand-added pin stays hand-added. Only
/// a port with no pin gets a new one, in the first edge slot that no surviving
/// pin, no pin dropped in this same sync and nothing in `blockers` (everything a pin there
/// would connect to: see `SheetPinContacts`) occupies. When the edge has
/// no such slot the port gets no pin; the caller reports that. Auto-generated
/// pins whose port is gone are dropped; hand-added pins with no port are kept.
pub(crate) fn reconcile_child_sheet_pins(
    child: &mut ChildSheet,
    ports: &[SheetPort],
    blockers: &SheetPinContacts,
) -> bool {
    let before = child.pins.clone();
    let mut existing = std::mem::take(&mut child.pins);
    let mut matched = Vec::new();
    let mut unmatched_ports = Vec::new();

    for port in ports {
        match existing
            .iter()
            .position(|pin| normalize_port_name(&pin.name) == Some(port.name.as_str()))
        {
            Some(idx) => matched.push(existing.remove(idx)),
            None => unmatched_ports.push(port),
        }
    }

    // Surviving pins: matched ones plus hand-added ones with no port.
    let mut next_pins = matched;
    let (kept, dropped): (Vec<_>, Vec<_>) =
        existing.into_iter().partition(|pin| !pin.auto_generated);
    next_pins.extend(kept);
    // A dropped pin's slot stays taken for this sync: its wire may still end there.
    let mut taken: Vec<Point> = next_pins
        .iter()
        .chain(dropped.iter())
        .map(|pin| pin.position)
        .collect();

    for port in unmatched_ports {
        let direction = normalize_sheet_pin_direction(&port.direction).to_string();
        let Some((x, y, rotation)) = free_pin_anchor(child, &direction, &taken, blockers) else {
            continue;
        };
        taken.push(Point::new(x, y));
        next_pins.push(SheetPin {
            uuid: uuid::Uuid::new_v4(),
            name: port.name.clone(),
            direction,
            position: Point::new(x, y),
            rotation,
            auto_generated: true,
            user_moved: false,
        });
    }

    child.pins = next_pins;

    before.len() != child.pins.len()
        || before
            .iter()
            .zip(child.pins.iter())
            .any(|(lhs, rhs)| !same_sheet_pin(lhs, rhs))
}

/// The first edge slot for `direction` that is neither in `taken` nor on a
/// blocker, or `None` when the edge is full. Slots past the symbol's last row
/// clamp onto it, so the search stops at the first repeated anchor.
fn free_pin_anchor(
    child: &ChildSheet,
    direction: &str,
    taken: &[Point],
    blockers: &SheetPinContacts,
) -> Option<(f64, f64, f64)> {
    let mut last = None;
    for slot in 0.. {
        let anchor = pin_anchor_for_direction(child, direction, slot);
        if last == Some((anchor.0, anchor.1)) {
            return None;
        }
        last = Some((anchor.0, anchor.1));
        let key = pt_key(&Point::new(anchor.0, anchor.1));
        let occupied = taken.iter().any(|p| pt_key(p) == key);
        if !occupied && !blockers.hits(anchor.0, anchor.1) {
            return Some(anchor);
        }
    }
    None
}

/// Ports that have no pin on a `filename` symbol after a reconcile (its edge
/// had no free slot), per symbol: `(symbol name, port names)`.
pub fn unplaced_sheet_ports(
    parent: &SchematicSheet,
    filename: &str,
    ports: &[SheetPort],
) -> Vec<(String, Vec<String>)> {
    parent
        .child_sheets
        .iter()
        .filter(|symbol| symbol.filename == filename)
        .filter_map(|symbol| {
            let missing: Vec<String> = ports
                .iter()
                .filter(|port| {
                    !symbol
                        .pins
                        .iter()
                        .any(|pin| normalize_port_name(&pin.name) == Some(port.name.as_str()))
                })
                .map(|port| port.name.clone())
                .collect();
            (!missing.is_empty()).then(|| (symbol.name.clone(), missing))
        })
        .collect()
}

// Keep the unused import silent — SchematicSheet is referenced via super.
#[expect(
    unused_imports,
    reason = "the re-export exists so `SchematicSheet` resolves via `super` in this module's paths"
)]
use SchematicSheet as _;

#[cfg(test)]
mod reconcile_tests;
#[cfg(test)]
mod tests;
