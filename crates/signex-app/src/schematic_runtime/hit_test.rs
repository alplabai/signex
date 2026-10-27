use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectionMode {
    #[default]
    Inside,
    Touching,
    Single,
}

/// The topmost item under the point, with no net-class styling, for tests:
/// production code hits with [`hit_test_styled`], so a click lands on the wire
/// at the width it is drawn at. `designators` are the ones the canvas draws in
/// place of `reference` (see [`Designators`]).
#[doc(hidden)]
pub fn hit_test(
    snapshot: &SchematicRenderSnapshot,
    designators: &Designators,
    world_x: f64,
    world_y: f64,
) -> Option<SelectedItem> {
    hit_test_styled(
        snapshot,
        designators,
        world_x,
        world_y,
        &WireClassStyles::new(),
    )
}

/// [`hit_test`] with each wire hit at the width it is drawn at, which a net
/// class can change.
pub fn hit_test_styled(
    snapshot: &SchematicRenderSnapshot,
    designators: &Designators,
    world_x: f64,
    world_y: f64,
    class_styles: &WireClassStyles,
) -> Option<SelectedItem> {
    let point = Point::new(world_x, world_y);
    hit_test_items(snapshot, designators, point, class_styles)
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
    class_styles: &WireClassStyles,
) -> Vec<SelectedItem> {
    let mut out = Vec::new();
    // `collect_item_bounds` emits one Wire (Bus) entry per `snapshot.wires`
    // (`buses`) element, in order, so walking it in reverse walks them in
    // reverse: the segment is an index, not a search by uuid per entry.
    let mut wire_index = snapshot.wires.len();
    let mut bus_index = snapshot.buses.len();

    for item in collect_item_bounds(snapshot, designators).into_iter().rev() {
        let hit = match item.item.kind {
            SelectedKind::Wire => {
                wire_index -= 1;
                hit_wire(&snapshot.wires[wire_index], point, class_styles)
            }
            SelectedKind::Bus => {
                bus_index -= 1;
                hit_bus(&snapshot.buses[bus_index], point)
            }
            _ => item.bbox.expand(0.25).contains(point.x, point.y),
        };
        if hit {
            out.push(item.item);
        }
    }

    out
}

fn hit_wire(
    wire: &signex_types::schematic::Wire,
    point: Point,
    class_styles: &WireClassStyles,
) -> bool {
    let tolerance = drawn_wire_width_mm(wire, class_styles.get(&wire.uuid))
        .max(signex_types::schematic::SCHEMATIC_HIT_WIRE_TOLERANCE_MM);
    point_to_segment_distance(point, wire.start, wire.end) <= tolerance
}

fn hit_bus(bus: &signex_types::schematic::Bus, point: Point) -> bool {
    point_to_segment_distance(point, bus.start, bus.end)
        <= signex_types::schematic::SCHEMATIC_HIT_BUS_TOLERANCE_MM
}
