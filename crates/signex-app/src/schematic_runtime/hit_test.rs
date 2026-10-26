use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectionMode {
    #[default]
    Inside,
    Touching,
    Single,
}

/// The topmost item under the point. `designators` are the ones the canvas
/// draws in place of `reference` (see [`Designators`]).
pub fn hit_test(
    snapshot: &SchematicRenderSnapshot,
    designators: &Designators,
    world_x: f64,
    world_y: f64,
) -> Option<SelectedItem> {
    let point = Point::new(world_x, world_y);
    hit_test_items(snapshot, designators, point)
        .into_iter()
        .next()
}

pub fn hit_test_polygon(
    snapshot: &SchematicRenderSnapshot,
    polygon: &[(f64, f64)],
) -> Vec<SelectedItem> {
    if polygon.len() < 3 {
        return Vec::new();
    }

    // Anchors only: the text a designator draws does not move its anchor.
    let mut out = Vec::new();
    for item in collect_item_bounds(snapshot, &Designators::new()) {
        if point_in_polygon((item.anchor.x, item.anchor.y), polygon) {
            out.push(item.item);
        }
    }
    out
}

pub fn hit_test_rect_mode(
    snapshot: &SchematicRenderSnapshot,
    designators: &Designators,
    rect: &Aabb,
    mode: SelectionMode,
) -> Vec<SelectedItem> {
    let mut out = Vec::new();

    for item in collect_item_bounds(snapshot, designators) {
        let inside = rect.contains(item.bbox.min_x, item.bbox.min_y)
            && rect.contains(item.bbox.max_x, item.bbox.max_y);
        let touching = aabb_overlaps(rect, &item.bbox);

        let keep = match mode {
            SelectionMode::Inside | SelectionMode::Single => inside,
            SelectionMode::Touching => touching,
        };

        if keep {
            out.push(item.item);
        }
    }

    out
}

fn hit_test_items(
    snapshot: &SchematicRenderSnapshot,
    designators: &Designators,
    point: Point,
) -> Vec<SelectedItem> {
    let mut out = Vec::new();

    for item in collect_item_bounds(snapshot, designators).into_iter().rev() {
        let hit = match item.item.kind {
            SelectedKind::Wire => hit_wire(snapshot, item.item.uuid, point),
            SelectedKind::Bus => hit_bus(snapshot, item.item.uuid, point),
            _ => item.bbox.expand(0.25).contains(point.x, point.y),
        };
        if hit {
            out.push(item.item);
        }
    }

    out
}

fn hit_wire(snapshot: &SchematicRenderSnapshot, uuid: uuid::Uuid, point: Point) -> bool {
    snapshot
        .wires
        .iter()
        .find(|wire| wire.uuid == uuid)
        .is_some_and(|wire| {
            let tolerance = wire
                .stroke_width
                .max(signex_types::schematic::SCHEMATIC_RENDER_MIN_STROKE_MM)
                .max(signex_types::schematic::SCHEMATIC_HIT_WIRE_TOLERANCE_MM);
            point_to_segment_distance(point, wire.start, wire.end) <= tolerance
        })
}

fn hit_bus(snapshot: &SchematicRenderSnapshot, uuid: uuid::Uuid, point: Point) -> bool {
    snapshot
        .buses
        .iter()
        .find(|bus| bus.uuid == uuid)
        .is_some_and(|bus| {
            point_to_segment_distance(point, bus.start, bus.end)
                <= signex_types::schematic::SCHEMATIC_HIT_BUS_TOLERANCE_MM
        })
}
