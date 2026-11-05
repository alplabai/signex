use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectionMode {
    #[default]
    Inside,
    Touching,
    Single,
}

impl SelectionMode {
    /// The mode a box drag selects with. With "Select by drag direction" on, a
    /// box dragged left to right takes what it fully contains and one dragged
    /// right to left takes what it touches; off, the global `fallback` decides
    /// for both directions.
    pub fn for_box_drag(
        direction: crate::canvas::BoxDirection,
        by_direction: bool,
        fallback: Self,
    ) -> Self {
        use crate::canvas::BoxDirection;
        match (by_direction, direction) {
            (false, _) => fallback,
            (true, BoxDirection::LeftToRight) => Self::Inside,
            (true, BoxDirection::RightToLeft) => Self::Touching,
        }
    }
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
        WireWidths::default(),
    )
}

/// [`hit_test`] with each wire and bus hit at the width it is drawn at, which
/// a net class and the project defaults can change.
pub fn hit_test_styled(
    snapshot: &SchematicRenderSnapshot,
    designators: &Designators,
    world_x: f64,
    world_y: f64,
    widths: WireWidths<'_>,
) -> Option<SelectedItem> {
    let point = Point::new(world_x, world_y);
    hit_test_items(snapshot, designators, point, widths)
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

    // Anchors only: neither a designator's text nor a wire's width moves one.
    let mut out = Vec::new();
    for item in collect_item_bounds(snapshot, &Designators::new(), WireWidths::default()) {
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
    widths: WireWidths<'_>,
) -> Vec<SelectedItem> {
    let mut out = Vec::new();

    for item in collect_item_bounds(snapshot, designators, widths) {
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
    widths: WireWidths<'_>,
) -> Vec<SelectedItem> {
    let mut out = Vec::new();
    // `collect_item_bounds` emits one Wire (Bus) entry per `snapshot.wires`
    // (`buses`) element, in order, so walking it in reverse walks them in
    // reverse: the segment is an index, not a search by uuid per entry.
    let mut wire_index = snapshot.wires.len();
    let mut bus_index = snapshot.buses.len();

    for item in collect_item_bounds(snapshot, designators, widths)
        .into_iter()
        .rev()
    {
        let hit = match item.item.kind {
            SelectedKind::Wire => {
                wire_index -= 1;
                hit_wire(&snapshot.wires[wire_index], point, widths)
            }
            SelectedKind::Bus => {
                bus_index -= 1;
                hit_bus(&snapshot.buses[bus_index], point, widths)
            }
            _ => item.bbox.expand(0.25).contains(point.x, point.y),
        };
        if hit {
            out.push(item.item);
        }
    }

    out
}

fn hit_wire(wire: &signex_types::schematic::Wire, point: Point, widths: WireWidths<'_>) -> bool {
    let tolerance = widths
        .wire_mm(wire)
        .max(signex_types::schematic::SCHEMATIC_HIT_WIRE_TOLERANCE_MM);
    point_to_segment_distance(point, wire.start, wire.end) <= tolerance
}

fn hit_bus(bus: &signex_types::schematic::Bus, point: Point, widths: WireWidths<'_>) -> bool {
    let tolerance = widths
        .bus_mm()
        .max(signex_types::schematic::SCHEMATIC_HIT_BUS_TOLERANCE_MM);
    point_to_segment_distance(point, bus.start, bus.end) <= tolerance
}
